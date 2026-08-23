//! MCP (Model Context Protocol) Server for Hone
//!
//! Exposes Hone data to LLMs via MCP tools for conversational financial queries.
//! All tools are read-only - no data modification through MCP.
//!
//! # Architecture
//!
//! The MCP server runs on a separate port from the main REST API,
//! using HTTP/SSE (Streamable HTTP) transport for local network access.
//!
//! # Example
//!
//! ```bash
//! # Start Hone with MCP enabled
//! hone serve --port 3000 --mcp-port 3001
//! ```
//!
//! # Available Tools
//!
//! - `search_transactions` - Find transactions by query, date, tag, amount
//! - `get_spending_summary` - Spending by category for a period
//! - `get_subscriptions` - Active/cancelled/all subscriptions
//! - `get_alerts` - Waste detection alerts
//! - `compare_spending` - Period-over-period comparison
//! - `get_merchants` - Top merchants by spending
//! - `get_account_summary` - Account balances and activity

pub(crate) mod oauth;
mod tools;

use std::sync::Arc;

use axum::{http::header, middleware, routing::get, Json};
use rmcp::{
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::{info, warn};

use hone_core::db::Database;
use hone_core::Error as CoreError;

use crate::{auth_middleware, AuthLayerState, ServerConfig};

pub use oauth::{mint_mcp_access_token, McpOAuthConfig, MCP_READ_SCOPE};
pub use tools::*;

/// Hone MCP Server state
#[derive(Clone)]
pub struct HoneMcpServer {
    /// Database connection (wrapped for thread-safe access)
    db: Arc<Mutex<Database>>,
    /// Tool router for MCP operations
    tool_router: ToolRouter<Self>,
}

impl HoneMcpServer {
    /// Create a new MCP server with the given database
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            tool_router: Self::tool_router(),
        }
    }

    /// Get database access for tool implementations
    pub(crate) async fn db(&self) -> tokio::sync::MutexGuard<'_, Database> {
        self.db.lock().await
    }
}

/// Serialize a tool implementation result, mapping bad inputs to MCP invalid-params.
fn tool_result<T: Serialize>(result: hone_core::Result<T>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(value) => Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&value).unwrap_or_default(),
        )])),
        Err(CoreError::InvalidData(msg)) => Err(McpError::invalid_params(msg, None)),
        Err(e) => Err(McpError::internal_error(e.to_string(), None)),
    }
}

#[tool_handler]
impl ServerHandler for HoneMcpServer {
    fn get_info(&self) -> ServerInfo {
        // rmcp 2.x marks these structs non_exhaustive, so build via constructors
        let mut server_info = Implementation::new("hone", env!("CARGO_PKG_VERSION"));
        server_info.title = Some("Hone Personal Finance".to_string());
        server_info.website_url = Some("https://github.com/heskew/hone".to_string());

        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info = server_info;
        info.instructions = Some(
            "Hone is a personal finance tool for tracking spending, subscriptions, and waste. \
             Use the available tools to query transactions, analyze spending patterns, \
             check subscriptions, and identify potential savings."
                .to_string(),
        );
        info
    }
}

#[tool_router]
impl HoneMcpServer {
    /// Search for transactions matching the given criteria
    #[tool(
        description = "Search for transactions. Returns matching transactions with amount, date, merchant, and tags."
    )]
    async fn search_transactions(
        &self,
        Parameters(params): Parameters<SearchTransactionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::search_transactions(&db, params))
    }

    /// Get spending summary by category for a time period
    #[tool(
        description = "Get spending breakdown by category. Returns total spending per category with percentages."
    )]
    async fn get_spending_summary(
        &self,
        Parameters(params): Parameters<SpendingSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::get_spending_summary(&db, params))
    }

    /// List subscriptions with their status
    #[tool(
        description = "List subscriptions. Shows recurring charges with amount, frequency, and status (active/cancelled/excluded)."
    )]
    async fn get_subscriptions(
        &self,
        Parameters(params): Parameters<SubscriptionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::get_subscriptions(&db, params))
    }

    /// Get active alerts for potential waste
    #[tool(
        description = "Get waste detection alerts. Shows zombie subscriptions, price increases, duplicates, and spending anomalies."
    )]
    async fn get_alerts(
        &self,
        Parameters(params): Parameters<AlertsParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::get_alerts(&db, params))
    }

    /// Compare spending between two time periods
    #[tool(
        description = "Compare spending between two periods. Shows changes by category with increase/decrease amounts."
    )]
    async fn compare_spending(
        &self,
        Parameters(params): Parameters<CompareSpendingParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::compare_spending(&db, params))
    }

    /// Get top merchants by spending
    #[tool(
        description = "Get top merchants by spending amount. Returns merchant name, total spent, and transaction count."
    )]
    async fn get_merchants(
        &self,
        Parameters(params): Parameters<MerchantsParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::get_merchants(&db, params))
    }

    /// Get account summary
    #[tool(description = "Get summary of all accounts with recent activity and totals.")]
    async fn get_account_summary(
        &self,
        Parameters(params): Parameters<AccountSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        tool_result(tools::get_account_summary(&db, params))
    }
}

/// Build the MCP HTTP router, gated by the same auth middleware as `/api`.
///
/// `extra_allowed_hosts` extends rmcp's loopback-only Host allowlist, which
/// exists to block DNS rebinding (RUSTSEC-2026-0189). That allowlist is not
/// authentication: when `config.require_auth` is true, requests still need
/// credentials (or a trusted-network match). `HONE_MCP_KEYS` are accepted
/// here and rejected on `/api`; `HONE_API_KEYS` still work on both.
pub(crate) fn create_mcp_router(
    db: Database,
    extra_allowed_hosts: &[String],
    config: ServerConfig,
) -> axum::Router {
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
    };

    let mut http_config = StreamableHttpServerConfig::default();
    http_config
        .allowed_hosts
        .extend(extra_allowed_hosts.iter().cloned());

    let auth_state = Arc::new(AuthLayerState {
        config: config.clone(),
        db: db.clone(),
    });

    let service = StreamableHttpService::new(
        move || Ok(HoneMcpServer::new(db.clone())),
        LocalSessionManager::default().into(),
        http_config,
    );

    // RFC 9728 metadata is public. Auth wraps `/mcp` only so a 401 challenge
    // can point clients at these well-known URLs (2026-07-28). Snapshot the
    // document at startup so this router stays `Router<()>` and merges with
    // the authenticated `/mcp` nest.
    let metadata = oauth::protected_resource_metadata(&config);
    let well_known = axum::Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get({
                let metadata = metadata.clone();
                move || {
                    let metadata = metadata.clone();
                    async move { prm_response(metadata) }
                }
            }),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get({
                let metadata = metadata.clone();
                move || {
                    let metadata = metadata.clone();
                    async move { prm_response(metadata) }
                }
            }),
        );

    let protected = axum::Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(auth_state, auth_middleware));

    well_known.merge(protected)
}

fn prm_response(
    metadata: serde_json::Value,
) -> (
    [(header::HeaderName, &'static str); 1],
    Json<serde_json::Value>,
) {
    ([(header::CACHE_CONTROL, "max-age=3600")], Json(metadata))
}

/// Start the MCP server on the given port
///
/// `extra_allowed_hosts` extends rmcp's loopback-only Host allowlist, which
/// exists to block DNS rebinding (RUSTSEC-2026-0189). Non-loopback clients
/// (e.g. LAN access via a hostname) are rejected with 403 unless their
/// authority is listed here. The Host allowlist is not a substitute for
/// `ServerConfig` authentication.
pub async fn start_mcp_server(
    db: Database,
    host: &str,
    port: u16,
    extra_allowed_hosts: &[String],
    config: ServerConfig,
) -> anyhow::Result<()> {
    info!("Starting MCP server at http://{}:{}/mcp", host, port);

    if !extra_allowed_hosts.is_empty() {
        info!(
            "MCP server allowing additional hosts: {}",
            extra_allowed_hosts.join(", ")
        );
    }
    if !config.require_auth {
        warn!("⚠️  MCP authentication disabled - do not expose to network!");
    }

    let router = create_mcp_router(db, extra_allowed_hosts, config)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("MCP server ready at http://{}/mcp", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            // Wait for shutdown signal
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn mcp_json_request() -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            ))
            .unwrap()
    }

    #[tokio::test]
    async fn mcp_rejects_unauthenticated_requests_when_auth_required() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            ..Default::default()
        };
        let app = create_mcp_router(db.clone(), &[], config);

        let response = app.oneshot(mcp_json_request()).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let entries = db.list_audit_log(20).unwrap();
        assert!(
            entries
                .iter()
                .any(|e| { e.action == "auth_deny" && e.details.as_deref() == Some("POST /mcp") }),
            "MCP auth deny must write method+path, got {entries:?}"
        );
    }

    #[tokio::test]
    async fn mcp_host_allowlist_is_not_authentication() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            ..Default::default()
        };
        let app = create_mcp_router(db, &["pi-hostname".to_string()], config);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("host", "pi-hostname")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mcp_accepts_api_key_when_auth_required() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            api_keys: vec!["test-mcp-key".to_string()],
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer test-mcp-key")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "HONE_API_KEYS still work on /mcp so existing setups keep working"
        );
    }

    fn bearer_mcp_request(key: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {key}"))
            .body(Body::from(
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            ))
            .unwrap()
    }

    #[tokio::test]
    async fn mcp_only_key_accepted_on_mcp() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_keys: vec!["mcp-only-token".to_string()],
            api_keys: vec!["api-only-token".to_string()],
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app
            .oneshot(bearer_mcp_request("mcp-only-token"))
            .await
            .unwrap();

        assert_ne!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "HONE_MCP_KEYS must be accepted on /mcp"
        );
    }

    #[tokio::test]
    async fn mcp_rejects_unauthenticated_when_mcp_keys_configured() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_keys: vec!["mcp-only-token".to_string()],
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app.oneshot(mcp_json_request()).await.unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "configuring HONE_MCP_KEYS must not leave /mcp open"
        );
    }

    #[tokio::test]
    async fn mcp_only_key_rejected_on_api() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_keys: vec!["mcp-only-token".to_string()],
            api_keys: vec!["api-only-token".to_string()],
            ..Default::default()
        };
        let app = crate::create_router(db, None, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tags")
                    .header("authorization", "Bearer mcp-only-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "HONE_MCP_KEYS must not open /api"
        );
    }

    #[tokio::test]
    async fn api_only_key_accepted_on_api() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_keys: vec!["mcp-only-token".to_string()],
            api_keys: vec!["api-only-token".to_string()],
            ..Default::default()
        };
        let app = crate::create_router(db, None, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tags")
                    .header("authorization", "Bearer api-only-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "HONE_API_KEYS must still open /api"
        );
    }

    fn mcp_oauth_config() -> crate::McpOAuthConfig {
        crate::McpOAuthConfig {
            resource: Some("http://127.0.0.1:3001/mcp".to_string()),
            jwt_secret: Some("test-mcp-jwt-secret".to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn mcp_401_includes_www_authenticate_resource_metadata() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_oauth: mcp_oauth_config(),
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app.oneshot(mcp_json_request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let challenge = response
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .expect("401 must include WWW-Authenticate");
        assert!(
            challenge.contains("resource_metadata="),
            "challenge should point at RFC 9728 metadata: {challenge}"
        );
        assert!(challenge.contains("scope=\"mcp:read\""), "{challenge}");
    }

    #[tokio::test]
    async fn mcp_protected_resource_metadata_is_public() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_oauth: mcp_oauth_config(),
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/.well-known/oauth-protected-resource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["resource"], "http://127.0.0.1:3001/mcp");
        assert_eq!(json["scopes_supported"][0], "mcp:read");
        assert!(json.get("authorization_servers").is_none());
    }

    #[tokio::test]
    async fn mcp_audience_jwt_accepted_on_mcp() {
        let db = Database::in_memory().unwrap();
        let mcp_oauth = mcp_oauth_config();
        let token = mint_mcp_access_token(
            mcp_oauth.jwt_secret.as_deref().unwrap(),
            mcp_oauth.resource.as_deref().unwrap(),
            60,
        )
        .unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_oauth,
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app.oneshot(bearer_mcp_request(&token)).await.unwrap();
        assert_ne!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "MCP-audience JWT must be accepted on /mcp"
        );
    }

    #[tokio::test]
    async fn api_audience_jwt_rejected_on_mcp() {
        let db = Database::in_memory().unwrap();
        let mcp_oauth = mcp_oauth_config();
        let token = mint_mcp_access_token(
            mcp_oauth.jwt_secret.as_deref().unwrap(),
            "http://127.0.0.1:3000/api",
            60,
        )
        .unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_oauth,
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app.oneshot(bearer_mcp_request(&token)).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "token bound to the API resource must not open /mcp"
        );
    }

    #[tokio::test]
    async fn mcp_audience_jwt_rejected_on_api() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();
        let mcp_oauth = mcp_oauth_config();
        let token = mint_mcp_access_token(
            mcp_oauth.jwt_secret.as_deref().unwrap(),
            mcp_oauth.resource.as_deref().unwrap(),
            60,
        )
        .unwrap();
        let config = ServerConfig {
            require_auth: true,
            mcp_oauth,
            api_keys: vec!["api-only-token".to_string()],
            ..Default::default()
        };
        let app = crate::create_router(db, None, config);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tags")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "MCP-audience JWT must not open /api"
        );
    }

    #[tokio::test]
    async fn mcp_allows_unauthenticated_when_no_auth() {
        let db = Database::in_memory().unwrap();
        let config = ServerConfig {
            require_auth: false,
            ..Default::default()
        };
        let app = create_mcp_router(db, &[], config);

        let response = app.oneshot(mcp_json_request()).await.unwrap();

        assert_ne!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "--no-auth must leave MCP open, consistent with the API"
        );
    }

    fn seed_period_sensitive_txns(db: &Database) {
        use chrono::{Duration, Utc};
        use hone_core::models::{Bank, NewTransaction};

        db.upsert_account("Test Checking", Bank::Chase, None)
            .unwrap();
        let today = Utc::now().date_naive();
        db.insert_transaction(
            1,
            &NewTransaction {
                date: today,
                description: "NETFLIX.COM".to_string(),
                amount: -15.99,
                category: None,
                import_hash: "mcp_hash_netflix".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();
        db.insert_transaction(
            1,
            &NewTransaction {
                date: today - Duration::days(35),
                description: "AMAZON.COM".to_string(),
                amount: -150.00,
                category: None,
                import_hash: "mcp_hash_amazon".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();
    }

    fn tool_payload(result: &CallToolResult) -> serde_json::Value {
        let encoded = serde_json::to_value(result).expect("CallToolResult is serializable");
        let text = encoded["content"][0]["text"]
            .as_str()
            .expect("tool result should be a text content block");
        serde_json::from_str(text).expect("tool text should be JSON")
    }

    fn schema_properties(tool_name: &str) -> serde_json::Map<String, serde_json::Value> {
        let server = HoneMcpServer::new(Database::in_memory().unwrap());
        let tool = server
            .tool_router
            .get(tool_name)
            .unwrap_or_else(|| panic!("{tool_name} should be registered"));
        let schema = serde_json::to_value(&tool.input_schema).expect("schema is JSON");
        schema
            .get("properties")
            .or_else(|| {
                schema
                    .pointer("/$defs")
                    .and_then(|_| schema.get("properties"))
            })
            .and_then(|p| p.as_object())
            .cloned()
            .unwrap_or_else(|| {
                // schemars/rmcp may nest properties under $ref
                if let Some(name) = schema.get("$ref").and_then(|r| r.as_str()) {
                    let key = name.rsplit('/').next().unwrap_or_default();
                    if let Some(props) = schema
                        .pointer(&format!("/$defs/{key}/properties"))
                        .or_else(|| schema.pointer(&format!("/definitions/{key}/properties")))
                    {
                        return props.as_object().cloned().unwrap_or_default();
                    }
                }
                // Empty-object tools (no advertised args) have `"type": "object"` and no properties.
                if schema.get("type").and_then(|t| t.as_str()) == Some("object") {
                    return serde_json::Map::new();
                }
                panic!("no properties in {tool_name} schema: {schema}");
            })
    }

    #[test]
    fn mcp_tool_schemas_advertise_period_and_query() {
        let search = schema_properties("search_transactions");
        assert!(search.contains_key("query"), "search schema: {search:?}");
        assert!(search.contains_key("period"), "search schema: {search:?}");
        assert!(search.contains_key("tag"), "search schema: {search:?}");

        let summary = schema_properties("get_spending_summary");
        assert!(
            summary.contains_key("period"),
            "spending summary schema: {summary:?}"
        );

        let merchants = schema_properties("get_merchants");
        assert!(
            merchants.contains_key("period"),
            "merchants schema: {merchants:?}"
        );

        let accounts = schema_properties("get_account_summary");
        assert!(
            !accounts.contains_key("include_archived"),
            "do not advertise unwired include_archived: {accounts:?}"
        );
    }

    #[tokio::test]
    async fn mcp_search_honors_query_and_period() {
        let db = Database::in_memory().unwrap();
        seed_period_sensitive_txns(&db);
        let server = HoneMcpServer::new(db);

        let default = server
            .search_transactions(Parameters(SearchTransactionsParams::default()))
            .await
            .unwrap();
        let default_json = tool_payload(&default);
        let default_count = default_json["total_count"].as_u64().unwrap();

        let all = server
            .search_transactions(Parameters(SearchTransactionsParams {
                period: Some("all".to_string()),
                ..Default::default()
            }))
            .await
            .unwrap();
        let all_json = tool_payload(&all);
        let all_count = all_json["total_count"].as_u64().unwrap();
        assert!(
            all_count > default_count,
            "period=all should include last-month Amazon; default this-month should not. default={default_json} all={all_json}"
        );

        let amazon = server
            .search_transactions(Parameters(SearchTransactionsParams {
                query: Some("Amazon".to_string()),
                period: Some("all".to_string()),
                ..Default::default()
            }))
            .await
            .unwrap();
        let amazon_json = tool_payload(&amazon);
        assert_eq!(amazon_json["total_count"], 1);
        assert!(
            amazon_json["transactions"][0]["description"]
                .as_str()
                .unwrap()
                .contains("AMAZON"),
            "{amazon_json}"
        );
    }

    #[tokio::test]
    async fn mcp_spending_summary_honors_period() {
        let db = Database::in_memory().unwrap();
        seed_period_sensitive_txns(&db);
        let server = HoneMcpServer::new(db);

        let this_month = server
            .get_spending_summary(Parameters(SpendingSummaryParams::default()))
            .await
            .unwrap();
        let this_month_json = tool_payload(&this_month);

        let all = server
            .get_spending_summary(Parameters(SpendingSummaryParams {
                period: Some("all".to_string()),
                ..Default::default()
            }))
            .await
            .unwrap();
        let all_json = tool_payload(&all);

        assert_eq!(all_json["period"], "all");
        assert_ne!(
            this_month_json["total_spending"], all_json["total_spending"],
            "non-default period must change the result. this_month={this_month_json} all={all_json}"
        );
    }

    #[tokio::test]
    async fn mcp_invalid_period_fails_cleanly() {
        let server = HoneMcpServer::new(Database::in_memory().unwrap());
        let err = server
            .get_spending_summary(Parameters(SpendingSummaryParams {
                period: Some("not-a-period".to_string()),
                ..Default::default()
            }))
            .await
            .expect_err("invalid period must not succeed");
        assert!(
            err.message.contains("Invalid period"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn mcp_invalid_argument_types_fail_to_deserialize() {
        let err = serde_json::from_value::<SearchTransactionsParams>(serde_json::json!({
            "min_amount": "twelve",
            "period": "all"
        }));
        assert!(
            err.is_err(),
            "typed tool params must reject a string min_amount"
        );
    }
}

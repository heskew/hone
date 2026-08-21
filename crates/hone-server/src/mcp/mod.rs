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

mod tools;

use std::sync::Arc;

use axum::middleware;
use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use tokio::sync::Mutex;
use tracing::{info, warn};

use hone_core::db::Database;

use crate::{auth_middleware, ServerConfig};

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
    async fn search_transactions(&self) -> Result<CallToolResult, McpError> {
        // For now, return a simple result - we'll add parameters later
        let db = self.db().await;
        let params = SearchTransactionsParams::default();
        match tools::search_transactions(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// Get spending summary by category for a time period
    #[tool(
        description = "Get spending breakdown by category. Returns total spending per category with percentages."
    )]
    async fn get_spending_summary(&self) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        let params = SpendingSummaryParams::default();
        match tools::get_spending_summary(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// List subscriptions with their status
    #[tool(
        description = "List subscriptions. Shows recurring charges with amount, frequency, and status (active/cancelled/excluded)."
    )]
    async fn get_subscriptions(&self) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        let params = SubscriptionsParams::default();
        match tools::get_subscriptions(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// Get active alerts for potential waste
    #[tool(
        description = "Get waste detection alerts. Shows zombie subscriptions, price increases, duplicates, and spending anomalies."
    )]
    async fn get_alerts(&self) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        let params = AlertsParams::default();
        match tools::get_alerts(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// Compare spending between two time periods
    #[tool(
        description = "Compare spending between two periods. Shows changes by category with increase/decrease amounts."
    )]
    async fn compare_spending(&self) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        let params = CompareSpendingParams::default();
        match tools::compare_spending(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// Get top merchants by spending
    #[tool(
        description = "Get top merchants by spending amount. Returns merchant name, total spent, and transaction count."
    )]
    async fn get_merchants(&self) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        let params = MerchantsParams::default();
        match tools::get_merchants(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    /// Get account summary
    #[tool(description = "Get summary of all accounts with recent activity and totals.")]
    async fn get_account_summary(&self) -> Result<CallToolResult, McpError> {
        let db = self.db().await;
        let params = AccountSummaryParams::default();
        match tools::get_account_summary(&db, params) {
            Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }
}

/// Build the MCP HTTP router, gated by the same auth as `/api`.
///
/// `extra_allowed_hosts` extends rmcp's loopback-only Host allowlist, which
/// exists to block DNS rebinding (RUSTSEC-2026-0189). That allowlist is not
/// authentication: when `config.require_auth` is true, requests still need
/// API credentials (or a trusted-network match).
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

    let service = StreamableHttpService::new(
        move || Ok(HoneMcpServer::new(db.clone())),
        LocalSessionManager::default().into(),
        http_config,
    );

    axum::Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            Arc::new(config),
            auth_middleware,
        ))
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
        let app = create_mcp_router(db, &[], config);

        let response = app.oneshot(mcp_json_request()).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
            "valid API key must pass the same auth gate as /api"
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
}

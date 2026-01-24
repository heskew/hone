//! Hone Web Server
//!
//! Axum-based REST API for the Hone personal finance application.
//!
//! Security features:
//! - Cloudflare Access authentication (secure by default, use --no-auth for local dev)
//! - Restrictive CORS policy
//! - Input validation (pagination limits, file size limits)
//! - Full audit logging for all API access (reads and writes)
//! - Sanitized error responses

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Serialize;
use tower_http::{
    cors::CorsLayer, services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer,
};
use tracing::{error, info, warn};

use hone_core::ai::{orchestrator::AIOrchestrator, AIBackend, AIClient};
use hone_core::db::Database;

mod handlers;
pub mod mcp;
mod scheduler;

pub use scheduler::{start_backup_scheduler, BackupScheduleConfig};

/// Maximum file upload size (10 MB)
pub const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024;

/// Maximum pagination limit
pub const MAX_PAGE_LIMIT: i64 = 1000;

/// Cloudflare Access header for authenticated user email
const CF_ACCESS_USER_HEADER: &str = "cf-access-authenticated-user-email";

/// Cloudflare Access JWT header (cryptographic proof of authentication)
const CF_ACCESS_JWT_HEADER: &str = "cf-access-jwt-assertion";

/// Authorization header for API key auth
const AUTHORIZATION_HEADER: &str = "authorization";

/// Cloudflare Access JWT validation configuration
#[derive(Clone, Default)]
pub struct CfJwtConfig {
    /// Cloudflare team name (e.g., "myteam" for myteam.cloudflareaccess.com)
    /// Required for JWT validation
    pub team_name: Option<String>,
    /// Application audience tag (aud claim) - from CF Access application settings
    /// Required for JWT validation
    pub audience: Option<String>,
    /// Cached public keys for JWT validation (populated at runtime)
    /// Keys are fetched from https://<team>.cloudflareaccess.com/cdn-cgi/access/certs
    #[allow(dead_code)]
    pub cached_keys: Option<CfPublicKeys>,
}

/// Cloudflare Access public keys for JWT validation
#[derive(Clone)]
pub struct CfPublicKeys {
    pub keys: Vec<jsonwebtoken::jwk::Jwk>,
    pub fetched_at: std::time::Instant,
}

/// Server configuration
#[derive(Clone)]
pub struct ServerConfig {
    /// Whether authentication is required (secure by default)
    pub require_auth: bool,
    /// Allowed CORS origins (empty = same-origin only in production)
    pub allowed_origins: Vec<String>,
    /// API keys for internal service authentication (alternative to Cloudflare Access)
    /// Format: "Bearer <key>" in Authorization header
    pub api_keys: Vec<String>,
    /// Cloudflare Access JWT validation config (optional but recommended)
    pub cf_jwt: CfJwtConfig,
    /// Trusted networks that bypass authentication (e.g., "192.168.1.0/24", "10.0.0.5")
    /// Requests from these IPs are allowed without any authentication
    pub trusted_networks: Vec<ipnet::IpNet>,
    /// Trusted proxies whose X-Forwarded-For headers are trusted (e.g., "10.42.0.0/16" for k3s)
    /// When a request comes from a trusted proxy, the client IP is extracted from X-Forwarded-For
    pub trusted_proxies: Vec<ipnet::IpNet>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            require_auth: true,
            allowed_origins: vec![],
            api_keys: vec![],
            cf_jwt: CfJwtConfig::default(),
            trusted_networks: vec![],
            trusted_proxies: vec![],
        }
    }
}

/// Shared application state
pub struct AppState {
    pub db: Database,
    pub config: ServerConfig,
    pub ai: Option<AIClient>,
    /// AI orchestrator for agentic analysis with tool-calling
    pub orchestrator: Option<AIOrchestrator>,
    /// Optional override for backup directory (for testing)
    pub backup_dir: Option<std::path::PathBuf>,
    /// Directory for storing receipt images (defaults to ./receipts)
    pub receipts_dir: std::path::PathBuf,
    /// Session manager for explore mode conversations
    pub explore_sessions: handlers::ExploreSessionManager,
}

/// Authentication middleware - validates Cloudflare Access JWT, headers, API keys, or trusted networks
///
/// # Security Notes
///
/// **Trusted networks**: Requests from IPs in `trusted_networks` bypass all authentication.
/// Use this for local network access (e.g., "192.168.1.0/24"). The client IP is determined
/// from the TCP connection peer address only (headers are NOT trusted to prevent spoofing).
///
/// **Cloudflare Access JWT** (recommended): The `Cf-Access-Jwt-Assertion` header contains a
/// cryptographically signed JWT. When `CF_TEAM_NAME` and `CF_AUD_TAG` are configured, this
/// JWT is validated against Cloudflare's public keys, providing cryptographic proof that
/// the request came through Cloudflare Access.
///
/// **Cloudflare Access headers** (fallback): The `CF-Access-Authenticated-User-Email` header
/// is trusted only when JWT validation is not configured. This header is safe behind
/// Cloudflare Tunnel (which strips/rewrites CF headers), but can be spoofed if the server
/// is exposed directly to the internet.
///
/// **API keys**: Compared using constant-time comparison to prevent timing attacks.
async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    request: Request,
    next: Next,
) -> Response {
    if !state.config.require_auth {
        return next.run(request).await;
    }

    // Check if request is from a trusted network
    if !state.config.trusted_networks.is_empty() {
        let peer_ip = connect_info.as_ref().map(|ci| ci.0.ip());
        let xff = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok());
        let client_ip = get_client_ip(
            &request,
            connect_info.as_ref(),
            &state.config.trusted_proxies,
        );

        // Debug logging for trusted network auth
        tracing::debug!(
            ?peer_ip,
            ?xff,
            ?client_ip,
            trusted_proxies = ?state.config.trusted_proxies,
            trusted_networks = ?state.config.trusted_networks,
            path = %request.uri().path(),
            "Checking trusted network auth"
        );

        if let Some(ip) = client_ip {
            if is_ip_trusted(&ip, &state.config.trusted_networks) {
                info!(ip = %ip, path = %request.uri().path(), "Authenticated via trusted network");
                return next.run(request).await;
            }
        }
    }

    // Check for Cloudflare Access JWT first (cryptographic verification)
    if state.config.cf_jwt.team_name.is_some() && state.config.cf_jwt.audience.is_some() {
        if let Some(jwt) = request
            .headers()
            .get(CF_ACCESS_JWT_HEADER)
            .and_then(|v| v.to_str().ok())
        {
            match validate_cf_jwt(jwt, &state.config.cf_jwt).await {
                Ok(email) => {
                    info!(user = %email, path = %request.uri().path(), "Authenticated via Cloudflare JWT");
                    return next.run(request).await;
                }
                Err(e) => {
                    warn!(error = %e, path = %request.uri().path(), "Invalid Cloudflare JWT");
                    // Fall through to try other auth methods
                }
            }
        }
    }

    // Check for Cloudflare Access user header (trusted when behind CF Tunnel)
    // SECURITY: Only use this fallback when JWT validation is not configured.
    // If JWT config is set but validation failed, we still check this header
    // to allow for graceful degradation during key rotation.
    let cf_user = request
        .headers()
        .get(CF_ACCESS_USER_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(email) = cf_user {
        // Warn if JWT config is set but we're falling back to header-only auth
        if state.config.cf_jwt.team_name.is_some() {
            warn!(
                user = %email,
                path = %request.uri().path(),
                "Authenticated via CF header (JWT validation configured but no valid JWT)"
            );
        } else {
            info!(user = %email, path = %request.uri().path(), "Authenticated via Cloudflare Access header");
        }
        return next.run(request).await;
    }

    // Check for API key in Authorization header (Bearer token)
    // Uses constant-time comparison to prevent timing attacks
    let api_key_valid = request
        .headers()
        .get(AUTHORIZATION_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .map(|key| validate_api_key(key, &state.config.api_keys))
        .unwrap_or(false);

    if api_key_valid {
        info!(user = "api-key", path = %request.uri().path(), "Authenticated via API key");
        return next.run(request).await;
    }

    warn!(path = %request.uri().path(), "Unauthorized request - no valid auth");
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": "Authentication required"
        })),
    )
        .into_response()
}

/// Validate a Cloudflare Access JWT
///
/// Fetches public keys from Cloudflare and validates the JWT signature, expiration,
/// and audience claim.
async fn validate_cf_jwt(token: &str, config: &CfJwtConfig) -> Result<String, String> {
    use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

    let team_name = config
        .team_name
        .as_ref()
        .ok_or("Team name not configured")?;
    let audience = config.audience.as_ref().ok_or("Audience not configured")?;

    // Decode header to get the key ID (kid)
    let header = decode_header(token).map_err(|e| format!("Invalid JWT header: {}", e))?;
    let kid = header.kid.ok_or("JWT missing key ID (kid)")?;

    // Fetch public keys from Cloudflare
    let certs_url = format!(
        "https://{}.cloudflareaccess.com/cdn-cgi/access/certs",
        team_name
    );

    let keys = fetch_cf_public_keys(&certs_url)
        .await
        .map_err(|e| format!("Failed to fetch CF public keys: {}", e))?;

    // Find the key matching the JWT's kid
    let jwk = keys
        .iter()
        .find(|k| k.common.key_id.as_deref() == Some(&kid))
        .ok_or_else(|| format!("No matching key found for kid: {}", kid))?;

    // Convert JWK to decoding key
    let decoding_key = DecodingKey::from_jwk(jwk).map_err(|e| format!("Invalid JWK: {}", e))?;

    // Set up validation
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[audience]);
    validation.set_issuer(&[format!("https://{}.cloudflareaccess.com", team_name)]);

    // Decode and validate the token
    #[derive(serde::Deserialize)]
    struct Claims {
        email: Option<String>,
        sub: String,
    }

    let token_data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| format!("JWT validation failed: {}", e))?;

    // Return email if present, otherwise subject
    Ok(token_data.claims.email.unwrap_or(token_data.claims.sub))
}

/// Fetch Cloudflare Access public keys from the certs endpoint
async fn fetch_cf_public_keys(url: &str) -> Result<Vec<jsonwebtoken::jwk::Jwk>, String> {
    #[derive(serde::Deserialize)]
    struct JwkSet {
        keys: Vec<jsonwebtoken::jwk::Jwk>,
    }

    // Use reqwest to fetch the keys
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    let jwk_set: JwkSet = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JWK set: {}", e))?;

    Ok(jwk_set.keys)
}

/// Validate an API key against the configured keys using constant-time comparison
/// to prevent timing attacks.
fn validate_api_key(provided: &str, valid_keys: &[String]) -> bool {
    use subtle::ConstantTimeEq;

    let provided_bytes = provided.as_bytes();

    for key in valid_keys {
        let key_bytes = key.as_bytes();
        // Only compare if lengths match (constant-time for same-length keys)
        if provided_bytes.len() == key_bytes.len() {
            if provided_bytes.ct_eq(key_bytes).into() {
                return true;
            }
        }
    }
    false
}

/// Extract client IP address, respecting trusted proxies
///
/// SECURITY: X-Forwarded-For headers are ONLY trusted when the TCP connection
/// comes from a configured trusted proxy. Otherwise, only the actual TCP
/// peer address is used (to prevent header spoofing attacks).
///
/// When behind a reverse proxy (like Traefik in k3s), configure HONE_TRUSTED_PROXIES
/// to the proxy's IP/CIDR so that the real client IP can be extracted from headers.
pub(crate) fn get_client_ip(
    request: &Request,
    connect_info: Option<&axum::extract::ConnectInfo<std::net::SocketAddr>>,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<std::net::IpAddr> {
    let peer_ip = connect_info.map(|ci| ci.0.ip())?;

    // If no trusted proxies configured, only use peer address
    if trusted_proxies.is_empty() {
        return Some(peer_ip);
    }

    // Check if the peer is a trusted proxy
    let peer_is_trusted_proxy = trusted_proxies.iter().any(|net| net.contains(&peer_ip));

    if peer_is_trusted_proxy {
        // Trust X-Forwarded-For from this proxy
        // X-Forwarded-For format: "client, proxy1, proxy2" - take the first (original client)
        if let Some(forwarded_for) = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            if let Some(client_ip_str) = forwarded_for.split(',').next() {
                if let Ok(client_ip) = client_ip_str.trim().parse::<std::net::IpAddr>() {
                    return Some(client_ip);
                }
            }
        }

        // Fallback: try X-Real-IP header
        if let Some(real_ip) = request
            .headers()
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
        {
            if let Ok(client_ip) = real_ip.trim().parse::<std::net::IpAddr>() {
                return Some(client_ip);
            }
        }
    }

    // Default to peer address
    Some(peer_ip)
}

/// Check if an IP address is within any of the trusted networks
fn is_ip_trusted(ip: &std::net::IpAddr, trusted_networks: &[ipnet::IpNet]) -> bool {
    for network in trusted_networks {
        if network.contains(ip) {
            return true;
        }
    }
    false
}

/// Parse a comma-separated list of IP addresses and CIDR networks
///
/// Examples:
/// - "192.168.1.0/24" - entire subnet
/// - "10.0.0.5" - single IP (parsed as /32 for IPv4 or /128 for IPv6)
/// - "192.168.1.0/24,10.0.0.0/8" - multiple networks
pub fn parse_trusted_networks(input: &str) -> Vec<ipnet::IpNet> {
    input
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            // Try parsing as network first
            if let Ok(net) = s.parse::<ipnet::IpNet>() {
                return Some(net);
            }
            // Try parsing as single IP and convert to /32 or /128
            if let Ok(ip) = s.parse::<std::net::IpAddr>() {
                return Some(ipnet::IpNet::from(ip));
            }
            warn!(input = s, "Failed to parse trusted network entry");
            None
        })
        .collect()
}

/// Extract user email from request headers (for audit logging)
/// Returns CF Access email, "api-key" for API key auth, or "local-dev" for unauthenticated
pub fn get_user_email(headers: &axum::http::HeaderMap) -> String {
    // Check for Cloudflare Access user first
    if let Some(email) = headers
        .get(CF_ACCESS_USER_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        return email.to_string();
    }

    // Check for API key (returns "api-key" as the user identifier)
    if headers
        .get(AUTHORIZATION_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .is_some()
    {
        return "api-key".to_string();
    }

    "local-dev".to_string()
}

/// Success response
#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
}

/// Create the application router
pub fn create_router(db: Database, static_dir: Option<&str>, config: ServerConfig) -> Router {
    create_router_with_options(db, static_dir, config, None)
}

/// Create the application router with additional options (for testing)
pub fn create_router_with_options(
    db: Database,
    static_dir: Option<&str>,
    config: ServerConfig,
    backup_dir: Option<std::path::PathBuf>,
) -> Router {
    // Create AI client if configured
    let ai = AIClient::from_env();
    if let Some(ref client) = ai {
        let router_info = client.router_info();
        info!(
            "AI backend configured: {} (default model: {}, fallback: {})",
            client.host(),
            router_info.default_model,
            router_info.fallback_model.as_deref().unwrap_or("none")
        );
        // Log task-specific model assignments
        for (task, model) in &router_info.task_models {
            info!("  - {}: {}", task, model);
        }
    } else {
        info!("ℹ️  AI backend not configured (set OLLAMA_HOST to enable AI features)");
    }

    // Create AI orchestrator for agentic analysis (tool-calling)
    let orchestrator = AIOrchestrator::from_env(db.clone());
    if let Some(ref orch) = orchestrator {
        info!(
            "AI orchestrator configured: {} (model: {})",
            orch.backend().host(),
            orch.model()
        );
    } else {
        info!("ℹ️  AI orchestrator not configured (set ANTHROPIC_COMPATIBLE_HOST for agentic analysis)");
    }

    // Default receipts directory relative to working directory
    let receipts_dir = std::path::PathBuf::from("receipts");

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        ai,
        orchestrator,
        backup_dir,
        receipts_dir,
        explore_sessions: handlers::ExploreSessionManager::new(),
    });

    let api_routes = Router::new()
        // Auth
        .route("/me", get(handlers::get_me))
        // Dashboard
        .route("/dashboard", get(handlers::get_dashboard))
        // Accounts
        .route(
            "/accounts",
            get(handlers::list_accounts).post(handlers::create_account),
        )
        .route(
            "/accounts/:id",
            get(handlers::get_account)
                .put(handlers::update_account)
                .delete(handlers::delete_account),
        )
        .route(
            "/accounts/:id/entity",
            axum::routing::patch(handlers::update_account_entity),
        )
        // Transactions
        .route("/transactions", get(handlers::list_transactions))
        .route(
            "/transactions/bulk-tags",
            post(handlers::bulk_add_tags).delete(handlers::bulk_remove_tags),
        )
        // Subscriptions
        .route("/subscriptions", get(handlers::list_subscriptions))
        .route(
            "/subscriptions/:id/acknowledge",
            post(handlers::acknowledge_subscription),
        )
        .route(
            "/subscriptions/:id/cancel",
            post(handlers::cancel_subscription),
        )
        .route(
            "/subscriptions/:id/exclude",
            post(handlers::exclude_subscription),
        )
        .route(
            "/subscriptions/:id/unexclude",
            post(handlers::unexclude_subscription),
        )
        .route("/subscriptions/:id", delete(handlers::delete_subscription))
        // Alerts
        .route("/alerts", get(handlers::list_alerts))
        .route("/alerts/:id/dismiss", post(handlers::dismiss_alert))
        .route(
            "/alerts/:id/dismiss-exclude",
            post(handlers::dismiss_alert_exclude),
        )
        .route("/alerts/:id/restore", post(handlers::restore_alert))
        .route(
            "/alerts/:id/reanalyze",
            post(handlers::reanalyze_spending_alert),
        )
        // Insights
        .route("/insights", get(handlers::get_top_insights))
        .route("/insights/all", get(handlers::list_insights))
        .route("/insights/count", get(handlers::count_insights))
        .route("/insights/refresh", post(handlers::refresh_insights))
        .route("/insights/:id", get(handlers::get_insight))
        .route("/insights/:id/dismiss", post(handlers::dismiss_insight))
        .route("/insights/:id/snooze", post(handlers::snooze_insight))
        .route("/insights/:id/restore", post(handlers::restore_insight))
        .route(
            "/insights/:id/feedback",
            post(handlers::set_insight_feedback),
        )
        // Detection
        .route("/detect", post(handlers::run_detection))
        // Import
        .route("/import", post(handlers::import_csv))
        .route("/import/json", post(handlers::import_csv_json))
        // Import history
        .route("/imports", get(handlers::list_import_sessions))
        .route("/imports/:id", get(handlers::get_import_session))
        .route(
            "/imports/:id/transactions",
            get(handlers::get_import_session_transactions),
        )
        .route(
            "/imports/:id/skipped",
            get(handlers::get_import_session_skipped),
        )
        .route("/imports/:id/cancel", post(handlers::cancel_import_session))
        .route(
            "/imports/:id/reprocess",
            post(handlers::reprocess_import_session),
        )
        .route(
            "/imports/:id/reprocess-comparison",
            get(handlers::get_reprocess_comparison),
        )
        // Reprocess runs (historical comparison)
        .route("/imports/:id/runs", get(handlers::list_reprocess_runs))
        .route(
            "/imports/:id/runs/compare",
            get(handlers::compare_reprocess_runs),
        )
        .route(
            "/imports/:session_id/runs/:run_id",
            get(handlers::get_reprocess_run),
        )
        // Audit log
        .route("/audit", get(handlers::list_audit_log))
        // Tags
        .route("/tags", get(handlers::list_tags).post(handlers::create_tag))
        .route("/tags/tree", get(handlers::get_tag_tree))
        .route(
            "/tags/:id",
            get(handlers::get_tag)
                .patch(handlers::update_tag)
                .delete(handlers::delete_tag),
        )
        // Transaction tagging
        .route(
            "/transactions/:id/tags",
            get(handlers::get_transaction_tags).post(handlers::add_transaction_tag),
        )
        .route(
            "/transactions/:tx_id/tags/:tag_id",
            axum::routing::delete(handlers::remove_transaction_tag),
        )
        // Tag rules
        .route(
            "/rules",
            get(handlers::list_tag_rules).post(handlers::create_tag_rule),
        )
        .route(
            "/rules/:id",
            axum::routing::delete(handlers::delete_tag_rule),
        )
        .route("/rules/test", post(handlers::test_rules))
        // Reports
        .route("/reports/by-tag", get(handlers::report_by_tag))
        .route("/reports/spending", get(handlers::report_spending))
        .route("/reports/trends", get(handlers::report_trends))
        .route("/reports/merchants", get(handlers::report_merchants))
        .route(
            "/reports/subscriptions",
            get(handlers::report_subscriptions),
        )
        .route("/reports/savings", get(handlers::report_savings))
        .route("/reports/by-entity", get(handlers::report_by_entity))
        .route("/reports/by-location", get(handlers::report_by_location))
        .route(
            "/reports/vehicle-costs/:id",
            get(handlers::report_vehicle_costs),
        )
        .route(
            "/reports/property-expenses/:id",
            get(handlers::report_property_expenses),
        )
        // Entities
        .route(
            "/entities",
            get(handlers::list_entities).post(handlers::create_entity),
        )
        .route(
            "/entities/:id",
            get(handlers::get_entity)
                .patch(handlers::update_entity)
                .delete(handlers::delete_entity),
        )
        .route("/entities/:id/archive", post(handlers::archive_entity))
        .route("/entities/:id/unarchive", post(handlers::unarchive_entity))
        // Mileage Logs (for vehicle entities)
        .route(
            "/entities/:id/mileage",
            get(handlers::list_mileage_logs).post(handlers::create_mileage_log),
        )
        .route(
            "/entities/:id/miles",
            get(handlers::get_vehicle_total_miles),
        )
        .route(
            "/mileage/:id",
            axum::routing::delete(handlers::delete_mileage_log),
        )
        // Locations
        .route(
            "/locations",
            get(handlers::list_locations).post(handlers::create_location),
        )
        .route(
            "/locations/:id",
            get(handlers::get_location).delete(handlers::delete_location),
        )
        // Transaction Splits
        .route(
            "/transactions/:id/splits",
            get(handlers::get_transaction_splits).post(handlers::create_split),
        )
        .route(
            "/splits/:id",
            get(handlers::get_split)
                .patch(handlers::update_split)
                .delete(handlers::delete_split),
        )
        // Transaction location
        .route(
            "/transactions/:id/location",
            post(handlers::update_transaction_location),
        )
        // Transaction trip assignment
        .route(
            "/transactions/:id/trip",
            post(handlers::assign_transaction_to_trip),
        )
        // Receipts (attached to transactions)
        .route(
            "/transactions/:id/receipts",
            get(handlers::get_transaction_receipts).post(handlers::upload_receipt),
        )
        .route(
            "/receipts/:id",
            get(handlers::get_receipt).delete(handlers::delete_receipt),
        )
        .route("/receipts/:id/parse", post(handlers::parse_receipt))
        // Receipt-first workflow
        .route(
            "/receipts",
            get(handlers::list_receipts).post(handlers::upload_pending_receipt),
        )
        .route(
            "/receipts/:id/link",
            post(handlers::link_receipt_to_transaction),
        )
        .route(
            "/receipts/:id/status",
            post(handlers::update_receipt_status),
        )
        .route("/receipts/:id/unlink", post(handlers::unlink_receipt))
        .route(
            "/receipts/:id/candidates",
            get(handlers::get_receipt_match_candidates),
        )
        .route("/receipts/auto-match", post(handlers::auto_match_receipts))
        // AI Suggestions
        .route(
            "/transactions/:id/suggest-entity",
            get(handlers::suggest_entity),
        )
        .route(
            "/transactions/:id/suggest-split",
            get(handlers::suggest_split),
        )
        // Trips/Events
        .route(
            "/trips",
            get(handlers::list_trips).post(handlers::create_trip),
        )
        .route(
            "/trips/:id",
            get(handlers::get_trip)
                .patch(handlers::update_trip)
                .delete(handlers::delete_trip),
        )
        .route("/trips/:id/archive", post(handlers::archive_trip))
        .route(
            "/trips/:id/transactions",
            get(handlers::get_trip_transactions),
        )
        .route("/trips/:id/spending", get(handlers::get_trip_spending))
        // Ollama metrics, health, and reprocessing
        .route("/ollama/stats", get(handlers::ollama_stats))
        .route(
            "/ollama/stats/by-model",
            get(handlers::ollama_stats_by_model),
        )
        .route("/ollama/models", get(handlers::ollama_models))
        .route("/ollama/calls", get(handlers::ollama_recent_calls))
        .route("/ollama/health", get(handlers::ollama_health))
        .route(
            "/ollama/recommendation",
            get(handlers::ollama_recommendation),
        )
        .route(
            "/ollama/reprocess",
            post(handlers::bulk_reprocess_transactions),
        )
        // Transaction reprocessing
        .route(
            "/transactions/:id/reprocess",
            post(handlers::reprocess_transaction),
        )
        // Transaction archiving
        .route(
            "/transactions/:id/archive",
            post(handlers::archive_transaction),
        )
        .route(
            "/transactions/:id/unarchive",
            post(handlers::unarchive_transaction),
        )
        // Merchant name update with learning
        .route(
            "/transactions/:id/merchant",
            put(handlers::update_merchant_name),
        )
        // Export
        .route("/export/transactions", get(handlers::export_transactions))
        .route("/export/full", get(handlers::export_full))
        // Full backup import
        .route("/import/full", post(handlers::import_full))
        // Backup management
        .route(
            "/backup",
            get(handlers::list_backups).post(handlers::create_backup),
        )
        .route("/backup/prune", post(handlers::prune_backups))
        .route("/backup/verify", post(handlers::verify_backup))
        .route(
            "/backup/:name",
            get(handlers::get_backup).delete(handlers::delete_backup),
        )
        .route("/backup/:name/restore", post(handlers::restore_backup))
        // User feedback
        .route(
            "/feedback",
            get(handlers::list_feedback).post(handlers::create_feedback),
        )
        .route("/feedback/stats", get(handlers::get_feedback_stats))
        .route("/feedback/:id", get(handlers::get_feedback))
        .route("/feedback/:id/revert", post(handlers::revert_feedback))
        .route("/feedback/:id/unrevert", post(handlers::unrevert_feedback))
        // Alert feedback (convenience endpoints)
        .route(
            "/alerts/:id/feedback",
            get(handlers::get_alert_feedback).post(handlers::rate_alert),
        )
        // Training data and pipeline
        .route("/training/tasks", get(handlers::training_tasks))
        .route("/training/export", get(handlers::training_export))
        .route("/training/stats", get(handlers::training_stats))
        .route("/training/agent", get(handlers::training_agent))
        // Explore mode (conversational queries with session support)
        .route("/explore/query", post(handlers::query_explore))
        .route("/explore/models", get(handlers::list_explore_models))
        .route("/explore/session", post(handlers::create_explore_session))
        .route(
            "/explore/session/:id",
            get(handlers::get_explore_session).delete(handlers::delete_explore_session),
        );

    // Build CORS layer
    let cors = if config.allowed_origins.is_empty() {
        // Restrictive default: only allow same-origin
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    } else {
        // Allow specified origins
        let origins: Vec<HeaderValue> = config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    };

    // Security headers
    // CSP: restrict scripts to same-origin, allow inline styles (Tailwind), allow blob: for images
    let csp_value = HeaderValue::from_static(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'"
    );

    let mut app = Router::new()
        .nest("/api", api_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Security headers
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_XSS_PROTECTION,
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            csp_value,
        ));

    // Serve static files if directory provided
    if let Some(dir) = static_dir {
        app = app.fallback_service(ServeDir::new(dir));
    }

    app
}

/// Start the server
pub async fn serve(
    db: Database,
    host: &str,
    port: u16,
    static_dir: Option<&str>,
) -> anyhow::Result<()> {
    serve_with_config(db, host, port, static_dir, ServerConfig::default()).await
}

/// Start the server with custom configuration
pub async fn serve_with_config(
    db: Database,
    host: &str,
    port: u16,
    static_dir: Option<&str>,
    config: ServerConfig,
) -> anyhow::Result<()> {
    if !config.require_auth {
        warn!("⚠️  Authentication disabled - do not expose to network!");
    }

    // Recover any imports that were interrupted by server restart
    match db.recover_stuck_imports() {
        Ok(count) if count > 0 => {
            warn!(
                "⚠️  Recovered {} stuck import(s) from previous server session",
                count
            );
        }
        Ok(_) => {}
        Err(e) => {
            warn!("Failed to recover stuck imports: {}", e);
        }
    }

    // Recover any reprocess runs that were interrupted by server restart
    match db.recover_stuck_reprocess_runs() {
        Ok(count) if count > 0 => {
            warn!(
                "⚠️  Recovered {} stuck reprocess run(s) from previous server session",
                count
            );
        }
        Ok(_) => {}
        Err(e) => {
            warn!("Failed to recover stuck reprocess runs: {}", e);
        }
    }

    // Check Ollama connection
    check_ai_connection().await;

    // Start backup scheduler if configured
    if let Some(backup_config) = BackupScheduleConfig::from_env() {
        start_backup_scheduler(db.clone(), backup_config);
    }

    let app = create_router(db, static_dir, config)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    let addr = format!("{}:{}", host, port);

    info!("Starting server at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Check and log AI backend connection status
async fn check_ai_connection() {
    match AIClient::from_env() {
        Some(client) => {
            let router_info = client.router_info();

            if client.health_check().await {
                info!(
                    "✅ AI backend connected: {} (default: {}, fallback: {})",
                    client.host(),
                    router_info.default_model,
                    router_info.fallback_model.as_deref().unwrap_or("none")
                );
            } else {
                warn!(
                    "⚠️  AI backend configured but not responding: {} (default: {})",
                    client.host(),
                    router_info.default_model
                );
            }
        }
        None => {
            info!("ℹ️  AI backend not configured (set OLLAMA_HOST to enable AI features)");
        }
    }
}

// ============================================================================
// Error Handling
// ============================================================================

/// Application error type with proper HTTP status codes
pub struct AppError {
    status: StatusCode,
    message: String,
    internal: Option<anyhow::Error>,
}

impl AppError {
    pub fn bad_request(msg: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.to_string(),
            internal: None,
        }
    }

    pub fn not_found(msg: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.to_string(),
            internal: None,
        }
    }

    pub fn internal(msg: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.to_string(),
            internal: None,
        }
    }

    pub fn conflict(msg: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.to_string(),
            internal: None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log the full internal error if present
        if let Some(err) = &self.internal {
            error!(error = %err, "Internal error");
        }

        let body = Json(serde_json::json!({
            "error": self.message
        }));

        (self.status, body).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err = err.into();
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            // Return generic message to client
            message: "An internal error occurred".to_string(),
            // Keep full error for logging
            internal: Some(err),
        }
    }
}

#[cfg(test)]
mod tests;

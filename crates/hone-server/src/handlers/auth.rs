//! Authentication-related handlers

use axum::extract::Request;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::{get_client_ip, get_user_email, AppState};

/// Response for the /api/me endpoint
#[derive(Serialize)]
pub struct MeResponse {
    /// The authenticated user's email or identifier
    pub user: String,
    /// How the user was authenticated
    pub auth_method: String,
}

/// Get the currently authenticated user
pub async fn get_me(
    State(state): State<Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    request: Request,
) -> Json<MeResponse> {
    let headers = request.headers();
    let header_user = get_user_email(headers);

    // Get the real client IP (respects trusted proxies)
    let client_ip = get_client_ip(
        &request,
        connect_info.as_ref(),
        &state.config.trusted_proxies,
    );

    // Check if this is a trusted network request (no CF headers, but auth passed)
    let is_trusted_network = header_user == "local-dev"
        && state.config.require_auth
        && !state.config.trusted_networks.is_empty()
        && client_ip
            .map(|ip| {
                state
                    .config
                    .trusted_networks
                    .iter()
                    .any(|net| net.contains(&ip))
            })
            .unwrap_or(false);

    let (user, auth_method) = if is_trusted_network {
        // Format IP as the user identifier
        let ip = client_ip.map(|ip| ip.to_string()).unwrap_or_default();
        (ip, "trusted_network")
    } else if header_user == "api-key" {
        (header_user, "api_key")
    } else if header_user == "local-dev" {
        (header_user, "none")
    } else if header_user.contains('@') {
        // Check if we have JWT validation configured
        let method =
            if state.config.cf_jwt.team_name.is_some() && state.config.cf_jwt.audience.is_some() {
                "cloudflare_jwt"
            } else {
                "cloudflare_header"
            };
        (header_user, method)
    } else {
        (header_user, "unknown")
    };

    Json(MeResponse {
        user,
        auth_method: auth_method.to_string(),
    })
}

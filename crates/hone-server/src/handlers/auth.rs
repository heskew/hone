//! Authentication-related handlers

use axum::extract::Request;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::{get_client_ip, get_user_email, AppState, AuthPrincipal};

/// Response for the /api/me endpoint
#[derive(Serialize)]
pub struct MeResponse {
    /// The authenticated user's email or identifier
    pub user: String,
    /// How the user was authenticated
    pub auth_method: String,
}

/// Get the currently authenticated user
pub async fn get_me(State(state): State<Arc<AppState>>, request: Request) -> Json<MeResponse> {
    // axum 0.8 removed the blanket Option<T> extractor; read ConnectInfo from
    // extensions so the handler still works when the router is built without
    // into_make_service_with_connect_info (as in tests)
    let connect_info = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .copied();
    // Get the real client IP (respects trusted proxies)
    let client_ip = get_client_ip(
        &request,
        connect_info.as_ref(),
        &state.config.trusted_proxies,
    );

    // Middleware principal. `cloudflare_jwt` is set only after the JWT check
    // succeeds — never because CF_TEAM_NAME / CF_AUD_TAG happen to be set.
    if let Some(principal) = request.extensions().get::<AuthPrincipal>().cloned() {
        if principal.method == "trusted_network" {
            let ip = client_ip.map(|ip| ip.to_string()).unwrap_or(principal.user);
            return Json(MeResponse {
                user: ip,
                auth_method: principal.method.to_string(),
            });
        }
        return Json(MeResponse {
            user: principal.user,
            auth_method: principal.method.to_string(),
        });
    }

    // `--no-auth` skips the middleware principal. Do not infer cloudflare_jwt.
    let header_user = get_user_email(request.headers());
    let (user, auth_method) = if header_user == "api-key" {
        (header_user, "api_key")
    } else if header_user == "local-dev" {
        (header_user, "none")
    } else if header_user.contains('@') {
        (header_user, "cloudflare_header")
    } else {
        (header_user, "unknown")
    };

    Json(MeResponse {
        user,
        auth_method: auth_method.to_string(),
    })
}

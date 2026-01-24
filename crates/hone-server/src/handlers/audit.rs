//! Audit log handlers

use std::sync::Arc;

use axum::{
    extract::{Query, Request, State},
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState, MAX_PAGE_LIMIT};
use hone_core::AuditEntry;

/// Query parameters for audit log
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default = "default_audit_limit")]
    pub limit: i64,
}

fn default_audit_limit() -> i64 {
    100
}

/// GET /api/audit - List audit log entries
pub async fn list_audit_log(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditQuery>,
    request: Request,
) -> Result<Json<Vec<AuditEntry>>, AppError> {
    let user_email = get_user_email(request.headers());
    let limit = params.limit.max(1).min(MAX_PAGE_LIMIT);

    let entries = state.db.list_audit_log(limit)?;

    // Audit log - viewing the audit log itself
    state.db.log_audit(
        &user_email,
        "list",
        Some("audit_log"),
        None,
        Some(&format!("limit={}", limit)),
    )?;

    Ok(Json(entries))
}

//! Subscription management handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::Subscription;

/// Query params for listing subscriptions
#[derive(Debug, Deserialize)]
pub struct ListSubscriptionsQuery {
    /// Filter by account ID
    pub account_id: Option<i64>,
}

/// GET /api/subscriptions - List all subscriptions
pub async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSubscriptionsQuery>,
    request: Request,
) -> Result<Json<Vec<Subscription>>, AppError> {
    let user_email = get_user_email(request.headers());

    let subscriptions = state.db.list_subscriptions(query.account_id)?;

    // Audit log - read access
    state.db.log_audit(
        &user_email,
        "list",
        Some("subscription"),
        None,
        Some(&format!(
            "count={}, account_id={:?}",
            subscriptions.len(),
            query.account_id
        )),
    )?;

    Ok(Json(subscriptions))
}

/// POST /api/subscriptions/:id/acknowledge - Mark subscription as known
pub async fn acknowledge_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.acknowledge_subscription(id)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "acknowledge",
        Some("subscription"),
        Some(id),
        None,
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// Response for cancelling a subscription
#[derive(Serialize)]
pub struct CancelResponse {
    pub success: bool,
    pub id: i64,
}

/// POST /api/subscriptions/:id/cancel - Mark subscription as cancelled
pub async fn cancel_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<CancelResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.cancel_subscription(id, None)?;

    state
        .db
        .log_audit(&user_email, "cancel", Some("subscription"), Some(id), None)?;

    Ok(Json(CancelResponse { success: true, id }))
}

/// POST /api/subscriptions/:id/exclude - Exclude from detection (not a subscription)
///
/// Marks a subscription as "excluded" so it won't be flagged by future detection.
/// This is used when a user says "this is not a subscription" (e.g., grocery store).
/// Also updates the merchant_subscription_cache to remember this choice.
pub async fn exclude_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify subscription exists
    state
        .db
        .get_subscription(id)?
        .ok_or_else(|| AppError::not_found(&format!("Subscription {} not found", id)))?;

    state.db.exclude_subscription(id)?;

    state.db.log_audit(
        &user_email,
        "exclude",
        Some("subscription"),
        Some(id),
        Some("marked as not a subscription"),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/subscriptions/:id/unexclude - Re-enable detection
///
/// Reverses the exclude action, setting the subscription back to active
/// and removing the user override from the cache.
pub async fn unexclude_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify subscription exists
    state
        .db
        .get_subscription(id)?
        .ok_or_else(|| AppError::not_found(&format!("Subscription {} not found", id)))?;

    state.db.unexclude_subscription(id)?;

    state.db.log_audit(
        &user_email,
        "unexclude",
        Some("subscription"),
        Some(id),
        Some("re-enabled detection"),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// DELETE /api/subscriptions/:id - Delete a subscription
///
/// Permanently removes a subscription and its associated alerts.
/// Use this to remove false positive subscriptions. The merchant's
/// cached classification is also cleared.
pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify subscription exists and get merchant for logging
    let subscription = state
        .db
        .get_subscription(id)?
        .ok_or_else(|| AppError::not_found(&format!("Subscription {} not found", id)))?;

    state.db.delete_subscription(id)?;

    state.db.log_audit(
        &user_email,
        "delete",
        Some("subscription"),
        Some(id),
        Some(&format!("merchant={}", subscription.merchant)),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

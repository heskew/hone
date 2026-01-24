//! User feedback handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::{
    FeedbackContext, FeedbackStats, FeedbackTargetType, FeedbackType, NewUserFeedback, UserFeedback,
};

/// Query parameters for listing feedback
#[derive(Debug, Deserialize)]
pub struct FeedbackQuery {
    /// Filter by target type (alert, insight, classification, explanation, receipt_match)
    pub target_type: Option<String>,
    /// Filter by feedback type (helpful, not_helpful, correction, dismissal)
    pub feedback_type: Option<String>,
    /// Include reverted feedback (default: false)
    #[serde(default)]
    pub include_reverted: bool,
    /// Max results (default: 100)
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    100
}

/// Request body for creating feedback
#[derive(Debug, Deserialize)]
pub struct CreateFeedbackRequest {
    pub feedback_type: String,
    pub target_type: String,
    pub target_id: Option<i64>,
    pub original_value: Option<String>,
    pub corrected_value: Option<String>,
    pub reason: Option<String>,
    pub context: Option<FeedbackContext>,
}

/// Request body for simple helpful/not helpful feedback
#[derive(Debug, Deserialize)]
pub struct SimpleFeedbackRequest {
    pub helpful: bool,
    pub reason: Option<String>,
}

/// Response for feedback creation
#[derive(Debug, Serialize)]
pub struct FeedbackResponse {
    pub id: i64,
    pub feedback: UserFeedback,
}

/// GET /api/feedback - List feedback records
pub async fn list_feedback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FeedbackQuery>,
    request: Request,
) -> Result<Json<Vec<UserFeedback>>, AppError> {
    let user_email = get_user_email(request.headers());

    let target_type = params
        .target_type
        .as_ref()
        .and_then(|s| s.parse::<FeedbackTargetType>().ok());

    let feedback_type = params
        .feedback_type
        .as_ref()
        .and_then(|s| s.parse::<FeedbackType>().ok());

    let limit = params.limit.min(crate::MAX_PAGE_LIMIT);

    let feedback = state.db.list_feedback(
        target_type,
        feedback_type,
        params.include_reverted,
        limit,
        params.offset,
    )?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("feedback"),
        None,
        Some(&format!("count={}", feedback.len())),
    )?;

    Ok(Json(feedback))
}

/// GET /api/feedback/stats - Get feedback statistics
pub async fn get_feedback_stats(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<FeedbackStats>, AppError> {
    let user_email = get_user_email(request.headers());

    let stats = state.db.get_feedback_stats()?;

    state
        .db
        .log_audit(&user_email, "view", Some("feedback_stats"), None, None)?;

    Ok(Json(stats))
}

/// GET /api/feedback/:id - Get a specific feedback record
pub async fn get_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<UserFeedback>, AppError> {
    let user_email = get_user_email(request.headers());

    let feedback = state.db.get_feedback(id)?;

    state
        .db
        .log_audit(&user_email, "view", Some("feedback"), Some(id), None)?;

    Ok(Json(feedback))
}

/// POST /api/feedback - Create a new feedback record
pub async fn create_feedback(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<FeedbackResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let body: CreateFeedbackRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let feedback_type: FeedbackType = body.feedback_type.parse().map_err(|_| {
        AppError::bad_request(&format!("Invalid feedback_type: {}", body.feedback_type))
    })?;

    let target_type: FeedbackTargetType = body.target_type.parse().map_err(|_| {
        AppError::bad_request(&format!("Invalid target_type: {}", body.target_type))
    })?;

    let new_feedback = NewUserFeedback {
        feedback_type,
        target_type,
        target_id: body.target_id,
        original_value: body.original_value,
        corrected_value: body.corrected_value,
        reason: body.reason,
        context: body.context,
    };

    let id = state.db.create_feedback(&new_feedback)?;
    let feedback = state.db.get_feedback(id)?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("feedback"),
        Some(id),
        Some(&format!(
            "type={}, target={}:{:?}",
            feedback_type.as_str(),
            target_type.as_str(),
            body.target_id
        )),
    )?;

    Ok(Json(FeedbackResponse { id, feedback }))
}

/// POST /api/feedback/:id/revert - Revert (undo) a feedback record
pub async fn revert_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.revert_feedback(id)?;

    state
        .db
        .log_audit(&user_email, "revert", Some("feedback"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/feedback/:id/unrevert - Unrevert (restore) a reverted feedback record
pub async fn unrevert_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.unrevert_feedback(id)?;

    state
        .db
        .log_audit(&user_email, "unrevert", Some("feedback"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/alerts/:id/feedback - Rate an alert's helpfulness
pub async fn rate_alert(
    State(state): State<Arc<AppState>>,
    Path(alert_id): Path<i64>,
    request: Request,
) -> Result<Json<FeedbackResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let body: SimpleFeedbackRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    // Verify alert exists
    let _alert = state.db.get_alert(alert_id)?;

    let id = state
        .db
        .record_explanation_feedback(alert_id, body.helpful, body.reason, None)?;

    let feedback = state.db.get_feedback(id)?;

    state.db.log_audit(
        &user_email,
        "rate",
        Some("alert"),
        Some(alert_id),
        Some(&format!("helpful={}", body.helpful)),
    )?;

    Ok(Json(FeedbackResponse { id, feedback }))
}

/// GET /api/alerts/:id/feedback - Get feedback for a specific alert
pub async fn get_alert_feedback(
    State(state): State<Arc<AppState>>,
    Path(alert_id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<UserFeedback>>, AppError> {
    let user_email = get_user_email(request.headers());

    let feedback = state
        .db
        .list_feedback_for_target(FeedbackTargetType::Alert, alert_id)?;

    state.db.log_audit(
        &user_email,
        "view",
        Some("alert_feedback"),
        Some(alert_id),
        Some(&format!("count={}", feedback.len())),
    )?;

    Ok(Json(feedback))
}

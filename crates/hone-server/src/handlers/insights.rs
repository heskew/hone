//! Insight handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::insights::{
    AnalysisContext, InsightEngine, InsightFinding, InsightStatus, InsightType,
};

/// Query parameters for listing insights
#[derive(Debug, Deserialize)]
pub struct InsightQuery {
    /// Filter by status (active, dismissed, snoozed)
    pub status: Option<String>,
    /// Filter by insight type
    pub insight_type: Option<String>,
    /// Limit for top insights (default 5)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

/// Request body for snoozing an insight
#[derive(Debug, Deserialize)]
pub struct SnoozeRequest {
    /// Number of days to snooze
    pub days: u32,
}

/// Request body for feedback on an insight
#[derive(Debug, Deserialize)]
pub struct InsightFeedbackRequest {
    /// User's feedback text
    pub feedback: String,
}

/// Response for insight refresh
#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub count: usize,
}

/// GET /api/insights - Get top N insights for dashboard
///
/// Returns the most relevant active insights, sorted by severity and recency.
pub async fn get_top_insights(
    State(state): State<Arc<AppState>>,
    Query(params): Query<InsightQuery>,
    request: Request,
) -> Result<Json<Vec<InsightFinding>>, AppError> {
    let user_email = get_user_email(request.headers());

    let insights = state.db.get_top_insights(params.limit)?;

    // Audit log - read access
    state.db.log_audit(
        &user_email,
        "list",
        Some("insight"),
        None,
        Some(&format!("limit={}, count={}", params.limit, insights.len())),
    )?;

    Ok(Json(insights))
}

/// GET /api/insights/all - List all insights with optional filters
pub async fn list_insights(
    State(state): State<Arc<AppState>>,
    Query(params): Query<InsightQuery>,
    request: Request,
) -> Result<Json<Vec<InsightFinding>>, AppError> {
    let user_email = get_user_email(request.headers());

    let status = params
        .status
        .as_ref()
        .and_then(|s| s.parse::<InsightStatus>().ok());

    let mut insights = state.db.list_insight_findings(status)?;

    // Filter by insight type if specified
    if let Some(ref type_str) = params.insight_type {
        if let Ok(insight_type) = type_str.parse::<InsightType>() {
            insights.retain(|i| i.insight_type == insight_type);
        }
    }

    // Audit log
    state.db.log_audit(
        &user_email,
        "list",
        Some("insight"),
        None,
        Some(&format!(
            "status={:?}, type={:?}, count={}",
            params.status,
            params.insight_type,
            insights.len()
        )),
    )?;

    Ok(Json(insights))
}

/// GET /api/insights/:id - Get a specific insight
pub async fn get_insight(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<InsightFinding>, AppError> {
    let user_email = get_user_email(request.headers());

    let insight = state
        .db
        .get_insight_finding(id)?
        .ok_or_else(|| AppError::not_found("Insight not found"))?;

    // Audit log
    state
        .db
        .log_audit(&user_email, "view", Some("insight"), Some(id), None)?;

    Ok(Json(insight))
}

/// POST /api/insights/:id/dismiss - Dismiss an insight
pub async fn dismiss_insight(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Check insight exists
    state
        .db
        .get_insight_finding(id)?
        .ok_or_else(|| AppError::not_found("Insight not found"))?;

    state.db.dismiss_insight(id)?;

    // Audit log
    state
        .db
        .log_audit(&user_email, "dismiss", Some("insight"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/insights/:id/snooze - Snooze an insight for N days
pub async fn snooze_insight(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SnoozeRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(&headers);

    // Validate days (1-90)
    if body.days < 1 || body.days > 90 {
        return Err(AppError::bad_request("Days must be between 1 and 90"));
    }

    // Check insight exists
    state
        .db
        .get_insight_finding(id)?
        .ok_or_else(|| AppError::not_found("Insight not found"))?;

    state.db.snooze_insight(id, body.days)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "snooze",
        Some("insight"),
        Some(id),
        Some(&format!("days={}", body.days)),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/insights/:id/restore - Restore a dismissed or snoozed insight
pub async fn restore_insight(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Check insight exists
    state
        .db
        .get_insight_finding(id)?
        .ok_or_else(|| AppError::not_found("Insight not found"))?;

    state.db.restore_insight(id)?;

    // Audit log
    state
        .db
        .log_audit(&user_email, "restore", Some("insight"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/insights/:id/feedback - Set feedback on an insight
pub async fn set_insight_feedback(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<InsightFeedbackRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(&headers);

    // Check insight exists
    state
        .db
        .get_insight_finding(id)?
        .ok_or_else(|| AppError::not_found("Insight not found"))?;

    state.db.set_insight_feedback(id, &body.feedback)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "feedback",
        Some("insight"),
        Some(id),
        Some(&format!("feedback_len={}", body.feedback.len())),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/insights/refresh - Re-run insight analysis
///
/// Runs all insight analyzers and persists the findings.
pub async fn refresh_insights(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<RefreshResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let engine = InsightEngine::new();
    let ctx = AnalysisContext::current_month(&state.db, state.ai.as_ref());

    let count = engine.run_and_persist(&ctx).await?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "refresh",
        Some("insight"),
        None,
        Some(&format!("findings_count={}", count)),
    )?;

    Ok(Json(RefreshResponse { count }))
}

/// GET /api/insights/count - Get count of active insights
pub async fn count_insights(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<i64>, AppError> {
    let user_email = get_user_email(request.headers());

    let count = state.db.count_active_insights()?;

    // Audit log
    state
        .db
        .log_audit(&user_email, "count", Some("insight"), None, None)?;

    Ok(Json(count))
}

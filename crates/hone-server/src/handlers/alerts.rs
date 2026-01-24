//! Alert handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use chrono::{Datelike, Utc};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::ai::AIBackend;
use hone_core::models::{Alert, AlertType, DashboardStats, FeedbackTargetType};

/// Query parameters for listing alerts
#[derive(Debug, Deserialize)]
pub struct AlertQuery {
    #[serde(default)]
    pub include_dismissed: bool,
}

/// GET /api/alerts - List alerts
pub async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlertQuery>,
    request: Request,
) -> Result<Json<Vec<Alert>>, AppError> {
    let user_email = get_user_email(request.headers());

    let alerts = state.db.list_alerts(params.include_dismissed)?;

    // Audit log - read access
    state.db.log_audit(
        &user_email,
        "list",
        Some("alert"),
        None,
        Some(&format!(
            "include_dismissed={}, count={}",
            params.include_dismissed,
            alerts.len()
        )),
    )?;

    Ok(Json(alerts))
}

/// POST /api/alerts/:id/dismiss - Dismiss an alert
pub async fn dismiss_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.dismiss_alert(id)?;

    // Audit log
    state
        .db
        .log_audit(&user_email, "dismiss", Some("alert"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/alerts/:id/dismiss-exclude - Dismiss an alert and exclude the subscription
///
/// Dismisses the alert and marks the associated subscription as "not a subscription"
/// so it won't be flagged by future detection.
pub async fn dismiss_alert_exclude(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get the alert first to find its subscription_id
    let alert = state.db.get_alert(id)?;

    // Dismiss the alert
    state.db.dismiss_alert(id)?;

    // Exclude the subscription if it exists
    if let Some(subscription_id) = alert.subscription_id {
        state.db.exclude_subscription(subscription_id)?;

        // Audit log for exclude
        state.db.log_audit(
            &user_email,
            "exclude",
            Some("subscription"),
            Some(subscription_id),
            Some("marked as not a subscription"),
        )?;
    }

    // Audit log for dismiss
    state.db.log_audit(
        &user_email,
        "dismiss",
        Some("alert"),
        Some(id),
        Some("exclude_merchant=true"),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/alerts/:id/restore - Restore (undismiss) an alert
pub async fn restore_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.restore_alert(id)?;

    // Audit log
    state
        .db
        .log_audit(&user_email, "restore", Some("alert"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// GET /api/dashboard - Dashboard statistics
pub async fn get_dashboard(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<DashboardStats>, AppError> {
    let user_email = get_user_email(request.headers());

    let stats = state.db.get_dashboard_stats()?;

    // Audit log - read access
    state
        .db
        .log_audit(&user_email, "view", Some("dashboard"), None, None)?;

    Ok(Json(stats))
}

/// POST /api/alerts/:id/reanalyze - Re-run Ollama analysis for spending anomaly
///
/// Re-runs the Ollama analysis for a spending anomaly alert using the current model.
/// This allows comparing results between different models or prompts.
pub async fn reanalyze_spending_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Alert>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get the alert
    let alert = state.db.get_alert(id)?;

    // Must be a spending anomaly alert
    if alert.alert_type != AlertType::SpendingAnomaly {
        return Err(AppError::bad_request(
            "Only spending anomaly alerts can be reanalyzed",
        ));
    }

    // Get spending anomaly data
    let anomaly = alert
        .spending_anomaly
        .ok_or_else(|| AppError::bad_request("Alert missing spending anomaly data"))?;

    // AI backend must be available
    let ai = state
        .ai
        .as_ref()
        .ok_or_else(|| AppError::bad_request("AI backend not configured"))?;

    // Calculate the date ranges for merchant lookup
    let today = Utc::now().date_naive();
    let current_month_start = today.with_day(1).expect("Day 1 always valid");
    let baseline_end = current_month_start - chrono::Duration::days(1);
    let baseline_start = baseline_end - chrono::Duration::days(90);

    // Get top merchants for this category in current period
    let merchants_report = state.db.get_top_merchants(
        current_month_start,
        today,
        5,
        Some(&anomaly.tag_name),
        None,
        None,
    )?;

    // Get top merchants for baseline period
    let baseline_merchants = state.db.get_top_merchants(
        baseline_start,
        baseline_end,
        10,
        Some(&anomaly.tag_name),
        None,
        None,
    )?;

    // Format merchant data
    let top_merchants: Vec<(String, f64, i32)> = merchants_report
        .merchants
        .iter()
        .map(|m| {
            (
                m.merchant.clone(),
                m.amount.abs(),
                m.transaction_count as i32,
            )
        })
        .collect();

    // Find new merchants
    let baseline_names: std::collections::HashSet<_> = baseline_merchants
        .merchants
        .iter()
        .map(|m| m.merchant.to_lowercase())
        .collect();

    let new_merchants: Vec<String> = merchants_report
        .merchants
        .iter()
        .filter(|m| !baseline_names.contains(&m.merchant.to_lowercase()))
        .map(|m| m.merchant.clone())
        .collect();

    // Get transaction counts from merchants report
    let current_tx_count: i32 = merchants_report
        .merchants
        .iter()
        .map(|m| m.transaction_count as i32)
        .sum();
    let baseline_tx_count: i32 = baseline_merchants
        .merchants
        .iter()
        .map(|m| m.transaction_count as i32)
        .sum();

    // Get user feedback to improve response quality
    let feedback = state
        .db
        .get_feedback_summary_for_prompt(FeedbackTargetType::Explanation)
        .ok()
        .filter(|f| !f.is_empty());

    // Call AI backend
    let explanation = ai
        .explain_spending_change(
            &anomaly.tag_name,
            anomaly.baseline_amount,
            anomaly.current_amount,
            baseline_tx_count,
            current_tx_count,
            &top_merchants,
            &new_merchants,
            feedback.as_deref(),
        )
        .await
        .map_err(|e| AppError::bad_request(&format!("Ollama analysis failed: {}", e)))?;

    // Update the alert with the new analysis
    state.db.update_spending_analysis(id, &explanation)?;

    // Reload the alert to get the updated data
    let updated_alert = state.db.get_alert(id)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "reanalyze",
        Some("alert"),
        Some(id),
        Some(&format!("model={}", explanation.model)),
    )?;

    Ok(Json(updated_alert))
}

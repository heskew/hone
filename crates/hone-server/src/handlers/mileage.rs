//! Mileage log handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState};
use hone_core::models::{MileageLog, NewMileageLog};

#[derive(Debug, Deserialize)]
pub struct CreateMileageLogRequest {
    pub date: String,
    pub odometer: f64,
    pub note: Option<String>,
}

/// POST /api/entities/:id/mileage - Create a mileage log entry
pub async fn create_mileage_log(
    State(state): State<Arc<AppState>>,
    Path(entity_id): Path<i64>,
    request: Request,
) -> Result<Json<MileageLog>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let body: CreateMileageLogRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let date = chrono::NaiveDate::parse_from_str(&body.date, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("Invalid date format (use YYYY-MM-DD)"))?;

    let new_log = NewMileageLog {
        entity_id,
        date,
        odometer: body.odometer,
        note: body.note,
    };

    let log_id = state.db.create_mileage_log(&new_log)?;
    let log = state
        .db
        .get_mileage_log(log_id)?
        .ok_or_else(|| AppError::internal("Failed to fetch created mileage log"))?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("mileage_log"),
        Some(log_id),
        Some(&format!(
            "entity_id={}, odometer={}",
            entity_id, body.odometer
        )),
    )?;

    Ok(Json(log))
}

/// GET /api/entities/:id/mileage - Get mileage logs for a vehicle
pub async fn list_mileage_logs(
    State(state): State<Arc<AppState>>,
    Path(entity_id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<MileageLog>>, AppError> {
    let user_email = get_user_email(request.headers());

    let logs = state.db.get_mileage_logs(entity_id)?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("mileage_logs"),
        Some(entity_id),
        Some(&format!("count={}", logs.len())),
    )?;

    Ok(Json(logs))
}

/// DELETE /api/mileage/:id - Delete a mileage log entry
pub async fn delete_mileage_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<StatusCode, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.delete_mileage_log(id)?;

    state
        .db
        .log_audit(&user_email, "delete", Some("mileage_log"), Some(id), None)?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/entities/:id/miles - Get total miles driven for a vehicle
pub async fn get_vehicle_total_miles(
    State(state): State<Arc<AppState>>,
    Path(entity_id): Path<i64>,
    request: Request,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_email = get_user_email(request.headers());

    let miles = state.db.get_vehicle_total_miles(entity_id)?;

    state.db.log_audit(
        &user_email,
        "read",
        Some("vehicle_miles"),
        Some(entity_id),
        None,
    )?;

    Ok(Json(serde_json::json!({
        "entity_id": entity_id,
        "total_miles": miles
    })))
}

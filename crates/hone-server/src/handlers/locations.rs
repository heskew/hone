//! Location management handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Request, State},
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::{Location, NewLocation};

/// GET /api/locations - List all locations
pub async fn list_locations(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Vec<Location>>, AppError> {
    let user_email = get_user_email(request.headers());

    let locations = state.db.list_locations()?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("location"),
        None,
        Some(&format!("count={}", locations.len())),
    )?;

    Ok(Json(locations))
}

/// GET /api/locations/:id - Get a specific location
pub async fn get_location(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Location>, AppError> {
    let user_email = get_user_email(request.headers());

    let location = state
        .db
        .get_location(id)?
        .ok_or_else(|| AppError::not_found("Location not found"))?;

    state
        .db
        .log_audit(&user_email, "view", Some("location"), Some(id), None)?;

    Ok(Json(location))
}

/// Request body for creating a location
#[derive(Debug, Deserialize)]
pub struct CreateLocationRequest {
    pub name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_type: Option<String>,
}

/// POST /api/locations - Create a new location
pub async fn create_location(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Location>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: CreateLocationRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let location_type = req
        .location_type
        .map(|t| t.parse())
        .transpose()
        .map_err(|_| {
            AppError::bad_request("Invalid location_type. Valid: home, work, store, online, travel")
        })?;

    let new_location = NewLocation {
        name: req.name,
        address: req.address,
        city: req.city.clone(),
        state: req.state,
        country: req.country,
        latitude: req.latitude,
        longitude: req.longitude,
        location_type,
    };

    let location_id = state.db.create_location(&new_location)?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("location"),
        Some(location_id),
        Some(&format!("city={:?}", req.city)),
    )?;

    let location = state
        .db
        .get_location(location_id)?
        .ok_or_else(|| AppError::internal("Location not found after creation"))?;

    Ok(Json(location))
}

/// DELETE /api/locations/:id - Delete a location
pub async fn delete_location(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify location exists
    state
        .db
        .get_location(id)?
        .ok_or_else(|| AppError::not_found("Location not found"))?;

    state.db.delete_location(id)?;

    state
        .db
        .log_audit(&user_email, "delete", Some("location"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

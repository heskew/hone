//! Trip/event management handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState};
use hone_core::models::{NewTrip, Transaction, Trip, TripWithSpending};

#[derive(Debug, Deserialize)]
pub struct CreateTripRequest {
    pub name: String,
    pub description: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location_id: Option<i64>,
    pub budget: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTripRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location_id: Option<i64>,
    pub budget: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ListTripsQuery {
    pub include_archived: Option<bool>,
}

/// POST /api/trips - Create a new trip/event
pub async fn create_trip(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Trip>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let body: CreateTripRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let start_date = body
        .start_date
        .as_ref()
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::bad_request("Invalid start_date format (use YYYY-MM-DD)"))?;

    let end_date = body
        .end_date
        .as_ref()
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::bad_request("Invalid end_date format (use YYYY-MM-DD)"))?;

    let new_trip = NewTrip {
        name: body.name.clone(),
        description: body.description,
        start_date,
        end_date,
        location_id: body.location_id,
        budget: body.budget,
    };

    let trip_id = state.db.create_trip(&new_trip)?;
    let trip = state
        .db
        .get_trip(trip_id)?
        .ok_or_else(|| AppError::internal("Failed to fetch created trip"))?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("trip"),
        Some(trip_id),
        Some(&format!("name={}", body.name)),
    )?;

    Ok(Json(trip))
}

/// GET /api/trips - List all trips
pub async fn list_trips(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTripsQuery>,
    request: Request,
) -> Result<Json<Vec<Trip>>, AppError> {
    let user_email = get_user_email(request.headers());

    let include_archived = params.include_archived.unwrap_or(false);
    let trips = state.db.list_trips(include_archived)?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("trips"),
        None,
        Some(&format!("count={}", trips.len())),
    )?;

    Ok(Json(trips))
}

/// GET /api/trips/:id - Get a single trip
pub async fn get_trip(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Trip>, AppError> {
    let user_email = get_user_email(request.headers());

    let trip = state
        .db
        .get_trip(id)?
        .ok_or_else(|| AppError::not_found("Trip not found"))?;

    state
        .db
        .log_audit(&user_email, "read", Some("trip"), Some(id), None)?;

    Ok(Json(trip))
}

/// PATCH /api/trips/:id - Update a trip
pub async fn update_trip(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Trip>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let body: UpdateTripRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    // Parse dates if provided - wrap in Option<Option<>> for nullable field updates
    let start_date: Option<Option<chrono::NaiveDate>> = if body.start_date.is_some() {
        Some(
            body.start_date
                .as_ref()
                .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                .transpose()
                .map_err(|_| AppError::bad_request("Invalid start_date format"))?,
        )
    } else {
        None
    };

    let end_date: Option<Option<chrono::NaiveDate>> = if body.end_date.is_some() {
        Some(
            body.end_date
                .as_ref()
                .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                .transpose()
                .map_err(|_| AppError::bad_request("Invalid end_date format"))?,
        )
    } else {
        None
    };

    state.db.update_trip(
        id,
        body.name.as_deref(),
        body.description.as_deref(),
        start_date,
        end_date,
        body.location_id.map(Some),
        body.budget.map(Some),
    )?;

    // Fetch updated trip
    let trip = state
        .db
        .get_trip(id)?
        .ok_or_else(|| AppError::not_found("Trip not found"))?;

    state
        .db
        .log_audit(&user_email, "update", Some("trip"), Some(id), None)?;

    Ok(Json(trip))
}

/// DELETE /api/trips/:id - Delete a trip
pub async fn delete_trip(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<StatusCode, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.delete_trip(id)?;

    state
        .db
        .log_audit(&user_email, "delete", Some("trip"), Some(id), None)?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/trips/:id/archive - Archive a trip
pub async fn archive_trip(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Trip>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.archive_trip(id)?;
    let trip = state
        .db
        .get_trip(id)?
        .ok_or_else(|| AppError::not_found("Trip not found"))?;

    state
        .db
        .log_audit(&user_email, "archive", Some("trip"), Some(id), None)?;

    Ok(Json(trip))
}

/// GET /api/trips/:id/transactions - Get transactions assigned to a trip
pub async fn get_trip_transactions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<Transaction>>, AppError> {
    let user_email = get_user_email(request.headers());

    let transactions = state.db.get_trip_transactions(id)?;

    state.db.log_audit(
        &user_email,
        "read",
        Some("trip_transactions"),
        Some(id),
        Some(&format!("count={}", transactions.len())),
    )?;

    Ok(Json(transactions))
}

/// GET /api/trips/:id/spending - Get spending summary for a trip
pub async fn get_trip_spending(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<TripWithSpending>, AppError> {
    let user_email = get_user_email(request.headers());

    let trip = state
        .db
        .get_trip(id)?
        .ok_or_else(|| AppError::not_found("Trip not found"))?;

    let (total_spent, transaction_count) = state.db.get_trip_spending(id)?;

    // Get location name if trip has a location
    let location_name = if let Some(loc_id) = trip.location_id {
        state
            .db
            .get_location(loc_id)?
            .map(|l| l.name.unwrap_or_default())
    } else {
        None
    };

    let spending = TripWithSpending {
        trip,
        total_spent,
        transaction_count,
        location_name,
    };

    state
        .db
        .log_audit(&user_email, "read", Some("trip_spending"), Some(id), None)?;

    Ok(Json(spending))
}

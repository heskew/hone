//! Transaction split handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Request, State},
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::{
    NewTransactionSplit, SplitType, TransactionSplit, TransactionSplitWithDetails,
};

/// GET /api/transactions/:id/splits - Get splits for a transaction
pub async fn get_transaction_splits(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<TransactionSplitWithDetails>>, AppError> {
    let user_email = get_user_email(request.headers());

    let splits = state.db.get_splits_with_details(transaction_id)?;

    state.db.log_audit(
        &user_email,
        "view",
        Some("transaction_splits"),
        Some(transaction_id),
        Some(&format!("count={}", splits.len())),
    )?;

    Ok(Json(splits))
}

/// GET /api/splits/:id - Get a specific split
pub async fn get_split(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<TransactionSplit>, AppError> {
    let user_email = get_user_email(request.headers());

    let split = state
        .db
        .get_split_by_id(id)?
        .ok_or_else(|| AppError::not_found("Split not found"))?;

    state
        .db
        .log_audit(&user_email, "view", Some("split"), Some(id), None)?;

    Ok(Json(split))
}

/// Request body for creating a split
#[derive(Debug, Deserialize)]
pub struct CreateSplitRequest {
    pub amount: f64,
    pub description: Option<String>,
    #[serde(default = "default_split_type")]
    pub split_type: String,
    pub entity_id: Option<i64>,
    pub purchaser_id: Option<i64>,
}

fn default_split_type() -> String {
    "item".to_string()
}

/// POST /api/transactions/:id/splits - Create a new split for a transaction
pub async fn create_split(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<i64>,
    request: Request,
) -> Result<Json<TransactionSplit>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: CreateSplitRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let split_type: SplitType = req.split_type.parse().map_err(|_| {
        AppError::bad_request("Invalid split_type. Valid: item, tax, tip, fee, discount, rewards")
    })?;

    let new_split = NewTransactionSplit {
        transaction_id,
        amount: req.amount,
        description: req.description.clone(),
        split_type,
        entity_id: req.entity_id,
        purchaser_id: req.purchaser_id,
    };

    let split_id = state.db.create_split(&new_split)?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("split"),
        Some(split_id),
        Some(&format!(
            "tx={}, amount={:.2}, type={}",
            transaction_id, req.amount, req.split_type
        )),
    )?;

    let split = state
        .db
        .get_split_by_id(split_id)?
        .ok_or_else(|| AppError::internal("Split not found after creation"))?;

    Ok(Json(split))
}

/// Request body for updating a split
#[derive(Debug, Deserialize)]
pub struct UpdateSplitRequest {
    pub amount: Option<f64>,
    pub description: Option<String>,
    pub split_type: Option<String>,
    pub entity_id: Option<Option<i64>>,
    pub purchaser_id: Option<Option<i64>>,
}

/// PATCH /api/splits/:id - Update a split
pub async fn update_split(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<TransactionSplit>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateSplitRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    // Verify split exists
    state
        .db
        .get_split_by_id(id)?
        .ok_or_else(|| AppError::not_found("Split not found"))?;

    let split_type = req.split_type.map(|t| t.parse()).transpose().map_err(|_| {
        AppError::bad_request("Invalid split_type. Valid: item, tax, tip, fee, discount, rewards")
    })?;

    state.db.update_split(
        id,
        req.amount,
        req.description.as_deref(),
        split_type,
        req.entity_id,
        req.purchaser_id,
    )?;

    state
        .db
        .log_audit(&user_email, "update", Some("split"), Some(id), None)?;

    let split = state
        .db
        .get_split_by_id(id)?
        .ok_or_else(|| AppError::not_found("Split not found"))?;

    Ok(Json(split))
}

/// DELETE /api/splits/:id - Delete a split
pub async fn delete_split(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify split exists
    state
        .db
        .get_split_by_id(id)?
        .ok_or_else(|| AppError::not_found("Split not found"))?;

    state.db.delete_split(id)?;

    state
        .db
        .log_audit(&user_email, "delete", Some("split"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

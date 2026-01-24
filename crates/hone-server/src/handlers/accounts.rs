//! Account management handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Request, State},
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::{Account, Bank};

/// Request body for creating an account
#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub bank: String,
}

/// GET /api/accounts - List all accounts
pub async fn list_accounts(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Vec<Account>>, AppError> {
    let user_email = get_user_email(request.headers());

    let accounts = state.db.list_accounts()?;

    // Audit log - read access
    state.db.log_audit(
        &user_email,
        "list",
        Some("account"),
        None,
        Some(&format!("count={}", accounts.len())),
    )?;

    Ok(Json(accounts))
}

/// POST /api/accounts - Create a new account
pub async fn create_account(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Account>, AppError> {
    let user_email = get_user_email(request.headers());

    // Extract JSON body
    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: CreateAccountRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let bank: Bank = req
        .bank
        .parse()
        .map_err(|_| AppError::bad_request(&format!("Unknown bank format: {}", req.bank)))?;

    let account_id = state.db.upsert_account(&req.name, bank, None)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "create",
        Some("account"),
        Some(account_id),
        Some(&format!("name={}, bank={}", req.name, req.bank)),
    )?;

    let accounts = state.db.list_accounts()?;
    let account = accounts
        .into_iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| AppError::internal("Account not found after creation"))?;

    Ok(Json(account))
}

/// Request body for updating an account
#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub name: String,
    pub bank: String,
}

/// Request body for updating account entity
#[derive(Debug, Deserialize)]
pub struct UpdateAccountEntityRequest {
    /// Entity ID to associate with this account (null to remove association)
    pub entity_id: Option<i64>,
}

/// GET /api/accounts/:id - Get a single account
pub async fn get_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Account>, AppError> {
    let user_email = get_user_email(request.headers());

    let account = state
        .db
        .get_account(id)?
        .ok_or_else(|| AppError::not_found(&format!("Account {} not found", id)))?;

    state
        .db
        .log_audit(&user_email, "get", Some("account"), Some(id), None)?;

    Ok(Json(account))
}

/// PATCH /api/accounts/:id/entity - Update account entity (owner) association
pub async fn update_account_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify account exists
    state
        .db
        .get_account(id)?
        .ok_or_else(|| AppError::not_found(&format!("Account {} not found", id)))?;

    let bytes = axum::body::to_bytes(request.into_body(), 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateAccountEntityRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    // Verify entity exists if provided
    if let Some(entity_id) = req.entity_id {
        state
            .db
            .get_entity(entity_id)?
            .ok_or_else(|| AppError::not_found(&format!("Entity {} not found", entity_id)))?;
    }

    state.db.update_account_entity(id, req.entity_id)?;

    state.db.log_audit(
        &user_email,
        "update_entity",
        Some("account"),
        Some(id),
        Some(&format!("entity_id={:?}", req.entity_id)),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// PUT /api/accounts/:id - Update an account
pub async fn update_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Account>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify account exists
    state
        .db
        .get_account(id)?
        .ok_or_else(|| AppError::not_found(&format!("Account {} not found", id)))?;

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateAccountRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let bank: Bank = req
        .bank
        .parse()
        .map_err(|_| AppError::bad_request(&format!("Unknown bank format: {}", req.bank)))?;

    state.db.update_account(id, &req.name, bank)?;

    state.db.log_audit(
        &user_email,
        "update",
        Some("account"),
        Some(id),
        Some(&format!("name={}, bank={}", req.name, req.bank)),
    )?;

    let account = state
        .db
        .get_account(id)?
        .ok_or_else(|| AppError::internal("Account not found after update"))?;

    Ok(Json(account))
}

/// DELETE /api/accounts/:id - Delete an account and its transactions
pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify account exists
    let account = state
        .db
        .get_account(id)?
        .ok_or_else(|| AppError::not_found(&format!("Account {} not found", id)))?;

    state.db.delete_account(id)?;

    state.db.log_audit(
        &user_email,
        "delete",
        Some("account"),
        Some(id),
        Some(&format!("name={}", account.name)),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

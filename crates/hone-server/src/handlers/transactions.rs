//! Transaction handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};

use tracing::warn;

use super::reports::resolve_period;
use crate::{get_user_email, AppError, AppState, MAX_PAGE_LIMIT};
use hone_core::models::{TagSource, Transaction, TransactionTagWithDetails};

/// Query parameters for listing transactions
#[derive(Debug, Deserialize)]
pub struct TransactionQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    pub account_id: Option<i64>,
    /// Filter by entity (account owner) ID
    pub entity_id: Option<i64>,
    /// Filter by card member name (exact match)
    pub card_member: Option<String>,
    /// Search query (filters by description or merchant)
    pub search: Option<String>,
    /// Filter by tag IDs (comma-separated). Includes child tags in hierarchy.
    pub tag_ids: Option<String>,
    /// Filter to only show untagged transactions
    pub untagged: Option<bool>,
    /// Period preset (this-month, last-month, etc)
    pub period: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub to: Option<String>,
    /// Sort field (date or amount)
    pub sort: Option<String>,
    /// Sort direction (asc or desc)
    pub order: Option<String>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub transactions: Vec<Transaction>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// GET /api/transactions - List transactions
pub async fn list_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TransactionQuery>,
    request: Request,
) -> Result<Json<TransactionResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Input validation: clamp pagination parameters
    let limit = params.limit.max(1).min(MAX_PAGE_LIMIT);
    let offset = params.offset.max(0);

    // Parse tag_ids from comma-separated string
    let tag_ids: Option<Vec<i64>> = params.tag_ids.as_ref().and_then(|s| {
        let ids: Vec<i64> = s
            .split(',')
            .filter_map(|id| id.trim().parse().ok())
            .collect();
        if ids.is_empty() {
            None
        } else {
            Some(ids)
        }
    });

    // Resolve date range if period or custom dates provided
    let date_range = if params.period.is_some() || (params.from.is_some() && params.to.is_some()) {
        let period = params.period.as_deref().unwrap_or("all");
        let (from_date, to_date) =
            resolve_period(period, params.from.as_deref(), params.to.as_deref())?;
        Some((from_date, to_date))
    } else {
        None
    };

    let search = params.search.as_deref();
    let card_member = params.card_member.as_deref();
    let sort_field = params.sort.as_deref();
    let sort_order = params.order.as_deref();
    let untagged = params.untagged.unwrap_or(false);
    let transactions = state.db.search_transactions_full(
        params.account_id,
        params.entity_id,
        card_member,
        search,
        tag_ids.as_deref(),
        untagged,
        date_range,
        sort_field,
        sort_order,
        false, // exclude archived transactions
        limit,
        offset,
    )?;
    let total = state.db.count_transactions_full(
        params.account_id,
        params.entity_id,
        card_member,
        search,
        tag_ids.as_deref(),
        untagged,
        date_range,
    )?;

    // Audit log - read access
    state.db.log_audit(
        &user_email,
        "list",
        Some("transaction"),
        None,
        Some(&format!(
            "limit={}, offset={}, account_id={:?}, entity_id={:?}, card_member={:?}, search={:?}, tag_ids={:?}, period={:?}, returned={}",
            limit,
            offset,
            params.account_id,
            params.entity_id,
            params.card_member,
            params.search,
            params.tag_ids,
            params.period,
            transactions.len()
        )),
    )?;

    Ok(Json(TransactionResponse {
        transactions,
        total,
        limit,
        offset,
    }))
}

/// GET /api/transactions/:id/tags - Get tags for a transaction
pub async fn get_transaction_tags(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<TransactionTagWithDetails>>, AppError> {
    let user_email = get_user_email(request.headers());

    let tags = state.db.get_transaction_tags_with_details(id)?;

    state.db.log_audit(
        &user_email,
        "get",
        Some("transaction_tags"),
        Some(id),
        Some(&format!("count={}", tags.len())),
    )?;

    Ok(Json(tags))
}

/// Request body for adding a tag to a transaction
#[derive(Debug, Deserialize)]
pub struct AddTagRequest {
    pub tag_id: i64,
    #[serde(default)]
    pub source: Option<String>,
    pub confidence: Option<f64>,
}

/// POST /api/transactions/:id/tags - Add a tag to a transaction
pub async fn add_transaction_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: AddTagRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let source = match req.source.as_deref() {
        Some("rule") => TagSource::Rule,
        Some("pattern") => TagSource::Pattern,
        Some("ollama") => TagSource::Ollama,
        _ => TagSource::Manual,
    };

    // Track Ollama corrections: if adding a manual tag when there's an existing Ollama tag
    if source == TagSource::Manual {
        let existing_tags = state.db.get_transaction_tags(id)?;
        for existing in existing_tags {
            if existing.source == TagSource::Ollama && existing.tag_id != req.tag_id {
                // This is a correction - user is adding a different tag than Ollama assigned
                if let Err(e) = state.db.record_ollama_correction(
                    id,
                    existing.tag_id,
                    existing.confidence,
                    req.tag_id,
                ) {
                    warn!("Failed to record Ollama correction: {}", e);
                }
            }
        }
    }

    state
        .db
        .add_transaction_tag(id, req.tag_id, source, req.confidence)?;

    // Learn from manual tag assignments for future imports
    if source == TagSource::Manual {
        if let Err(e) = state.db.learn_tag_from_manual_assignment(id, req.tag_id) {
            warn!("Failed to learn tag from manual assignment: {}", e);
        }
    }

    state.db.log_audit(
        &user_email,
        "add_tag",
        Some("transaction"),
        Some(id),
        Some(&format!("tag_id={}", req.tag_id)),
    )?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// DELETE /api/transactions/:tx_id/tags/:tag_id - Remove a tag from a transaction
pub async fn remove_transaction_tag(
    State(state): State<Arc<AppState>>,
    Path((tx_id, tag_id)): Path<(i64, i64)>,
    request: Request,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.remove_transaction_tag(tx_id, tag_id)?;

    state.db.log_audit(
        &user_email,
        "remove_tag",
        Some("transaction"),
        Some(tx_id),
        Some(&format!("tag_id={}", tag_id)),
    )?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// Request body for assigning transaction to trip
#[derive(Debug, Deserialize)]
pub struct AssignTripRequest {
    pub trip_id: Option<i64>,
}

/// POST /api/transactions/:id/trip - Assign a transaction to a trip
pub async fn assign_transaction_to_trip(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: AssignTripRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    state.db.assign_transaction_to_trip(id, req.trip_id)?;

    state.db.log_audit(
        &user_email,
        "assign_trip",
        Some("transaction"),
        Some(id),
        Some(&format!("trip_id={:?}", req.trip_id)),
    )?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// Request body for updating transaction location
#[derive(Debug, Deserialize)]
pub struct UpdateLocationRequest {
    pub purchase_location_id: Option<i64>,
    pub vendor_location_id: Option<i64>,
}

/// POST /api/transactions/:id/location - Update transaction location
pub async fn update_transaction_location(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateLocationRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    state
        .db
        .update_transaction_location(id, req.purchase_location_id, req.vendor_location_id)?;

    state.db.log_audit(
        &user_email,
        "update_location",
        Some("transaction"),
        Some(id),
        Some(&format!(
            "purchase={:?}, vendor={:?}",
            req.purchase_location_id, req.vendor_location_id
        )),
    )?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// POST /api/transactions/:id/archive - Archive a transaction
pub async fn archive_transaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify transaction exists
    state
        .db
        .get_transaction(id)?
        .ok_or_else(|| AppError::not_found(&format!("Transaction {} not found", id)))?;

    state.db.archive_transaction(id)?;

    state
        .db
        .log_audit(&user_email, "archive", Some("transaction"), Some(id), None)?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// POST /api/transactions/:id/unarchive - Unarchive a transaction
pub async fn unarchive_transaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify transaction exists
    state
        .db
        .get_transaction(id)?
        .ok_or_else(|| AppError::not_found(&format!("Transaction {} not found", id)))?;

    state.db.unarchive_transaction(id)?;

    state.db.log_audit(
        &user_email,
        "unarchive",
        Some("transaction"),
        Some(id),
        None,
    )?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// Request body for updating merchant name
#[derive(Debug, Deserialize)]
pub struct UpdateMerchantRequest {
    pub merchant_name: String,
}

/// Response for merchant name update
#[derive(Serialize)]
pub struct UpdateMerchantResponse {
    pub success: bool,
    pub updated_count: i64,
}

/// PUT /api/transactions/:id/merchant - Update merchant name and learn for future
pub async fn update_merchant_name(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<UpdateMerchantResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateMerchantRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    // Validate merchant name
    let merchant_name = req.merchant_name.trim();
    if merchant_name.is_empty() {
        return Err(AppError::bad_request("Merchant name cannot be empty"));
    }

    // Update merchant name and cache for learning
    let updated_count = state
        .db
        .update_merchant_name_with_learning(id, merchant_name)?;

    state.db.log_audit(
        &user_email,
        "update_merchant",
        Some("transaction"),
        Some(id),
        Some(&format!(
            "merchant_name={}, updated_count={}",
            merchant_name, updated_count
        )),
    )?;

    Ok(Json(UpdateMerchantResponse {
        success: true,
        updated_count,
    }))
}

/// Request body for bulk adding tags
#[derive(Debug, Deserialize)]
pub struct BulkAddTagsRequest {
    pub transaction_ids: Vec<i64>,
    pub tag_ids: Vec<i64>,
}

/// Request body for bulk removing tags
#[derive(Debug, Deserialize)]
pub struct BulkRemoveTagsRequest {
    pub transaction_ids: Vec<i64>,
    pub tag_ids: Vec<i64>,
}

/// Response for bulk tag operations
#[derive(Debug, Serialize)]
pub struct BulkTagsResponse {
    pub processed: i64,
    pub success_count: i64,
    pub failed_count: i64,
}

/// POST /api/transactions/bulk-tags - Add tags to multiple transactions
pub async fn bulk_add_tags(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<BulkTagsResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 100)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: BulkAddTagsRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    if req.transaction_ids.is_empty() {
        return Err(AppError::bad_request("No transaction IDs provided"));
    }
    if req.tag_ids.is_empty() {
        return Err(AppError::bad_request("No tag IDs provided"));
    }

    let mut success_count = 0i64;
    let mut failed_count = 0i64;

    for tx_id in &req.transaction_ids {
        for tag_id in &req.tag_ids {
            match state
                .db
                .add_transaction_tag(*tx_id, *tag_id, TagSource::Manual, None)
            {
                Ok(_) => {
                    success_count += 1;
                    // Learn from manual tag assignment for future imports
                    if let Err(e) = state.db.learn_tag_from_manual_assignment(*tx_id, *tag_id) {
                        warn!("Failed to learn tag from bulk assignment: {}", e);
                    }
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }
    }

    let processed = req.transaction_ids.len() as i64;

    state.db.log_audit(
        &user_email,
        "bulk_add_tags",
        Some("transaction"),
        None,
        Some(&format!(
            "transactions={}, tags={:?}, success={}, failed={}",
            processed, req.tag_ids, success_count, failed_count
        )),
    )?;

    Ok(Json(BulkTagsResponse {
        processed,
        success_count,
        failed_count,
    }))
}

/// DELETE /api/transactions/bulk-tags - Remove tags from multiple transactions
pub async fn bulk_remove_tags(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<BulkTagsResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 100)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: BulkRemoveTagsRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    if req.transaction_ids.is_empty() {
        return Err(AppError::bad_request("No transaction IDs provided"));
    }
    if req.tag_ids.is_empty() {
        return Err(AppError::bad_request("No tag IDs provided"));
    }

    let mut success_count = 0i64;
    let mut failed_count = 0i64;

    for tx_id in &req.transaction_ids {
        for tag_id in &req.tag_ids {
            match state.db.remove_transaction_tag(*tx_id, *tag_id) {
                Ok(_) => {
                    success_count += 1;
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }
    }

    let processed = req.transaction_ids.len() as i64;

    state.db.log_audit(
        &user_email,
        "bulk_remove_tags",
        Some("transaction"),
        None,
        Some(&format!(
            "transactions={}, tags={:?}, success={}, failed={}",
            processed, req.tag_ids, success_count, failed_count
        )),
    )?;

    Ok(Json(BulkTagsResponse {
        processed,
        success_count,
        failed_count,
    }))
}

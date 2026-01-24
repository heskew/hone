//! Receipt handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::{get_user_email, AppError, AppState, SuccessResponse, MAX_UPLOAD_SIZE};
use hone_core::ai::{AIBackend, AIClient, ParsedReceipt};
use hone_core::models::{
    NewOllamaMetric, NewReceipt, OllamaOperation, Receipt, ReceiptMatchCandidate, ReceiptStatus,
};

/// GET /api/transactions/:id/receipts - Get receipts for a transaction
pub async fn get_transaction_receipts(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<Receipt>>, AppError> {
    let user_email = get_user_email(request.headers());

    let receipts = state.db.get_receipts_for_transaction(transaction_id)?;

    state.db.log_audit(
        &user_email,
        "view",
        Some("transaction_receipts"),
        Some(transaction_id),
        Some(&format!("count={}", receipts.len())),
    )?;

    Ok(Json(receipts))
}

/// GET /api/receipts/:id - Get a specific receipt
pub async fn get_receipt(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Receipt>, AppError> {
    let user_email = get_user_email(request.headers());

    let receipt = state
        .db
        .get_receipt(id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    state
        .db
        .log_audit(&user_email, "view", Some("receipt"), Some(id), None)?;

    Ok(Json(receipt))
}

/// Response for receipt upload
#[derive(Debug, Serialize)]
pub struct ReceiptUploadResponse {
    pub receipt: Receipt,
    pub image_path: String,
}

/// POST /api/transactions/:id/receipts - Upload a receipt image
pub async fn upload_receipt(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<i64>,
    request: Request,
) -> Result<Json<ReceiptUploadResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify transaction exists
    state
        .db
        .get_transaction(transaction_id)?
        .ok_or_else(|| AppError::not_found("Transaction not found"))?;

    // Read the image data
    let bytes = axum::body::to_bytes(request.into_body(), MAX_UPLOAD_SIZE)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body or file too large (max 10MB)"))?;

    if bytes.is_empty() {
        return Err(AppError::bad_request("No image data provided"));
    }

    // Create receipts directory if it doesn't exist
    let receipts_dir = &state.receipts_dir;
    if !receipts_dir.exists() {
        std::fs::create_dir_all(receipts_dir).map_err(|e| {
            AppError::internal(&format!("Failed to create receipts directory: {}", e))
        })?;
    }

    // Generate unique filename
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("receipt_{}_{}.jpg", transaction_id, timestamp);
    let image_path = receipts_dir.join(&filename);

    // Save the image
    std::fs::write(&image_path, &bytes)
        .map_err(|e| AppError::internal(&format!("Failed to save receipt image: {}", e)))?;

    let path_str = image_path.to_string_lossy().to_string();

    // Create receipt record
    let receipt_id = state.db.create_receipt(transaction_id, Some(&path_str))?;

    state.db.log_audit(
        &user_email,
        "upload",
        Some("receipt"),
        Some(receipt_id),
        Some(&format!("tx={}, path={}", transaction_id, path_str)),
    )?;

    let receipt = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::internal("Receipt not found after creation"))?;

    Ok(Json(ReceiptUploadResponse {
        receipt,
        image_path: path_str,
    }))
}

/// DELETE /api/receipts/:id - Delete a receipt
pub async fn delete_receipt(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get receipt to find image path
    let receipt = state
        .db
        .get_receipt(id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    // Delete the image file if it exists and is within receipts_dir (path traversal protection)
    if let Some(path) = &receipt.image_path {
        let image_path = std::path::Path::new(path);
        if let (Ok(canonical_image), Ok(canonical_dir)) = (
            std::fs::canonicalize(image_path),
            std::fs::canonicalize(&state.receipts_dir),
        ) {
            if canonical_image.starts_with(&canonical_dir) {
                let _ = std::fs::remove_file(path);
            } else {
                warn!(
                    "Receipt image path outside receipts directory, skipping delete: {}",
                    path
                );
            }
        }
    }

    state.db.delete_receipt(id)?;

    state
        .db
        .log_audit(&user_email, "delete", Some("receipt"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// Response for receipt parsing
#[derive(Debug, Serialize)]
pub struct ReceiptParseResponse {
    pub receipt_id: i64,
    pub parsed: ParsedReceipt,
    pub raw_json: String,
}

/// POST /api/receipts/:id/parse - Parse a receipt using AI
pub async fn parse_receipt(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<ReceiptParseResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get receipt
    let receipt = state
        .db
        .get_receipt(id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    // Get image data
    let image_path = receipt
        .image_path
        .ok_or_else(|| AppError::bad_request("Receipt has no image to parse"))?;

    let image_data = std::fs::read(&image_path)
        .map_err(|e| AppError::internal(&format!("Failed to read receipt image: {}", e)))?;

    // Get Ollama client from state
    let ollama = state.ai.as_ref().ok_or_else(|| {
        AppError::bad_request("Ollama not configured. Set OLLAMA_HOST environment variable.")
    })?;

    // Parse the receipt
    let parsed = ollama
        .parse_receipt(&image_data, None)
        .await
        .map_err(|e| AppError::internal(&format!("Failed to parse receipt: {}", e)))?;

    // Store parsed JSON
    let raw_json = serde_json::to_string(&parsed)
        .map_err(|e| AppError::internal(&format!("Failed to serialize parsed receipt: {}", e)))?;

    state.db.update_receipt_parsed(id, &raw_json)?;

    state.db.log_audit(
        &user_email,
        "parse",
        Some("receipt"),
        Some(id),
        Some(&format!("items={}", parsed.items.len())),
    )?;

    Ok(Json(ReceiptParseResponse {
        receipt_id: id,
        parsed,
        raw_json,
    }))
}

/// Query params for listing receipts
#[derive(Debug, Deserialize)]
pub struct ListReceiptsQuery {
    /// Filter by status (pending, matched, manual_review, orphaned)
    pub status: Option<String>,
}

/// GET /api/receipts - List receipts, optionally filtered by status
pub async fn list_receipts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListReceiptsQuery>,
    request: Request,
) -> Result<Json<Vec<Receipt>>, AppError> {
    let user_email = get_user_email(request.headers());

    let receipts = if let Some(status_str) = &query.status {
        let status: ReceiptStatus = status_str.parse().map_err(|_| {
            AppError::bad_request("Invalid status. Use: pending, matched, manual_review, orphaned")
        })?;
        state.db.get_receipts_by_status(status)?
    } else {
        // Default to pending receipts for the workflow
        state.db.get_pending_receipts()?
    };

    state.db.log_audit(
        &user_email,
        "list",
        Some("receipts"),
        None,
        Some(&format!(
            "status={:?}, count={}",
            query.status,
            receipts.len()
        )),
    )?;

    Ok(Json(receipts))
}

/// Response for pending receipt upload
#[derive(Debug, Serialize)]
pub struct PendingReceiptResponse {
    pub receipt: Receipt,
    pub image_path: String,
    pub parsed: Option<ParsedReceipt>,
}

/// POST /api/receipts - Upload a receipt without transaction (receipt-first workflow)
pub async fn upload_pending_receipt(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<PendingReceiptResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Read the image data
    let bytes = axum::body::to_bytes(request.into_body(), MAX_UPLOAD_SIZE)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body or file too large (max 10MB)"))?;

    if bytes.is_empty() {
        return Err(AppError::bad_request("No image data provided"));
    }

    // Compute content hash for deduplication
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let content_hash = format!("{:x}", hasher.finalize());

    // Check for duplicate receipt
    if let Some(existing) = state.db.get_receipt_by_hash(&content_hash)? {
        return Err(AppError::conflict(&format!(
            "Receipt already exists with ID {}",
            existing.id
        )));
    }

    // Create receipts directory if it doesn't exist
    let receipts_dir = &state.receipts_dir;
    if !receipts_dir.exists() {
        std::fs::create_dir_all(receipts_dir).map_err(|e| {
            AppError::internal(&format!("Failed to create receipts directory: {}", e))
        })?;
    }

    // Generate unique filename
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("receipt_pending_{}.jpg", timestamp);
    let image_path = receipts_dir.join(&filename);

    // Save the image
    std::fs::write(&image_path, &bytes)
        .map_err(|e| AppError::internal(&format!("Failed to save receipt image: {}", e)))?;

    let path_str = image_path.to_string_lossy().to_string();

    // Try to parse the receipt with Ollama if available
    let parsed = if let Some(ref ollama) = state.ai {
        match ollama.parse_receipt(&bytes, None).await {
            Ok(p) => Some(p),
            Err(e) => {
                warn!("Failed to parse receipt: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create receipt with parsed data
    let new_receipt = NewReceipt {
        transaction_id: None,
        image_path: Some(path_str.clone()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: parsed.as_ref().and_then(|p| {
            p.date
                .as_ref()
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        }),
        receipt_total: parsed.as_ref().and_then(|p| p.total),
        receipt_merchant: parsed.as_ref().and_then(|p| p.merchant.clone()),
        content_hash: Some(content_hash),
    };

    let receipt_id = state.db.create_receipt_full(&new_receipt)?;

    // If we have parsed data, store it
    if let Some(ref p) = parsed {
        let json = serde_json::to_string(p).map_err(|e| {
            AppError::internal(&format!("Failed to serialize parsed receipt: {}", e))
        })?;
        state.db.update_receipt_parsed(receipt_id, &json)?;
    }

    state.db.log_audit(
        &user_email,
        "upload",
        Some("pending_receipt"),
        Some(receipt_id),
        Some(&format!("path={}, parsed={}", path_str, parsed.is_some())),
    )?;

    let receipt = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::internal("Receipt not found after creation"))?;

    Ok(Json(PendingReceiptResponse {
        receipt,
        image_path: path_str,
        parsed,
    }))
}

/// Request body for linking receipt to transaction
#[derive(Debug, Deserialize)]
pub struct LinkReceiptRequest {
    pub transaction_id: i64,
}

/// POST /api/receipts/:id/link - Link a pending receipt to a transaction
pub async fn link_receipt_to_transaction(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(receipt_id): Path<i64>,
    Json(body): Json<LinkReceiptRequest>,
) -> Result<Json<Receipt>, AppError> {
    let user_email = get_user_email(&headers);

    // Verify receipt exists and is pending
    let receipt = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    if receipt.status != ReceiptStatus::Pending && receipt.status != ReceiptStatus::ManualReview {
        return Err(AppError::bad_request(
            "Receipt is already matched or cannot be linked",
        ));
    }

    // Verify transaction exists
    state
        .db
        .get_transaction(body.transaction_id)?
        .ok_or_else(|| AppError::not_found("Transaction not found"))?;

    // Link the receipt
    state
        .db
        .link_receipt_to_transaction(receipt_id, body.transaction_id)?;

    state.db.log_audit(
        &user_email,
        "link",
        Some("receipt"),
        Some(receipt_id),
        Some(&format!("tx_id={}", body.transaction_id)),
    )?;

    let updated = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::internal("Receipt not found after linking"))?;

    Ok(Json(updated))
}

/// Request body for updating receipt status
#[derive(Debug, Deserialize)]
pub struct UpdateReceiptStatusRequest {
    /// New status (pending, matched, manual_review, orphaned)
    pub status: String,
}

/// POST /api/receipts/:id/status - Update receipt status
pub async fn update_receipt_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(receipt_id): Path<i64>,
    Json(body): Json<UpdateReceiptStatusRequest>,
) -> Result<Json<Receipt>, AppError> {
    let user_email = get_user_email(&headers);

    // Verify receipt exists
    state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    // Parse status
    let status: ReceiptStatus = body.status.parse().map_err(|_| {
        AppError::bad_request("Invalid status. Use: pending, matched, manual_review, orphaned")
    })?;

    // Update status
    state.db.update_receipt_status(receipt_id, status)?;

    state.db.log_audit(
        &user_email,
        "update_status",
        Some("receipt"),
        Some(receipt_id),
        Some(&format!("status={}", body.status)),
    )?;

    let updated = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::internal("Receipt not found after update"))?;

    Ok(Json(updated))
}

/// POST /api/receipts/:id/unlink - Unlink a receipt from its transaction
pub async fn unlink_receipt(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(receipt_id): Path<i64>,
) -> Result<Json<Receipt>, AppError> {
    let user_email = get_user_email(&headers);

    // Verify receipt exists
    let receipt = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    // Only linked receipts can be unlinked
    if receipt.transaction_id.is_none() {
        return Err(AppError::bad_request(
            "Receipt is not linked to a transaction",
        ));
    }

    // Unlink the receipt
    state.db.unlink_receipt(receipt_id)?;

    state.db.log_audit(
        &user_email,
        "unlink",
        Some("receipt"),
        Some(receipt_id),
        Some(&format!("transaction_id={:?}", receipt.transaction_id)),
    )?;

    let updated = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::internal("Receipt not found after unlink"))?;

    Ok(Json(updated))
}

/// Thresholds for Ollama-enhanced matching
const AMBIGUOUS_SCORE_LOW: f64 = 0.5; // Below this, don't bother with Ollama
const AMBIGUOUS_SCORE_HIGH: f64 = 0.85; // Above this, match is confident enough
const OLLAMA_WEIGHT: f64 = 0.4; // How much Ollama affects final score
const ALGO_WEIGHT: f64 = 0.6; // How much algorithmic score affects final score

/// GET /api/receipts/:id/candidates - Get transaction match candidates for a receipt
pub async fn get_receipt_match_candidates(
    State(state): State<Arc<AppState>>,
    Path(receipt_id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<ReceiptMatchCandidate>>, AppError> {
    let user_email = get_user_email(request.headers());

    // Check receipt exists first for proper 404 response
    let receipt = state
        .db
        .get_receipt(receipt_id)?
        .ok_or_else(|| AppError::not_found("Receipt not found"))?;

    let mut candidates = state.db.find_matching_transactions(&receipt)?;

    // Enhance ambiguous matches with Ollama if available
    if let Some(ref ollama) = state.ai {
        candidates = enhance_candidates_with_ollama(candidates, &receipt, ollama, &state.db).await;
    }

    state.db.log_audit(
        &user_email,
        "get_candidates",
        Some("receipt"),
        Some(receipt_id),
        Some(&format!("count={}", candidates.len())),
    )?;

    Ok(Json(candidates))
}

/// Enhance match candidates with Ollama evaluation for ambiguous matches
async fn enhance_candidates_with_ollama(
    mut candidates: Vec<ReceiptMatchCandidate>,
    receipt: &Receipt,
    ollama: &AIClient,
    db: &hone_core::db::Database,
) -> Vec<ReceiptMatchCandidate> {
    use std::time::Instant;

    for candidate in candidates.iter_mut() {
        // Only evaluate ambiguous matches (0.5 <= score < 0.85)
        if candidate.score < AMBIGUOUS_SCORE_LOW || candidate.score >= AMBIGUOUS_SCORE_HIGH {
            continue;
        }

        let start = Instant::now();
        let tx = &candidate.transaction;

        // Call Ollama for evaluation
        let result = ollama
            .evaluate_receipt_match(
                receipt.receipt_merchant.as_deref(),
                receipt
                    .receipt_date
                    .as_ref()
                    .map(|d| d.to_string())
                    .as_deref(),
                receipt.receipt_total,
                &tx.description,
                &tx.date.to_string(),
                tx.amount,
                tx.merchant_normalized.as_deref(),
            )
            .await;

        let latency_ms = start.elapsed().as_millis() as i64;

        let input_text = format!(
            "receipt: {} ${:.2} | tx: {} ${:.2}",
            receipt.receipt_merchant.as_deref().unwrap_or("?"),
            receipt.receipt_total.unwrap_or(0.0),
            tx.description,
            tx.amount.abs()
        );

        match result {
            Ok(evaluation) => {
                // Record success metric
                let _ = db.record_ollama_metric(&NewOllamaMetric {
                    operation: OllamaOperation::EvaluateReceiptMatch,
                    model: ollama.model().to_string(),
                    latency_ms,
                    success: true,
                    error_message: None,
                    confidence: Some(evaluation.confidence),
                    transaction_id: Some(tx.id),
                    input_text: Some(input_text.clone()),
                    result_text: Some(evaluation.reason.clone()),
                    metadata: None,
                });

                // Combine algorithmic score with Ollama evaluation
                let ollama_score = if evaluation.is_match {
                    evaluation.confidence
                } else {
                    1.0 - evaluation.confidence // Invert for non-matches
                };

                // Weighted combination
                let original_score = candidate.score;
                candidate.score = ALGO_WEIGHT * original_score + OLLAMA_WEIGHT * ollama_score;

                // Store the evaluation in match factors
                candidate.match_factors.ollama_evaluation = Some(evaluation);
            }
            Err(e) => {
                // Record failure metric
                let _ = db.record_ollama_metric(&NewOllamaMetric {
                    operation: OllamaOperation::EvaluateReceiptMatch,
                    model: ollama.model().to_string(),
                    latency_ms,
                    success: false,
                    error_message: Some(e.to_string()),
                    confidence: None,
                    transaction_id: Some(tx.id),
                    input_text: Some(input_text),
                    result_text: None,
                    metadata: None,
                });
                // Continue without Ollama enhancement for this candidate
                warn!("Ollama receipt match evaluation failed: {}", e);
            }
        }
    }

    // Re-sort by updated scores
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    candidates
}

/// Response for auto-match endpoint
#[derive(Debug, Serialize)]
pub struct AutoMatchResponse {
    pub matched: usize,
    pub checked: usize,
}

/// POST /api/receipts/auto-match - Run auto-matching on pending receipts
pub async fn auto_match_receipts(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<AutoMatchResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let (matched, checked) = state.db.auto_match_receipts()?;

    state.db.log_audit(
        &user_email,
        "auto_match",
        Some("receipts"),
        None,
        Some(&format!("matched={}, checked={}", matched, checked)),
    )?;

    Ok(Json(AutoMatchResponse { matched, checked }))
}

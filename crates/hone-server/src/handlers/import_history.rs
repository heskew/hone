//! Import history handlers

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::{get_user_email, AppError, AppState, MAX_PAGE_LIMIT};
use hone_core::{
    ai::{AIBackend, AIClient, MerchantContext},
    db::Database,
    detect::WasteDetector,
    models::{
        ImportSessionWithAccount, ImportTaggingBreakdown, NewOllamaMetric, NewReprocessRun,
        OllamaOperation, ReprocessComparison, ReprocessRunSummary, ReprocessRunWithComparison,
        RunComparison, SkippedTransaction, Transaction,
    },
    tags::TagAssigner,
};

/// Query parameters for listing import sessions
#[derive(Debug, Deserialize)]
pub struct ImportSessionsQuery {
    /// Filter by account ID
    pub account_id: Option<i64>,
    /// Maximum number of results (default 50, max 100)
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination (default 0)
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Response for listing import sessions
#[derive(Debug, Serialize)]
pub struct ImportSessionsResponse {
    pub sessions: Vec<ImportSessionWithAccount>,
    pub total: i64,
}

/// GET /api/imports - List import sessions
pub async fn list_import_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ImportSessionsQuery>,
    request: Request,
) -> Result<Json<ImportSessionsResponse>, AppError> {
    let user_email = get_user_email(request.headers());
    let limit = params.limit.max(1).min(MAX_PAGE_LIMIT);

    let sessions = state
        .db
        .list_import_sessions(params.account_id, limit, params.offset)?;
    let total = state.db.count_import_sessions(params.account_id)?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("import_sessions"),
        None,
        Some(&format!(
            "account_id={:?}, limit={}, offset={}",
            params.account_id, limit, params.offset
        )),
    )?;

    Ok(Json(ImportSessionsResponse { sessions, total }))
}

/// GET /api/imports/:id - Get a single import session
pub async fn get_import_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<ImportSessionWithAccount>, AppError> {
    let user_email = get_user_email(request.headers());

    let session = state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    state
        .db
        .log_audit(&user_email, "view", Some("import_session"), Some(id), None)?;

    Ok(Json(session))
}

/// Query parameters for listing import session transactions
#[derive(Debug, Deserialize)]
pub struct ImportTransactionsQuery {
    /// Maximum number of results (default 50, max 100)
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination (default 0)
    #[serde(default)]
    pub offset: i64,
}

/// Response for listing import session transactions
#[derive(Debug, Serialize)]
pub struct ImportTransactionsResponse {
    pub transactions: Vec<Transaction>,
    pub total: i64,
}

/// GET /api/imports/:id/transactions - Get transactions from an import session
pub async fn get_import_session_transactions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<ImportTransactionsQuery>,
    request: Request,
) -> Result<Json<ImportTransactionsResponse>, AppError> {
    let user_email = get_user_email(request.headers());
    let limit = params.limit.max(1).min(MAX_PAGE_LIMIT);

    // Verify session exists
    state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    let transactions = state
        .db
        .get_import_session_transactions(id, limit, params.offset)?;
    let total = state.db.count_import_session_transactions(id)?;

    state.db.log_audit(
        &user_email,
        "view",
        Some("import_transactions"),
        Some(id),
        Some(&format!("limit={}, offset={}", limit, params.offset)),
    )?;

    Ok(Json(ImportTransactionsResponse {
        transactions,
        total,
    }))
}

/// GET /api/imports/:id/skipped - Get skipped (duplicate) transactions from an import session
pub async fn get_import_session_skipped(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<SkippedTransaction>>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify session exists
    state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    let skipped = state.db.get_skipped_transactions(id)?;

    state.db.log_audit(
        &user_email,
        "view",
        Some("import_skipped"),
        Some(id),
        Some(&format!("count={}", skipped.len())),
    )?;

    Ok(Json(skipped))
}

/// Response for cancelling an import session
#[derive(Debug, Serialize)]
pub struct CancelImportResponse {
    pub cancelled: bool,
    pub message: String,
}

/// POST /api/imports/:id/cancel - Cancel an import session that is stuck in processing
pub async fn cancel_import_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<CancelImportResponse>, AppError> {
    use hone_core::models::ImportStatus;

    let user_email = get_user_email(request.headers());

    // Verify session exists
    let session = state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    // Check if it's actually in processing state
    if session.session.status != ImportStatus::Processing {
        return Ok(Json(CancelImportResponse {
            cancelled: false,
            message: format!(
                "Import is not in processing state (current: {})",
                session.session.status
            ),
        }));
    }

    // Cancel the import
    let cancelled = state.db.cancel_import(id)?;

    state.db.log_audit(
        &user_email,
        "cancel",
        Some("import_session"),
        Some(id),
        Some(&format!("cancelled={}", cancelled)),
    )?;

    if cancelled {
        info!("Import session {} cancelled by {}", id, user_email);
        Ok(Json(CancelImportResponse {
            cancelled: true,
            message: "Import cancelled successfully".to_string(),
        }))
    } else {
        Ok(Json(CancelImportResponse {
            cancelled: false,
            message: "Import was not in processing state".to_string(),
        }))
    }
}

/// Response for reprocessing an import session (async start)
#[derive(Debug, Serialize)]
pub struct ReprocessStartResponse {
    /// Session ID being reprocessed
    pub session_id: i64,
    /// Reprocess run ID for tracking this specific run
    pub run_id: i64,
    /// Run number within this session
    pub run_number: i64,
    /// Status message
    pub message: String,
}

/// Optional request body for reprocessing
#[derive(Debug, Deserialize, Default)]
pub struct ReprocessRequest {
    /// Optional model override (uses server default if not specified)
    #[serde(default)]
    pub model: Option<String>,
}

/// POST /api/imports/:id/reprocess - Start async reprocess of an import session
///
/// Captures a "before" snapshot, then spawns a background task to:
/// - Clear auto-assigned tags and merchant normalizations
/// - Re-run tagging, normalization, and detection
/// - Capture an "after" snapshot for comparison
///
/// Accepts optional JSON body with model override:
/// ```json
/// { "model": "llama3.3" }
/// ```
///
/// Returns immediately. Poll GET /api/imports/:id for progress.
pub async fn reprocess_import_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
    body: Option<Json<ReprocessRequest>>,
) -> Result<Json<ReprocessStartResponse>, AppError> {
    use hone_core::models::ImportStatus;
    use tracing::error;

    let user_email = get_user_email(&headers);
    let model_override = body.and_then(|b| b.model.clone());

    // Verify session exists
    let session = state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    // Check if already processing
    if session.session.status == ImportStatus::Processing {
        return Err(AppError::bad_request(
            "Import session is already being processed",
        ));
    }

    // Get transaction count for progress tracking
    let total_transactions = state.db.count_import_session_transactions(id)?;

    info!(
        "Starting async reprocess of import session {} with {} transactions",
        id, total_transactions
    );

    // Apply model override if specified
    let effective_ai = match (state.ai.as_ref(), model_override.as_deref()) {
        (Some(ai), Some(model)) => Some(ai.with_model(model)),
        (Some(ai), None) => Some(ai.clone()),
        _ => None,
    };
    let effective_model = effective_ai.as_ref().map(|o| o.model().to_string());

    // 1. Create a new reprocess run record
    let new_run = NewReprocessRun {
        import_session_id: id,
        ollama_model: effective_model.clone(),
        initiated_by: Some(user_email.clone()),
        reason: None,
    };
    let run_id = state.db.create_reprocess_run(&new_run)?;

    // Get run details for response
    let run = state
        .db
        .get_reprocess_run(run_id)?
        .ok_or_else(|| AppError::internal("Failed to retrieve created run"))?;

    // 2. Capture "before" snapshot linked to this run
    let before_snapshot = state.db.capture_reprocess_snapshot(id)?;
    state
        .db
        .store_reprocess_snapshot(id, "before", &before_snapshot, Some(run_id))?;

    // 3. Mark session as processing
    state
        .db
        .update_import_status(id, ImportStatus::Processing)?;
    state
        .db
        .update_import_progress(id, "clearing", 0, total_transactions)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "reprocess_start",
        Some("import_session"),
        Some(id),
        Some(&format!(
            "transactions={}, run_id={}, model={:?}",
            total_transactions, run_id, effective_model
        )),
    )?;

    // 4. Spawn background task
    let db = state.db.clone();
    let ollama = effective_ai;
    let imported_count = session.session.imported_count;
    let skipped_count = session.session.skipped_count;
    let receipts_matched = session.session.receipts_matched;
    let model_override_owned = model_override;

    tokio::spawn(async move {
        // Create orchestrator inside task with cloned db (AIOrchestrator contains Database)
        // Apply model override if specified
        let orchestrator = match model_override_owned.as_deref() {
            Some(model) => hone_core::ai::orchestrator::AIOrchestrator::from_env(db.clone())
                .map(|o| o.with_model(model)),
            None => hone_core::ai::orchestrator::AIOrchestrator::from_env(db.clone()),
        };

        if let Err(e) = run_async_reprocess(
            &db,
            ollama.as_ref(),
            orchestrator.as_ref(),
            id,
            run_id,
            total_transactions,
            imported_count,
            skipped_count,
            receipts_matched,
        )
        .await
        {
            error!(
                "Background reprocess failed for session {}, run {}: {}",
                id, run_id, e
            );
            if let Err(e2) = db.mark_import_failed(id, &e.to_string()) {
                error!("Failed to mark import as failed: {}", e2);
            }
            // Also mark the run as failed
            if let Err(e2) = db.fail_reprocess_run(run_id) {
                error!("Failed to mark reprocess run as failed: {}", e2);
            }
        }
    });

    Ok(Json(ReprocessStartResponse {
        session_id: id,
        run_id,
        run_number: run.run_number,
        message: "Reprocess started. Poll GET /api/imports/:id for progress.".to_string(),
    }))
}

/// Run the async reprocess (clearing, tagging, normalization, detection)
async fn run_async_reprocess(
    db: &Database,
    ollama: Option<&AIClient>,
    orchestrator: Option<&hone_core::ai::orchestrator::AIOrchestrator>,
    session_id: i64,
    run_id: i64,
    total_transactions: i64,
    imported_count: i64,
    skipped_count: i64,
    receipts_matched: i64,
) -> Result<(), hone_core::error::Error> {
    use std::time::Instant;

    info!(
        "Starting async reprocess for session {}, run {}",
        session_id, run_id
    );
    let total_start = Instant::now();

    // Get all transaction IDs for this session
    let transactions = db.get_import_session_transactions(session_id, total_transactions, 0)?;
    let transaction_ids: Vec<i64> = transactions.iter().map(|tx| tx.id).collect();

    // Phase 1: Clearing - remove auto tags and merchant names
    db.update_import_progress(session_id, "clearing", 0, total_transactions)?;

    let tags_cleared = db.clear_auto_tags_for_transactions(&transaction_ids)?;
    info!("Cleared {} auto-assigned tags", tags_cleared);

    let merchants_cleared = db.clear_merchant_normalized_for_transactions(&transaction_ids)?;
    info!("Cleared {} merchant normalizations", merchants_cleared);

    db.update_import_progress(
        session_id,
        "clearing",
        total_transactions,
        total_transactions,
    )?;

    // Phase 2: Tagging
    let tagging_start = Instant::now();
    db.update_import_progress(session_id, "tagging", 0, total_transactions)?;

    let assigner = TagAssigner::new(db, ollama);

    // Create progress callback
    let db_for_tagging = db.clone();
    let tagging_progress: Box<dyn Fn(i64, i64) + Send + Sync> = Box::new(move |current, total| {
        if let Err(e) = db_for_tagging.update_import_progress(session_id, "tagging", current, total)
        {
            warn!("Failed to update tagging progress: {}", e);
        }
    });

    let backfill = assigner
        .backfill_tags_for_session_with_progress(session_id, Some(&tagging_progress))
        .await?;
    let tagging_breakdown = ImportTaggingBreakdown {
        by_learned: backfill.by_learned,
        by_rule: backfill.by_rule,
        by_pattern: backfill.by_pattern,
        by_ollama: backfill.by_ollama,
        by_bank_category: backfill.by_bank_category,
        fallback: backfill.fallback_to_other,
    };

    let tagging_duration_ms = tagging_start.elapsed().as_millis() as i64;
    if let Err(e) = db.update_import_phase_duration(session_id, "tagging", tagging_duration_ms) {
        warn!("Failed to update tagging duration: {}", e);
    }

    info!(
        "Re-tagged {}/{} transactions for session {} in {}ms",
        backfill.transactions_tagged,
        backfill.transactions_processed,
        session_id,
        tagging_duration_ms
    );

    // Save tagging results immediately
    if let Err(e) = db.update_import_session_tagging(session_id, &tagging_breakdown) {
        warn!("Failed to update tagging breakdown: {}", e);
    }

    // Phase 3: Merchant normalization
    let normalizing_start = Instant::now();
    if let Some(ollama_client) = ollama {
        db.update_import_progress(session_id, "normalizing", 0, 0)?;
        let normalized =
            normalize_merchants_for_transactions(db, ollama_client, &transaction_ids).await;
        info!("Re-normalized {} merchant names", normalized);
    }
    let normalizing_duration_ms = normalizing_start.elapsed().as_millis() as i64;
    if let Err(e) =
        db.update_import_phase_duration(session_id, "normalizing", normalizing_duration_ms)
    {
        warn!("Failed to update normalizing duration: {}", e);
    }

    // Phase 4: Detection
    let detecting_start = Instant::now();
    db.update_import_progress(session_id, "detecting", 0, 1)?;

    // Build detector with best available AI capabilities
    let detector = match (orchestrator, ollama) {
        (Some(orch), Some(ai)) => WasteDetector::with_ai_and_orchestrator(db, ai, orch),
        (Some(orch), None) => WasteDetector::with_orchestrator(db, orch),
        (None, Some(ai)) => WasteDetector::with_ai(db, ai),
        (None, None) => WasteDetector::new(db),
    };

    // Create progress callback
    let db_for_progress = db.clone();
    let progress_callback: Box<dyn Fn(&str, i64, i64) + Send + Sync> =
        Box::new(move |phase, current, total| {
            if let Err(e) =
                db_for_progress.update_import_progress(session_id, phase, current, total)
            {
                warn!("Failed to update detection progress: {}", e);
            }
        });

    let detection_results = detector
        .detect_all_with_progress(Some(&progress_callback))
        .await?;
    let detecting_duration_ms = detecting_start.elapsed().as_millis() as i64;
    if let Err(e) = db.update_import_phase_duration(session_id, "detecting", detecting_duration_ms)
    {
        warn!("Failed to update detecting duration: {}", e);
    }

    // Record total duration
    let total_duration_ms = total_start.elapsed().as_millis() as i64;
    if let Err(e) = db.update_import_total_duration(session_id, total_duration_ms) {
        warn!("Failed to update total duration: {}", e);
    }

    // Update session with final results
    db.update_import_session_results(
        session_id,
        imported_count,
        skipped_count,
        &tagging_breakdown,
        detection_results.subscriptions_found as i64,
        detection_results.zombies_detected as i64,
        detection_results.price_increases_detected as i64,
        detection_results.duplicates_detected as i64,
        receipts_matched,
    )?;

    // Capture "after" snapshot linked to this run
    let after_snapshot = db.capture_reprocess_snapshot(session_id)?;
    db.store_reprocess_snapshot(session_id, "after", &after_snapshot, Some(run_id))?;

    // Mark run as completed
    db.complete_reprocess_run(run_id)?;

    // Mark session as completed
    db.mark_import_completed(session_id)?;

    info!(
        "Async reprocess completed for session {}, run {} in {}ms - subs: {}, zombies: {}, increases: {}, duplicates: {}",
        session_id,
        run_id,
        total_duration_ms,
        detection_results.subscriptions_found,
        detection_results.zombies_detected,
        detection_results.price_increases_detected,
        detection_results.duplicates_detected
    );

    Ok(())
}

/// Normalize merchant names for a specific set of transactions
async fn normalize_merchants_for_transactions(
    db: &Database,
    ollama: &AIClient,
    transaction_ids: &[i64],
) -> i64 {
    if transaction_ids.is_empty() {
        return 0;
    }

    // Get the transactions that need normalization
    let mut transactions = Vec::new();
    for &id in transaction_ids {
        if let Ok(Some(tx)) = db.get_transaction(id) {
            if tx.merchant_normalized.is_none() {
                transactions.push(tx);
            }
        }
    }

    if transactions.is_empty() {
        return 0;
    }

    info!(
        "Normalizing {} transactions without merchant names",
        transactions.len()
    );

    // Collect unique descriptions with representative transaction for context
    let mut unique_descriptions: HashMap<String, (Vec<i64>, Option<Transaction>)> = HashMap::new();
    for tx in transactions {
        unique_descriptions
            .entry(tx.description.clone())
            .or_insert_with(|| (Vec::new(), Some(tx.clone())))
            .0
            .push(tx.id);
    }

    info!(
        "Normalizing {} unique merchant descriptions via Ollama",
        unique_descriptions.len()
    );

    let mut normalized_count = 0;

    for (description, (tx_ids, representative_tx)) in unique_descriptions {
        // 1. CHECK CACHE FIRST - use learned/cached merchant names
        if let Ok(Some(cached_name)) = db.get_cached_merchant_name(&description) {
            info!(
                "Using cached merchant name for '{}' -> '{}' ({} transactions)",
                description,
                cached_name,
                tx_ids.len()
            );
            for tx_id in &tx_ids {
                if let Err(e) = db.update_merchant_normalized(*tx_id, &cached_name) {
                    warn!(
                        "Failed to update merchant_normalized for tx {}: {}",
                        tx_id, e
                    );
                } else {
                    normalized_count += 1;
                }
            }
            continue;
        }

        // 2. Not in cache - call Ollama
        let category_hint = tx_ids.first().and_then(|&tx_id| {
            db.get_transaction_tags(tx_id)
                .ok()
                .and_then(|tags| tags.first().map(|t| t.tag_id))
                .and_then(|tag_id| db.get_tag(tag_id).ok().flatten())
                .map(|tag| tag.name)
        });

        // Extract Amex context from original_data if available
        let context = representative_tx
            .as_ref()
            .and_then(|tx| extract_amex_context(tx, &description, category_hint.as_deref()));

        let start = std::time::Instant::now();

        // Use context-aware normalization for Amex, regular for others
        let result = if let Some(ref ctx) = context {
            ollama
                .normalize_merchant_with_context(&description, ctx)
                .await
        } else {
            ollama
                .normalize_merchant(&description, category_hint.as_deref())
                .await
        };

        match result {
            Ok(normalized) => {
                let latency_ms = start.elapsed().as_millis() as i64;
                debug!("Normalized '{}' -> '{}'", description, normalized);

                // Cache the result for future imports
                if let Err(e) = db.cache_merchant_name(&description, &normalized, "ollama", 0.8) {
                    warn!("Failed to cache merchant name: {}", e);
                }

                // Record success metric
                let metric = NewOllamaMetric {
                    operation: OllamaOperation::NormalizeMerchant,
                    model: ollama.model().to_string(),
                    latency_ms,
                    success: true,
                    error_message: None,
                    confidence: None,
                    transaction_id: tx_ids.first().copied(),
                    input_text: Some(description.clone()),
                    result_text: Some(normalized.clone()),
                    metadata: None,
                };
                if let Err(e) = db.record_ollama_metric(&metric) {
                    warn!("Failed to record Ollama metric: {}", e);
                }

                // Update all transactions with this description
                for tx_id in &tx_ids {
                    if let Err(e) = db.update_merchant_normalized(*tx_id, &normalized) {
                        warn!(
                            "Failed to update merchant_normalized for tx {}: {}",
                            tx_id, e
                        );
                    } else {
                        normalized_count += 1;
                    }
                }
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as i64;
                warn!("Failed to normalize '{}': {}", description, e);

                // Record failure metric
                let metric = NewOllamaMetric {
                    operation: OllamaOperation::NormalizeMerchant,
                    model: ollama.model().to_string(),
                    latency_ms,
                    success: false,
                    error_message: Some(e.to_string()),
                    confidence: None,
                    transaction_id: tx_ids.first().copied(),
                    input_text: Some(description.clone()),
                    result_text: None,
                    metadata: None,
                };
                if let Err(me) = db.record_ollama_metric(&metric) {
                    warn!("Failed to record Ollama metric: {}", me);
                }
            }
        }
    }

    normalized_count
}

/// Extract Amex context from transaction original_data for merchant normalization
fn extract_amex_context(
    tx: &Transaction,
    _description: &str,
    category_hint: Option<&str>,
) -> Option<MerchantContext> {
    // Check if this is an Amex transaction with original_data
    let original_data = tx.original_data.as_ref()?;

    // Only process if import_format indicates Amex
    let format = tx.import_format.as_ref()?;
    if !format.contains("amex") {
        return None;
    }

    // Parse the JSON original_data
    let data: serde_json::Value = serde_json::from_str(original_data).ok()?;

    // Extract Amex-specific fields
    let statement_as = data.get("Appears On Your Statement As")?.as_str();
    let extended_details = data.get("Extended Details")?.as_str();

    Some(MerchantContext {
        extracted_merchant: None,
        statement_as: statement_as.map(String::from),
        extended_details: extended_details.map(String::from),
        category: category_hint.map(String::from),
    })
}

/// GET /api/imports/:id/reprocess-comparison - Get before/after comparison from latest reprocess
/// (Legacy endpoint - returns comparison from most recent run)
pub async fn get_reprocess_comparison(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Option<ReprocessComparison>>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify session exists
    state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    // Try to get comparison from the latest run first
    let comparison = if let Some(latest_run) = state.db.get_latest_reprocess_run(id)? {
        state.db.get_reprocess_comparison_for_run(latest_run.id)?
    } else {
        // Fall back to legacy comparison (snapshots without run_id)
        state.db.get_reprocess_comparison(id)?
    };

    state.db.log_audit(
        &user_email,
        "view",
        Some("reprocess_comparison"),
        Some(id),
        None,
    )?;

    Ok(Json(comparison))
}

// ============================================================================
// Reprocess Run Endpoints
// ============================================================================

/// GET /api/imports/:id/runs - List all reprocess runs for an import session
pub async fn list_reprocess_runs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Vec<ReprocessRunSummary>>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify session exists
    state
        .db
        .get_import_session(id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    let runs = state.db.list_reprocess_runs(id)?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("reprocess_runs"),
        Some(id),
        Some(&format!("count={}", runs.len())),
    )?;

    Ok(Json(runs))
}

/// GET /api/imports/:id/runs/:run_id - Get a specific reprocess run with its comparison data
pub async fn get_reprocess_run(
    State(state): State<Arc<AppState>>,
    Path((session_id, run_id)): Path<(i64, i64)>,
    request: Request,
) -> Result<Json<ReprocessRunWithComparison>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify session exists
    state
        .db
        .get_import_session(session_id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    let run = state
        .db
        .get_reprocess_run_with_comparison(run_id)?
        .ok_or_else(|| AppError::not_found("Reprocess run not found"))?;

    // Verify run belongs to this session
    if run.run.import_session_id != session_id {
        return Err(AppError::not_found("Reprocess run not found"));
    }

    state.db.log_audit(
        &user_email,
        "view",
        Some("reprocess_run"),
        Some(run_id),
        None,
    )?;

    Ok(Json(run))
}

/// Query parameters for comparing runs
#[derive(Debug, Deserialize)]
pub struct CompareRunsQuery {
    /// First run ID (typically older)
    pub run_a: i64,
    /// Second run ID (typically newer)
    pub run_b: i64,
}

/// GET /api/imports/:id/runs/compare - Compare two specific reprocess runs
///
/// Special case: run_a=0 means compare to the initial import state (before any reprocessing)
pub async fn compare_reprocess_runs(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<i64>,
    Query(params): Query<CompareRunsQuery>,
    request: Request,
) -> Result<Json<RunComparison>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify session exists
    state
        .db
        .get_import_session(session_id)?
        .ok_or_else(|| AppError::not_found("Import session not found"))?;

    // Handle special case: run_a=0 means compare to initial import
    let comparison = if params.run_a == 0 {
        // Verify run_b exists and belongs to this session
        let run_b = state
            .db
            .get_reprocess_run(params.run_b)?
            .ok_or_else(|| AppError::not_found("Run B not found"))?;

        if run_b.import_session_id != session_id {
            return Err(AppError::bad_request(
                "Run must belong to the specified import session",
            ));
        }

        state
            .db
            .compare_run_to_initial(session_id, params.run_b)?
            .ok_or_else(|| {
                AppError::bad_request("Cannot compare to initial: run is missing 'after' snapshot")
            })?
    } else {
        // Normal case: compare two runs
        let run_a = state
            .db
            .get_reprocess_run(params.run_a)?
            .ok_or_else(|| AppError::not_found("Run A not found"))?;
        let run_b = state
            .db
            .get_reprocess_run(params.run_b)?
            .ok_or_else(|| AppError::not_found("Run B not found"))?;

        if run_a.import_session_id != session_id || run_b.import_session_id != session_id {
            return Err(AppError::bad_request(
                "Both runs must belong to the specified import session",
            ));
        }

        state
            .db
            .compare_runs(params.run_a, params.run_b)?
            .ok_or_else(|| {
                AppError::bad_request(
                    "Cannot compare runs: one or both are missing 'after' snapshots",
                )
            })?
    };

    state.db.log_audit(
        &user_email,
        "compare",
        Some("reprocess_runs"),
        Some(session_id),
        Some(&format!("run_a={}, run_b={}", params.run_a, params.run_b)),
    )?;

    Ok(Json(comparison))
}

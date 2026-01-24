//! Detection and import handlers

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Multipart, Request, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::{get_user_email, AppError, AppState, MAX_UPLOAD_SIZE};
use hone_core::{
    ai::{AIBackend, AIClient, MerchantContext},
    db::{Database, TransactionInsertResult},
    detect::WasteDetector,
    import::{detect_bank_format, parse_csv},
    models::{
        ImportTaggingBreakdown, NewImportSession, NewOllamaMetric, OllamaOperation, Transaction,
    },
    tags::TagAssigner,
};

/// Detection request parameters
#[derive(Debug, Deserialize)]
pub struct DetectRequest {
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "all".to_string()
}

/// Detection response
#[derive(Serialize)]
pub struct DetectResponse {
    pub subscriptions_found: usize,
    pub zombies_detected: usize,
    pub price_increases_detected: usize,
    pub duplicates_detected: usize,
}

/// POST /api/detect - Run waste detection
pub async fn run_detection(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<DetectResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Extract JSON body
    let bytes = axum::body::to_bytes(request.into_body(), 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let params: DetectRequest = serde_json::from_slice(&bytes).unwrap_or(DetectRequest {
        kind: default_kind(),
    });

    // Build detector with best available AI capabilities
    let detector = match (&state.orchestrator, &state.ai) {
        (Some(ref orch), Some(ref ai)) => {
            WasteDetector::with_ai_and_orchestrator(&state.db, ai, orch)
        }
        (Some(ref orch), None) => WasteDetector::with_orchestrator(&state.db, orch),
        (None, Some(ref ai)) => WasteDetector::with_ai(&state.db, ai),
        (None, None) => WasteDetector::new(&state.db),
    };

    let results = match params.kind.as_str() {
        "zombies" => detector.detect_zombies_only().await?,
        "increases" => detector.detect_increases_only().await?,
        "duplicates" => detector.detect_duplicates_only().await?,
        _ => detector.detect_all().await?,
    };

    // Audit log
    state.db.log_audit(
        &user_email,
        "detect",
        None,
        None,
        Some(&format!(
            "kind={}, subscriptions={}, zombies={}, increases={}, duplicates={}",
            params.kind,
            results.subscriptions_found,
            results.zombies_detected,
            results.price_increases_detected,
            results.duplicates_detected
        )),
    )?;

    Ok(Json(DetectResponse {
        subscriptions_found: results.subscriptions_found,
        zombies_detected: results.zombies_detected,
        price_increases_detected: results.price_increases_detected,
        duplicates_detected: results.duplicates_detected,
    }))
}

/// Response for import endpoint
#[derive(Serialize)]
pub struct ImportResponse {
    pub imported: usize,
    pub skipped: usize,
    pub account_name: String,
    pub bank: String,
    // Import session ID for retrieving history
    pub import_session_id: i64,
    // Tagging results (auto-run after import)
    pub transactions_tagged: i64,
    // Tagging breakdown by source
    pub tagging_breakdown: ImportTaggingBreakdown,
    // Receipt matching results (auto-run after import)
    pub receipts_matched: usize,
    // Detection results (auto-run after import)
    pub subscriptions_found: usize,
    pub zombies_detected: usize,
    pub price_increases_detected: usize,
    pub duplicates_detected: usize,
}

/// POST /api/import - Import transactions from CSV
///
/// Expects multipart form with:
/// - file: CSV file (required, max 10MB)
/// - account_id: Account ID to import into (required)
/// - model: AI model to use (optional, uses server default if not specified)
pub async fn import_csv(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ImportResponse>, AppError> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut account_id: Option<i64> = None;
    let mut model_override: Option<String> = None;
    let mut total_size: usize = 0;

    // Extract fields from multipart form
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(&format!("Failed to read form field: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad_request("Failed to read file data"))?;
                total_size += bytes.len();

                // Check file size limit
                if total_size > MAX_UPLOAD_SIZE {
                    return Err(AppError::bad_request(&format!(
                        "File too large. Maximum size is {} MB",
                        MAX_UPLOAD_SIZE / 1024 / 1024
                    )));
                }

                file_data = Some(bytes.to_vec());
            }
            "account_id" => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad_request("Failed to read account_id"))?;
                account_id = Some(value.parse().map_err(|_| {
                    AppError::bad_request(&format!("Invalid account_id: {}", value))
                })?);
            }
            "model" => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad_request("Failed to read model"))?;
                if !value.is_empty() {
                    model_override = Some(value);
                }
            }
            _ => {}
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| AppError::bad_request("Missing file field"))?;
    let account_id = account_id.ok_or_else(|| AppError::bad_request("Missing account_id field"))?;

    // Delegate to core import logic
    import_csv_core(
        &state,
        &headers,
        file_data,
        account_id,
        model_override.as_deref(),
    )
    .await
}

/// Core import logic - separated for testability
///
/// This function contains all the business logic for importing CSV data,
/// separated from multipart form parsing.
///
/// The import runs in two phases:
/// 1. Synchronous: Parse CSV, insert transactions, return immediately
/// 2. Asynchronous: Run AI processing (tagging, normalization, detection) in background
///
/// # Arguments
/// * `model_override` - Optional model name to use instead of server default
pub async fn import_csv_core(
    state: &AppState,
    headers: &HeaderMap,
    file_data: Vec<u8>,
    account_id: i64,
    model_override: Option<&str>,
) -> Result<Json<ImportResponse>, AppError> {
    let user_email = get_user_email(headers);
    let user_email_opt = if user_email.is_empty() {
        None
    } else {
        Some(user_email.clone())
    };

    // Get the account to determine bank format
    let accounts = state.db.list_accounts()?;
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| AppError::not_found("Account not found"))?;

    let bank = account.bank;
    let account_name = account.name.clone();

    // Read first line to validate format (optional warning)
    let file_str = String::from_utf8_lossy(&file_data);
    if let Some(header_line) = file_str.lines().next() {
        if let Some(detected) = detect_bank_format(header_line) {
            if detected != bank {
                info!(
                    "CSV format detected as {:?} but account is configured for {:?}",
                    detected, bank
                );
            }
        }
    }

    // Parse the CSV
    let transactions = parse_csv(file_data.as_slice(), bank)?;

    // Apply model override if specified
    let effective_ai = match (state.ai.as_ref(), model_override) {
        (Some(ai), Some(model)) => Some(ai.with_model(model)),
        (Some(ai), None) => Some(ai.clone()),
        _ => None,
    };
    let effective_model = effective_ai.as_ref().map(|o| o.model().to_string());

    // Create import session to track this import
    let new_session = NewImportSession {
        account_id,
        filename: None, // filename not available from multipart
        file_size_bytes: Some(file_data.len() as i64),
        bank,
        user_email: user_email_opt.clone(),
        ollama_model: effective_model.clone(),
    };
    let import_session_id = state.db.create_import_session(&new_session)?;
    info!("Created import session {}", import_session_id);

    // Import transactions with session tracking (synchronous phase)
    let mut imported = 0;
    let mut skipped = 0;

    for tx in &transactions {
        match state
            .db
            .insert_transaction_with_session(account_id, tx, import_session_id)?
        {
            TransactionInsertResult::Inserted(_) => imported += 1,
            TransactionInsertResult::Duplicate(existing_id) => {
                skipped += 1;
                // Record the skipped transaction for history
                if let Err(e) = state.db.record_skipped_transaction(
                    import_session_id,
                    tx.date,
                    &tx.description,
                    tx.amount,
                    &tx.import_hash,
                    Some(existing_id),
                ) {
                    warn!("Failed to record skipped transaction: {}", e);
                }
            }
        }
    }

    // Update session with initial import counts
    if let Err(e) = state.db.update_import_session_results(
        import_session_id,
        imported as i64,
        skipped as i64,
        &ImportTaggingBreakdown::default(),
        0,
        0,
        0,
        0,
        0,
    ) {
        warn!("Failed to update import session results: {}", e);
    }

    // Audit log for the sync phase
    state.db.log_audit(
        &user_email,
        "import",
        Some("transaction"),
        None,
        Some(&format!(
            "session={}, account={}, file_size={}, imported={}, skipped={}",
            import_session_id,
            account_name,
            file_data.len(),
            imported,
            skipped,
        )),
    )?;

    // Spawn background task for AI processing if we imported transactions
    if imported > 0 {
        let db = state.db.clone();
        let ollama = effective_ai;
        let imported_count = imported as i64;
        let model_override_owned = model_override.map(String::from);

        tokio::spawn(async move {
            // Create orchestrator inside task with cloned db (AIOrchestrator contains Database)
            // Apply model override if specified
            let orchestrator = match model_override_owned.as_deref() {
                Some(model) => hone_core::ai::orchestrator::AIOrchestrator::from_env(db.clone())
                    .map(|o| o.with_model(model)),
                None => hone_core::ai::orchestrator::AIOrchestrator::from_env(db.clone()),
            };

            if let Err(e) = run_async_import_processing(
                &db,
                ollama.as_ref(),
                orchestrator.as_ref(),
                import_session_id,
                imported_count,
            )
            .await
            {
                error!(
                    "Background import processing failed for session {}: {}",
                    import_session_id, e
                );
                if let Err(e2) = db.mark_import_failed(import_session_id, &e.to_string()) {
                    error!("Failed to mark import as failed: {}", e2);
                }
            }
        });
    } else {
        // No transactions imported, mark as completed immediately
        if let Err(e) = state.db.mark_import_completed(import_session_id) {
            warn!("Failed to mark import as completed: {}", e);
        }
    }

    // Return immediately with import counts (AI processing runs in background)
    Ok(Json(ImportResponse {
        imported,
        skipped,
        account_name,
        bank: bank.as_str().to_string(),
        import_session_id,
        // These will be 0 initially - UI should poll for updates
        transactions_tagged: 0,
        tagging_breakdown: ImportTaggingBreakdown::default(),
        receipts_matched: 0,
        subscriptions_found: 0,
        zombies_detected: 0,
        price_increases_detected: 0,
        duplicates_detected: 0,
    }))
}

/// Run the async import processing (tagging, normalization, detection)
///
/// This runs in a background task after the initial import returns.
async fn run_async_import_processing(
    db: &Database,
    ollama: Option<&AIClient>,
    orchestrator: Option<&hone_core::ai::orchestrator::AIOrchestrator>,
    session_id: i64,
    imported_count: i64,
) -> Result<(), hone_core::error::Error> {
    use std::time::Instant;

    info!(
        "Starting async import processing for session {}",
        session_id
    );
    let total_start = Instant::now();

    // Phase 1: Tagging - only tag transactions from this import session
    let tagging_start = Instant::now();
    db.update_import_progress(session_id, "tagging", 0, imported_count)?;

    let assigner = TagAssigner::new(db, ollama);

    // Create progress callback that updates import session
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
        "Auto-tagged {}/{} transactions for session {} in {}ms (learned: {}, rules: {}, patterns: {}, ollama: {}, bank_cat: {}, other: {})",
        backfill.transactions_tagged,
        backfill.transactions_processed,
        session_id,
        tagging_duration_ms,
        backfill.by_learned,
        backfill.by_rule,
        backfill.by_pattern,
        backfill.by_ollama,
        backfill.by_bank_category,
        backfill.fallback_to_other
    );

    // Save tagging results immediately so UI shows them during processing
    if let Err(e) = db.update_import_session_tagging(session_id, &tagging_breakdown) {
        warn!("Failed to update tagging breakdown: {}", e);
    }

    // Phase 2: Merchant normalization via Ollama
    let normalizing_start = Instant::now();
    if let Some(ollama_client) = ollama {
        db.update_import_progress(session_id, "normalizing", 0, 0)?;
        let normalized = normalize_merchants(db, ollama_client, imported_count, session_id).await;
        info!("Normalized {} merchant names", normalized);
    }
    let normalizing_duration_ms = normalizing_start.elapsed().as_millis() as i64;
    if let Err(e) =
        db.update_import_phase_duration(session_id, "normalizing", normalizing_duration_ms)
    {
        warn!("Failed to update normalizing duration: {}", e);
    }

    // Phase 3: Receipt matching
    let matching_start = Instant::now();
    db.update_import_progress(session_id, "matching_receipts", 0, 1)?;
    let receipts_matched = match db.auto_match_receipts() {
        Ok((matched, checked)) => {
            if matched > 0 {
                info!(
                    "Auto-matched {} of {} pending receipts to transactions",
                    matched, checked
                );
            }
            matched
        }
        Err(e) => {
            warn!("Failed to auto-match receipts: {}", e);
            0
        }
    };
    let matching_duration_ms = matching_start.elapsed().as_millis() as i64;
    if let Err(e) =
        db.update_import_phase_duration(session_id, "matching_receipts", matching_duration_ms)
    {
        warn!("Failed to update matching duration: {}", e);
    }

    // Phase 4: Detection with granular progress
    let detecting_start = Instant::now();
    db.update_import_progress(session_id, "detecting", 0, 1)?;

    // Build detector with best available AI capabilities
    let detector = match (orchestrator, ollama) {
        (Some(orch), Some(ai)) => WasteDetector::with_ai_and_orchestrator(db, ai, orch),
        (Some(orch), None) => WasteDetector::with_orchestrator(db, orch),
        (None, Some(ai)) => WasteDetector::with_ai(db, ai),
        (None, None) => WasteDetector::new(db),
    };

    // Create progress callback that updates import session
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

    // Get current session to preserve imported/skipped counts
    let session = db
        .get_import_session(session_id)?
        .ok_or_else(|| hone_core::error::Error::NotFound("Import session not found".to_string()))?;

    // Update session with final results
    db.update_import_session_results(
        session_id,
        session.session.imported_count,
        session.session.skipped_count,
        &tagging_breakdown,
        detection_results.subscriptions_found as i64,
        detection_results.zombies_detected as i64,
        detection_results.price_increases_detected as i64,
        detection_results.duplicates_detected as i64,
        receipts_matched as i64,
    )?;

    // Mark as completed
    db.mark_import_completed(session_id)?;

    info!(
        "Async import processing completed for session {} in {}ms - subs: {}, zombies: {}, increases: {}, duplicates: {}",
        session_id,
        total_duration_ms,
        detection_results.subscriptions_found,
        detection_results.zombies_detected,
        detection_results.price_increases_detected,
        detection_results.duplicates_detected
    );

    Ok(())
}

/// Import CSV data directly (for JSON-based API, used by tests)
///
/// Request body contains:
/// - account_id: Account ID to import into
/// - csv_data: Base64-encoded CSV content
/// - model: Optional AI model override for testing different models
#[derive(Debug, Deserialize)]
pub struct ImportCsvJsonRequest {
    pub account_id: i64,
    pub csv_data: String, // Base64-encoded CSV content
    /// Optional model override (uses server default if not specified)
    #[serde(default)]
    pub model: Option<String>,
}

/// POST /api/import/json - Import transactions from CSV via JSON (for testing)
pub async fn import_csv_json(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ImportCsvJsonRequest>,
) -> Result<Json<ImportResponse>, AppError> {
    use base64::Engine;

    // Decode base64 CSV data
    let file_data = base64::engine::general_purpose::STANDARD
        .decode(&req.csv_data)
        .map_err(|e| AppError::bad_request(&format!("Invalid base64 data: {}", e)))?;

    // Check file size limit
    if file_data.len() > MAX_UPLOAD_SIZE {
        return Err(AppError::bad_request(&format!(
            "File too large. Maximum size is {} MB",
            MAX_UPLOAD_SIZE / 1024 / 1024
        )));
    }

    // Delegate to core import logic with model override
    import_csv_core(
        &state,
        &headers,
        file_data,
        req.account_id,
        req.model.as_deref(),
    )
    .await
}

/// Strip common payment method prefixes from merchant descriptions
/// This helps group transactions that are the same merchant but with different payment methods
fn strip_payment_prefix(description: &str) -> &str {
    let prefixes = [
        "AplPay ",
        "APLPAY ",
        "ApplePay ",
        "APPLEPAY ",
        "SP * ",
        "SP *",
        "SQ * ",
        "SQ *",
        "TST* ",
        "TST*",
        "GOOGLE *",
        "Google *",
    ];

    for prefix in prefixes {
        if let Some(stripped) = description.strip_prefix(prefix) {
            return stripped;
        }
    }
    description
}

/// Normalize merchant names for recently imported transactions via Ollama
async fn normalize_merchants(db: &Database, ollama: &AIClient, limit: i64, session_id: i64) -> i64 {
    // Get transactions without normalized merchant names
    let transactions = match db.get_unnormalized_transactions(limit) {
        Ok(txs) => txs,
        Err(e) => {
            warn!("Failed to get unnormalized transactions: {}", e);
            return 0;
        }
    };

    info!(
        "Found {} transactions without normalized merchant names",
        transactions.len()
    );

    if transactions.is_empty() {
        return 0;
    }

    // Collect unique descriptions with representative transaction for context
    // We keep one transaction per description to extract original_data for Amex context
    // Key by stripped description (without payment prefix) so "AplPay X" and "X" group together
    let mut unique_descriptions: HashMap<String, (Vec<i64>, Option<Transaction>)> = HashMap::new();
    for tx in transactions {
        let key = strip_payment_prefix(&tx.description).to_string();
        unique_descriptions
            .entry(key)
            .or_insert_with(|| (Vec::new(), Some(tx.clone())))
            .0
            .push(tx.id);
    }

    let total_unique = unique_descriptions.len() as i64;
    info!(
        "Normalizing {} unique merchant descriptions via Ollama",
        total_unique
    );

    // Update progress with total count now that we know it
    if let Err(e) = db.update_import_progress(session_id, "normalizing", 0, total_unique) {
        warn!("Failed to update import progress: {}", e);
    }

    let mut normalized_count = 0;
    let mut processed_count = 0;

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
            // Update progress even for cache hits
            processed_count += 1;
            let _ =
                db.update_import_progress(session_id, "normalizing", processed_count, total_unique);
            continue;
        }

        // 2. Not in cache - call Ollama
        // Get the category hint from the first transaction's tag
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

                // 3. CACHE THE RESULT for future imports
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

        // Update progress after each Ollama call
        processed_count += 1;
        let _ = db.update_import_progress(session_id, "normalizing", processed_count, total_unique);
    }

    normalized_count
}

/// Extract Amex-specific context from transaction's original_data
/// Returns None if not an Amex transaction or no useful context available
fn extract_amex_context(
    tx: &Transaction,
    description: &str,
    category: Option<&str>,
) -> Option<MerchantContext> {
    // Only process Amex transactions
    if tx.import_format.as_deref() != Some("amex_csv") {
        return None;
    }

    let original_data = tx.original_data.as_ref()?;
    let data: serde_json::Value = serde_json::from_str(original_data).ok()?;

    let extended_details = data
        .get("Extended Details")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let statement_as = data
        .get("Appears On Your Statement As")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let bank_category = data
        .get("Category")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    // Only return context if we have extended details (that's the valuable part)
    if extended_details.is_none() {
        return None;
    }

    Some(MerchantContext {
        extracted_merchant: Some(description.to_string()),
        statement_as,
        extended_details,
        // Prefer bank category (e.g., "Transportation-Fuel") over Hone tag for normalization
        // Bank categories are more specific and informative for identifying merchant types
        category: bank_category.or_else(|| category.map(|s| s.to_string())),
    })
}

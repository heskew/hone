//! Ollama metrics, health, and reprocessing handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{get_user_email, AppError, AppState};
use hone_core::ai::{AIBackend, MerchantContext};
use hone_core::models::Transaction;
use hone_core::models::{
    ModelComparisonStats, ModelRecommendation, OllamaHealthStatus, OllamaMetric, OllamaStats,
    StatsSummary, TagSource,
};
use hone_core::tags::TagAssigner;

use super::reports::resolve_period;

/// Query parameters for Ollama stats
#[derive(Debug, Deserialize)]
pub struct OllamaStatsQuery {
    /// Period preset (this-month, last-month, last-30-days, all, etc)
    pub period: Option<String>,
}

/// GET /api/ollama/stats - Get aggregated Ollama metrics
pub async fn ollama_stats(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OllamaStatsQuery>,
    request: Request,
) -> Result<Json<OllamaStats>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("last-30-days");
    let (from_date, to_date) = resolve_period(period, None, None)?;

    let stats = state.db.get_ollama_stats(from_date, to_date)?;

    state.db.log_audit(
        &user_email,
        "ollama",
        Some("stats"),
        None,
        Some(&format!(
            "period={}, calls={}, success_rate={:.2}%",
            period,
            stats.total_calls,
            stats.success_rate * 100.0
        )),
    )?;

    Ok(Json(stats))
}

/// Query parameters for recent Ollama calls
#[derive(Debug, Deserialize)]
pub struct OllamaCallsQuery {
    /// Number of calls to return (default 50, max 200)
    pub limit: Option<i64>,
}

/// GET /api/ollama/calls - Get recent Ollama calls for debugging
pub async fn ollama_recent_calls(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OllamaCallsQuery>,
    request: Request,
) -> Result<Json<Vec<OllamaMetric>>, AppError> {
    let user_email = get_user_email(request.headers());

    let limit = params.limit.unwrap_or(50).min(200);
    let calls = state.db.get_recent_ollama_calls(limit)?;

    state.db.log_audit(
        &user_email,
        "ollama",
        Some("calls"),
        None,
        Some(&format!("limit={}, returned={}", limit, calls.len())),
    )?;

    Ok(Json(calls))
}

/// GET /api/ollama/health - Get Ollama health status
pub async fn ollama_health(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<OllamaHealthStatus>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get database-tracked health info
    let mut health = state.db.get_ollama_health()?;

    // Do a live health check for standard AI client
    if let Some(ref client) = state.ai {
        health.available = client.health_check().await;
    }

    // Do a live health check for orchestrator (agentic mode)
    if let Some(ref orchestrator) = state.orchestrator {
        health.orchestrator_available = orchestrator.backend().health_check().await;
    }

    state.db.log_audit(
        &user_email,
        "ollama",
        Some("health"),
        None,
        Some(&format!(
            "available={}, orchestrator={}, error_rate={:.2}%",
            health.available,
            health.orchestrator_available,
            health.recent_error_rate * 100.0
        )),
    )?;

    Ok(Json(health))
}

/// GET /api/ollama/recommendation - Get model switch recommendation
pub async fn ollama_recommendation(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<ModelRecommendation>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get stats for the last 30 days
    let today = chrono::Utc::now().date_naive();
    let from_date = today - chrono::Duration::days(30);
    let stats = state.db.get_ollama_stats(from_date, today)?;

    // Get latency trend
    let trend = state.db.get_latency_trend(7)?;
    let latency_trend = if trend.len() >= 2 {
        let first_avg = trend.first().map(|(_, l)| *l).unwrap_or(0.0);
        let last_avg = trend.last().map(|(_, l)| *l).unwrap_or(0.0);
        if last_avg > first_avg * 1.2 {
            "degrading".to_string()
        } else if last_avg < first_avg * 0.8 {
            "improving".to_string()
        } else {
            "stable".to_string()
        }
    } else {
        "unknown".to_string()
    };

    // Build recommendations
    let mut recommendations = Vec::new();
    let mut should_switch = false;

    // Check success rate
    if stats.success_rate < 0.95 && stats.total_calls > 10 {
        recommendations.push("Success rate below 95% - check Ollama connectivity".to_string());
        should_switch = true;
    }

    // Check latency
    if stats.avg_latency_ms > 5000.0 && stats.total_calls > 10 {
        recommendations.push(
            "Average latency over 5 seconds - consider a smaller model for speed".to_string(),
        );
    }

    // Check accuracy
    if stats.accuracy.estimated_accuracy < 0.85 && stats.accuracy.total_ollama_tags > 20 {
        recommendations.push(
            "Estimated accuracy below 85% - consider a larger model for accuracy".to_string(),
        );
        should_switch = true;
    }

    // Check latency trend
    if latency_trend == "degrading" {
        recommendations.push("Latency is trending upward - consider restarting Ollama".to_string());
    }

    // All good message
    if recommendations.is_empty() {
        recommendations.push("Current setup performing well".to_string());
    }

    let current_model = std::env::var("OLLAMA_MODEL").ok();

    let recommendation = ModelRecommendation {
        current_model: current_model.clone(),
        stats_summary: StatsSummary {
            success_rate: stats.success_rate,
            avg_latency_ms: stats.avg_latency_ms,
            estimated_accuracy: stats.accuracy.estimated_accuracy,
            latency_trend,
        },
        recommendations,
        should_switch,
    };

    state.db.log_audit(
        &user_email,
        "ollama",
        Some("recommendation"),
        None,
        Some(&format!(
            "model={:?}, should_switch={}",
            current_model, should_switch
        )),
    )?;

    Ok(Json(recommendation))
}

/// GET /api/ollama/models - Get list of models that have been used
pub async fn ollama_models(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Vec<String>>, AppError> {
    let user_email = get_user_email(request.headers());

    let models = state.db.get_ollama_models()?;

    state.db.log_audit(
        &user_email,
        "ollama",
        Some("models"),
        None,
        Some(&format!("count={}", models.len())),
    )?;

    Ok(Json(models))
}

/// GET /api/ollama/stats/by-model - Get Ollama stats grouped by model for comparison
pub async fn ollama_stats_by_model(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OllamaStatsQuery>,
    request: Request,
) -> Result<Json<ModelComparisonStats>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("last-30-days");
    let (from_date, to_date) = resolve_period(period, None, None)?;

    let stats = state.db.get_ollama_stats_by_model(from_date, to_date)?;

    state.db.log_audit(
        &user_email,
        "ollama",
        Some("stats_by_model"),
        None,
        Some(&format!("period={}, models={}", period, stats.models.len())),
    )?;

    Ok(Json(stats))
}

/// Response for reprocess operation
#[derive(Debug, Serialize)]
pub struct ReprocessResponse {
    /// Transaction ID that was reprocessed
    pub transaction_id: i64,
    /// Whether reprocessing was successful
    pub success: bool,
    /// The new tag assigned (if any)
    pub new_tag: Option<String>,
    /// The normalized merchant name (if Ollama provided one)
    pub normalized_merchant: Option<String>,
    /// Classification source (ollama, pattern, rule, etc)
    pub source: Option<String>,
    /// Confidence score
    pub confidence: Option<f64>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Response for bulk reprocess operation
#[derive(Debug, Serialize)]
pub struct BulkReprocessResponse {
    /// Total transactions processed
    pub processed: i64,
    /// Successfully reprocessed count
    pub success_count: i64,
    /// Failed count
    pub failed_count: i64,
    /// Individual results
    pub results: Vec<ReprocessResponse>,
}

/// Request body for bulk reprocess
#[derive(Debug, Deserialize)]
pub struct BulkReprocessRequest {
    /// Transaction IDs to reprocess
    pub transaction_ids: Vec<i64>,
}

/// POST /api/transactions/:id/reprocess - Reprocess a single transaction with Ollama
pub async fn reprocess_transaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<ReprocessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get the transaction
    let transaction = state
        .db
        .get_transaction(id)?
        .ok_or_else(|| AppError::not_found(&format!("Transaction {} not found", id)))?;

    // Check if Ollama is available
    let ollama = match &state.ai {
        Some(client) => client,
        None => {
            return Ok(Json(ReprocessResponse {
                transaction_id: id,
                success: false,
                new_tag: None,
                normalized_merchant: None,
                source: None,
                confidence: None,
                error: Some("Ollama not configured".to_string()),
            }));
        }
    };

    // Remove existing Ollama tags before reprocessing
    let existing_tags = state.db.get_transaction_tags(id)?;
    for tag in &existing_tags {
        if tag.source == TagSource::Ollama {
            state.db.remove_transaction_tag(id, tag.tag_id)?;
        }
    }

    // Run classification
    let assigner = TagAssigner::new(&state.db, Some(ollama));
    let result = assigner.assign_tags(&transaction).await;

    // Also run merchant normalization separately (uses better prompt with more examples)
    // Get category hint from the transaction's tags for better normalization
    let category_hint = state
        .db
        .get_transaction_tags(id)
        .ok()
        .and_then(|tags| tags.first().map(|t| t.tag_id))
        .and_then(|tag_id| state.db.get_tag(tag_id).ok().flatten())
        .map(|tag| tag.name);

    // Use context-aware normalization for Amex transactions (extended details often has full merchant name)
    let context = extract_amex_context(
        &transaction,
        &transaction.description,
        category_hint.as_deref(),
    );
    let normalized_merchant = if let Some(ref ctx) = context {
        match ollama
            .normalize_merchant_with_context(&transaction.description, ctx)
            .await
        {
            Ok(normalized) => Some(normalized),
            Err(e) => {
                warn!("Failed to normalize merchant for tx {}: {}", id, e);
                None
            }
        }
    } else {
        match ollama
            .normalize_merchant(&transaction.description, category_hint.as_deref())
            .await
        {
            Ok(normalized) => Some(normalized),
            Err(e) => {
                warn!("Failed to normalize merchant for tx {}: {}", id, e);
                None
            }
        }
    };

    // Update normalized merchant in DB if we got one
    // Also update the cache so future imports get the corrected value
    if let Some(ref normalized) = normalized_merchant {
        if let Err(e) = state.db.update_merchant_normalized(id, normalized) {
            warn!("Failed to update merchant_normalized for tx {}: {}", id, e);
        }
        // Update cache with fresh Ollama result (overwrites old cached value)
        if let Err(e) =
            state
                .db
                .cache_merchant_name(&transaction.description, normalized, "ollama", 0.8)
        {
            warn!("Failed to update merchant name cache for tx {}: {}", id, e);
        }
    }

    let response = match result {
        Ok(Some(assignment)) => {
            // Add the new tag
            state.db.add_transaction_tag(
                id,
                assignment.tag_id,
                assignment.source.clone(),
                assignment.confidence,
            )?;

            ReprocessResponse {
                transaction_id: id,
                success: true,
                new_tag: Some(assignment.tag_name),
                normalized_merchant: normalized_merchant.clone(),
                source: Some(format!("{:?}", assignment.source)),
                confidence: assignment.confidence,
                error: None,
            }
        }
        Ok(None) => ReprocessResponse {
            transaction_id: id,
            success: true,
            new_tag: None,
            normalized_merchant,
            source: None,
            confidence: None,
            error: Some("No tag assigned".to_string()),
        },
        Err(e) => ReprocessResponse {
            transaction_id: id,
            success: false,
            new_tag: None,
            normalized_merchant: None,
            source: None,
            confidence: None,
            error: Some(e.to_string()),
        },
    };

    state.db.log_audit(
        &user_email,
        "reprocess",
        Some("transaction"),
        Some(id),
        Some(&format!(
            "success={}, tag={:?}, source={:?}",
            response.success, response.new_tag, response.source
        )),
    )?;

    Ok(Json(response))
}

/// POST /api/ollama/reprocess - Bulk reprocess multiple transactions
pub async fn bulk_reprocess_transactions(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<BulkReprocessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Extract JSON body
    let bytes = axum::body::to_bytes(request.into_body(), 64 * 1024)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let body: BulkReprocessRequest = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::bad_request(&format!("Invalid JSON: {}", e)))?;

    // Check if Ollama is available
    let ollama = match &state.ai {
        Some(client) => client,
        None => {
            return Err(AppError::bad_request("Ollama not configured"));
        }
    };

    let mut results = Vec::new();
    let mut success_count = 0i64;
    let mut failed_count = 0i64;

    let assigner = TagAssigner::new(&state.db, Some(ollama));

    for tx_id in &body.transaction_ids {
        // Get the transaction
        let transaction = match state.db.get_transaction(*tx_id)? {
            Some(tx) => tx,
            None => {
                results.push(ReprocessResponse {
                    transaction_id: *tx_id,
                    success: false,
                    new_tag: None,
                    normalized_merchant: None,
                    source: None,
                    confidence: None,
                    error: Some("Transaction not found".to_string()),
                });
                failed_count += 1;
                continue;
            }
        };

        // Remove existing Ollama tags
        let existing_tags = state.db.get_transaction_tags(*tx_id)?;
        for tag in &existing_tags {
            if tag.source == TagSource::Ollama {
                state.db.remove_transaction_tag(*tx_id, tag.tag_id)?;
            }
        }

        // Run classification
        let result = assigner.assign_tags(&transaction).await;

        // Also run merchant normalization separately (uses better prompt with more examples)
        // Get category hint from the transaction's tags for better normalization
        let category_hint = state
            .db
            .get_transaction_tags(*tx_id)
            .ok()
            .and_then(|tags| tags.first().map(|t| t.tag_id))
            .and_then(|tag_id| state.db.get_tag(tag_id).ok().flatten())
            .map(|tag| tag.name);

        // Use context-aware normalization for Amex transactions
        let context = extract_amex_context(
            &transaction,
            &transaction.description,
            category_hint.as_deref(),
        );
        let normalized_merchant = if let Some(ref ctx) = context {
            match ollama
                .normalize_merchant_with_context(&transaction.description, ctx)
                .await
            {
                Ok(normalized) => Some(normalized),
                Err(e) => {
                    warn!("Failed to normalize merchant for tx {}: {}", tx_id, e);
                    None
                }
            }
        } else {
            match ollama
                .normalize_merchant(&transaction.description, category_hint.as_deref())
                .await
            {
                Ok(normalized) => Some(normalized),
                Err(e) => {
                    warn!("Failed to normalize merchant for tx {}: {}", tx_id, e);
                    None
                }
            }
        };

        // Update normalized merchant in DB if we got one
        if let Some(ref normalized) = normalized_merchant {
            if let Err(e) = state.db.update_merchant_normalized(*tx_id, normalized) {
                warn!(
                    "Failed to update merchant_normalized for tx {}: {}",
                    tx_id, e
                );
            }
        }

        match result {
            Ok(Some(assignment)) => {
                // Add the new tag
                state.db.add_transaction_tag(
                    *tx_id,
                    assignment.tag_id,
                    assignment.source.clone(),
                    assignment.confidence,
                )?;

                results.push(ReprocessResponse {
                    transaction_id: *tx_id,
                    success: true,
                    new_tag: Some(assignment.tag_name),
                    normalized_merchant: normalized_merchant.clone(),
                    source: Some(format!("{:?}", assignment.source)),
                    confidence: assignment.confidence,
                    error: None,
                });
                success_count += 1;
            }
            Ok(None) => {
                results.push(ReprocessResponse {
                    transaction_id: *tx_id,
                    success: true,
                    new_tag: None,
                    normalized_merchant,
                    source: None,
                    confidence: None,
                    error: Some("No tag assigned".to_string()),
                });
                success_count += 1;
            }
            Err(e) => {
                results.push(ReprocessResponse {
                    transaction_id: *tx_id,
                    success: false,
                    new_tag: None,
                    normalized_merchant: None,
                    source: None,
                    confidence: None,
                    error: Some(e.to_string()),
                });
                failed_count += 1;
            }
        }
    }

    state.db.log_audit(
        &user_email,
        "bulk_reprocess",
        Some("transaction"),
        None,
        Some(&format!(
            "total={}, success={}, failed={}",
            body.transaction_ids.len(),
            success_count,
            failed_count
        )),
    )?;

    Ok(Json(BulkReprocessResponse {
        processed: body.transaction_ids.len() as i64,
        success_count,
        failed_count,
        results,
    }))
}

/// Extract Amex context from a transaction's original_data for better normalization
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
        // Prefer bank category over Hone tag for normalization
        category: bank_category.or_else(|| category.map(|s| s.to_string())),
    })
}

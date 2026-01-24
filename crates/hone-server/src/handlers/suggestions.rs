//! AI suggestion handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Request, State},
    Json,
};
use serde::Serialize;

use crate::{get_user_email, AppError, AppState};
use hone_core::ai::{AIBackend, SplitRecommendation};

/// Entity suggestion response
#[derive(Debug, Serialize)]
pub struct EntitySuggestionResponse {
    pub transaction_id: i64,
    pub suggested_entity: Option<String>,
    pub available_entities: Vec<String>,
}

/// GET /api/transactions/:id/suggest-entity - Get entity suggestion for a transaction
pub async fn suggest_entity(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<i64>,
    request: Request,
) -> Result<Json<EntitySuggestionResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get transaction
    let transaction = state
        .db
        .get_transaction(transaction_id)?
        .ok_or_else(|| AppError::not_found("Transaction not found"))?;

    // Get available entities
    let entities = state.db.list_entities(false)?;
    let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();

    if entity_names.is_empty() {
        return Ok(Json(EntitySuggestionResponse {
            transaction_id,
            suggested_entity: None,
            available_entities: vec![],
        }));
    }

    // Get transaction tags for category context
    let tags = state.db.get_transaction_tags_with_details(transaction_id)?;
    let category = tags
        .first()
        .map(|t| t.tag_name.clone())
        .unwrap_or_else(|| "Other".to_string());

    // Get Ollama suggestion
    let suggested = if let Some(ref ollama) = state.ai {
        ollama
            .suggest_entity(&transaction.description, &category, &entity_names)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    state.db.log_audit(
        &user_email,
        "suggest_entity",
        Some("transaction"),
        Some(transaction_id),
        Some(&format!("suggested={:?}", suggested)),
    )?;

    Ok(Json(EntitySuggestionResponse {
        transaction_id,
        suggested_entity: suggested,
        available_entities: entity_names,
    }))
}

/// Split recommendation response
#[derive(Debug, Serialize)]
pub struct SplitSuggestionResponse {
    pub transaction_id: i64,
    pub merchant: String,
    pub recommendation: Option<SplitRecommendation>,
}

/// GET /api/transactions/:id/suggest-split - Check if transaction should be split
pub async fn suggest_split(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<i64>,
    request: Request,
) -> Result<Json<SplitSuggestionResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    // Get transaction
    let transaction = state
        .db
        .get_transaction(transaction_id)?
        .ok_or_else(|| AppError::not_found("Transaction not found"))?;

    // Get Ollama recommendation
    let recommendation = if let Some(ref ollama) = state.ai {
        ollama
            .should_suggest_split(&transaction.description)
            .await
            .ok()
    } else {
        None
    };

    state.db.log_audit(
        &user_email,
        "suggest_split",
        Some("transaction"),
        Some(transaction_id),
        Some(&format!(
            "should_split={:?}",
            recommendation.as_ref().map(|r| r.should_split)
        )),
    )?;

    Ok(Json(SplitSuggestionResponse {
        transaction_id,
        merchant: transaction.description.clone(),
        recommendation,
    }))
}

//! Entity management handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::Deserialize;

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::{Entity, EntityType, NewEntity};

/// Query parameters for listing entities
#[derive(Debug, Deserialize)]
pub struct EntityQuery {
    /// Entity type filter: person, pet, vehicle, property
    pub entity_type: Option<String>,
    /// Include archived entities
    #[serde(default)]
    pub include_archived: bool,
}

/// GET /api/entities - List all entities
pub async fn list_entities(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EntityQuery>,
    request: Request,
) -> Result<Json<Vec<Entity>>, AppError> {
    let user_email = get_user_email(request.headers());

    let entities = if let Some(ref type_str) = params.entity_type {
        let entity_type: EntityType = type_str.parse().map_err(|_| {
            AppError::bad_request(&format!(
                "Invalid entity_type: {}. Valid: person, pet, vehicle, property",
                type_str
            ))
        })?;
        state.db.list_entities_by_type(entity_type)?
    } else {
        state.db.list_entities(params.include_archived)?
    };

    state.db.log_audit(
        &user_email,
        "list",
        Some("entity"),
        None,
        Some(&format!(
            "type={:?}, include_archived={}, count={}",
            params.entity_type,
            params.include_archived,
            entities.len()
        )),
    )?;

    Ok(Json(entities))
}

/// GET /api/entities/:id - Get a specific entity
pub async fn get_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Entity>, AppError> {
    let user_email = get_user_email(request.headers());

    let entity = state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    state
        .db
        .log_audit(&user_email, "view", Some("entity"), Some(id), None)?;

    Ok(Json(entity))
}

/// Request body for creating an entity
#[derive(Debug, Deserialize)]
pub struct CreateEntityRequest {
    pub name: String,
    pub entity_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// POST /api/entities - Create a new entity
pub async fn create_entity(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Entity>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: CreateEntityRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let entity_type: EntityType = req.entity_type.parse().map_err(|_| {
        AppError::bad_request(&format!(
            "Invalid entity_type: {}. Valid: person, pet, vehicle, property",
            req.entity_type
        ))
    })?;

    let new_entity = NewEntity {
        name: req.name.clone(),
        entity_type,
        icon: req.icon,
        color: req.color,
    };

    let entity_id = state.db.create_entity(&new_entity)?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("entity"),
        Some(entity_id),
        Some(&format!("name={}, type={}", req.name, req.entity_type)),
    )?;

    let entity = state
        .db
        .get_entity(entity_id)?
        .ok_or_else(|| AppError::internal("Entity not found after creation"))?;

    Ok(Json(entity))
}

/// Request body for updating an entity
#[derive(Debug, Deserialize)]
pub struct UpdateEntityRequest {
    pub name: Option<String>,
    pub icon: Option<Option<String>>,
    pub color: Option<Option<String>>,
}

/// PATCH /api/entities/:id - Update an entity
pub async fn update_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Entity>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateEntityRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    // Verify entity exists
    state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    state.db.update_entity(
        id,
        req.name.as_deref(),
        req.icon.as_ref().and_then(|o| o.as_deref()),
        req.color.as_ref().and_then(|o| o.as_deref()),
    )?;

    state
        .db
        .log_audit(&user_email, "update", Some("entity"), Some(id), None)?;

    let entity = state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    Ok(Json(entity))
}

/// Query parameters for deleting an entity
#[derive(Debug, Deserialize)]
pub struct DeleteEntityQuery {
    #[serde(default)]
    pub force: bool,
}

/// DELETE /api/entities/:id - Delete an entity
pub async fn delete_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeleteEntityQuery>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let entity = state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    // Check if entity has associated splits
    let split_count = state.db.count_splits_by_entity(id)?;
    if split_count > 0 && !params.force {
        return Err(AppError::bad_request(&format!(
            "Entity '{}' has {} associated splits. Use ?force=true to delete anyway.",
            entity.name, split_count
        )));
    }

    state.db.delete_entity(id)?;

    state.db.log_audit(
        &user_email,
        "delete",
        Some("entity"),
        Some(id),
        Some(&format!("name={}, force={}", entity.name, params.force)),
    )?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /api/entities/:id/archive - Archive an entity
pub async fn archive_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Entity>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify entity exists
    state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    state.db.archive_entity(id)?;

    state
        .db
        .log_audit(&user_email, "archive", Some("entity"), Some(id), None)?;

    let entity = state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    Ok(Json(entity))
}

/// POST /api/entities/:id/unarchive - Unarchive an entity
pub async fn unarchive_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Entity>, AppError> {
    let user_email = get_user_email(request.headers());

    // Verify entity exists
    state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    state.db.unarchive_entity(id)?;

    state
        .db
        .log_audit(&user_email, "unarchive", Some("entity"), Some(id), None)?;

    let entity = state
        .db
        .get_entity(id)?
        .ok_or_else(|| AppError::not_found("Entity not found"))?;

    Ok(Json(entity))
}

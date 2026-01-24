//! Tag management handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{get_user_email, AppError, AppState, SuccessResponse};
use hone_core::models::{PatternType, Tag, TagRuleWithTag, TagWithPath};

/// GET /api/tags - List all tags (flat list)
pub async fn list_tags(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Vec<Tag>>, AppError> {
    let user_email = get_user_email(request.headers());

    let tags = state.db.list_tags()?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("tag"),
        None,
        Some(&format!("count={}", tags.len())),
    )?;

    Ok(Json(tags))
}

/// GET /api/tags/tree - Get hierarchical tag tree
pub async fn get_tag_tree(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Vec<TagWithPath>>, AppError> {
    let user_email = get_user_email(request.headers());

    let tree = state.db.get_tag_tree()?;

    state
        .db
        .log_audit(&user_email, "view", Some("tag_tree"), None, None)?;

    Ok(Json(tree))
}

/// GET /api/tags/:id - Get a specific tag
pub async fn get_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Tag>, AppError> {
    let user_email = get_user_email(request.headers());

    let tag = state
        .db
        .get_tag(id)?
        .ok_or_else(|| AppError::not_found("Tag not found"))?;

    state
        .db
        .log_audit(&user_email, "view", Some("tag"), Some(id), None)?;

    Ok(Json(tag))
}

/// Request body for creating a tag
#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub auto_patterns: Option<String>,
}

/// POST /api/tags - Create a new tag
pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Tag>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: CreateTagRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let tag_id = state.db.create_tag(
        &req.name,
        req.parent_id,
        req.color.as_deref(),
        req.icon.as_deref(),
        req.auto_patterns.as_deref(),
    )?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("tag"),
        Some(tag_id),
        Some(&format!("name={}", req.name)),
    )?;

    let tag = state
        .db
        .get_tag(tag_id)?
        .ok_or_else(|| AppError::internal("Tag not found after creation"))?;

    Ok(Json(tag))
}

/// Request body for updating a tag
#[derive(Debug, Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub parent_id: Option<Option<i64>>,
    pub color: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub auto_patterns: Option<Option<String>>,
}

/// PATCH /api/tags/:id - Update a tag
pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<Tag>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: UpdateTagRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    state.db.update_tag(
        id,
        req.name.as_deref(),
        req.parent_id,
        req.color.as_ref().map(|o| o.as_deref()),
        req.icon.as_ref().map(|o| o.as_deref()),
        req.auto_patterns.as_ref().map(|o| o.as_deref()),
    )?;

    state
        .db
        .log_audit(&user_email, "update", Some("tag"), Some(id), None)?;

    let tag = state
        .db
        .get_tag(id)?
        .ok_or_else(|| AppError::not_found("Tag not found"))?;

    Ok(Json(tag))
}

/// Query parameters for deleting a tag
#[derive(Debug, Deserialize)]
pub struct DeleteTagQuery {
    #[serde(default)]
    pub reparent_to_parent: bool,
}

/// DELETE /api/tags/:id - Delete a tag
pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeleteTagQuery>,
    request: Request,
) -> Result<Json<hone_core::models::DeleteTagResult>, AppError> {
    let user_email = get_user_email(request.headers());

    let result = state.db.delete_tag(id, params.reparent_to_parent)?;

    state.db.log_audit(
        &user_email,
        "delete",
        Some("tag"),
        Some(id),
        Some(&format!(
            "reparent={}, transactions_moved={}, children_affected={}",
            params.reparent_to_parent, result.transactions_moved, result.children_affected
        )),
    )?;

    Ok(Json(result))
}

/// GET /api/rules - List all tag rules
pub async fn list_tag_rules(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<Vec<TagRuleWithTag>>, AppError> {
    let user_email = get_user_email(request.headers());

    let rules = state.db.list_tag_rules()?;

    state.db.log_audit(
        &user_email,
        "list",
        Some("tag_rule"),
        None,
        Some(&format!("count={}", rules.len())),
    )?;

    Ok(Json(rules))
}

/// Request body for creating a tag rule
#[derive(Debug, Deserialize)]
pub struct CreateTagRuleRequest {
    pub tag_id: i64,
    pub pattern: String,
    #[serde(default = "default_pattern_type")]
    pub pattern_type: String,
    #[serde(default)]
    pub priority: i32,
}

fn default_pattern_type() -> String {
    "contains".to_string()
}

/// POST /api/rules - Create a new tag rule
pub async fn create_tag_rule(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<TagRuleWithTag>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: CreateTagRuleRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let pattern_type: PatternType = req.pattern_type.parse().map_err(|_| {
        AppError::bad_request(&format!("Invalid pattern_type: {}", req.pattern_type))
    })?;

    let rule_id = state
        .db
        .create_tag_rule(req.tag_id, &req.pattern, pattern_type, req.priority)?;

    state.db.log_audit(
        &user_email,
        "create",
        Some("tag_rule"),
        Some(rule_id),
        Some(&format!("tag_id={}, pattern={}", req.tag_id, req.pattern)),
    )?;

    // Find the created rule
    let rules = state.db.list_tag_rules()?;
    let rule = rules
        .into_iter()
        .find(|r| r.rule.id == rule_id)
        .ok_or_else(|| AppError::internal("Rule not found after creation"))?;

    Ok(Json(rule))
}

/// DELETE /api/rules/:id - Delete a tag rule
pub async fn delete_tag_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    state.db.delete_tag_rule(id)?;

    state
        .db
        .log_audit(&user_email, "delete", Some("tag_rule"), Some(id), None)?;

    Ok(Json(SuccessResponse { success: true }))
}

/// Request body for testing rules
#[derive(Debug, Deserialize)]
pub struct TestRulesRequest {
    pub description: String,
}

/// Response for testing rules
#[derive(Debug, Serialize)]
pub struct TestRulesResponse {
    pub matches: Vec<RuleMatch>,
}

#[derive(Debug, Serialize)]
pub struct RuleMatch {
    pub rule_id: i64,
    pub tag_id: i64,
    pub tag_name: String,
    pub pattern: String,
    pub pattern_type: String,
    pub priority: i32,
}

/// POST /api/rules/test - Test which rules match a description
pub async fn test_rules(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<TestRulesResponse>, AppError> {
    let user_email = get_user_email(request.headers());

    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::bad_request("Invalid request body"))?;
    let req: TestRulesRequest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let matches = hone_core::tags::test_rules_against(&state.db, &req.description)?;

    state.db.log_audit(
        &user_email,
        "test",
        Some("tag_rule"),
        None,
        Some(&format!(
            "description='{}', matches={}",
            req.description,
            matches.len()
        )),
    )?;

    let response = TestRulesResponse {
        matches: matches
            .into_iter()
            .map(|(rule, tag)| RuleMatch {
                rule_id: rule.id,
                tag_id: tag.id,
                tag_name: tag.name,
                pattern: rule.pattern,
                pattern_type: rule.pattern_type.as_str().to_string(),
                priority: rule.priority,
            })
            .collect(),
    };

    Ok(Json(response))
}

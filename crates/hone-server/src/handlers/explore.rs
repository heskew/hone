//! Explore mode handler - conversational financial queries
//!
//! Uses the AI orchestrator to execute agentic queries with tool calling.
//! Supports multi-turn conversations via session management.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::{get_user_email, AppError, AppState};
use hone_core::ai::{Message, ToolCallRecord};
use hone_core::models::{NewOllamaMetric, OllamaOperation};
use hone_core::prompts::{PromptId, PromptLibrary};
use hone_core::tools::hone_tools;

/// Session timeout (30 minutes of inactivity)
const SESSION_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Maximum messages to keep in history (to limit context size)
const MAX_HISTORY_MESSAGES: usize = 20;

/// An explore session with conversation history
#[derive(Debug, Clone)]
pub struct ExploreSession {
    /// Session creation time
    pub created_at: Instant,
    /// Last activity time
    pub last_activity: Instant,
    /// Conversation history (user/assistant pairs)
    pub messages: Vec<Message>,
}

impl ExploreSession {
    fn new() -> Self {
        Self {
            created_at: Instant::now(),
            last_activity: Instant::now(),
            messages: Vec::new(),
        }
    }

    fn is_expired(&self) -> bool {
        self.last_activity.elapsed() > SESSION_TIMEOUT
    }

    fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    fn add_messages(&mut self, new_messages: Vec<Message>) {
        self.messages = new_messages;
        // Trim if too long (keep most recent)
        if self.messages.len() > MAX_HISTORY_MESSAGES {
            let start = self.messages.len() - MAX_HISTORY_MESSAGES;
            self.messages = self.messages[start..].to_vec();
        }
        self.touch();
    }
}

/// In-memory session manager
#[derive(Debug, Default)]
pub struct ExploreSessionManager {
    sessions: RwLock<HashMap<String, ExploreSession>>,
}

impl ExploreSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new session and return its ID
    pub async fn create_session(&self) -> String {
        // Generate a unique session ID from timestamp + counter
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_le_bytes());
        let hash = hasher.finalize();
        let session_id = format!("exp_{:x}", hash)[..20].to_string();

        let mut sessions = self.sessions.write().await;

        // Clean up expired sessions while we're here
        sessions.retain(|_, s| !s.is_expired());

        sessions.insert(session_id.clone(), ExploreSession::new());
        session_id
    }

    /// Get a session's messages (returns empty if not found or expired)
    pub async fn get_messages(&self, session_id: &str) -> Vec<Message> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .filter(|s| !s.is_expired())
            .map(|s| s.messages.clone())
            .unwrap_or_default()
    }

    /// Update a session's messages
    pub async fn update_session(&self, session_id: &str, messages: Vec<Message>) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.add_messages(messages);
        } else {
            // Create session if it doesn't exist
            let mut session = ExploreSession::new();
            session.add_messages(messages);
            sessions.insert(session_id.to_string(), session);
        }
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id).is_some()
    }

    /// Get session info
    pub async fn get_session_info(&self, session_id: &str) -> Option<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .filter(|s| !s.is_expired())
            .map(|s| SessionInfo {
                session_id: session_id.to_string(),
                message_count: s.messages.len(),
                created_at_secs_ago: s.created_at.elapsed().as_secs(),
                last_activity_secs_ago: s.last_activity.elapsed().as_secs(),
            })
    }
}

/// Request to query the explore assistant
#[derive(Debug, Deserialize)]
pub struct ExploreQuery {
    pub query: String,
    /// Optional session ID for conversation continuity
    #[serde(default)]
    pub session_id: Option<String>,
    /// Optional model override (uses default if not specified)
    #[serde(default)]
    pub model: Option<String>,
}

/// Response from the explore assistant
#[derive(Debug, Serialize)]
pub struct ExploreResponse {
    pub response: String,
    pub processing_time_ms: u64,
    /// Session ID for follow-up queries
    pub session_id: String,
    /// Model used for this query
    pub model: String,
    /// Tool calls made during this query
    pub tool_calls: Vec<ToolCallRecord>,
    /// Number of orchestrator iterations
    pub iterations: usize,
}

/// Session info response
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub message_count: usize,
    pub created_at_secs_ago: u64,
    pub last_activity_secs_ago: u64,
}

/// POST /api/explore/session - Create a new explore session
pub async fn create_explore_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<SessionInfo>, AppError> {
    let user_email = get_user_email(&headers);

    let session_id = state.explore_sessions.create_session().await;

    debug!(session_id = %session_id, user = %user_email, "Created explore session");

    // Audit log
    state.db.log_audit(
        &user_email,
        "explore_session_create",
        Some("explore"),
        None,
        Some(&session_id),
    )?;

    Ok(Json(SessionInfo {
        session_id,
        message_count: 0,
        created_at_secs_ago: 0,
        last_activity_secs_ago: 0,
    }))
}

/// DELETE /api/explore/session/:id - Delete an explore session
pub async fn delete_explore_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_email = get_user_email(&headers);

    let deleted = state.explore_sessions.delete_session(&session_id).await;

    debug!(session_id = %session_id, deleted = deleted, "Deleted explore session");

    // Audit log
    state.db.log_audit(
        &user_email,
        "explore_session_delete",
        Some("explore"),
        None,
        Some(&session_id),
    )?;

    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

/// GET /api/explore/session/:id - Get session info
pub async fn get_explore_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionInfo>, AppError> {
    let info = state
        .explore_sessions
        .get_session_info(&session_id)
        .await
        .ok_or_else(|| AppError::not_found("Session not found or expired"))?;

    Ok(Json(info))
}

/// POST /api/explore/query - Query the explore assistant
pub async fn query_explore(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ExploreQuery>,
) -> Result<Json<ExploreResponse>, AppError> {
    let start = Instant::now();
    let user_email = get_user_email(&headers);

    // Check if orchestrator is configured
    let orchestrator = state.orchestrator.as_ref().ok_or_else(|| {
        AppError::bad_request(
            "Explore mode requires AI backend. Set ANTHROPIC_COMPATIBLE_HOST and ANTHROPIC_COMPATIBLE_MODEL.",
        )
    })?;

    // Get or create session
    let session_id = match &payload.session_id {
        Some(id) => id.clone(),
        None => state.explore_sessions.create_session().await,
    };

    // Get existing conversation history
    let prior_messages = state.explore_sessions.get_messages(&session_id).await;

    debug!(
        session_id = %session_id,
        history_len = prior_messages.len(),
        "Processing explore query"
    );

    // Load the explore prompt from the library
    let mut prompt_lib = PromptLibrary::new();
    let prompt = prompt_lib.get(PromptId::ExploreAgent).map_err(|e| {
        error!("Failed to load explore prompt: {}", e);
        AppError::internal("Failed to load explore prompt")
    })?;

    // Get the system prompt section
    let system_prompt = prompt
        .system_section()
        .ok_or_else(|| AppError::internal("Explore prompt missing system section"))?;

    // Get all available tools
    let tools = hone_tools();

    // Use model override if specified, otherwise use default
    let effective_orchestrator;
    let orchestrator_ref = if let Some(ref model) = payload.model {
        effective_orchestrator = orchestrator.with_model(model);
        &effective_orchestrator
    } else {
        orchestrator
    };

    // Execute the query through the orchestrator with full tracking
    let model_name = orchestrator_ref.model().to_string();
    let result = orchestrator_ref
        .execute_with_tracking(system_prompt, &payload.query, &tools, prior_messages)
        .await;

    // Record metrics regardless of success/failure
    let latency_ms = start.elapsed().as_millis() as i64;
    let (success, error_message, response_text, metadata_json) = match &result {
        Ok(r) => {
            // Serialize tool calls and iterations for metrics storage
            let metadata = serde_json::json!({
                "tool_calls": r.tool_calls,
                "iterations": r.iterations,
            });
            let metadata_str = serde_json::to_string(&metadata).ok();
            (true, None, Some(r.response.clone()), metadata_str)
        }
        Err(e) => (false, Some(e.to_string()), None, None),
    };

    // Record the metric (with tool calls in metadata)
    let metric = NewOllamaMetric {
        operation: OllamaOperation::ExploreQuery,
        model: model_name.clone(),
        latency_ms,
        success,
        error_message: error_message.clone(),
        confidence: None,
        transaction_id: None,
        input_text: Some(payload.query.clone()),
        result_text: response_text.clone(),
        metadata: metadata_json,
    };

    if let Err(e) = state.db.record_ollama_metric(&metric) {
        error!("Failed to record explore query metric: {}", e);
    }

    // Now handle the result
    let orchestrator_result = result.map_err(|e| {
        let err_str = e.to_string();
        error!("AI query failed: {}", err_str);

        // Provide a more helpful error message based on the error type
        let user_message = if err_str.contains("does not support tools") {
            format!(
                "The model '{}' doesn't support tool calling. Please select a different model like llama3.1 or qwen3-coder.",
                model_name
            )
        } else if err_str.contains("not found") {
            format!(
                "AI model '{}' not found. It may not be pulled on the Ollama server. Try: ollama pull {}",
                model_name, model_name
            )
        } else if err_str.contains("connection refused")
            || err_str.contains("Connection refused")
        {
            format!(
                "Cannot connect to AI backend. Is Ollama running at {}?",
                orchestrator_ref.backend().host()
            )
        } else if err_str.contains("timeout") || err_str.contains("timed out") {
            "AI query timed out. The model may be overloaded or the query too complex."
                .to_string()
        } else {
            format!("AI query failed: {}", err_str)
        };

        AppError::internal(&user_message)
    })?;

    // Update session with new messages
    state
        .explore_sessions
        .update_session(&session_id, orchestrator_result.messages)
        .await;

    // Audit log
    state.db.log_audit(
        &user_email,
        "explore_query",
        Some("explore"),
        None,
        Some(&payload.query),
    )?;

    debug!(
        tool_calls = orchestrator_result.tool_calls.len(),
        iterations = orchestrator_result.iterations,
        "Explore query completed"
    );

    Ok(Json(ExploreResponse {
        response: orchestrator_result.response,
        processing_time_ms: start.elapsed().as_millis() as u64,
        session_id,
        model: model_name,
        tool_calls: orchestrator_result.tool_calls,
        iterations: orchestrator_result.iterations,
    }))
}

/// Response for listing available models
#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    /// List of available model names
    pub models: Vec<String>,
    /// Default model (from environment)
    pub default_model: String,
}

/// GET /api/explore/models - List available models for explore mode
pub async fn list_explore_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsResponse>, AppError> {
    // Check if orchestrator is configured
    let orchestrator = state.orchestrator.as_ref().ok_or_else(|| {
        AppError::bad_request(
            "Explore mode requires AI backend. Set ANTHROPIC_COMPATIBLE_HOST and ANTHROPIC_COMPATIBLE_MODEL.",
        )
    })?;

    let default_model = orchestrator.model().to_string();

    // Fetch available models from Ollama
    let models = orchestrator.backend().list_models().await.map_err(|e| {
        error!("Failed to list models: {}", e);
        AppError::internal(&format!("Failed to list models: {}", e))
    })?;

    Ok(Json(ModelsResponse {
        models,
        default_model,
    }))
}

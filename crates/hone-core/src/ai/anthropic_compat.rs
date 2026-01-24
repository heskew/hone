//! Anthropic-compatible backend for local Ollama
//!
//! Uses Ollama's Anthropic Messages API compatibility (introduced in Ollama 0.14+)
//! for tool-calling capabilities. This enables agentic workflows where the AI
//! can dynamically query financial data via MCP tools.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         Hone (Pi)                                │
//! │                                                                  │
//! │   Detection/Import  →  AI Orchestrator  →  Tool execution loop   │
//! │                              │                    │              │
//! │                              │      ┌─────────────┘              │
//! │                              │      ▼                            │
//! │                              │   MCP Tools (in-process)          │
//! └──────────────────────────────┼──────────────────────────────────┘
//!                                │ HTTP (local network only)
//!                                ▼
//!                       ┌────────────────┐
//!                       │  Ollama (Mac)  │
//!                       │ /v1/messages   │  ← Anthropic Messages API (local)
//!                       └────────────────┘
//!
//! ⚠️ ALL TRAFFIC STAYS ON LOCAL NETWORK - NO CLOUD SERVICES
//! ```
//!
//! # Configuration
//!
//! Environment variables:
//! - `ANTHROPIC_COMPATIBLE_HOST`: Ollama server URL (e.g., `http://mac:11434`)
//! - `ANTHROPIC_COMPATIBLE_MODEL`: Model to use (e.g., `qwen3-coder`, `gpt-oss:20b`)
//!
//! The `ANTHROPIC_COMPATIBLE_*` prefix makes it clear this is the local Ollama server
//! using an Anthropic-compatible protocol, NOT the actual Anthropic cloud service.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{Error, Result};

/// Anthropic Messages API request
#[derive(Debug, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

/// Message in conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user", "assistant"
    pub content: MessageContent,
}

impl Message {
    /// Create a user message with text content
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create an assistant message with text content
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create a user message containing tool results
    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Blocks(results),
        }
    }

    /// Create an assistant message with content blocks
    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Blocks(blocks),
        }
    }
}

/// Message content (text or blocks)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Content block types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl ContentBlock {
    /// Create a text block
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create a tool result block
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: None,
        }
    }

    /// Create an error tool result block
    pub fn tool_error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: error.into(),
            is_error: Some(true),
        }
    }
}

/// Tool definition (Anthropic format - simpler than OpenAI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value, // JSON Schema
}

impl Tool {
    /// Create a new tool definition
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Anthropic Messages API response
#[derive(Debug, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>, // "end_turn", "tool_use", "max_tokens"
    pub stop_sequence: Option<String>,
    pub usage: Option<Usage>,
}

/// Token usage information
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl MessagesResponse {
    /// Check if the response is complete (no more tool calls needed)
    pub fn is_complete(&self) -> bool {
        self.stop_reason.as_deref() == Some("end_turn")
    }

    /// Check if the response requests tool use
    pub fn has_tool_use(&self) -> bool {
        self.stop_reason.as_deref() == Some("tool_use")
            || self
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    }

    /// Extract all tool use blocks
    pub fn tool_uses(&self) -> Vec<(&str, &str, &serde_json::Value)> {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.as_str(), name.as_str(), input))
                }
                _ => None,
            })
            .collect()
    }

    /// Extract text content from the response
    pub fn text(&self) -> Option<String> {
        let texts: Vec<_> = self
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    }
}

/// Anthropic-compatible backend for Ollama (local only - no cloud!)
///
/// Uses Ollama's Anthropic Messages API (`/v1/messages`) for tool-calling.
/// Requires Ollama 0.14+ running on the local network.
#[derive(Clone)]
pub struct AnthropicCompatBackend {
    http_client: Client,
    base_url: String, // http://mac:11434 for local Ollama
    model: String,    // e.g., "qwen3-coder" or "gpt-oss:20b"
}

impl AnthropicCompatBackend {
    /// Create a new Anthropic-compatible backend
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            http_client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    /// Create from environment (ANTHROPIC_COMPATIBLE_* - local Ollama only!)
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("ANTHROPIC_COMPATIBLE_HOST").ok()?;
        let model = std::env::var("ANTHROPIC_COMPATIBLE_MODEL")
            .unwrap_or_else(|_| "qwen3-coder".to_string());
        Some(Self::new(&base_url, &model))
    }

    /// Get the model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the base URL
    pub fn host(&self) -> &str {
        &self.base_url
    }

    /// Create a new backend with a different model (same host)
    pub fn with_model(&self, model: &str) -> Self {
        Self {
            http_client: self.http_client.clone(),
            base_url: self.base_url.clone(),
            model: model.to_string(),
        }
    }

    /// Send messages request with optional tools
    ///
    /// This is the main method for tool-calling interactions.
    pub async fn messages(
        &self,
        system: Option<&str>,
        messages: Vec<Message>,
        tools: Option<&[Tool]>,
    ) -> Result<MessagesResponse> {
        self.messages_with_max_tokens(system, messages, tools, 4096)
            .await
    }

    /// Send messages request with custom max_tokens
    pub async fn messages_with_max_tokens(
        &self,
        system: Option<&str>,
        messages: Vec<Message>,
        tools: Option<&[Tool]>,
        max_tokens: u32,
    ) -> Result<MessagesResponse> {
        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens,
            messages,
            system: system.map(String::from),
            tools: tools.map(|t| t.to_vec()),
        };

        debug!(
            model = %self.model,
            tools_count = tools.map(|t| t.len()).unwrap_or(0),
            "Sending Anthropic-compat request to local Ollama"
        );

        let response = self
            .http_client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", "ollama") // Ollama ignores but requires
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::InvalidData(format!(
                "Anthropic-compat API error ({}): {}",
                status, body
            )));
        }

        let messages_response: MessagesResponse = response.json().await?;

        debug!(
            stop_reason = ?messages_response.stop_reason,
            tool_uses = messages_response.tool_uses().len(),
            "Received Anthropic-compat response from local Ollama"
        );

        Ok(messages_response)
    }

    /// Simple text completion without tools
    ///
    /// Convenience method for non-agentic use cases.
    pub async fn complete(&self, system: Option<&str>, prompt: &str) -> Result<String> {
        let messages = vec![Message::user(prompt)];
        let response = self.messages(system, messages, None).await?;

        response
            .text()
            .ok_or_else(|| Error::InvalidData("No text in response".into()))
    }

    /// Health check - verify Ollama is reachable
    pub async fn health_check(&self) -> bool {
        // Try the tags endpoint (simpler than messages)
        match self
            .http_client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// List available models from Ollama
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let response = self
            .http_client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::InvalidData(format!(
                "Failed to list models: HTTP {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct TagsResponse {
            models: Vec<ModelInfo>,
        }

        #[derive(Deserialize)]
        struct ModelInfo {
            name: String,
        }

        let tags: TagsResponse = response.json().await?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, "user");

        let assistant_msg = Message::assistant("Hi there");
        assert_eq!(assistant_msg.role, "assistant");
    }

    #[test]
    fn test_message_assistant_blocks() {
        let blocks = vec![
            ContentBlock::text("Let me search for that"),
            ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "search".to_string(),
                input: serde_json::json!({}),
            },
        ];
        let msg = Message::assistant_blocks(blocks);
        assert_eq!(msg.role, "assistant");
        match msg.content {
            MessageContent::Blocks(blocks) => assert_eq!(blocks.len(), 2),
            _ => panic!("Expected blocks"),
        }
    }

    #[test]
    fn test_message_tool_results() {
        let results = vec![
            ContentBlock::tool_result("tool-1", "result data"),
            ContentBlock::tool_result("tool-2", "more data"),
        ];
        let msg = Message::tool_results(results);
        assert_eq!(msg.role, "user");
        match msg.content {
            MessageContent::Blocks(blocks) => assert_eq!(blocks.len(), 2),
            _ => panic!("Expected blocks"),
        }
    }

    #[test]
    fn test_tool_result_creation() {
        let result = ContentBlock::tool_result("tool-123", r#"{"data": "value"}"#);
        match result {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool-123");
                assert!(content.contains("data"));
                assert!(is_error.is_none());
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_tool_error_creation() {
        let error = ContentBlock::tool_error("tool-456", "Something went wrong");
        match error {
            ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "tool-456");
                assert_eq!(is_error, Some(true));
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_content_block_text() {
        let block = ContentBlock::text("Hello world");
        match block {
            ContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_tool_definition() {
        let tool = Tool::new(
            "search_transactions",
            "Search transactions by query",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                }
            }),
        );

        assert_eq!(tool.name, "search_transactions");
        assert!(tool.description.contains("Search"));
        assert!(tool.input_schema.is_object());
    }

    #[test]
    fn test_message_content_serialization() {
        let text_content = MessageContent::Text("Hello".into());
        let json = serde_json::to_string(&text_content).unwrap();
        assert_eq!(json, r#""Hello""#);

        let blocks_content = MessageContent::Blocks(vec![ContentBlock::text("Hello")]);
        let json = serde_json::to_string(&blocks_content).unwrap();
        assert!(json.contains("text"));
    }

    #[test]
    fn test_messages_response_is_complete() {
        let response = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::text("Done!")],
            model: "test-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: None,
        };
        assert!(response.is_complete());

        let incomplete = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![],
            model: "test-model".to_string(),
            stop_reason: Some("tool_use".to_string()),
            stop_sequence: None,
            usage: None,
        };
        assert!(!incomplete.is_complete());
    }

    #[test]
    fn test_messages_response_has_tool_use() {
        let response_with_tool = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "search".to_string(),
                input: serde_json::json!({}),
            }],
            model: "test-model".to_string(),
            stop_reason: Some("tool_use".to_string()),
            stop_sequence: None,
            usage: None,
        };
        assert!(response_with_tool.has_tool_use());

        let response_no_tool = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::text("No tools needed")],
            model: "test-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: None,
        };
        assert!(!response_no_tool.has_tool_use());
    }

    #[test]
    fn test_messages_response_tool_uses() {
        let response = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![
                ContentBlock::text("Let me search"),
                ContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "search_transactions".to_string(),
                    input: serde_json::json!({"query": "test"}),
                },
                ContentBlock::ToolUse {
                    id: "tool-2".to_string(),
                    name: "get_merchants".to_string(),
                    input: serde_json::json!({}),
                },
            ],
            model: "test-model".to_string(),
            stop_reason: Some("tool_use".to_string()),
            stop_sequence: None,
            usage: None,
        };

        let tool_uses = response.tool_uses();
        assert_eq!(tool_uses.len(), 2);
        assert_eq!(tool_uses[0].0, "tool-1");
        assert_eq!(tool_uses[0].1, "search_transactions");
        assert_eq!(tool_uses[1].0, "tool-2");
        assert_eq!(tool_uses[1].1, "get_merchants");
    }

    #[test]
    fn test_messages_response_text() {
        let response = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::text("Hello"), ContentBlock::text("World")],
            model: "test-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: None,
        };

        let text = response.text();
        assert!(text.is_some());
        assert_eq!(text.unwrap(), "Hello\nWorld");

        let empty_response = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![],
            model: "test-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: None,
        };
        assert!(empty_response.text().is_none());
    }

    #[test]
    fn test_backend_new() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "qwen3-coder");
        assert_eq!(backend.model(), "qwen3-coder");
        assert_eq!(backend.host(), "http://localhost:11434");
    }

    #[test]
    fn test_backend_new_trims_trailing_slash() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434/", "test-model");
        assert_eq!(backend.host(), "http://localhost:11434");
    }

    #[test]
    fn test_from_env_missing() {
        // Clear any existing env vars
        std::env::remove_var("ANTHROPIC_COMPATIBLE_HOST");
        std::env::remove_var("ANTHROPIC_COMPATIBLE_MODEL");

        let backend = AnthropicCompatBackend::from_env();
        assert!(backend.is_none());
    }

    #[test]
    fn test_messages_request_serialization() {
        let request = MessagesRequest {
            model: "test-model".to_string(),
            max_tokens: 4096,
            messages: vec![Message::user("Hello")],
            system: Some("You are a helpful assistant.".to_string()),
            tools: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test-model"));
        assert!(json.contains("4096"));
        assert!(json.contains("Hello"));
        assert!(json.contains("You are a helpful assistant."));
    }

    #[test]
    fn test_messages_request_with_tools() {
        let tools = vec![Tool::new(
            "search",
            "Search for items",
            serde_json::json!({"type": "object"}),
        )];

        let request = MessagesRequest {
            model: "test-model".to_string(),
            max_tokens: 1024,
            messages: vec![Message::user("Search for stuff")],
            system: None,
            tools: Some(tools),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("search"));
        assert!(json.contains("Search for items"));
    }

    #[tokio::test]
    async fn test_health_check_unreachable() {
        // Create backend pointing to non-existent server
        let backend = AnthropicCompatBackend::new("http://localhost:99999", "test-model");
        let healthy = backend.health_check().await;
        assert!(!healthy);
    }
}

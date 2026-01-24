//! AI Orchestrator for agentic workflows
//!
//! Executes an agentic loop using Ollama's Anthropic-compatible API for tool calling.
//! The orchestrator can dynamically query financial data via MCP tools, enabling
//! richer context and more informed decisions.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    AI Orchestrator Loop                          │
//! │                                                                  │
//! │   1. Send prompt + tools to Ollama                              │
//! │   2. If response has tool_use blocks:                           │
//! │      a. Execute each tool against the database                  │
//! │      b. Send tool results back to Ollama                        │
//! │      c. Repeat until end_turn or max_iterations                 │
//! │   3. Return final text response                                 │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use hone_core::ai::orchestrator::AIOrchestrator;
//! use hone_core::tools::spending_analysis_tools;
//!
//! let orchestrator = AIOrchestrator::from_env(db)?;
//!
//! let explanation = orchestrator.execute(
//!     "You are a financial analyst. Explain spending changes.",
//!     "Why did my Dining spending increase 50%?",
//!     &spending_analysis_tools(),
//! ).await?;
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::db::Database;
use crate::error::{Error, Result};
use crate::tools;

use super::anthropic_compat::{
    AnthropicCompatBackend, ContentBlock, Message, MessagesResponse, Tool,
};

/// Parsed tool call from XML-style output
#[derive(Debug, Clone)]
struct ParsedToolCall {
    name: String,
    params: serde_json::Value,
}

/// Record of a tool call made during orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool name (e.g., "search_transactions")
    pub name: String,
    /// Input parameters as JSON
    pub input: serde_json::Value,
    /// Whether the call succeeded
    pub success: bool,
    /// Output or error message
    pub output: Option<String>,
}

/// Result of an orchestrator execution including tool call history
#[derive(Debug, Clone)]
pub struct OrchestratorResult {
    /// The final text response
    pub response: String,
    /// Updated conversation history
    pub messages: Vec<Message>,
    /// All tool calls made during this execution
    pub tool_calls: Vec<ToolCallRecord>,
    /// Number of orchestrator iterations
    pub iterations: usize,
}

/// AI Orchestrator for agentic workflows with tool calling
pub struct AIOrchestrator {
    backend: AnthropicCompatBackend,
    db: Database,
    max_iterations: usize,
}

impl AIOrchestrator {
    /// Create a new orchestrator
    pub fn new(backend: AnthropicCompatBackend, db: Database) -> Self {
        Self {
            backend,
            db,
            max_iterations: 5,
        }
    }

    /// Create with custom max iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Create from environment (uses ANTHROPIC_COMPAT_* - local Ollama only!)
    pub fn from_env(db: Database) -> Option<Self> {
        AnthropicCompatBackend::from_env().map(|backend| Self::new(backend, db))
    }

    /// Get the underlying backend
    pub fn backend(&self) -> &AnthropicCompatBackend {
        &self.backend
    }

    /// Get the model name
    pub fn model(&self) -> &str {
        self.backend.model()
    }

    /// Create a new orchestrator using a different model (same host and database)
    pub fn with_model(&self, model: &str) -> Self {
        Self {
            backend: self.backend.with_model(model),
            db: self.db.clone(),
            max_iterations: self.max_iterations,
        }
    }

    /// Execute an agentic task with tool calling
    ///
    /// Returns the final text response after all tool calls are resolved.
    pub async fn execute(
        &self,
        system_prompt: &str,
        user_message: &str,
        available_tools: &[Tool],
    ) -> Result<String> {
        self.execute_with_tracking(system_prompt, user_message, available_tools, Vec::new())
            .await
            .map(|result| result.response)
    }

    /// Execute with conversation history support
    ///
    /// Takes prior messages and returns both the response and the updated message history.
    /// This enables multi-turn conversations where context is preserved.
    ///
    /// The returned messages include only the user/assistant text exchanges (no tool calls),
    /// which is suitable for passing to subsequent calls.
    pub async fn execute_with_history(
        &self,
        system_prompt: &str,
        user_message: &str,
        available_tools: &[Tool],
        prior_messages: Vec<Message>,
    ) -> Result<(String, Vec<Message>)> {
        self.execute_with_tracking(system_prompt, user_message, available_tools, prior_messages)
            .await
            .map(|result| (result.response, result.messages))
    }

    /// Execute with full tracking including tool call history
    ///
    /// Returns the complete result including response, conversation history,
    /// and all tool calls made during execution.
    pub async fn execute_with_tracking(
        &self,
        system_prompt: &str,
        user_message: &str,
        available_tools: &[Tool],
        prior_messages: Vec<Message>,
    ) -> Result<OrchestratorResult> {
        let history_len = prior_messages.len();
        let mut messages = prior_messages.clone();
        messages.push(Message::user(user_message));

        // Track the conversation history (user/assistant pairs only, no tool internals)
        let mut conversation_history = prior_messages;
        conversation_history.push(Message::user(user_message));

        // Track all tool calls made during this execution
        let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

        info!(
            model = %self.backend.model(),
            tools = available_tools.len(),
            history_len,
            "Starting orchestrator execution"
        );

        for iteration in 0..self.max_iterations {
            debug!(iteration, "Orchestrator iteration");

            let response = self
                .backend
                .messages(Some(system_prompt), messages.clone(), Some(available_tools))
                .await?;

            // Check for tool use (proper format)
            let tool_uses = response.tool_uses();

            // If no proper tool_use blocks, check for XML-style tool calls in text
            // We do this BEFORE checking is_complete() because some models output
            // XML tool calls with stop_reason="end_turn"
            let xml_tool_calls = if tool_uses.is_empty() {
                if let Some(text) = response.text() {
                    let calls = Self::parse_xml_tool_calls(&text);
                    if !calls.is_empty() {
                        info!(
                            iteration,
                            count = calls.len(),
                            "Found XML-style tool calls in text output"
                        );
                    }
                    calls
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            // Check if we're done (no tool calls of either type)
            if tool_uses.is_empty() && xml_tool_calls.is_empty() {
                info!(iteration, "Orchestrator complete (no tool calls)");
                let text = self.extract_text(&response)?;
                conversation_history.push(Message::assistant(text.clone()));
                return Ok(OrchestratorResult {
                    response: text,
                    messages: conversation_history,
                    tool_calls,
                    iterations: iteration + 1,
                });
            }

            // Handle proper tool_use blocks
            if !tool_uses.is_empty() {
                debug!(
                    iteration,
                    tool_count = tool_uses.len(),
                    "Executing tool calls (proper format)"
                );

                // Add assistant response to messages
                messages.push(Message::assistant_blocks(response.content.clone()));

                // Execute each tool and collect results
                let mut tool_results = Vec::new();
                for (id, name, input) in tool_uses {
                    debug!(tool = name, "Executing tool");

                    let result = self.execute_tool(name, input).await;
                    match &result {
                        Ok(output) => {
                            debug!(tool = name, output_len = output.len(), "Tool succeeded");
                            tool_results.push(ContentBlock::tool_result(id, output.clone()));
                            tool_calls.push(ToolCallRecord {
                                name: name.to_string(),
                                input: input.clone(),
                                success: true,
                                output: Some(output.clone()),
                            });
                        }
                        Err(e) => {
                            warn!(tool = name, error = %e, "Tool failed");
                            tool_results.push(ContentBlock::tool_error(id, e.to_string()));
                            tool_calls.push(ToolCallRecord {
                                name: name.to_string(),
                                input: input.clone(),
                                success: false,
                                output: Some(e.to_string()),
                            });
                        }
                    }
                }

                // Add tool results as user message
                messages.push(Message::tool_results(tool_results));
            } else {
                // Handle XML-style tool calls
                debug!(
                    iteration,
                    tool_count = xml_tool_calls.len(),
                    "Executing XML-style tool calls"
                );

                // Get the preamble text (before tool calls) to preserve context
                let preamble = if let Some(text) = response.text() {
                    Self::strip_xml_tool_calls(&text)
                } else {
                    String::new()
                };

                // Add assistant's partial response
                if !preamble.is_empty() {
                    messages.push(Message::assistant(&preamble));
                }

                // Execute XML tool calls and format results
                let mut tool_outputs = Vec::new();
                for (i, call) in xml_tool_calls.iter().enumerate() {
                    debug!(tool = %call.name, "Executing XML tool");

                    let result = self.execute_tool(&call.name, &call.params).await;
                    match &result {
                        Ok(output) => {
                            debug!(tool = %call.name, output_len = output.len(), "Tool succeeded");
                            tool_outputs.push(format!(
                                "Tool {} ({}) result:\n{}",
                                i + 1,
                                call.name,
                                output
                            ));
                            tool_calls.push(ToolCallRecord {
                                name: call.name.clone(),
                                input: call.params.clone(),
                                success: true,
                                output: Some(output.clone()),
                            });
                        }
                        Err(e) => {
                            warn!(tool = %call.name, error = %e, "Tool failed");
                            tool_outputs.push(format!(
                                "Tool {} ({}) error: {}",
                                i + 1,
                                call.name,
                                e
                            ));
                            tool_calls.push(ToolCallRecord {
                                name: call.name.clone(),
                                input: call.params.clone(),
                                success: false,
                                output: Some(e.to_string()),
                            });
                        }
                    }
                }

                // Add tool results as user message (since model expects response)
                let results_msg = format!(
                    "Here are the results from the tools you requested:\n\n{}",
                    tool_outputs.join("\n\n")
                );
                messages.push(Message::user(&results_msg));
            }
        }

        warn!(
            max_iterations = self.max_iterations,
            "Orchestrator hit max iterations"
        );
        Err(Error::InvalidData(format!(
            "Max iterations ({}) reached without completion",
            self.max_iterations
        )))
    }

    /// Execute a single tool call
    async fn execute_tool(&self, name: &str, input: &serde_json::Value) -> Result<String> {
        match name {
            "search_transactions" => {
                let params: tools::SearchTransactionsParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::search_transactions(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            "get_spending_summary" => {
                let params: tools::SpendingSummaryParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::get_spending_summary(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            "get_subscriptions" => {
                let params: tools::SubscriptionsParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::get_subscriptions(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            "get_alerts" => {
                let params: tools::AlertsParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::get_alerts(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            "compare_spending" => {
                let params: tools::CompareSpendingParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::compare_spending(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            "get_merchants" => {
                let params: tools::MerchantsParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::get_merchants(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            "get_account_summary" => {
                let params: tools::AccountSummaryParams = serde_json::from_value(input.clone())
                    .map_err(|e| Error::InvalidData(format!("Invalid params: {}", e)))?;
                let result = tools::get_account_summary(&self.db, params)?;
                serde_json::to_string(&result)
                    .map_err(|e| Error::InvalidData(format!("Failed to serialize: {}", e)))
            }
            _ => Err(Error::InvalidData(format!("Unknown tool: {}", name))),
        }
    }

    /// Extract text from a response
    fn extract_text(&self, response: &MessagesResponse) -> Result<String> {
        response
            .text()
            .ok_or_else(|| Error::InvalidData("No text in response".into()))
    }

    /// Parse XML-style tool calls from text output
    ///
    /// Some models output tool calls as XML text instead of proper tool_use blocks:
    /// ```text
    /// <function=search_transactions>
    /// <parameter=query>starbucks</parameter>
    /// <parameter=limit>100</parameter>
    /// </function>
    /// ```
    ///
    /// This function extracts those calls so we can execute them.
    fn parse_xml_tool_calls(text: &str) -> Vec<ParsedToolCall> {
        let mut calls = Vec::new();

        // Match <function=name>...</function> blocks
        // Using (?s) for DOTALL mode so . matches newlines
        let function_re =
            Regex::new(r"(?s)<function=([^>]+)>(.*?)</function>").expect("valid regex");
        let param_re = Regex::new(r"<parameter=([^>]+)>([^<]*)</parameter>").expect("valid regex");

        for func_cap in function_re.captures_iter(text) {
            let name = func_cap[1].trim().to_string();
            let body = &func_cap[2];

            // Extract parameters
            let mut params = serde_json::Map::new();
            for param_cap in param_re.captures_iter(body) {
                let key = param_cap[1].trim().to_string();
                let value = param_cap[2].trim();

                // Try to parse as number or keep as string
                let json_value = if let Ok(n) = value.parse::<i64>() {
                    serde_json::Value::Number(n.into())
                } else if let Ok(f) = value.parse::<f64>() {
                    serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or_else(|| serde_json::Value::String(value.to_string()))
                } else if value == "true" {
                    serde_json::Value::Bool(true)
                } else if value == "false" {
                    serde_json::Value::Bool(false)
                } else {
                    serde_json::Value::String(value.to_string())
                };

                params.insert(key, json_value);
            }

            calls.push(ParsedToolCall {
                name,
                params: serde_json::Value::Object(params),
            });
        }

        calls
    }

    /// Extract text content without XML tool calls
    fn strip_xml_tool_calls(text: &str) -> String {
        let function_re =
            Regex::new(r"(?s)<function=([^>]+)>(.*?)</function>").expect("valid regex");
        let result = function_re.replace_all(text, "");

        // Also remove </Tool_call> or similar artifacts
        let cleanup_re = Regex::new(r"(?i)</?\s*tool_?call\s*>").expect("valid regex");
        cleanup_re.replace_all(&result, "").trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Bank, Frequency, NewTransaction};

    fn create_test_db() -> Database {
        let db = Database::in_memory().unwrap();

        // Seed some test data
        db.upsert_account("Test Account", Bank::Chase, None)
            .unwrap();

        let today = chrono::Utc::now().date_naive();
        db.insert_transaction(
            1,
            &NewTransaction {
                date: today,
                description: "NETFLIX.COM".to_string(),
                amount: -15.99,
                category: None,
                import_hash: "hash1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.upsert_subscription(
            "Netflix",
            Some(1),
            Some(15.99),
            Some(Frequency::Monthly),
            Some(today),
            Some(today),
        )
        .unwrap();

        db
    }

    #[test]
    fn test_orchestrator_creation() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        assert_eq!(orchestrator.model(), "test-model");
        assert_eq!(orchestrator.max_iterations, 5);
    }

    #[test]
    fn test_orchestrator_with_max_iterations() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db).with_max_iterations(10);

        assert_eq!(orchestrator.max_iterations, 10);
    }

    #[test]
    fn test_from_env_without_vars() {
        // Clear env vars
        std::env::remove_var("ANTHROPIC_COMPATIBLE_HOST");

        let result = AnthropicCompatBackend::from_env();
        assert!(result.is_none());
    }

    #[test]
    fn test_backend_accessor() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        assert_eq!(orchestrator.backend().model(), "test-model");
        assert_eq!(orchestrator.backend().host(), "http://localhost:11434");
    }

    #[tokio::test]
    async fn test_execute_tool_search_transactions() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({
            "period": "all"
        });

        let result = orchestrator
            .execute_tool("search_transactions", &input)
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should be valid JSON containing transactions
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("transactions").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_get_spending_summary() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({
            "period": "this-month"
        });

        let result = orchestrator
            .execute_tool("get_spending_summary", &input)
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("categories").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_get_subscriptions() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({});

        let result = orchestrator.execute_tool("get_subscriptions", &input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("subscriptions").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_get_alerts() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({});

        let result = orchestrator.execute_tool("get_alerts", &input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("alerts").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_compare_spending() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({
            "current_period": "this-month",
            "baseline_period": "last-month"
        });

        let result = orchestrator.execute_tool("compare_spending", &input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("current_total").is_some());
        assert!(parsed.get("baseline_total").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_get_merchants() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({
            "period": "all"
        });

        let result = orchestrator.execute_tool("get_merchants", &input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("merchants").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_get_account_summary() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({});

        let result = orchestrator
            .execute_tool("get_account_summary", &input)
            .await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("accounts").is_some());
        assert!(parsed.get("total_accounts").is_some());
    }

    #[tokio::test]
    async fn test_execute_tool_unknown() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        let input = serde_json::json!({});

        let result = orchestrator.execute_tool("unknown_tool", &input).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_execute_tool_invalid_params() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        // Invalid period value
        let input = serde_json::json!({
            "period": 12345  // Should be string
        });

        let result = orchestrator
            .execute_tool("search_transactions", &input)
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_text_from_response() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        // Create a response with text content
        let response = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::text("Hello, world!")],
            model: "test-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: None,
        };

        let result = orchestrator.extract_text(&response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world!");
    }

    #[test]
    fn test_extract_text_from_empty_response() {
        let backend = AnthropicCompatBackend::new("http://localhost:11434", "test-model");
        let db = create_test_db();
        let orchestrator = AIOrchestrator::new(backend, db);

        // Create a response with no text content
        let response = MessagesResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![],
            model: "test-model".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: None,
        };

        let result = orchestrator.extract_text(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_call_record() {
        let record = ToolCallRecord {
            name: "search_transactions".to_string(),
            input: serde_json::json!({"period": "all"}),
            success: true,
            output: Some("[]".to_string()),
        };

        assert_eq!(record.name, "search_transactions");
        assert!(record.success);
    }

    #[test]
    fn test_orchestrator_result() {
        let result = OrchestratorResult {
            response: "Analysis complete".to_string(),
            messages: vec![],
            iterations: 2,
            tool_calls: vec![ToolCallRecord {
                name: "get_merchants".to_string(),
                input: serde_json::json!({}),
                success: true,
                output: Some("{}".to_string()),
            }],
        };

        assert_eq!(result.response, "Analysis complete");
        assert_eq!(result.iterations, 2);
        assert_eq!(result.tool_calls.len(), 1);
    }

    #[test]
    fn test_parse_xml_tool_calls_basic() {
        let text = r#"I'll search for those transactions.

<function=search_transactions>
<parameter=query>starbucks</parameter>
<parameter=limit>100</parameter>
</function>
</Tool_call>"#;

        let calls = AIOrchestrator::parse_xml_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search_transactions");
        assert_eq!(calls[0].params["query"], "starbucks");
        assert_eq!(calls[0].params["limit"], 100);
    }

    #[test]
    fn test_parse_xml_tool_calls_multiple() {
        let text = r#"Let me get that information.

<function=search_transactions>
<parameter=query>coffee</parameter>
</function>

<function=get_spending_summary>
<parameter=period>last-month</parameter>
</function>"#;

        let calls = AIOrchestrator::parse_xml_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "search_transactions");
        assert_eq!(calls[1].name, "get_spending_summary");
    }

    #[test]
    fn test_parse_xml_tool_calls_with_dates() {
        let text = r#"<function=search_transactions>
<parameter=from_date>2023-01-01</parameter>
<parameter=to_date>2023-12-31</parameter>
<parameter=query>netflix</parameter>
</function>"#;

        let calls = AIOrchestrator::parse_xml_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].params["from_date"], "2023-01-01");
        assert_eq!(calls[0].params["to_date"], "2023-12-31");
        assert_eq!(calls[0].params["query"], "netflix");
    }

    #[test]
    fn test_parse_xml_tool_calls_empty() {
        let text = "Here's your answer without any tools.";
        let calls = AIOrchestrator::parse_xml_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_strip_xml_tool_calls() {
        let text = r#"I'll help you check your Starbucks spending. Let me search for those transactions.

<function=search_transactions>
<parameter=query>starbucks</parameter>
</function>
</Tool_call>"#;

        let stripped = AIOrchestrator::strip_xml_tool_calls(text);
        assert!(!stripped.contains("<function"));
        assert!(!stripped.contains("Tool_call"));
        assert!(stripped.contains("I'll help you check your Starbucks spending"));
    }

    #[test]
    fn test_parse_xml_tool_calls_boolean_and_numbers() {
        let text = r#"<function=get_subscriptions>
<parameter=include_cancelled>true</parameter>
<parameter=limit>50</parameter>
</function>"#;

        let calls = AIOrchestrator::parse_xml_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].params["include_cancelled"], true);
        assert_eq!(calls[0].params["limit"], 50);
    }
}

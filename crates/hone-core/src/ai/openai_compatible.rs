//! OpenAI-compatible backend implementation
//!
//! Works with any server that implements the OpenAI chat completions API:
//! - Docker Model Runner (http://localhost:12434)
//! - vLLM (http://localhost:8000)
//! - LocalAI (http://localhost:8080)
//! - llama-server / llama.cpp (http://localhost:8080)
//! - text-generation-inference
//!
//! # Configuration
//!
//! Environment variables:
//! - `OPENAI_COMPATIBLE_HOST`: Server URL (required)
//! - `OPENAI_COMPATIBLE_MODEL`: Model name (default: gpt-3.5-turbo)
//! - `OPENAI_COMPATIBLE_API_KEY`: API key if required (optional)

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::{Error, Result};
use crate::model_router::ModelRouter;
use crate::models::SpendingChangeExplanation;
use crate::prompts::{PromptId, PromptLibrary};

use super::parsing::{
    parse_classification, parse_duplicate_analysis, parse_entity_suggestion, parse_normalization,
    parse_receipt_match_evaluation, parse_receipt_response, parse_spending_explanation,
    parse_split_recommendation, parse_subscription_classification,
};
use super::types::{
    DuplicateAnalysis, MerchantClassification, MerchantContext, ParsedReceipt,
    ReceiptMatchEvaluation, RouterInfo, SplitRecommendation, SubscriptionClassification,
};
use super::AIBackend;

/// OpenAI-compatible backend
///
/// Works with any server implementing the OpenAI `/v1/chat/completions` API.
/// This includes Docker Model Runner, vLLM, LocalAI, llama-server, and more.
///
/// # Example
///
/// ```rust,ignore
/// // Docker Model Runner
/// export OPENAI_COMPATIBLE_HOST="http://localhost:12434"
/// export OPENAI_COMPATIBLE_MODEL="llama3.2"
///
/// // vLLM
/// export OPENAI_COMPATIBLE_HOST="http://192.168.1.100:8000"
/// export OPENAI_COMPATIBLE_MODEL="meta-llama/Llama-3.2-3B-Instruct"
///
/// // LocalAI
/// export OPENAI_COMPATIBLE_HOST="http://192.168.1.100:8080"
/// export OPENAI_COMPATIBLE_MODEL="llama-3.2-3b"
/// ```
pub struct OpenAICompatibleBackend {
    http_client: Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
    router: Arc<RwLock<ModelRouter>>,
    prompts: Arc<RwLock<PromptLibrary>>,
}

impl Clone for OpenAICompatibleBackend {
    fn clone(&self) -> Self {
        Self {
            http_client: self.http_client.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            router: self.router.clone(),
            prompts: self.prompts.clone(),
        }
    }
}

impl OpenAICompatibleBackend {
    /// Create a new OpenAI-compatible backend
    pub fn new(base_url: &str, model: &str) -> Self {
        let router = ModelRouter::new().unwrap_or_default();
        Self {
            http_client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: None,
            router: Arc::new(RwLock::new(router)),
            prompts: Arc::new(RwLock::new(PromptLibrary::new())),
        }
    }

    /// Create with an API key
    pub fn with_api_key(base_url: &str, model: &str, api_key: &str) -> Self {
        let router = ModelRouter::new().unwrap_or_default();
        Self {
            http_client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            router: Arc::new(RwLock::new(router)),
            prompts: Arc::new(RwLock::new(PromptLibrary::new())),
        }
    }

    /// Create a new instance with a different model
    ///
    /// Used for runtime model override (e.g., user selects a different model for testing)
    pub fn with_model(&self, model: &str) -> Self {
        Self {
            http_client: self.http_client.clone(),
            base_url: self.base_url.clone(),
            model: model.to_string(),
            api_key: self.api_key.clone(),
            router: self.router.clone(),
            prompts: self.prompts.clone(),
        }
    }

    /// Create from environment variables
    ///
    /// Required: `OPENAI_COMPATIBLE_HOST`
    /// Optional: `OPENAI_COMPATIBLE_MODEL` (default: gpt-3.5-turbo)
    /// Optional: `OPENAI_COMPATIBLE_API_KEY`
    pub fn from_env() -> Option<Self> {
        let host = std::env::var("OPENAI_COMPATIBLE_HOST").ok()?;
        let model = std::env::var("OPENAI_COMPATIBLE_MODEL")
            .unwrap_or_else(|_| "gpt-3.5-turbo".to_string());
        let api_key = std::env::var("OPENAI_COMPATIBLE_API_KEY").ok();

        let mut backend = Self::new(&host, &model);
        backend.api_key = api_key;
        Some(backend)
    }

    /// Make a chat completion request
    async fn chat_completion(&self, prompt: &str) -> Result<String> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(prompt.to_string()),
            }],
            temperature: Some(0.1),
            max_tokens: None,
            stream: false,
        };

        let mut req_builder = self
            .http_client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&request);

        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req_builder.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::InvalidData(format!(
                "OpenAI API error {}: {}",
                status, body
            )));
        }

        let chat_response: ChatCompletionResponse = response.json().await?;

        chat_response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::InvalidData("No response from OpenAI API".into()))
    }

    /// Make a vision request (for receipt parsing)
    async fn vision_completion(&self, prompt: &str, image_data: &[u8]) -> Result<String> {
        let base64_image = base64::engine::general_purpose::STANDARD.encode(image_data);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::Text {
                        text: prompt.to_string(),
                    },
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: format!("data:image/jpeg;base64,{}", base64_image),
                        },
                    },
                ]),
            }],
            temperature: Some(0.1),
            max_tokens: Some(4096),
            stream: false,
        };

        let mut req_builder = self
            .http_client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&request);

        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req_builder.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::InvalidData(format!(
                "OpenAI API error {}: {}",
                status, body
            )));
        }

        let chat_response: ChatCompletionResponse = response.json().await?;

        chat_response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::InvalidData("No response from OpenAI API".into()))
    }
}

/// OpenAI chat completion request
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}

/// Chat message
#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: ChatContent,
}

/// Chat message content (text or multimodal)
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ChatContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

/// Content part for multimodal messages
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

/// Image URL for vision requests
#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

/// OpenAI chat completion response
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

/// Chat completion choice
#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

/// Chat response message
#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[async_trait]
impl AIBackend for OpenAICompatibleBackend {
    async fn classify_merchant(&self, merchant: &str) -> Result<MerchantClassification> {
        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::ClassifyMerchant)?;
            let mut vars = HashMap::new();
            vars.insert("merchant", merchant);
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!("OpenAI-compatible response: {}", response);

        parse_classification(&response)
    }

    async fn classify_merchants(
        &self,
        merchants: &[String],
    ) -> Vec<(String, Option<MerchantClassification>)> {
        let mut results = Vec::new();

        for merchant in merchants {
            let classification = match self.classify_merchant(merchant).await {
                Ok(c) => Some(c),
                Err(e) => {
                    warn!("Failed to classify {}: {}", merchant, e);
                    None
                }
            };
            results.push((merchant.clone(), classification));
        }

        results
    }

    async fn normalize_merchant(
        &self,
        description: &str,
        category_hint: Option<&str>,
    ) -> Result<String> {
        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::NormalizeMerchant)?;
            let mut vars = HashMap::new();
            vars.insert("description", description);
            if let Some(cat) = category_hint {
                vars.insert("category", cat);
            }
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!("OpenAI-compatible normalize response: {}", response);

        parse_normalization(&response)
    }

    async fn normalize_merchant_with_context(
        &self,
        description: &str,
        context: &MerchantContext,
    ) -> Result<String> {
        // Build context block from MerchantContext fields
        let mut context_lines = Vec::new();
        if let Some(ref extracted) = context.extracted_merchant {
            context_lines.push(format!("Our extraction: \"{}\"", extracted));
        }
        if let Some(ref statement) = context.statement_as {
            context_lines.push(format!("Statement field: \"{}\"", statement));
        }
        if let Some(ref extended) = context.extended_details {
            context_lines.push(format!("Extended details: \"{}\"", extended));
        }
        if let Some(ref category) = context.category {
            context_lines.push(format!("Category: {}", category));
        }
        let context_block = context_lines.join("\n");

        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::NormalizeMerchantWithContext)?;
            let mut vars = HashMap::new();
            vars.insert("description", description);
            if !context_block.is_empty() {
                vars.insert("context_block", context_block.as_str());
            }
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!(
            "OpenAI-compatible normalize (with context) response: {}",
            response
        );

        parse_normalization(&response)
    }

    async fn parse_receipt(
        &self,
        image_data: &[u8],
        _vision_model: Option<&str>,
    ) -> Result<ParsedReceipt> {
        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::ParseReceipt)?;
            let vars = HashMap::new();
            template.render_user(&vars)
        };

        let response = self.vision_completion(&prompt, image_data).await?;
        debug!("OpenAI-compatible receipt parsing response: {}", response);

        parse_receipt_response(&response)
    }

    async fn suggest_entity(
        &self,
        merchant: &str,
        category: &str,
        entities: &[String],
    ) -> Result<Option<String>> {
        if entities.is_empty() {
            return Ok(None);
        }

        let entities_list = entities.join(", ");
        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::SuggestEntity)?;
            let mut vars = HashMap::new();
            vars.insert("merchant", merchant);
            vars.insert("category", category);
            vars.insert("entities", &entities_list);
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!("OpenAI-compatible entity suggestion response: {}", response);

        parse_entity_suggestion(&response)
    }

    async fn is_subscription_service(&self, merchant: &str) -> Result<SubscriptionClassification> {
        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::ClassifySubscription)?;
            let mut vars = HashMap::new();
            vars.insert("merchant", merchant);
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!(
            "OpenAI-compatible subscription classification response: {}",
            response
        );

        parse_subscription_classification(&response)
    }

    async fn should_suggest_split(&self, merchant: &str) -> Result<SplitRecommendation> {
        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::SuggestSplit)?;
            let mut vars = HashMap::new();
            vars.insert("merchant", merchant);
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!(
            "OpenAI-compatible split recommendation response: {}",
            response
        );

        parse_split_recommendation(&response)
    }

    async fn evaluate_receipt_match(
        &self,
        receipt_merchant: Option<&str>,
        receipt_date: Option<&str>,
        receipt_total: Option<f64>,
        transaction_description: &str,
        transaction_date: &str,
        transaction_amount: f64,
        transaction_merchant_normalized: Option<&str>,
    ) -> Result<ReceiptMatchEvaluation> {
        let receipt_merchant_str = receipt_merchant.unwrap_or("unknown").to_string();
        let receipt_date_str = receipt_date.unwrap_or("unknown").to_string();
        let receipt_total_str = receipt_total
            .map(|t| format!("${:.2}", t))
            .unwrap_or_else(|| "unknown".to_string());
        let transaction_merchant_normalized_str = transaction_merchant_normalized
            .unwrap_or("unknown")
            .to_string();
        let transaction_amount_str = format!("${:.2}", transaction_amount.abs());

        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::EvaluateReceiptMatch)?;
            let mut vars = HashMap::new();
            vars.insert("receipt_merchant", receipt_merchant_str.as_str());
            vars.insert("receipt_date", receipt_date_str.as_str());
            vars.insert("receipt_total", receipt_total_str.as_str());
            vars.insert("transaction_description", transaction_description);
            vars.insert(
                "transaction_merchant_normalized",
                transaction_merchant_normalized_str.as_str(),
            );
            vars.insert("transaction_date", transaction_date);
            vars.insert("transaction_amount", transaction_amount_str.as_str());
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!(
            "OpenAI-compatible receipt match evaluation response: {}",
            response
        );

        parse_receipt_match_evaluation(&response)
    }

    async fn analyze_duplicate_services(
        &self,
        category: &str,
        services: &[&str],
        feedback: Option<&str>,
    ) -> Result<DuplicateAnalysis> {
        let services_list = services.join(", ");
        let feedback_str = feedback.unwrap_or("");

        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::AnalyzeDuplicates)?;
            let mut vars = HashMap::new();
            vars.insert("category", category);
            vars.insert("services", services_list.as_str());
            vars.insert("feedback", feedback_str);
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!(
            "OpenAI-compatible duplicate analysis response: {}",
            response
        );

        parse_duplicate_analysis(&response)
    }

    async fn explain_spending_change(
        &self,
        category: &str,
        baseline_amount: f64,
        current_amount: f64,
        baseline_tx_count: i32,
        current_tx_count: i32,
        top_merchants: &[(String, f64, i32)],
        new_merchants: &[String],
        feedback: Option<&str>,
    ) -> Result<SpendingChangeExplanation> {
        // Format merchants list for prompt
        let merchants_list = top_merchants
            .iter()
            .map(|(name, amount, count)| {
                format!("{}: ${:.2} ({} transactions)", name, amount, count)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let new_merchants_list = if new_merchants.is_empty() {
            "none".to_string()
        } else {
            new_merchants.join(", ")
        };

        let change_direction = if current_amount > baseline_amount {
            "increased"
        } else {
            "decreased"
        };
        let percent_change_val =
            ((current_amount - baseline_amount) / baseline_amount * 100.0).abs();

        let baseline_amount_str = format!("{:.2}", baseline_amount);
        let baseline_tx_count_str = format!("{}", baseline_tx_count / 3); // approximate monthly avg
        let current_amount_str = format!("{:.2}", current_amount);
        let current_tx_count_str = format!("{}", current_tx_count);
        let percent_change_str = format!("{:.0}", percent_change_val);
        let feedback_str = feedback.unwrap_or("");

        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::ExplainSpending)?;
            let mut vars = HashMap::new();
            vars.insert("category", category);
            vars.insert("baseline_amount", baseline_amount_str.as_str());
            vars.insert("baseline_tx_count", baseline_tx_count_str.as_str());
            vars.insert("current_amount", current_amount_str.as_str());
            vars.insert("current_tx_count", current_tx_count_str.as_str());
            vars.insert("change_direction", change_direction);
            vars.insert("percent_change", percent_change_str.as_str());
            vars.insert("merchants_list", merchants_list.as_str());
            vars.insert("new_merchants_list", new_merchants_list.as_str());
            vars.insert("feedback", feedback_str);
            template.render_user(&vars)
        };

        let response = self.chat_completion(&prompt).await?;
        debug!(
            "OpenAI-compatible spending change explanation response: {}",
            response
        );

        parse_spending_explanation(&response, &self.model)
    }

    async fn health_check(&self) -> bool {
        // Try /v1/models first (standard OpenAI endpoint)
        if let Ok(resp) = self
            .http_client
            .get(format!("{}/v1/models", self.base_url))
            .send()
            .await
        {
            if resp.status().is_success() {
                return true;
            }
        }

        // Try /health (common for Docker Model Runner, LocalAI)
        if let Ok(resp) = self
            .http_client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
        {
            if resp.status().is_success() {
                return true;
            }
        }

        // Try root endpoint (some servers return 200 on /)
        if let Ok(resp) = self.http_client.get(&self.base_url).send().await {
            if resp.status().is_success() {
                return true;
            }
        }

        false
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn host(&self) -> &str {
        &self.base_url
    }

    fn router_info(&self) -> RouterInfo {
        // OpenAI-compatible backend uses a single model (no task-based routing yet)
        RouterInfo {
            default_model: self.model.clone(),
            fallback_model: None,
            task_models: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_new() {
        let backend = OpenAICompatibleBackend::new("http://localhost:12434", "llama3.2");
        assert_eq!(backend.model(), "llama3.2");
        assert_eq!(backend.host(), "http://localhost:12434");
    }

    #[test]
    fn test_backend_new_trims_trailing_slash() {
        let backend = OpenAICompatibleBackend::new("http://localhost:12434/", "llama3.2");
        assert_eq!(backend.host(), "http://localhost:12434");
    }

    #[test]
    fn test_backend_with_api_key() {
        let backend =
            OpenAICompatibleBackend::with_api_key("http://localhost:12434", "gpt-4", "sk-test123");
        assert_eq!(backend.model(), "gpt-4");
        assert_eq!(backend.host(), "http://localhost:12434");
        assert_eq!(backend.api_key, Some("sk-test123".to_string()));
    }

    #[test]
    fn test_backend_from_env_missing() {
        std::env::remove_var("OPENAI_COMPATIBLE_HOST");
        std::env::remove_var("OPENAI_COMPATIBLE_MODEL");
        std::env::remove_var("OPENAI_COMPATIBLE_API_KEY");

        let result = OpenAICompatibleBackend::from_env();
        assert!(result.is_none());
    }

    #[test]
    fn test_backend_clone() {
        let backend = OpenAICompatibleBackend::new("http://localhost:12434", "llama3.2");
        let cloned = backend.clone();

        assert_eq!(cloned.model(), backend.model());
        assert_eq!(cloned.host(), backend.host());
    }

    #[test]
    fn test_router_info() {
        let backend = OpenAICompatibleBackend::new("http://localhost:12434", "llama3.2");
        let info = backend.router_info();

        assert_eq!(info.default_model, "llama3.2");
        assert!(info.fallback_model.is_none());
        assert!(info.task_models.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_unreachable() {
        let backend = OpenAICompatibleBackend::new("http://localhost:99999", "llama3.2");
        let healthy = backend.health_check().await;
        assert!(!healthy);
    }

    #[test]
    fn test_chat_completion_request_serialization() {
        let request = ChatCompletionRequest {
            model: "llama3.2".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Hello".to_string()),
            }],
            temperature: Some(0.1),
            max_tokens: None,
            stream: false,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "llama3.2");
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"], "Hello");
        // Use approximate comparison for floating point
        let temp = json["temperature"].as_f64().unwrap();
        assert!((temp - 0.1).abs() < 0.001);
        assert_eq!(json["stream"], false);
        // max_tokens should be omitted when None
        assert!(json.get("max_tokens").is_none());
    }

    #[test]
    fn test_chat_content_text_serialization() {
        let content = ChatContent::Text("Hello world".to_string());
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json, "Hello world");
    }

    #[test]
    fn test_chat_content_parts_serialization() {
        let content = ChatContent::Parts(vec![
            ContentPart::Text {
                text: "Describe this image".to_string(),
            },
            ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "data:image/jpeg;base64,abc123".to_string(),
                },
            },
        ]);

        let json = serde_json::to_value(&content).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0]["type"], "text");
        assert_eq!(json[0]["text"], "Describe this image");
        assert_eq!(json[1]["type"], "image_url");
        assert_eq!(json[1]["image_url"]["url"], "data:image/jpeg;base64,abc123");
    }

    #[test]
    fn test_chat_completion_response_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "llama3.2",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?"
                },
                "finish_reason": "stop"
            }]
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(
            response.choices[0].message.content,
            "Hello! How can I help you?"
        );
    }

    #[test]
    fn test_chat_message_creation() {
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Text("Response text".to_string()),
        };

        assert_eq!(message.role, "assistant");
        if let ChatContent::Text(text) = message.content {
            assert_eq!(text, "Response text");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_image_url_creation() {
        let image_url = ImageUrl {
            url: "https://example.com/image.jpg".to_string(),
        };
        assert_eq!(image_url.url, "https://example.com/image.jpg");
    }
}

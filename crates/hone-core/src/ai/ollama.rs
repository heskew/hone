//! Ollama backend implementation
//!
//! HTTP client for Ollama API. Uses the model router for task-based model selection
//! and the prompt library for customizable prompts.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::{Error, Result};
use crate::model_router::{ModelRouter, TaskType};
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

/// Ollama backend with model router integration
///
/// Uses `ModelRouter` to select the appropriate model for each task type.
/// It also tracks health and falls back automatically when models fail.
///
/// # Task Types
///
/// - `FastClassification`: merchant classification, subscription detection
/// - `StructuredExtraction`: receipt parsing (JSON output)
/// - `Reasoning`: spending explanations, duplicate analysis
/// - `Vision`: receipt OCR
/// - `Narrative`: reports, summaries
///
/// # Configuration
///
/// Configure routing via `~/.local/share/hone/config/models.toml`:
///
/// ```toml
/// [default]
/// model = "gemma3"
///
/// [tasks.vision]
/// model = "llama3.2-vision:11b"
///
/// [tasks.reasoning]
/// model = "nemotron-3-nano"
/// timeout_secs = 90
/// ```
pub struct OllamaBackend {
    http_client: Client,
    base_url: String,
    router: Arc<RwLock<ModelRouter>>,
    default_model: String,
    prompts: Arc<RwLock<PromptLibrary>>,
}

impl Clone for OllamaBackend {
    fn clone(&self) -> Self {
        Self {
            http_client: self.http_client.clone(),
            base_url: self.base_url.clone(),
            router: self.router.clone(),
            default_model: self.default_model.clone(),
            prompts: self.prompts.clone(),
        }
    }
}

impl OllamaBackend {
    /// Create a new Ollama backend
    pub fn new(base_url: &str, default_model: &str) -> Self {
        let router = ModelRouter::new().unwrap_or_default();
        Self {
            http_client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            router: Arc::new(RwLock::new(router)),
            default_model: default_model.to_string(),
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
            router: self.router.clone(),
            default_model: model.to_string(),
            prompts: self.prompts.clone(),
        }
    }

    /// Create with a custom router
    pub fn with_router(base_url: &str, default_model: &str, router: ModelRouter) -> Self {
        Self {
            http_client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            router: Arc::new(RwLock::new(router)),
            default_model: default_model.to_string(),
            prompts: Arc::new(RwLock::new(PromptLibrary::new())),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let host = std::env::var("OLLAMA_HOST").ok()?;
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
        Some(Self::new(&host, &model))
    }
}

/// Request to Ollama API
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

/// Request to Ollama API with images (for vision models)
#[derive(Debug, Serialize)]
struct OllamaVisionRequest {
    model: String,
    prompt: String,
    images: Vec<String>,
    stream: bool,
}

/// Response from Ollama API
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

#[async_trait]
impl AIBackend for OllamaBackend {
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!("Ollama response: {}", ollama_response.response);

        parse_classification(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!("Ollama normalize response: {}", ollama_response.response);

        parse_normalization(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama normalize (with context) response: {}",
            ollama_response.response
        );

        parse_normalization(&ollama_response.response)
    }

    async fn parse_receipt(
        &self,
        image_data: &[u8],
        vision_model: Option<&str>,
    ) -> Result<ParsedReceipt> {
        let model = vision_model.unwrap_or("llava");
        let base64_image = base64::engine::general_purpose::STANDARD.encode(image_data);

        let prompt = {
            let mut prompts = self
                .prompts
                .write()
                .map_err(|_| Error::InvalidData("Failed to acquire prompt library lock".into()))?;
            let template = prompts.get(PromptId::ParseReceipt)?;
            let vars = HashMap::new();
            template.render_user(&vars)
        };

        let request = OllamaVisionRequest {
            model: model.to_string(),
            prompt,
            images: vec![base64_image],
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama receipt parsing response: {}",
            ollama_response.response
        );

        parse_receipt_response(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama entity suggestion response: {}",
            ollama_response.response
        );

        parse_entity_suggestion(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama subscription classification response: {}",
            ollama_response.response
        );

        parse_subscription_classification(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama split recommendation response: {}",
            ollama_response.response
        );

        parse_split_recommendation(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama receipt match evaluation response: {}",
            ollama_response.response
        );

        parse_receipt_match_evaluation(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama duplicate analysis response: {}",
            ollama_response.response
        );

        parse_duplicate_analysis(&ollama_response.response)
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

        let request = OllamaRequest {
            model: self.default_model.clone(),
            prompt,
            stream: false,
        };

        let response = self
            .http_client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Http(response.error_for_status().unwrap_err()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        debug!(
            "Ollama spending change explanation response: {}",
            ollama_response.response
        );

        parse_spending_explanation(&ollama_response.response, &self.default_model)
    }

    async fn health_check(&self) -> bool {
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

    fn model(&self) -> &str {
        &self.default_model
    }

    fn host(&self) -> &str {
        &self.base_url
    }

    fn router_info(&self) -> RouterInfo {
        let mut task_models = Vec::new();
        let mut default_model = self.default_model.clone();
        let mut fallback_model = None;

        if let Ok(router) = self.router.read() {
            default_model = router.config().default_model.clone();
            fallback_model = router.config().fallback_model.clone();

            for task in TaskType::all() {
                let model = router.model_for_task(*task);
                // Only include if different from default
                if model != default_model {
                    task_models.push((task.as_str().to_string(), model.to_string()));
                }
            }
        }

        RouterInfo {
            default_model,
            fallback_model,
            task_models,
        }
    }
}

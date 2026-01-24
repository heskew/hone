//! Pluggable local AI backend abstraction
//!
//! This module provides a backend-agnostic interface for AI operations.
//! All backends run locally (no cloud APIs) - Ollama, OpenAI-compatible servers, etc.
//!
//! # Architecture
//!
//! - `AIBackend` trait: defines the interface for all AI operations
//! - `AIClient` enum: concrete wrapper providing Clone + compile-time dispatch
//! - Backend implementations: `OllamaBackend`, `OpenAICompatibleBackend`, `MockBackend`
//!
//! # Usage
//!
//! ```rust,ignore
//! // Create from environment
//! let ai = AIClient::from_env();
//!
//! // Use for classification
//! if let Some(ref client) = ai {
//!     let result = client.classify_merchant("NETFLIX.COM").await?;
//!     println!("Category: {}", result.category);
//! }
//! ```
//!
//! # Configuration
//!
//! Environment variables:
//! - `AI_BACKEND`: Backend to use (ollama, openai_compatible, mock). Default: ollama
//! - `OLLAMA_HOST`: Ollama server URL (required for ollama backend)
//! - `OLLAMA_MODEL`: Default model name (default: llama3.2)
//! - `OPENAI_COMPATIBLE_HOST`: Server URL (required for openai_compatible backend)
//! - `OPENAI_COMPATIBLE_MODEL`: Model name (default: gpt-3.5-turbo)
//! - `OPENAI_COMPATIBLE_API_KEY`: API key if required (optional)

pub mod anthropic_compat;
mod mock;
mod ollama;
mod openai_compatible;
pub mod orchestrator;
pub mod parsing;
pub mod types;

pub use anthropic_compat::{AnthropicCompatBackend, Message};
pub use mock::MockBackend;
pub use ollama::OllamaBackend;
pub use openai_compatible::OpenAICompatibleBackend;
pub use orchestrator::{AIOrchestrator, OrchestratorResult, ToolCallRecord};
pub use types::*;

use async_trait::async_trait;

use crate::error::Result;
use crate::models::SpendingChangeExplanation;

/// Trait defining the interface for all AI backends
///
/// All backends must implement these methods for AI-powered features.
/// Backends should be Send + Sync to allow use across async tasks.
#[async_trait]
pub trait AIBackend: Send + Sync {
    /// Classify a merchant name into a normalized name and category
    async fn classify_merchant(&self, merchant: &str) -> Result<MerchantClassification>;

    /// Classify multiple merchants in batch
    async fn classify_merchants(
        &self,
        merchants: &[String],
    ) -> Vec<(String, Option<MerchantClassification>)>;

    /// Normalize a merchant name (extract clean name from bank description)
    async fn normalize_merchant(
        &self,
        description: &str,
        category_hint: Option<&str>,
    ) -> Result<String>;

    /// Normalize with extended context (Amex CSV fields)
    async fn normalize_merchant_with_context(
        &self,
        description: &str,
        context: &MerchantContext,
    ) -> Result<String>;

    /// Parse a receipt image and extract line items
    async fn parse_receipt(
        &self,
        image_data: &[u8],
        vision_model: Option<&str>,
    ) -> Result<ParsedReceipt>;

    /// Suggest an entity for a transaction
    async fn suggest_entity(
        &self,
        merchant: &str,
        category: &str,
        entities: &[String],
    ) -> Result<Option<String>>;

    /// Check if a merchant is a subscription service vs retail
    async fn is_subscription_service(&self, merchant: &str) -> Result<SubscriptionClassification>;

    /// Check if a merchant typically requires splitting
    async fn should_suggest_split(&self, merchant: &str) -> Result<SplitRecommendation>;

    /// Evaluate whether a receipt matches a transaction
    async fn evaluate_receipt_match(
        &self,
        receipt_merchant: Option<&str>,
        receipt_date: Option<&str>,
        receipt_total: Option<f64>,
        transaction_description: &str,
        transaction_date: &str,
        transaction_amount: f64,
        transaction_merchant_normalized: Option<&str>,
    ) -> Result<ReceiptMatchEvaluation>;

    /// Analyze duplicate services to explain overlap
    async fn analyze_duplicate_services(
        &self,
        category: &str,
        services: &[&str],
        feedback: Option<&str>,
    ) -> Result<DuplicateAnalysis>;

    /// Explain why spending changed in a category
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
    ) -> Result<SpendingChangeExplanation>;

    /// Check if the backend is available
    async fn health_check(&self) -> bool;

    /// Get the model name (for metrics)
    fn model(&self) -> &str;

    /// Get the host URL (for logging)
    fn host(&self) -> &str;

    /// Get router configuration info
    fn router_info(&self) -> RouterInfo;
}

/// Concrete AI client enum
///
/// Provides Clone and compile-time dispatch without Box<dyn> overhead.
/// All variants implement the same AIBackend operations.
#[derive(Clone)]
pub enum AIClient {
    /// Ollama backend (HTTP API)
    Ollama(OllamaBackend),
    /// OpenAI-compatible backend (Docker Model Runner, vLLM, LocalAI, llama-server, etc.)
    OpenAICompatible(OpenAICompatibleBackend),
    /// Mock backend for testing
    Mock(MockBackend),
}

impl AIClient {
    /// Create an AI client from environment variables
    ///
    /// Checks `AI_BACKEND` to determine which backend to use:
    /// - `ollama` (default): Uses OLLAMA_HOST and OLLAMA_MODEL
    /// - `openai_compatible`: Uses OPENAI_COMPATIBLE_HOST and OPENAI_COMPATIBLE_MODEL
    ///   (works with Docker Model Runner, vLLM, LocalAI, llama-server, etc.)
    /// - `mock`: Creates a mock backend for testing
    ///
    /// Returns None if the required environment variables are not set.
    pub fn from_env() -> Option<Self> {
        let backend = std::env::var("AI_BACKEND").unwrap_or_else(|_| "ollama".to_string());

        match backend.to_lowercase().as_str() {
            "ollama" => OllamaBackend::from_env().map(AIClient::Ollama),
            "openai_compatible" | "openai" | "vllm" | "localai" | "llamacpp" => {
                OpenAICompatibleBackend::from_env().map(AIClient::OpenAICompatible)
            }
            "mock" => Some(AIClient::Mock(MockBackend::new())),
            _ => {
                tracing::warn!(backend = %backend, "Unknown AI_BACKEND, falling back to ollama");
                OllamaBackend::from_env().map(AIClient::Ollama)
            }
        }
    }

    /// Create an Ollama backend directly
    pub fn ollama(host: &str, model: &str) -> Self {
        AIClient::Ollama(OllamaBackend::new(host, model))
    }

    /// Create a mock backend for testing
    pub fn mock() -> Self {
        AIClient::Mock(MockBackend::new())
    }

    /// Create a new instance with a different model
    ///
    /// Used for runtime model override (e.g., user selects a different model for testing)
    pub fn with_model(&self, model: &str) -> Self {
        match self {
            AIClient::Ollama(b) => AIClient::Ollama(b.with_model(model)),
            AIClient::OpenAICompatible(b) => AIClient::OpenAICompatible(b.with_model(model)),
            AIClient::Mock(b) => AIClient::Mock(b.with_model(model)),
        }
    }
}

// Implement AIBackend for AIClient by delegating to the inner backend
#[async_trait]
impl AIBackend for AIClient {
    async fn classify_merchant(&self, merchant: &str) -> Result<MerchantClassification> {
        match self {
            AIClient::Ollama(b) => b.classify_merchant(merchant).await,
            AIClient::OpenAICompatible(b) => b.classify_merchant(merchant).await,
            AIClient::Mock(b) => b.classify_merchant(merchant).await,
        }
    }

    async fn classify_merchants(
        &self,
        merchants: &[String],
    ) -> Vec<(String, Option<MerchantClassification>)> {
        match self {
            AIClient::Ollama(b) => b.classify_merchants(merchants).await,
            AIClient::OpenAICompatible(b) => b.classify_merchants(merchants).await,
            AIClient::Mock(b) => b.classify_merchants(merchants).await,
        }
    }

    async fn normalize_merchant(
        &self,
        description: &str,
        category_hint: Option<&str>,
    ) -> Result<String> {
        match self {
            AIClient::Ollama(b) => b.normalize_merchant(description, category_hint).await,
            AIClient::OpenAICompatible(b) => b.normalize_merchant(description, category_hint).await,
            AIClient::Mock(b) => b.normalize_merchant(description, category_hint).await,
        }
    }

    async fn normalize_merchant_with_context(
        &self,
        description: &str,
        context: &MerchantContext,
    ) -> Result<String> {
        match self {
            AIClient::Ollama(b) => {
                b.normalize_merchant_with_context(description, context)
                    .await
            }
            AIClient::OpenAICompatible(b) => {
                b.normalize_merchant_with_context(description, context)
                    .await
            }
            AIClient::Mock(b) => {
                b.normalize_merchant_with_context(description, context)
                    .await
            }
        }
    }

    async fn parse_receipt(
        &self,
        image_data: &[u8],
        vision_model: Option<&str>,
    ) -> Result<ParsedReceipt> {
        match self {
            AIClient::Ollama(b) => b.parse_receipt(image_data, vision_model).await,
            AIClient::OpenAICompatible(b) => b.parse_receipt(image_data, vision_model).await,
            AIClient::Mock(b) => b.parse_receipt(image_data, vision_model).await,
        }
    }

    async fn suggest_entity(
        &self,
        merchant: &str,
        category: &str,
        entities: &[String],
    ) -> Result<Option<String>> {
        match self {
            AIClient::Ollama(b) => b.suggest_entity(merchant, category, entities).await,
            AIClient::OpenAICompatible(b) => b.suggest_entity(merchant, category, entities).await,
            AIClient::Mock(b) => b.suggest_entity(merchant, category, entities).await,
        }
    }

    async fn is_subscription_service(&self, merchant: &str) -> Result<SubscriptionClassification> {
        match self {
            AIClient::Ollama(b) => b.is_subscription_service(merchant).await,
            AIClient::OpenAICompatible(b) => b.is_subscription_service(merchant).await,
            AIClient::Mock(b) => b.is_subscription_service(merchant).await,
        }
    }

    async fn should_suggest_split(&self, merchant: &str) -> Result<SplitRecommendation> {
        match self {
            AIClient::Ollama(b) => b.should_suggest_split(merchant).await,
            AIClient::OpenAICompatible(b) => b.should_suggest_split(merchant).await,
            AIClient::Mock(b) => b.should_suggest_split(merchant).await,
        }
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
        match self {
            AIClient::Ollama(b) => {
                b.evaluate_receipt_match(
                    receipt_merchant,
                    receipt_date,
                    receipt_total,
                    transaction_description,
                    transaction_date,
                    transaction_amount,
                    transaction_merchant_normalized,
                )
                .await
            }
            AIClient::OpenAICompatible(b) => {
                b.evaluate_receipt_match(
                    receipt_merchant,
                    receipt_date,
                    receipt_total,
                    transaction_description,
                    transaction_date,
                    transaction_amount,
                    transaction_merchant_normalized,
                )
                .await
            }
            AIClient::Mock(b) => {
                b.evaluate_receipt_match(
                    receipt_merchant,
                    receipt_date,
                    receipt_total,
                    transaction_description,
                    transaction_date,
                    transaction_amount,
                    transaction_merchant_normalized,
                )
                .await
            }
        }
    }

    async fn analyze_duplicate_services(
        &self,
        category: &str,
        services: &[&str],
        feedback: Option<&str>,
    ) -> Result<DuplicateAnalysis> {
        match self {
            AIClient::Ollama(b) => {
                b.analyze_duplicate_services(category, services, feedback)
                    .await
            }
            AIClient::OpenAICompatible(b) => {
                b.analyze_duplicate_services(category, services, feedback)
                    .await
            }
            AIClient::Mock(b) => {
                b.analyze_duplicate_services(category, services, feedback)
                    .await
            }
        }
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
        match self {
            AIClient::Ollama(b) => {
                b.explain_spending_change(
                    category,
                    baseline_amount,
                    current_amount,
                    baseline_tx_count,
                    current_tx_count,
                    top_merchants,
                    new_merchants,
                    feedback,
                )
                .await
            }
            AIClient::OpenAICompatible(b) => {
                b.explain_spending_change(
                    category,
                    baseline_amount,
                    current_amount,
                    baseline_tx_count,
                    current_tx_count,
                    top_merchants,
                    new_merchants,
                    feedback,
                )
                .await
            }
            AIClient::Mock(b) => {
                b.explain_spending_change(
                    category,
                    baseline_amount,
                    current_amount,
                    baseline_tx_count,
                    current_tx_count,
                    top_merchants,
                    new_merchants,
                    feedback,
                )
                .await
            }
        }
    }

    async fn health_check(&self) -> bool {
        match self {
            AIClient::Ollama(b) => b.health_check().await,
            AIClient::OpenAICompatible(b) => b.health_check().await,
            AIClient::Mock(b) => b.health_check().await,
        }
    }

    fn model(&self) -> &str {
        match self {
            AIClient::Ollama(b) => b.model(),
            AIClient::OpenAICompatible(b) => b.model(),
            AIClient::Mock(b) => b.model(),
        }
    }

    fn host(&self) -> &str {
        match self {
            AIClient::Ollama(b) => b.host(),
            AIClient::OpenAICompatible(b) => b.host(),
            AIClient::Mock(b) => b.host(),
        }
    }

    fn router_info(&self) -> RouterInfo {
        match self {
            AIClient::Ollama(b) => b.router_info(),
            AIClient::OpenAICompatible(b) => b.router_info(),
            AIClient::Mock(b) => b.router_info(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_client_mock() {
        let client = AIClient::mock();
        assert_eq!(client.model(), "mock");
        assert_eq!(client.host(), "mock://localhost");
    }

    #[tokio::test]
    async fn test_mock_health_check() {
        let client = AIClient::mock();
        assert!(client.health_check().await);
    }

    #[tokio::test]
    async fn test_mock_classify_merchant() {
        let client = AIClient::mock();
        let result = client.classify_merchant("NETFLIX.COM").await.unwrap();
        assert!(!result.merchant.is_empty());
        assert!(!result.category.is_empty());
    }
}

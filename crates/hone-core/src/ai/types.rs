//! AI backend response types
//!
//! These types are backend-agnostic and used across all AI implementations.

use serde::{Deserialize, Serialize};

/// Extended context for merchant normalization (Amex CSV fields)
#[derive(Debug, Default, Clone)]
pub struct MerchantContext {
    /// Our extracted merchant name (from Extended Details parsing)
    pub extracted_merchant: Option<String>,
    /// "Appears On Your Statement As" field
    pub statement_as: Option<String>,
    /// "Extended Details" field (often has full merchant name)
    pub extended_details: Option<String>,
    /// Bank-provided category
    pub category: Option<String>,
}

/// Router configuration information for display
#[derive(Debug, Clone)]
pub struct RouterInfo {
    /// Default model for all tasks
    pub default_model: String,
    /// Fallback model when primary fails
    pub fallback_model: Option<String>,
    /// Task-specific model overrides (only non-default)
    pub task_models: Vec<(String, String)>,
}

/// Result of merchant classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantClassification {
    /// Normalized merchant name (e.g., "Netflix" from "NETFLIX.COM*1234")
    pub merchant: String,
    /// Category (e.g., "streaming", "music", "utilities")
    pub category: String,
}

/// A line item extracted from a receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedReceiptItem {
    pub description: String,
    pub amount: f64,
    pub split_type: String,
    #[serde(default)]
    pub category_hint: Option<String>,
    #[serde(default)]
    pub entity_hint: Option<String>,
}

/// Result of receipt parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedReceipt {
    pub merchant: Option<String>,
    pub date: Option<String>,
    pub items: Vec<ParsedReceiptItem>,
    #[serde(default)]
    pub subtotal: Option<f64>,
    #[serde(default)]
    pub tax: Option<f64>,
    #[serde(default)]
    pub tip: Option<f64>,
    #[serde(default)]
    pub total: Option<f64>,
}

/// Entity suggestion from AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySuggestion {
    pub entity: Option<String>,
    pub confidence: f64,
    pub reason: String,
}

/// Split recommendation for a merchant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitRecommendation {
    pub should_split: bool,
    pub reason: String,
    #[serde(default)]
    pub typical_categories: Vec<String>,
}

/// Subscription classification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionClassification {
    /// Whether this merchant is a subscription service
    pub is_subscription: bool,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
    /// Brief explanation of the classification
    pub reason: String,
}

/// Receipt-to-transaction match evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptMatchEvaluation {
    /// Whether AI believes this is the same purchase
    pub is_match: bool,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
    /// Explanation of the match decision
    pub reason: String,
    /// Explanation for amount difference (tip, tax, etc.)
    #[serde(default)]
    pub amount_explanation: Option<String>,
}

/// Duplicate services analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateAnalysis {
    /// What the services have in common
    pub overlap: String,
    /// What's unique to each service
    pub unique_features: Vec<ServiceFeature>,
}

/// Unique feature of a service in duplicate analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceFeature {
    /// Service name
    pub service: String,
    /// What makes this service unique
    pub unique: String,
}

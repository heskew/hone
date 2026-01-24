//! Legacy Ollama module
//!
//! This module re-exports types from the `ai` module for any code that still
//! imports from `hone_core::ollama`. New code should use `hone_core::ai` directly.
//!
//! # Migration
//!
//! ```rust,ignore
//! // Old:
//! use hone_core::ollama::{MerchantClassification, OllamaBackend};
//!
//! // New:
//! use hone_core::ai::{AIBackend, AIClient, MerchantClassification, OllamaBackend};
//! ```

// Re-export types from ai module for backwards compatibility
pub use crate::ai::{
    DuplicateAnalysis, MerchantClassification, MerchantContext, OllamaBackend, ParsedReceipt,
    ParsedReceiptItem, ReceiptMatchEvaluation, RouterInfo, ServiceFeature, SplitRecommendation,
    SubscriptionClassification,
};

// Type alias for code that still uses RoutedOllamaClient
pub type RoutedOllamaClient = OllamaBackend;

//! Mock backend for testing
//!
//! Provides configurable mock responses for all AI operations.
//! Useful for unit tests and development without a running LLM server.

use async_trait::async_trait;

use crate::error::Result;
use crate::models::SpendingChangeExplanation;

use super::types::{
    DuplicateAnalysis, MerchantClassification, MerchantContext, ParsedReceipt, ParsedReceiptItem,
    ReceiptMatchEvaluation, RouterInfo, ServiceFeature, SplitRecommendation,
    SubscriptionClassification,
};
use super::AIBackend;

/// Mock AI backend for testing
///
/// Returns predictable responses for all AI operations.
/// Can be configured with custom responses for specific tests.
#[derive(Clone, Default)]
pub struct MockBackend {
    /// Whether health_check should return true
    pub healthy: bool,
}

impl MockBackend {
    /// Create a new mock backend (healthy by default)
    pub fn new() -> Self {
        Self { healthy: true }
    }

    /// Create an unhealthy mock backend
    pub fn unhealthy() -> Self {
        Self { healthy: false }
    }

    /// Create a new instance with a different model (no-op for mock)
    pub fn with_model(&self, _model: &str) -> Self {
        self.clone()
    }
}

#[async_trait]
impl AIBackend for MockBackend {
    async fn classify_merchant(&self, merchant: &str) -> Result<MerchantClassification> {
        // Simple mock: extract category from well-known merchants
        let (normalized, category) = match merchant.to_uppercase().as_str() {
            m if m.contains("NETFLIX") => ("Netflix", "Subscriptions"),
            m if m.contains("SPOTIFY") => ("Spotify", "Subscriptions"),
            m if m.contains("AMAZON") => ("Amazon", "Shopping"),
            m if m.contains("STARBUCKS") => ("Starbucks", "Dining"),
            m if m.contains("UBER") => ("Uber", "Transport"),
            m if m.contains("WHOLE FOODS") || m.contains("WHOLEFDS") => {
                ("Whole Foods", "Groceries")
            }
            m if m.contains("COSTCO") => ("Costco", "Shopping"),
            m if m.contains("SHELL") || m.contains("CHEVRON") || m.contains("EXXON") => {
                ("Gas Station", "Transport")
            }
            _ => (merchant, "Other"),
        };

        Ok(MerchantClassification {
            merchant: normalized.to_string(),
            category: category.to_string(),
        })
    }

    async fn classify_merchants(
        &self,
        merchants: &[String],
    ) -> Vec<(String, Option<MerchantClassification>)> {
        let mut results = Vec::new();
        for merchant in merchants {
            let classification = self.classify_merchant(merchant).await.ok();
            results.push((merchant.clone(), classification));
        }
        results
    }

    async fn normalize_merchant(
        &self,
        description: &str,
        _category_hint: Option<&str>,
    ) -> Result<String> {
        // Simple normalization: strip common prefixes/suffixes
        let normalized = description
            .trim()
            .trim_start_matches("SQ *")
            .trim_start_matches("TST*")
            .trim_start_matches("SP ")
            .trim_end_matches(" LLC")
            .trim_end_matches(" INC")
            .trim_end_matches(" CORP")
            .to_string();

        // Title case
        let result = normalized
            .split_whitespace()
            .map(|word| {
                let mut chars: Vec<char> = word.chars().collect();
                if !chars.is_empty() {
                    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
                    for c in chars.iter_mut().skip(1) {
                        *c = c.to_lowercase().next().unwrap_or(*c);
                    }
                }
                chars.into_iter().collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ");

        Ok(result)
    }

    async fn normalize_merchant_with_context(
        &self,
        description: &str,
        context: &MerchantContext,
    ) -> Result<String> {
        // Prefer extracted merchant if available
        if let Some(ref extracted) = context.extracted_merchant {
            return Ok(extracted.clone());
        }
        self.normalize_merchant(description, context.category.as_deref())
            .await
    }

    async fn parse_receipt(
        &self,
        _image_data: &[u8],
        _vision_model: Option<&str>,
    ) -> Result<ParsedReceipt> {
        // Return a mock receipt
        Ok(ParsedReceipt {
            merchant: Some("Mock Store".to_string()),
            date: Some("2024-01-15".to_string()),
            items: vec![
                ParsedReceiptItem {
                    description: "Item 1".to_string(),
                    amount: 10.00,
                    split_type: "item".to_string(),
                    category_hint: Some("Shopping".to_string()),
                    entity_hint: None,
                },
                ParsedReceiptItem {
                    description: "Tax".to_string(),
                    amount: 0.80,
                    split_type: "tax".to_string(),
                    category_hint: None,
                    entity_hint: None,
                },
            ],
            subtotal: Some(10.00),
            tax: Some(0.80),
            tip: None,
            total: Some(10.80),
        })
    }

    async fn suggest_entity(
        &self,
        merchant: &str,
        _category: &str,
        entities: &[String],
    ) -> Result<Option<String>> {
        if entities.is_empty() {
            return Ok(None);
        }

        // Simple heuristic: pet stores suggest first pet entity
        if merchant.to_lowercase().contains("pet") {
            return Ok(entities.first().cloned());
        }

        Ok(None)
    }

    async fn is_subscription_service(&self, merchant: &str) -> Result<SubscriptionClassification> {
        let merchant_upper = merchant.to_uppercase();
        let is_subscription = merchant_upper.contains("NETFLIX")
            || merchant_upper.contains("SPOTIFY")
            || merchant_upper.contains("DISNEY")
            || merchant_upper.contains("HBO")
            || merchant_upper.contains("HULU")
            || merchant_upper.contains("GYM")
            || merchant_upper.contains("FITNESS");

        Ok(SubscriptionClassification {
            is_subscription,
            confidence: if is_subscription { 0.95 } else { 0.90 },
            reason: if is_subscription {
                "Known subscription service".to_string()
            } else {
                "Not a typical subscription".to_string()
            },
        })
    }

    async fn should_suggest_split(&self, merchant: &str) -> Result<SplitRecommendation> {
        let merchant_upper = merchant.to_uppercase();
        let should_split = merchant_upper.contains("COSTCO")
            || merchant_upper.contains("TARGET")
            || merchant_upper.contains("WALMART")
            || merchant_upper.contains("AMAZON");

        Ok(SplitRecommendation {
            should_split,
            reason: if should_split {
                "Multi-category retailer".to_string()
            } else {
                "Single-category merchant".to_string()
            },
            typical_categories: if should_split {
                vec![
                    "Groceries".to_string(),
                    "Household".to_string(),
                    "Shopping".to_string(),
                ]
            } else {
                vec![]
            },
        })
    }

    async fn evaluate_receipt_match(
        &self,
        receipt_merchant: Option<&str>,
        _receipt_date: Option<&str>,
        receipt_total: Option<f64>,
        transaction_description: &str,
        _transaction_date: &str,
        transaction_amount: f64,
        transaction_merchant_normalized: Option<&str>,
    ) -> Result<ReceiptMatchEvaluation> {
        // Simple matching logic
        let merchant_match = receipt_merchant
            .map(|rm| {
                let rm_lower = rm.to_lowercase();
                transaction_description.to_lowercase().contains(&rm_lower)
                    || transaction_merchant_normalized
                        .map(|n| n.to_lowercase().contains(&rm_lower))
                        .unwrap_or(false)
            })
            .unwrap_or(false);

        let amount_match = receipt_total
            .map(|rt| (rt - transaction_amount.abs()).abs() < 1.0)
            .unwrap_or(false);

        let is_match = merchant_match || amount_match;

        Ok(ReceiptMatchEvaluation {
            is_match,
            confidence: if is_match { 0.85 } else { 0.15 },
            reason: if is_match {
                "Merchant and/or amount match".to_string()
            } else {
                "No clear match".to_string()
            },
            amount_explanation: if is_match && !amount_match {
                Some("Tip or tax difference".to_string())
            } else {
                None
            },
        })
    }

    async fn analyze_duplicate_services(
        &self,
        category: &str,
        services: &[&str],
        _feedback: Option<&str>,
    ) -> Result<DuplicateAnalysis> {
        let overlap = format!("All provide {} services", category.to_lowercase());

        let unique_features = services
            .iter()
            .map(|s| ServiceFeature {
                service: s.to_string(),
                unique: format!("Unique features of {}", s),
            })
            .collect();

        Ok(DuplicateAnalysis {
            overlap,
            unique_features,
        })
    }

    async fn explain_spending_change(
        &self,
        category: &str,
        baseline_amount: f64,
        current_amount: f64,
        _baseline_tx_count: i32,
        _current_tx_count: i32,
        _top_merchants: &[(String, f64, i32)],
        _new_merchants: &[String],
        _feedback: Option<&str>,
    ) -> Result<SpendingChangeExplanation> {
        let direction = if current_amount > baseline_amount {
            "increased"
        } else {
            "decreased"
        };

        Ok(SpendingChangeExplanation {
            summary: format!(
                "{} spending {} from ${:.0} to ${:.0}",
                category, direction, baseline_amount, current_amount
            ),
            reasons: vec![format!("Mock explanation for {} spending change", category)],
            model: "mock".to_string(),
            analyzed_at: chrono::Utc::now(),
        })
    }

    async fn health_check(&self) -> bool {
        self.healthy
    }

    fn model(&self) -> &str {
        "mock"
    }

    fn host(&self) -> &str {
        "mock://localhost"
    }

    fn router_info(&self) -> RouterInfo {
        RouterInfo {
            default_model: "mock".to_string(),
            fallback_model: None,
            task_models: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_classify_merchant() {
        let mock = MockBackend::new();
        let result = mock.classify_merchant("NETFLIX.COM").await.unwrap();
        assert_eq!(result.merchant, "Netflix");
        assert_eq!(result.category, "Subscriptions");
    }

    #[tokio::test]
    async fn test_mock_normalize_merchant() {
        let mock = MockBackend::new();
        let result = mock
            .normalize_merchant("SQ *COFFEE SHOP LLC", None)
            .await
            .unwrap();
        assert_eq!(result, "Coffee Shop");
    }

    #[tokio::test]
    async fn test_mock_is_subscription() {
        let mock = MockBackend::new();

        let netflix = mock.is_subscription_service("NETFLIX").await.unwrap();
        assert!(netflix.is_subscription);

        let grocery = mock.is_subscription_service("SAFEWAY").await.unwrap();
        assert!(!grocery.is_subscription);
    }

    #[tokio::test]
    async fn test_mock_health_check() {
        let healthy = MockBackend::new();
        assert!(healthy.health_check().await);

        let unhealthy = MockBackend::unhealthy();
        assert!(!unhealthy.health_check().await);
    }
}

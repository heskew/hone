//! JSON parsing helpers for AI backend responses
//!
//! These functions extract JSON from AI model responses, which often include
//! extra text before/after the JSON payload.

use crate::error::{Error, Result};
use crate::models::SpendingChangeExplanation;

use super::types::{
    DuplicateAnalysis, EntitySuggestion, MerchantClassification, ParsedReceipt,
    ReceiptMatchEvaluation, SplitRecommendation, SubscriptionClassification,
};

/// Parse classification from AI response
pub fn parse_classification(response: &str) -> Result<MerchantClassification> {
    // Try to find JSON in the response
    let response = response.trim();

    // Look for JSON object
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            serde_json::from_str(json_str).map_err(|e| {
                // Truncate long responses for the error message
                let truncated = if json_str.len() > 200 {
                    format!("{}...", &json_str[..200])
                } else {
                    json_str.to_string()
                };
                Error::InvalidData(format!("Invalid JSON from AI: {} | Raw: {}", e, truncated))
            })
        }
        _ => Err(Error::InvalidData(format!(
            "No JSON found in AI response | Raw: {}",
            if response.len() > 200 {
                format!("{}...", &response[..200])
            } else {
                response.to_string()
            }
        ))),
    }
}

/// Parse receipt from AI response
pub fn parse_receipt_response(response: &str) -> Result<ParsedReceipt> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            serde_json::from_str(json_str)
                .map_err(|e| Error::InvalidData(format!("Invalid receipt JSON from AI: {}", e)))
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI receipt response".into(),
        )),
    }
}

/// Parse entity suggestion from AI response
pub fn parse_entity_suggestion(response: &str) -> Result<Option<String>> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            let suggestion: EntitySuggestion = serde_json::from_str(json_str).map_err(|e| {
                Error::InvalidData(format!("Invalid entity suggestion JSON: {}", e))
            })?;

            // Only return entity if confidence is above threshold
            if suggestion.confidence > 0.5 {
                Ok(suggestion.entity)
            } else {
                Ok(None)
            }
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI entity suggestion response".into(),
        )),
    }
}

/// Parse split recommendation from AI response
pub fn parse_split_recommendation(response: &str) -> Result<SplitRecommendation> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            serde_json::from_str(json_str).map_err(|e| {
                Error::InvalidData(format!("Invalid split recommendation JSON: {}", e))
            })
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI split recommendation response".into(),
        )),
    }
}

/// Parse subscription classification from AI response
pub fn parse_subscription_classification(response: &str) -> Result<SubscriptionClassification> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            serde_json::from_str(json_str).map_err(|e| {
                Error::InvalidData(format!("Invalid subscription classification JSON: {}", e))
            })
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI subscription classification response".into(),
        )),
    }
}

/// Parse receipt match evaluation from AI response
pub fn parse_receipt_match_evaluation(response: &str) -> Result<ReceiptMatchEvaluation> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            serde_json::from_str(json_str).map_err(|e| {
                Error::InvalidData(format!("Invalid receipt match evaluation JSON: {}", e))
            })
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI receipt match evaluation response".into(),
        )),
    }
}

/// Parse duplicate analysis from AI response
pub fn parse_duplicate_analysis(response: &str) -> Result<DuplicateAnalysis> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            serde_json::from_str(json_str)
                .map_err(|e| Error::InvalidData(format!("Invalid duplicate analysis JSON: {}", e)))
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI duplicate analysis response".into(),
        )),
    }
}

/// Raw spending explanation response from AI (without model/timestamp metadata)
#[derive(Debug, serde::Deserialize)]
struct SpendingExplanationResponse {
    summary: String,
    reasons: Vec<String>,
}

/// Parse spending change explanation from AI response
pub fn parse_spending_explanation(
    response: &str,
    model: &str,
) -> Result<SpendingChangeExplanation> {
    let response = response.trim();
    let start = response.find('{');
    let end = response.rfind('}');

    match (start, end) {
        (Some(s), Some(e)) if s < e => {
            let json_str = &response[s..=e];
            let raw: SpendingExplanationResponse = serde_json::from_str(json_str).map_err(|e| {
                Error::InvalidData(format!("Invalid spending explanation JSON: {}", e))
            })?;

            // Add model and timestamp metadata
            Ok(SpendingChangeExplanation {
                summary: raw.summary,
                reasons: raw.reasons,
                model: model.to_string(),
                analyzed_at: chrono::Utc::now(),
            })
        }
        _ => Err(Error::InvalidData(
            "No JSON found in AI spending explanation response".into(),
        )),
    }
}

/// Normalization response (just merchant name)
#[derive(Debug, serde::Deserialize)]
struct NormalizationResponse {
    merchant: String,
}

/// Parse normalization from AI response
pub fn parse_normalization(response: &str) -> Result<String> {
    let response = response.trim();

    // Find the first JSON object by matching braces
    if let Some(start) = response.find('{') {
        let mut depth = 0;
        let mut end = None;

        for (i, c) in response[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + i);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(e) = end {
            let json_str = &response[start..=e];
            let parsed: NormalizationResponse = serde_json::from_str(json_str).map_err(|err| {
                Error::InvalidData(format!("Invalid normalization JSON: {}", err))
            })?;
            return Ok(parsed.merchant);
        }
    }

    Err(Error::InvalidData(
        "No JSON found in AI normalization response".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_classification() {
        let response = r#"{"merchant": "Netflix", "category": "streaming"}"#;
        let result = parse_classification(response).unwrap();
        assert_eq!(result.merchant, "Netflix");
        assert_eq!(result.category, "streaming");
    }

    #[test]
    fn test_parse_classification_with_text() {
        let response = r#"Here's the classification:
{"merchant": "Netflix", "category": "streaming"}
Done!"#;
        let result = parse_classification(response).unwrap();
        assert_eq!(result.merchant, "Netflix");
    }

    #[test]
    fn test_parse_receipt_response() {
        let response = r#"{
            "merchant": "Target",
            "date": "2024-01-15",
            "items": [
                {"description": "T-shirt", "amount": 25.00, "split_type": "item", "category_hint": "Shopping"},
                {"description": "Tax", "amount": 2.00, "split_type": "tax"}
            ],
            "subtotal": 25.00,
            "tax": 2.00,
            "total": 27.00
        }"#;
        let result = parse_receipt_response(response).unwrap();
        assert_eq!(result.merchant, Some("Target".to_string()));
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].description, "T-shirt");
        assert_eq!(result.items[0].amount, 25.00);
        assert_eq!(result.total, Some(27.00));
    }

    #[test]
    fn test_parse_entity_suggestion_high_confidence() {
        let response = r#"{"entity": "Rex", "confidence": 0.8, "reason": "Pet store purchase"}"#;
        let result = parse_entity_suggestion(response).unwrap();
        assert_eq!(result, Some("Rex".to_string()));
    }

    #[test]
    fn test_parse_entity_suggestion_low_confidence() {
        let response = r#"{"entity": "Rex", "confidence": 0.3, "reason": "Could be for pet"}"#;
        let result = parse_entity_suggestion(response).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_normalization() {
        let response = r#"{"merchant": "Netflix"}"#;
        let result = parse_normalization(response).unwrap();
        assert_eq!(result, "Netflix");
    }

    #[test]
    fn test_parse_normalization_with_text() {
        let response = r#"Here is the result:
{"merchant": "Amazon Marketplace"}
That's it!"#;
        let result = parse_normalization(response).unwrap();
        assert_eq!(result, "Amazon Marketplace");
    }

    #[test]
    fn test_parse_normalization_with_apostrophe() {
        let response = r#"{"merchant": "Trader Joe's"}"#;
        let result = parse_normalization(response).unwrap();
        assert_eq!(result, "Trader Joe's");
    }

    #[test]
    fn test_parse_subscription_classification() {
        let response =
            r#"{"is_subscription": true, "confidence": 0.99, "reason": "streaming service"}"#;
        let result = parse_subscription_classification(response).unwrap();
        assert!(result.is_subscription);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_parse_duplicate_analysis() {
        let response = r#"{"overlap": "All offer on-demand streaming", "unique_features": [{"service": "Netflix", "unique": "International content"}, {"service": "Disney+", "unique": "Family content, Marvel"}]}"#;
        let result = parse_duplicate_analysis(response).unwrap();
        assert_eq!(result.overlap, "All offer on-demand streaming");
        assert_eq!(result.unique_features.len(), 2);
    }

    #[test]
    fn test_parse_receipt_match_evaluation() {
        let response = r#"{"is_match": true, "confidence": 0.95, "reason": "Same restaurant, date matches", "amount_explanation": "Likely $9.10 tip added"}"#;
        let result = parse_receipt_match_evaluation(response).unwrap();
        assert!(result.is_match);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_parse_split_recommendation() {
        let response = r#"{"should_split": true, "reason": "Multi-category retailer", "typical_categories": ["Groceries", "Pharmacy", "Household"]}"#;
        let result = parse_split_recommendation(response).unwrap();
        assert!(result.should_split);
        assert_eq!(result.typical_categories.len(), 3);
    }
}

//! Test utilities for hone-core
//!
//! This module provides testing infrastructure including a mock Ollama server
//! that can be used for development and integration tests.

use axum::{
    extract::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::sync::oneshot;

/// Mock Ollama server for testing and development
pub struct MockOllamaServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockOllamaServer {
    /// Start the mock server on an available port
    pub async fn start() -> Self {
        let app = Router::new()
            .route("/api/tags", get(handle_tags))
            .route("/api/generate", post(handle_generate));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .unwrap();
        });

        Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Get the base URL for this mock server
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Stop the mock server
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for MockOllamaServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Ollama tags endpoint response (health check)
async fn handle_tags() -> Json<TagsResponse> {
    Json(TagsResponse {
        models: vec![ModelInfo {
            name: "llama3.2:latest".to_string(),
            modified_at: "2024-01-01T00:00:00Z".to_string(),
            size: 4_000_000_000,
        }],
    })
}

/// Ollama generate endpoint
async fn handle_generate(Json(request): Json<GenerateRequest>) -> Json<GenerateResponse> {
    // Detect what type of request this is based on prompt content
    // These patterns match the prompt files in prompts/*.md
    let response = if (request.prompt.contains("Description: \"")
        || request.prompt.contains("Extract the merchant name from:"))
        && (request.prompt.contains(r#"{"merchant":"#)
            || request.prompt.contains(r#"{"merchant":"#))
    {
        // Normalization request (normalize_merchant.md pattern)
        handle_normalize_mock(&request.prompt)
    } else if request.prompt.contains("Classify this merchant: ") {
        // Merchant classification (classify_merchant.md pattern)
        let classification = classify_merchant_mock(&request.prompt);
        serde_json::to_string(&classification).unwrap()
    } else if request.prompt.contains("subscription service")
        || request.prompt.contains("SUBSCRIPTION services")
    {
        // Subscription classification (classify_subscription.md pattern)
        handle_subscription_mock(&request.prompt)
    } else if request.prompt.contains("should_split")
        || request.prompt.contains("multi-category merchants")
    {
        // Split recommendation (suggest_split.md pattern)
        handle_split_mock(&request.prompt)
    } else if request.prompt.contains("Available entities:")
        || request.prompt.contains("suggest which entity")
    {
        // Entity suggestion (suggest_entity.md pattern)
        handle_entity_mock(&request.prompt)
    } else if request.prompt.contains("split_type") && request.prompt.contains("category_hint") {
        // Receipt parsing (vision) - return mock receipt (parse_receipt.md pattern)
        handle_receipt_mock()
    } else {
        // Default: try merchant classification
        let classification = classify_merchant_mock(&request.prompt);
        serde_json::to_string(&classification).unwrap()
    };

    Json(GenerateResponse {
        model: request.model,
        response,
        done: true,
    })
}

/// Handle normalize merchant request
fn handle_normalize_mock(prompt: &str) -> String {
    let merchant = extract_merchant_from_prompt_normalize(prompt);
    let m = merchant.to_uppercase();

    let normalized = if m.contains("NETFLIX") {
        "Netflix"
    } else if m.contains("TRADER JOE") {
        "Trader Joe's"
    } else if m.contains("MCDONALD") {
        "McDonald's"
    } else if m.contains("AMAZON") || m.contains("AMZN") {
        "Amazon"
    } else if m.contains("STARBUCKS") {
        "Starbucks"
    } else if m.contains("TARGET") {
        "Target"
    } else if m.contains("INTEREST CHARGE") {
        "Interest Charge"
    } else if m.contains("ANNUAL FEE") {
        "Annual Fee"
    } else if m.contains("PAYMENT") {
        "Payment"
    } else {
        // Clean up the name
        &merchant
    };

    format!(r#"{{"merchant": "{}"}}"#, normalized)
}

/// Extract merchant from normalize prompt (uses "Description:" instead of "Merchant:")
fn extract_merchant_from_prompt_normalize(prompt: &str) -> String {
    // Current format: Extract the merchant name from: "{{description}}"
    if let Some(start) = prompt.find("Extract the merchant name from: \"") {
        let after_start = &prompt[start + 33..];
        if let Some(end) = after_start.find('"') {
            return after_start[..end].to_string();
        }
    }
    // Legacy format: Description: "merchant"
    if let Some(start) = prompt.find("Description: \"") {
        let after_start = &prompt[start + 14..];
        if let Some(end) = after_start.find('"') {
            return after_start[..end].to_string();
        }
    }
    "Unknown".to_string()
}

/// Handle subscription classification request
fn handle_subscription_mock(prompt: &str) -> String {
    let merchant = extract_merchant_from_prompt(prompt);
    let m = merchant.to_uppercase();

    let (is_subscription, confidence, reason) = if m.contains("NETFLIX")
        || m.contains("SPOTIFY")
        || m.contains("HULU")
        || m.contains("HBO")
        || m.contains("DISNEY")
    {
        (true, 0.99, "streaming service")
    } else if m.contains("PLANET FITNESS") || m.contains("GYM") {
        (true, 0.95, "gym membership")
    } else if m.contains("HELLO FRESH") || m.contains("BLUE APRON") {
        (true, 0.95, "meal kit service")
    } else if m.contains("TRADER JOE") || m.contains("SAFEWAY") || m.contains("WHOLE FOODS") {
        (false, 0.99, "grocery store")
    } else if m.contains("STARBUCKS") || m.contains("MCDONALD") {
        (false, 0.95, "restaurant/cafe")
    } else {
        (false, 0.6, "retail purchase")
    };

    format!(
        r#"{{"is_subscription": {}, "confidence": {}, "reason": "{}"}}"#,
        is_subscription, confidence, reason
    )
}

/// Handle split recommendation request
fn handle_split_mock(prompt: &str) -> String {
    let merchant = extract_merchant_from_prompt(prompt);
    let m = merchant.to_uppercase();

    let (should_split, reason, categories) =
        if m.contains("TARGET") || m.contains("WALMART") || m.contains("COSTCO") {
            (
                true,
                "Multi-category retailer",
                vec!["Groceries", "Household", "Pharmacy"],
            )
        } else if m.contains("AMAZON") {
            (
                true,
                "Online marketplace with many categories",
                vec!["Shopping", "Electronics", "Books"],
            )
        } else {
            (false, "Single category merchant", vec![])
        };

    let cats_json = serde_json::to_string(&categories).unwrap();
    format!(
        r#"{{"should_split": {}, "reason": "{}", "typical_categories": {}}}"#,
        should_split, reason, cats_json
    )
}

/// Handle entity suggestion request
fn handle_entity_mock(prompt: &str) -> String {
    let merchant = extract_merchant_from_prompt(prompt);
    let m = merchant.to_uppercase();

    if m.contains("PET") || m.contains("PETCO") || m.contains("PETSMART") {
        r#"{"entity": "Rex", "confidence": 0.85, "reason": "Pet store purchase"}"#.to_string()
    } else if m.contains("TOY") || m.contains("GAME") {
        r#"{"entity": "Kids", "confidence": 0.7, "reason": "Toys/games purchase"}"#.to_string()
    } else {
        r#"{"entity": null, "confidence": 0.3, "reason": "General household purchase"}"#.to_string()
    }
}

/// Handle receipt parsing request (mock response)
fn handle_receipt_mock() -> String {
    r#"{
        "merchant": "Target",
        "date": "2024-01-15",
        "items": [
            {"description": "T-shirt", "amount": 25.00, "split_type": "item", "category_hint": "Shopping"},
            {"description": "Tax", "amount": 2.00, "split_type": "tax"}
        ],
        "subtotal": 25.00,
        "tax": 2.00,
        "total": 27.00
    }"#
    .to_string()
}

/// Mock merchant classification logic
/// This contains the hardcoded patterns for testing/dev purposes
fn classify_merchant_mock(prompt: &str) -> MerchantClassificationResponse {
    // Extract the merchant name from the prompt
    let merchant = extract_merchant_from_prompt(prompt);
    let m = merchant.to_uppercase();

    let (normalized, category) = if m.contains("NETFLIX") {
        ("Netflix", "streaming")
    } else if m.contains("HULU") {
        ("Hulu", "streaming")
    } else if m.contains("DISNEY") {
        ("Disney+", "streaming")
    } else if m.contains("HBO") {
        ("HBO Max", "streaming")
    } else if m.contains("PEACOCK") {
        ("Peacock", "streaming")
    } else if m.contains("PARAMOUNT") {
        ("Paramount+", "streaming")
    } else if m.contains("SPOTIFY") {
        ("Spotify", "music")
    } else if m.contains("APPLE MUSIC") {
        ("Apple Music", "music")
    } else if m.contains("PANDORA") {
        ("Pandora", "music")
    } else if m.contains("TIDAL") {
        ("Tidal", "music")
    } else if m.contains("ICLOUD") {
        ("iCloud", "cloud_storage")
    } else if m.contains("GOOGLE ONE") {
        ("Google One", "cloud_storage")
    } else if m.contains("DROPBOX") {
        ("Dropbox", "cloud_storage")
    } else if m.contains("AMAZON") || m.contains("AMZN") {
        ("Amazon", "shopping")
    } else if m.contains("TARGET") {
        ("Target", "shopping")
    } else if m.contains("WALMART") {
        ("Walmart", "shopping")
    } else if m.contains("STARBUCKS") {
        ("Starbucks", "food_delivery")
    } else if m.contains("DOORDASH") {
        ("DoorDash", "food_delivery")
    } else if m.contains("UBER EATS") {
        ("Uber Eats", "food_delivery")
    } else if m.contains("GRUBHUB") {
        ("Grubhub", "food_delivery")
    } else if m.contains("NYT") || m.contains("NEW YORK TIMES") {
        ("New York Times", "news")
    } else if m.contains("WSJ") || m.contains("WALL STREET") {
        ("Wall Street Journal", "news")
    } else if m.contains("PELOTON") {
        ("Peloton", "fitness")
    } else if m.contains("STRAVA") {
        ("Strava", "fitness")
    } else if m.contains("SHELL") || m.contains("CHEVRON") || m.contains("EXXON") {
        ("Gas Station", "utilities")
    } else if m.contains("ELECTRIC") || m.contains("POWER") || m.contains("ENERGY") {
        ("Electric Utility", "utilities")
    } else {
        // Clean up the name as best we can
        let cleaned: String = merchant
            .split(|c: char| !c.is_alphanumeric() && c != ' ')
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        return MerchantClassificationResponse {
            merchant: if cleaned.is_empty() {
                "Unknown".to_string()
            } else {
                cleaned
            },
            category: "other".to_string(),
        };
    };

    MerchantClassificationResponse {
        merchant: normalized.to_string(),
        category: category.to_string(),
    }
}

/// Extract merchant name from the Ollama prompt
fn extract_merchant_from_prompt(prompt: &str) -> String {
    // Current format: Classify this merchant: {{merchant}}
    if let Some(start) = prompt.find("Classify this merchant: ") {
        let after_start = &prompt[start + 24..];
        // Look for newline or end of string
        let end = after_start.find('\n').unwrap_or(after_start.len());
        return after_start[..end].trim().to_string();
    }
    // Legacy/Subscription format: Merchant: "SOMETHING"
    if let Some(start) = prompt.find("Merchant: \"") {
        let after_start = &prompt[start + 11..];
        if let Some(end) = after_start.find('"') {
            return after_start[..end].to_string();
        }
    }
    // Fallback: just use the whole prompt (risky if examples contain merchant names)
    prompt.to_string()
}

// Request/Response types for the mock server

#[derive(Debug, Serialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
struct ModelInfo {
    name: String,
    modified_at: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    #[allow(dead_code)]
    stream: bool,
}

#[derive(Debug, Serialize)]
struct GenerateResponse {
    model: String,
    response: String,
    done: bool,
}

#[derive(Debug, Serialize)]
struct MerchantClassificationResponse {
    merchant: String,
    category: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{AIBackend, OllamaBackend};

    #[tokio::test]
    async fn test_mock_server_health_check() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        assert!(client.health_check().await);
    }

    #[tokio::test]
    async fn test_mock_server_classify_netflix() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client.classify_merchant("NETFLIX.COM*12345").await.unwrap();
        assert_eq!(result.merchant, "Netflix");
        assert_eq!(result.category, "streaming");
    }

    #[tokio::test]
    async fn test_mock_server_classify_spotify() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client.classify_merchant("SPOTIFY PREMIUM").await.unwrap();
        assert_eq!(result.merchant, "Spotify");
        assert_eq!(result.category, "music");
    }

    #[tokio::test]
    async fn test_mock_server_classify_unknown() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client
            .classify_merchant("RANDOM MERCHANT XYZ123")
            .await
            .unwrap();
        assert_eq!(result.category, "other");
    }

    #[tokio::test]
    async fn test_mock_server_normalize_merchant() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client
            .normalize_merchant("NETFLIX.COM*12345ABC", None)
            .await
            .unwrap();
        assert_eq!(result, "Netflix");
    }

    #[tokio::test]
    async fn test_mock_server_normalize_trader_joes() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client
            .normalize_merchant("TRADER JOE'S #456", Some("Groceries"))
            .await
            .unwrap();
        assert_eq!(result, "Trader Joe's");
    }

    #[tokio::test]
    async fn test_mock_server_is_subscription_streaming() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client.is_subscription_service("NETFLIX").await.unwrap();
        assert!(result.is_subscription);
        assert!(result.confidence > 0.9);
    }

    #[tokio::test]
    async fn test_mock_server_is_subscription_grocery() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client
            .is_subscription_service("TRADER JOE'S")
            .await
            .unwrap();
        assert!(!result.is_subscription);
        assert!(result.confidence > 0.9);
    }

    #[tokio::test]
    async fn test_mock_server_should_split_target() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client.should_suggest_split("TARGET").await.unwrap();
        assert!(result.should_split);
        assert!(!result.typical_categories.is_empty());
    }

    #[tokio::test]
    async fn test_mock_server_should_split_netflix() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let result = client.should_suggest_split("NETFLIX").await.unwrap();
        assert!(!result.should_split);
    }

    #[tokio::test]
    async fn test_mock_server_suggest_entity_pet_store() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let entities = vec!["Rex".to_string(), "Kids".to_string()];
        let result = client
            .suggest_entity("PETCO", "Pets", &entities)
            .await
            .unwrap();
        assert_eq!(result, Some("Rex".to_string()));
    }

    #[tokio::test]
    async fn test_mock_server_suggest_entity_general() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let entities = vec!["Rex".to_string(), "Kids".to_string()];
        let result = client
            .suggest_entity("SAFEWAY", "Groceries", &entities)
            .await
            .unwrap();
        assert_eq!(result, None); // Low confidence, returns None
    }

    #[tokio::test]
    async fn test_mock_server_suggest_entity_empty_list() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        let entities: Vec<String> = vec![];
        let result = client
            .suggest_entity("PETCO", "Pets", &entities)
            .await
            .unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_mock_server_parse_receipt() {
        let server = MockOllamaServer::start().await;
        let client = OllamaBackend::new(&server.url(), "test-model");

        // Dummy image data
        let image_data = b"fake image data";
        let result = client.parse_receipt(image_data, None).await.unwrap();

        assert_eq!(result.merchant, Some("Target".to_string()));
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.total, Some(27.00));
    }

    #[tokio::test]
    async fn test_ollama_client_model_and_host() {
        let client = OllamaBackend::new("http://localhost:11434", "llama3.2");
        assert_eq!(client.model(), "llama3.2");
        assert_eq!(client.host(), "http://localhost:11434");
    }

    #[tokio::test]
    async fn test_ollama_client_from_env_not_set() {
        // When OLLAMA_HOST is not set, from_env returns None
        std::env::remove_var("OLLAMA_HOST");
        let client = OllamaBackend::from_env();
        assert!(client.is_none());
    }
}

//! Server API tests

use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64;
use hone_core::db::Database;
use hone_core::models::{
    Bank, EntityType, LocationType, NewEntity, NewLocation, NewMileageLog, NewTransactionSplit,
    NewTrip, PatternType, SplitType, TagSource,
};
use http_body_util::BodyExt;
use std::path::PathBuf;
use tempfile::TempDir;
use tower::ServiceExt;

fn setup_test_app() -> Router {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    create_router(db, None, config)
}

async fn get_body_json(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ========== Tag API Tests ==========

#[tokio::test]
async fn test_list_tags() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let tags = json.as_array().unwrap();
    assert!(!tags.is_empty());
}

#[tokio::test]
async fn test_get_tag_tree() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags/tree")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let tree = json.as_array().unwrap();
    assert!(!tree.is_empty());

    // Root tags should have "path" and "depth" fields
    let first = &tree[0];
    assert!(first.get("path").is_some());
    assert!(first.get("depth").is_some());
}

#[tokio::test]
async fn test_create_tag() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "TestTag",
        "color": "#ff0000"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tags")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "TestTag");
    assert_eq!(json["color"], "#ff0000");
}

#[tokio::test]
async fn test_create_child_tag() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Get the Transport tag ID
    let transport = db.get_tag_by_path("Transport").unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Use a name that isn't already seeded (Gas is now seeded)
    let body = serde_json::json!({
        "name": "TestChild",
        "parent_id": transport.id,
        "auto_patterns": "TEST_PATTERN"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tags")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "TestChild");
    assert_eq!(json["parent_id"], transport.id);
}

#[tokio::test]
async fn test_get_tag_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_tag() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let tag_id = db.create_tag("TestUpdate", None, None, None, None).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "name": "UpdatedName",
        "color": "#00ff00"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/tags/{}", tag_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "UpdatedName");
    assert_eq!(json["color"], "#00ff00");
}

#[tokio::test]
async fn test_delete_tag() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let tag_id = db.create_tag("ToDelete", None, None, None, None).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/tags/{}", tag_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["deleted_tag_id"], tag_id);
}

// ========== Tag Rules API Tests ==========

#[tokio::test]
async fn test_list_tag_rules_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let rules = json.as_array().unwrap();
    assert!(rules.is_empty());
}

#[tokio::test]
async fn test_create_tag_rule() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let transport = db.get_tag_by_path("Transport").unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "tag_id": transport.id,
        "pattern": "SHELL|CHEVRON",
        "pattern_type": "contains",
        "priority": 10
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rules")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["pattern"], "SHELL|CHEVRON");
    assert_eq!(json["priority"], 10);
    assert_eq!(json["tag_name"], "Transport");
}

#[tokio::test]
async fn test_test_rules() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let transport = db.get_tag_by_path("Transport").unwrap().unwrap();
    db.create_tag_rule(transport.id, "SHELL|CHEVRON", PatternType::Contains, 10)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "description": "SHELL GAS STATION"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rules/test")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let matches = json["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["tag_name"], "Transport");
}

// ========== Transaction Tagging API Tests ==========

#[tokio::test]
async fn test_transaction_tags_flow() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create an account and transaction
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "NETFLIX.COM".to_string(),
        amount: -15.99,
        category: None,
        import_hash: "test_hash_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();
    let groceries = db.get_tag_by_path("Groceries").unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Add a tag to the transaction
    let body = serde_json::json!({
        "tag_id": groceries.id,
        "source": "manual"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/tags", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Get tags for the transaction
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/tags", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let tags = json.as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["tag_id"], groceries.id);

    // Remove the tag
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/transactions/{}/tags/{}", tx_id, groceries.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Reports API Tests ==========

#[tokio::test]
async fn test_report_by_tag_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-tag")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let spending = json.as_array().unwrap();
    assert!(spending.is_empty());
}

#[tokio::test]
async fn test_report_by_tag_with_data() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create account, transaction, and tag it
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "SAFEWAY".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "report_test_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();
    let groceries = db.get_tag_by_path("Groceries").unwrap().unwrap();
    db.add_transaction_tag(tx_id, groceries.id, TagSource::Manual, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-tag")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let spending = json.as_array().unwrap();
    assert!(!spending.is_empty());

    // Find Groceries in the results
    let groceries_spending = spending.iter().find(|s| s["tag_name"] == "Groceries");
    assert!(groceries_spending.is_some());
    // Amount is stored as negative (expense), so check absolute value
    let direct_amount = groceries_spending.unwrap()["direct_amount"]
        .as_f64()
        .unwrap();
    assert!((direct_amount.abs() - 50.0).abs() < 0.01);
}

#[tokio::test]
async fn test_report_by_tag_date_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-tag?from=2024-01-01&to=2024-12-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_report_by_tag_invalid_date() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-tag?from=invalid-date")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========== Dashboard API Tests ==========

#[tokio::test]
async fn test_get_dashboard() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total_transactions").is_some());
    assert!(json.get("total_accounts").is_some());
    assert!(json.get("active_subscriptions").is_some());
}

// ========== Authentication Tests ==========

#[tokio::test]
async fn test_auth_required() {
    let db = Database::in_memory().unwrap();
    let config = ServerConfig {
        require_auth: true, // Auth required
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should get 401 without CF header
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_with_header() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: true,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags")
                .header("cf-access-authenticated-user-email", "test@example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== New Reports API Tests ==========

#[tokio::test]
async fn test_report_spending_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total").is_some());
    assert!(json.get("categories").is_some());
}

#[tokio::test]
async fn test_report_spending_with_period() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?period=last-month")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_report_spending_with_custom_dates() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?from=2024-01-01&to=2024-12-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_report_trends_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("granularity").is_some());
    assert!(json.get("data").is_some());
}

#[tokio::test]
async fn test_report_trends_weekly() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends?granularity=weekly")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["granularity"], "weekly");
}

#[tokio::test]
async fn test_report_merchants_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/merchants")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("merchants").is_some());
}

#[tokio::test]
async fn test_report_merchants_with_limit() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/merchants?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["limit"], 5);
}

#[tokio::test]
async fn test_report_subscriptions() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/subscriptions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total_monthly").is_some());
    assert!(json.get("active_count").is_some());
}

#[tokio::test]
async fn test_report_savings() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/savings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total_savings").is_some());
    assert!(json.get("cancelled_count").is_some());
}

#[tokio::test]
async fn test_cancel_subscription() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a subscription
    {
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
             VALUES ('Netflix', 15.99, 'monthly', '2024-01-01', '2024-06-01', 'active')",
            [],
        )
        .unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subscriptions/1/cancel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
    assert_eq!(json["id"], 1);
}

#[tokio::test]
async fn test_cancel_subscription_nonexistent() {
    // Note: Currently the API returns 200 OK even for non-existent subscriptions
    // since the UPDATE just does nothing. This could be improved in the future.
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subscriptions/9999/cancel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Entity API Tests ==========

#[tokio::test]
async fn test_list_entities_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/entities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let entities = json.as_array().unwrap();
    assert!(entities.is_empty());
}

#[tokio::test]
async fn test_create_entity() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "Marcus",
        "entity_type": "person",
        "icon": "👤"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/entities")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Marcus");
    assert_eq!(json["entity_type"], "person");
    assert_eq!(json["icon"], "👤");
}

#[tokio::test]
async fn test_create_entity_pet() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "Rex",
        "entity_type": "pet",
        "icon": "🐕",
        "color": "#8B4513"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/entities")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Rex");
    assert_eq!(json["entity_type"], "pet");
    assert_eq!(json["color"], "#8B4513");
}

#[tokio::test]
async fn test_create_entity_invalid_type() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "Test",
        "entity_type": "invalid"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/entities")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let new_entity = NewEntity {
        name: "Sarah".to_string(),
        entity_type: EntityType::Person,
        icon: Some("👩".to_string()),
        color: None,
    };
    let entity_id = db.create_entity(&new_entity).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/entities/{}", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Sarah");
    assert_eq!(json["entity_type"], "person");
}

#[tokio::test]
async fn test_get_entity_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/entities/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let new_entity = NewEntity {
        name: "OldName".to_string(),
        entity_type: EntityType::Person,
        icon: None,
        color: None,
    };
    let entity_id = db.create_entity(&new_entity).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "name": "NewName",
        "icon": "👤"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/entities/{}", entity_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "NewName");
    assert_eq!(json["icon"], "👤");
}

#[tokio::test]
async fn test_delete_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let new_entity = NewEntity {
        name: "ToDelete".to_string(),
        entity_type: EntityType::Person,
        icon: None,
        color: None,
    };
    let entity_id = db.create_entity(&new_entity).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/entities/{}", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_archive_and_unarchive_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let new_entity = NewEntity {
        name: "ToArchive".to_string(),
        entity_type: EntityType::Pet,
        icon: Some("🐱".to_string()),
        color: None,
    };
    let entity_id = db.create_entity(&new_entity).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Archive
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/entities/{}/archive", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["archived"], true);

    // Unarchive
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/entities/{}/unarchive", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["archived"], false);
}

#[tokio::test]
async fn test_list_entities_by_type() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create entities of different types
    db.create_entity(&NewEntity {
        name: "Person1".to_string(),
        entity_type: EntityType::Person,
        icon: None,
        color: None,
    })
    .unwrap();
    db.create_entity(&NewEntity {
        name: "Pet1".to_string(),
        entity_type: EntityType::Pet,
        icon: None,
        color: None,
    })
    .unwrap();
    db.create_entity(&NewEntity {
        name: "Pet2".to_string(),
        entity_type: EntityType::Pet,
        icon: None,
        color: None,
    })
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Filter by pet type
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/entities?entity_type=pet")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let entities = json.as_array().unwrap();
    assert_eq!(entities.len(), 2);
    for entity in entities {
        assert_eq!(entity["entity_type"], "pet");
    }
}

// ========== Location API Tests ==========

#[tokio::test]
async fn test_list_locations_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/locations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let locations = json.as_array().unwrap();
    assert!(locations.is_empty());
}

#[tokio::test]
async fn test_create_location() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "Home",
        "city": "San Francisco",
        "state": "CA",
        "country": "US",
        "location_type": "home"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/locations")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Home");
    assert_eq!(json["city"], "San Francisco");
    assert_eq!(json["location_type"], "home");
}

#[tokio::test]
async fn test_get_location() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let loc_id = db
        .create_location(&NewLocation {
            name: Some("Office".to_string()),
            address: Some("123 Main St".to_string()),
            city: Some("San Francisco".to_string()),
            state: Some("CA".to_string()),
            country: Some("US".to_string()),
            latitude: Some(37.7749),
            longitude: Some(-122.4194),
            location_type: Some(LocationType::Work),
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/locations/{}", loc_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Office");
    assert_eq!(json["latitude"], 37.7749);
}

#[tokio::test]
async fn test_get_location_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/locations/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_location() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let loc_id = db
        .create_location(&NewLocation {
            name: Some("To Delete".to_string()),
            address: None,
            city: None,
            state: None,
            country: None,
            latitude: None,
            longitude: None,
            location_type: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/locations/{}", loc_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Transaction Splits API Tests ==========

#[tokio::test]
async fn test_get_transaction_splits_empty() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "TARGET".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "split_test_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/splits", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let splits = json.as_array().unwrap();
    assert!(splits.is_empty());
}

#[tokio::test]
async fn test_create_transaction_split() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "TARGET".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "split_test_2".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "amount": 25.00,
        "description": "T-Shirt",
        "split_type": "item"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/splits", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["amount"], 25.00);
    assert_eq!(json["description"], "T-Shirt");
    assert_eq!(json["split_type"], "item");
}

#[tokio::test]
async fn test_create_split_with_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "PETCO".to_string(),
        amount: -75.00,
        category: None,
        import_hash: "split_test_3".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Rex".to_string(),
            entity_type: EntityType::Pet,
            icon: Some("🐕".to_string()),
            color: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "amount": 50.00,
        "description": "Dog Food",
        "split_type": "item",
        "entity_id": entity_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/splits", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["entity_id"], entity_id);
}

#[tokio::test]
async fn test_delete_split() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "STORE".to_string(),
        amount: -30.00,
        category: None,
        import_hash: "split_test_4".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let split_id = db
        .create_split(&NewTransactionSplit {
            transaction_id: tx_id,
            amount: 15.00,
            description: Some("Item".to_string()),
            split_type: SplitType::Item,
            entity_id: None,
            purchaser_id: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/splits/{}", split_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Trips API Tests ==========

#[tokio::test]
async fn test_list_trips_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/trips")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let trips = json.as_array().unwrap();
    assert!(trips.is_empty());
}

#[tokio::test]
async fn test_create_trip() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "Paris Vacation",
        "start_date": "2024-06-01",
        "end_date": "2024-06-15"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/trips")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Paris Vacation");
    assert_eq!(json["start_date"], "2024-06-01");
}

#[tokio::test]
async fn test_get_trip() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let trip_id = db
        .create_trip(&NewTrip {
            name: "Business Trip".to_string(),
            description: Some("Conference in NYC".to_string()),
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
            end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 3, 5).unwrap()),
            location_id: None,
            budget: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/trips/{}", trip_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Business Trip");
    assert_eq!(json["description"], "Conference in NYC");
}

#[tokio::test]
async fn test_update_trip() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let trip_id = db
        .create_trip(&NewTrip {
            name: "Old Name".to_string(),
            description: None,
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 4, 1).unwrap()),
            end_date: None,
            location_id: None,
            budget: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "name": "New Name",
        "description": "Updated description"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/trips/{}", trip_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "New Name");
    assert_eq!(json["description"], "Updated description");
}

#[tokio::test]
async fn test_delete_trip() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let trip_id = db
        .create_trip(&NewTrip {
            name: "To Delete".to_string(),
            description: None,
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 5, 1).unwrap()),
            end_date: None,
            location_id: None,
            budget: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/trips/{}", trip_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // DELETE can return 200 or 204
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_archive_trip() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let trip_id = db
        .create_trip(&NewTrip {
            name: "Past Trip".to_string(),
            description: None,
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            end_date: Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 7).unwrap()),
            location_id: None,
            budget: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/trips/{}/archive", trip_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["archived"], true);
}

// ========== Mileage Log API Tests ==========

#[tokio::test]
async fn test_list_mileage_logs_empty() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Honda Civic".to_string(),
            entity_type: EntityType::Vehicle,
            icon: Some("🚗".to_string()),
            color: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/entities/{}/mileage", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let logs = json.as_array().unwrap();
    assert!(logs.is_empty());
}

#[tokio::test]
async fn test_create_mileage_log() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Toyota Camry".to_string(),
            entity_type: EntityType::Vehicle,
            icon: None,
            color: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "date": "2024-01-15",
        "odometer": 50000,
        "note": "Oil change"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/entities/{}/mileage", entity_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["odometer"].as_f64().unwrap(), 50000.0);
    assert_eq!(json["note"], "Oil change");
}

#[tokio::test]
async fn test_get_vehicle_total_miles() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Ford F-150".to_string(),
            entity_type: EntityType::Vehicle,
            icon: None,
            color: None,
        })
        .unwrap();

    db.create_mileage_log(&NewMileageLog {
        entity_id,
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        odometer: 10000.0,
        note: None,
    })
    .unwrap();

    db.create_mileage_log(&NewMileageLog {
        entity_id,
        date: chrono::NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
        odometer: 15000.0,
        note: None,
    })
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/entities/{}/miles", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["total_miles"].as_f64().unwrap(), 5000.0);
}

// ========== Report Endpoints Tests ==========

#[tokio::test]
async fn test_report_by_entity_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-entity")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Response can be empty array or object with empty data
    if let Some(arr) = json.as_array() {
        assert!(arr.is_empty());
    }
}

#[tokio::test]
async fn test_report_by_location_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-location")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Response can be empty array or object with empty data
    if let Some(arr) = json.as_array() {
        assert!(arr.is_empty());
    }
}

#[tokio::test]
async fn test_report_vehicle_costs() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Test Car".to_string(),
            entity_type: EntityType::Vehicle,
            icon: None,
            color: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/reports/vehicle-costs/{}", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_report_property_expenses() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Main House".to_string(),
            entity_type: EntityType::Property,
            icon: None,
            color: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/reports/property-expenses/{}", entity_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Account API Tests ==========

#[tokio::test]
async fn test_list_accounts() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let accounts = json.as_array().unwrap();
    assert!(accounts.is_empty());
}

#[tokio::test]
async fn test_create_account() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "My Chase Account",
        "bank": "chase"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["name"], "My Chase Account");
    assert_eq!(json["bank"], "chase");
}

// ========== Alerts and Subscriptions API Tests ==========

#[tokio::test]
async fn test_list_alerts() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/alerts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_subscriptions() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/subscriptions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_transactions() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_transactions_with_pagination() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    for i in 0..5 {
        let tx = hone_core::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, i + 1).unwrap(),
            description: format!("TX {}", i),
            amount: -(10.0 * (i + 1) as f64),
            category: None,
            import_hash: format!("pagination_test_{}", i),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?limit=2&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Response can be array or object with data field
    if let Some(arr) = json.as_array() {
        assert_eq!(arr.len(), 2);
    } else if let Some(data) = json.get("data") {
        let arr = data.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }
}

#[tokio::test]
async fn test_run_detection() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "kind": "all"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("subscriptions_found").is_some());
}

#[tokio::test]
async fn test_audit_log() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/audit")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_dismiss_alert() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create an alert
    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO alerts (type, message, dismissed) VALUES ('zombie', 'Test alert', 0)",
        [],
    )
    .unwrap();
    let alert_id: i64 = conn
        .query_row(
            "SELECT id FROM alerts WHERE message = 'Test alert'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{}/dismiss", alert_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_restore_alert() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a dismissed alert
    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO alerts (type, message, dismissed) VALUES ('zombie', 'Test alert', 1)",
        [],
    )
    .unwrap();
    let alert_id: i64 = conn
        .query_row(
            "SELECT id FROM alerts WHERE message = 'Test alert'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{}/restore", alert_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify alert is now undismissed
    let conn = db.conn().unwrap();
    let dismissed: bool = conn
        .query_row(
            "SELECT dismissed FROM alerts WHERE id = ?",
            [alert_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!dismissed);
}

#[tokio::test]
async fn test_acknowledge_subscription() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a subscription
    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
         VALUES ('Netflix', 15.99, 'monthly', '2024-01-01', '2024-06-01', 'zombie')",
        [],
    )
    .unwrap();
    let sub_id: i64 = conn
        .query_row(
            "SELECT id FROM subscriptions WHERE merchant = 'Netflix'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/subscriptions/{}/acknowledge", sub_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_trip_spending() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let trip_id = db
        .create_trip(&NewTrip {
            name: "Test Trip".to_string(),
            description: None,
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 7).unwrap()),
            location_id: None,
            budget: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/trips/{}/spending", trip_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_trip_transactions() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let trip_id = db
        .create_trip(&NewTrip {
            name: "Test Trip 2".to_string(),
            description: None,
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 2, 1).unwrap()),
            end_date: None,
            location_id: None,
            budget: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/trips/{}/transactions", trip_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_delete_mileage_log() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let entity_id = db
        .create_entity(&NewEntity {
            name: "Test Vehicle".to_string(),
            entity_type: EntityType::Vehicle,
            icon: None,
            color: None,
        })
        .unwrap();

    let mileage_id = db
        .create_mileage_log(&NewMileageLog {
            entity_id,
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            odometer: 25000.0,
            note: Some("To delete".to_string()),
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/mileage/{}", mileage_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NO_CONTENT);
}

// ========== Receipt Workflow Tests ==========

#[tokio::test]
async fn test_list_pending_receipts() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create some receipts with different statuses
    let pending1 = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/receipts/test1.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(25.00),
        receipt_merchant: Some("Store A".to_string()),
        content_hash: Some("hash1".to_string()),
    };

    let matched = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/receipts/test2.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Matched,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: Some("Store B".to_string()),
        content_hash: Some("hash2".to_string()),
    };

    db.create_receipt_full(&pending1).unwrap();
    db.create_receipt_full(&matched).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Default (no status filter) returns pending
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/receipts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let receipts = json.as_array().unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["status"], "pending");
}

#[tokio::test]
async fn test_list_receipts_by_status() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let pending = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(25.00),
        receipt_merchant: None,
        content_hash: Some("status_test1".to_string()),
    };

    let orphaned = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::Orphaned,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(100.00),
        receipt_merchant: None,
        content_hash: Some("status_test2".to_string()),
    };

    db.create_receipt_full(&pending).unwrap();
    db.create_receipt_full(&orphaned).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Query for orphaned
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/receipts?status=orphaned")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let receipts = json.as_array().unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["status"], "orphaned");
}

#[tokio::test]
async fn test_link_receipt_to_transaction() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "TARGET".to_string(),
        amount: -87.43,
        category: None,
        import_hash: "link_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create pending receipt
    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/receipts/target.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(87.43),
        receipt_merchant: Some("Target".to_string()),
        content_hash: Some("link_test_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    let body = serde_json::json!({
        "transaction_id": tx_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/receipts/{}/link", receipt_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["status"], "matched");
    assert_eq!(json["transaction_id"], tx_id);

    // Verify receipt is now linked
    let linked = db.get_receipt(receipt_id).unwrap().unwrap();
    assert_eq!(linked.transaction_id, Some(tx_id));
    assert_eq!(linked.status, hone_core::models::ReceiptStatus::Matched);
}

#[tokio::test]
async fn test_link_receipt_not_found() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "transaction_id": 999
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/receipts/999/link")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_receipt_status() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("status_update_test".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    let body = serde_json::json!({
        "status": "manual_review"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/receipts/{}/status", receipt_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["status"], "manual_review");

    // Verify in DB
    let updated = db.get_receipt(receipt_id).unwrap().unwrap();
    assert_eq!(
        updated.status,
        hone_core::models::ReceiptStatus::ManualReview
    );
}

#[tokio::test]
async fn test_update_receipt_status_invalid() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("invalid_status_test".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "status": "invalid_status"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/receipts/{}/status", receipt_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_link_already_matched_receipt() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "STORE".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "already_matched_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create already matched receipt
    let receipt = hone_core::models::NewReceipt {
        transaction_id: Some(tx_id),
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::Matched,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("already_matched_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "transaction_id": tx_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/receipts/{}/link", receipt_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========== Suggestions API Tests ==========

#[tokio::test]
async fn test_suggest_entity_no_entities() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "PETCO ANIMAL SUPPLIES".to_string(),
        amount: -75.00,
        category: None,
        import_hash: "suggest_entity_test_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/suggest-entity", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["transaction_id"], tx_id);
    assert!(json["suggested_entity"].is_null());
    assert!(json["available_entities"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_suggest_entity_with_entities() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create some entities
    db.create_entity(&NewEntity {
        name: "Rex".to_string(),
        entity_type: EntityType::Pet,
        icon: Some("🐕".to_string()),
        color: None,
    })
    .unwrap();
    db.create_entity(&NewEntity {
        name: "Sarah".to_string(),
        entity_type: EntityType::Person,
        icon: None,
        color: None,
    })
    .unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "PETCO ANIMAL SUPPLIES".to_string(),
        amount: -75.00,
        category: None,
        import_hash: "suggest_entity_test_2".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/suggest-entity", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["transaction_id"], tx_id);
    // Without Ollama, suggested_entity will be null
    // But available_entities should contain our entities
    let available = json["available_entities"].as_array().unwrap();
    assert_eq!(available.len(), 2);
    assert!(available.iter().any(|e| e == "Rex"));
    assert!(available.iter().any(|e| e == "Sarah"));
}

#[tokio::test]
async fn test_suggest_entity_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions/99999/suggest-entity")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_suggest_split() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction at a multi-category store
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "TARGET STORE #1234".to_string(),
        amount: -150.00,
        category: None,
        import_hash: "suggest_split_test_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/suggest-split", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["transaction_id"], tx_id);
    assert_eq!(json["merchant"], "TARGET STORE #1234");
    // Without Ollama, recommendation will be null
}

#[tokio::test]
async fn test_suggest_split_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions/99999/suggest-split")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ========== Additional Receipt Tests ==========

#[tokio::test]
async fn test_get_receipt() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/receipts/test.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
        receipt_total: Some(42.50),
        receipt_merchant: Some("Grocery Store".to_string()),
        content_hash: Some("get_receipt_test".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/receipts/{}", receipt_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["id"], receipt_id);
    assert_eq!(json["receipt_total"], 42.50);
    assert_eq!(json["receipt_merchant"], "Grocery Store");
}

#[tokio::test]
async fn test_get_receipt_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/receipts/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_transaction_receipts() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "TARGET".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "tx_receipts_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create receipts linked to the transaction
    let receipt1 = hone_core::models::NewReceipt {
        transaction_id: Some(tx_id),
        image_path: Some("/receipts/r1.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Matched,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(100.00),
        receipt_merchant: None,
        content_hash: Some("tx_receipt_1".to_string()),
    };
    let receipt2 = hone_core::models::NewReceipt {
        transaction_id: Some(tx_id),
        image_path: Some("/receipts/r2.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Matched,
        role: hone_core::models::ReceiptRole::Supplementary,
        receipt_date: None,
        receipt_total: None,
        receipt_merchant: None,
        content_hash: Some("tx_receipt_2".to_string()),
    };
    db.create_receipt_full(&receipt1).unwrap();
    db.create_receipt_full(&receipt2).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/receipts", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let receipts = json.as_array().unwrap();
    assert_eq!(receipts.len(), 2);
}

#[tokio::test]
async fn test_delete_receipt() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None, // No actual file to delete
        image_data: None,
        status: hone_core::models::ReceiptStatus::Orphaned,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: None,
        receipt_merchant: None,
        content_hash: Some("delete_receipt_test".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/receipts/{}", receipt_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);

    // Verify deleted
    assert!(db.get_receipt(receipt_id).unwrap().is_none());
}

#[tokio::test]
async fn test_list_receipts_all_statuses() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create receipts with each status
    for (i, status) in [
        hone_core::models::ReceiptStatus::Pending,
        hone_core::models::ReceiptStatus::Matched,
        hone_core::models::ReceiptStatus::ManualReview,
        hone_core::models::ReceiptStatus::Orphaned,
    ]
    .iter()
    .enumerate()
    {
        let receipt = hone_core::models::NewReceipt {
            transaction_id: None,
            image_path: None,
            image_data: None,
            status: status.clone(),
            role: hone_core::models::ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: Some((i + 1) as f64 * 10.0),
            receipt_merchant: None,
            content_hash: Some(format!("all_status_test_{}", i)),
        };
        db.create_receipt_full(&receipt).unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Test each status filter
    for status in ["pending", "matched", "manual_review", "orphaned"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/receipts?status={}", status))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = get_body_json(response).await;
        let receipts = json.as_array().unwrap();
        assert_eq!(
            receipts.len(),
            1,
            "Expected 1 receipt with status {}",
            status
        );
        assert_eq!(receipts[0]["status"], status);
    }
}

#[tokio::test]
async fn test_link_receipt_manual_review_allowed() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "STORE".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "manual_review_link_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create receipt in manual_review status
    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::ManualReview,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("manual_review_link".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "transaction_id": tx_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/receipts/{}/link", receipt_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // manual_review status should be allowed to link
    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["status"], "matched");
}

// ========== Detection Tests with Data ==========

#[tokio::test]
async fn test_detection_with_recurring_transactions() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

    // Create recurring transactions that should be detected as a subscription
    // Netflix charges for 4 consecutive months
    for month in 1..=4 {
        let tx = hone_core::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2024, month, 15).unwrap(),
            description: "NETFLIX.COM".to_string(),
            amount: -15.99,
            category: None,
            import_hash: format!("netflix_recurring_{}", month),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "kind": "all"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Should detect the Netflix subscription
    assert!(json["subscriptions_found"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_detection_zombies_only() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "kind": "zombies"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("zombies_detected").is_some());
}

#[tokio::test]
async fn test_detection_increases_only() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "kind": "increases"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("price_increases_detected").is_some());
}

#[tokio::test]
async fn test_detection_duplicates_only() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "kind": "duplicates"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("duplicates_detected").is_some());
}

#[tokio::test]
async fn test_detection_with_price_increase() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

    // Create subscription-like transactions with a price increase
    // First 4 months at $9.99, then jumps to $14.99 for 2 months
    // (Need enough data points for the subscription detection algorithm)
    for month in 1..=4 {
        let tx = hone_core::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2024, month, 1).unwrap(),
            description: "STREAMING SERVICE INC".to_string(),
            amount: -9.99,
            category: None,
            import_hash: format!("price_increase_test_{}", month),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
    }

    // Price increase in months 5-6
    for month in 5..=6 {
        let tx = hone_core::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2024, month, 1).unwrap(),
            description: "STREAMING SERVICE INC".to_string(),
            amount: -14.99,
            category: None,
            import_hash: format!("price_increase_test_{}", month),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "kind": "all"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Detection runs successfully - may or may not find subscriptions depending on algorithm thresholds
    assert!(json.get("subscriptions_found").is_some());
    assert!(json.get("price_increases_detected").is_some());
}

#[tokio::test]
async fn test_detection_empty_body() {
    let app = setup_test_app();

    // Empty body should use defaults
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Should run "all" detection by default
    assert!(json.get("subscriptions_found").is_some());
    assert!(json.get("zombies_detected").is_some());
    assert!(json.get("price_increases_detected").is_some());
    assert!(json.get("duplicates_detected").is_some());
}

#[tokio::test]
async fn test_detection_invalid_kind_defaults_to_all() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "kind": "invalid_kind"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Invalid kind should fall through to "all" detection
    assert!(json.get("subscriptions_found").is_some());
}

// ========== Additional Transaction Tests ==========

#[tokio::test]
async fn test_get_transaction_tags() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "GROCERY STORE".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "get_tx_tags_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Add a tag to the transaction
    let tags = db.list_tags().unwrap();
    let groceries_tag = tags.iter().find(|t| t.name == "Groceries").unwrap();
    db.add_transaction_tag(
        tx_id,
        groceries_tag.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/tags", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let tags = json.as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["tag_name"], "Groceries");
}

#[tokio::test]
async fn test_add_transaction_tag_with_source() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "AMAZON".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "add_tag_source_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let tags = db.list_tags().unwrap();
    let shopping_tag = tags.iter().find(|t| t.name == "Shopping").unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // Test with "rule" source
    let body = serde_json::json!({
        "tag_id": shopping_tag.id,
        "source": "rule",
        "confidence": 0.95
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/tags", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify the tag was added with correct source
    let tx_tags = db.get_transaction_tags_with_details(tx_id).unwrap();
    assert_eq!(tx_tags.len(), 1);
    assert_eq!(tx_tags[0].source, hone_core::models::TagSource::Rule);
}

#[tokio::test]
async fn test_remove_transaction_tag() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction with a tag
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "GAS STATION".to_string(),
        amount: -40.00,
        category: None,
        import_hash: "remove_tag_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let tags = db.list_tags().unwrap();
    let transport_tag = tags.iter().find(|t| t.name == "Transport").unwrap();
    db.add_transaction_tag(
        tx_id,
        transport_tag.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/transactions/{}/tags/{}",
                    tx_id, transport_tag.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify tag was removed
    let tx_tags = db.get_transaction_tags_with_details(tx_id).unwrap();
    assert!(tx_tags.is_empty());
}

#[tokio::test]
async fn test_assign_transaction_to_trip() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
        description: "HOTEL BOOKING".to_string(),
        amount: -250.00,
        category: None,
        import_hash: "assign_trip_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create a trip
    let trip = hone_core::models::NewTrip {
        name: "Summer Vacation".to_string(),
        description: None,
        start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 6, 10).unwrap()),
        end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 6, 20).unwrap()),
        location_id: None,
        budget: Some(1000.00),
    };
    let trip_id = db.create_trip(&trip).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "trip_id": trip_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/trip", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_unassign_transaction_from_trip() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
        description: "RESTAURANT".to_string(),
        amount: -75.00,
        category: None,
        import_hash: "unassign_trip_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create a trip and assign transaction
    let trip = hone_core::models::NewTrip {
        name: "Business Trip".to_string(),
        description: None,
        start_date: None,
        end_date: None,
        location_id: None,
        budget: None,
    };
    let trip_id = db.create_trip(&trip).unwrap();
    db.assign_transaction_to_trip(tx_id, Some(trip_id)).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Unassign by setting trip_id to null
    let body = serde_json::json!({
        "trip_id": null
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/trip", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_update_transaction_location() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "COFFEE SHOP".to_string(),
        amount: -5.00,
        category: None,
        import_hash: "update_location_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create locations
    let purchase_loc = hone_core::models::NewLocation {
        name: Some("Downtown Coffee".to_string()),
        address: Some("123 Main St".to_string()),
        city: Some("Seattle".to_string()),
        state: Some("WA".to_string()),
        country: None,
        latitude: None,
        longitude: None,
        location_type: None,
    };
    let purchase_loc_id = db.create_location(&purchase_loc).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "purchase_location_id": purchase_loc_id,
        "vendor_location_id": null
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/location", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_list_transactions_with_account_filter() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create two accounts
    let account1_id = db.upsert_account("Checking", Bank::Chase, None).unwrap();
    let account2_id = db.upsert_account("Credit", Bank::Amex, None).unwrap();

    // Create transactions in each account
    for i in 1..=3 {
        let tx = hone_core::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, i as u32).unwrap(),
            description: format!("TX Checking {}", i),
            amount: -(i as f64 * 10.0),
            category: None,
            import_hash: format!("account_filter_checking_{}", i),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account1_id, &tx).unwrap();
    }

    for i in 1..=2 {
        let tx = hone_core::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, i as u32).unwrap(),
            description: format!("TX Credit {}", i),
            amount: -(i as f64 * 20.0),
            category: None,
            import_hash: format!("account_filter_credit_{}", i),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account2_id, &tx).unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Filter by account1
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions?account_id={}", account1_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let transactions = json["transactions"].as_array().unwrap();
    assert_eq!(transactions.len(), 3);
}

// ========== Additional Split Tests ==========

#[tokio::test]
async fn test_get_split() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "COSTCO".to_string(),
        amount: -200.00,
        category: None,
        import_hash: "get_split_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create a split
    let split = hone_core::models::NewTransactionSplit {
        transaction_id: tx_id,
        amount: 50.00,
        description: Some("Groceries".to_string()),
        split_type: hone_core::models::SplitType::Item,
        entity_id: None,
        purchaser_id: None,
    };
    let split_id = db.create_split(&split).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/splits/{}", split_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["id"], split_id);
    assert_eq!(json["amount"], 50.00);
    assert_eq!(json["description"], "Groceries");
}

#[tokio::test]
async fn test_get_split_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/splits/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_split() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "RESTAURANT".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "update_split_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create a split
    let split = hone_core::models::NewTransactionSplit {
        transaction_id: tx_id,
        amount: 80.00,
        description: Some("Food".to_string()),
        split_type: hone_core::models::SplitType::Item,
        entity_id: None,
        purchaser_id: None,
    };
    let split_id = db.create_split(&split).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "amount": 75.00,
        "description": "Dinner",
        "split_type": "item"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/splits/{}", split_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["amount"], 75.00);
    assert_eq!(json["description"], "Dinner");
}

#[tokio::test]
async fn test_update_split_not_found() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "amount": 50.00
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/splits/99999")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_split_with_different_types() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "RESTAURANT".to_string(),
        amount: -120.00,
        category: None,
        import_hash: "split_types_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Test creating splits with different types
    for (split_type, amount) in [
        ("item", 80.00),
        ("tax", 8.00),
        ("tip", 20.00),
        ("fee", 2.00),
    ] {
        let body = serde_json::json!({
            "amount": amount,
            "description": format!("Test {}", split_type),
            "split_type": split_type
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/transactions/{}/splits", tx_id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Failed for split_type: {}",
            split_type
        );

        let json = get_body_json(response).await;
        assert_eq!(json["split_type"], split_type);
    }
}

#[tokio::test]
async fn test_create_split_invalid_type() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "STORE".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "invalid_split_type_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "amount": 50.00,
        "split_type": "invalid_type"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/splits", tx_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_split_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/splits/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_transaction_splits_with_data() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "TARGET".to_string(),
        amount: -150.00,
        category: None,
        import_hash: "splits_with_data_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Create multiple splits
    for i in 1..=3 {
        let split = hone_core::models::NewTransactionSplit {
            transaction_id: tx_id,
            amount: i as f64 * 25.0,
            description: Some(format!("Item {}", i)),
            split_type: hone_core::models::SplitType::Item,
            entity_id: None,
            purchaser_id: None,
        };
        db.create_split(&split).unwrap();
    }

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/transactions/{}/splits", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let splits = json.as_array().unwrap();
    assert_eq!(splits.len(), 3);
}

// ========== Additional Account Tests ==========

#[tokio::test]
async fn test_create_account_different_banks() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Test creating accounts with different banks
    for bank in ["chase", "amex", "bofa", "capitalone"] {
        let body = serde_json::json!({
            "name": format!("Test {}", bank),
            "bank": bank
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/accounts")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Failed for bank: {}",
            bank
        );

        let json = get_body_json(response).await;
        assert_eq!(json["bank"], bank);
    }
}

#[tokio::test]
async fn test_create_account_invalid_bank() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "Test Account",
        "bank": "invalid_bank"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========== Additional Receipt Upload Tests ==========

#[tokio::test]
async fn test_upload_receipt_no_data() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "STORE".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "upload_no_data_test".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Upload with empty body
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/transactions/{}/receipts", tx_id))
                .header("content-type", "application/octet-stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_upload_receipt_transaction_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/99999/receipts")
                .header("content-type", "application/octet-stream")
                .body(Body::from(vec![0xFF, 0xD8, 0xFF, 0xE0])) // Fake JPEG header
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_upload_pending_receipt_no_data() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/receipts")
                .header("content-type", "application/octet-stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_parse_receipt_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/receipts/99999/parse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_parse_receipt_no_image() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a receipt without an image
    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: None,
        receipt_merchant: None,
        content_hash: Some("parse_no_image_test".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/receipts/{}/parse", receipt_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_receipt_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/receipts/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_receipts_invalid_status() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/receipts?status=invalid_status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========== Receipt Auto-Matching Tests ==========

#[tokio::test]
async fn test_get_receipt_match_candidates() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create an account and transaction
    let account_id = db.upsert_account("Test Card", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        description: "STARBUCKS STORE 12345".to_string(),
        amount: -5.75,
        category: None,
        import_hash: "test_hash_match_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    db.insert_transaction(account_id, &tx).unwrap();
    db.update_merchant_normalized(1, "Starbucks").unwrap();

    // Create a pending receipt
    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/test/receipt.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
        receipt_total: Some(5.75),
        receipt_merchant: Some("Starbucks".to_string()),
        content_hash: Some("match_test_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Get match candidates
    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/receipts/{}/candidates", receipt_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let candidates = json.as_array().unwrap();
    assert!(!candidates.is_empty(), "Should find at least one candidate");

    // Verify candidate structure
    let first = &candidates[0];
    assert!(first["score"].as_f64().unwrap() > 0.5);
    assert!(first["transaction"]["id"].as_i64().is_some());
    assert!(first["match_factors"]["amount_score"].as_f64().is_some());
}

#[tokio::test]
async fn test_get_receipt_match_candidates_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/receipts/99999/candidates")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_auto_match_receipts_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/receipts/auto-match")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["matched"].as_i64().unwrap(), 0);
    assert_eq!(json["checked"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_auto_match_receipts_with_match() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create an account and transaction
    let account_id = db.upsert_account("Test Card", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        description: "TARGET STORE 789".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "test_hash_match_2".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    db.insert_transaction(account_id, &tx).unwrap();
    db.update_merchant_normalized(1, "Target").unwrap();

    // Create a pending receipt that should match
    let receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/test/receipt.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
        receipt_total: Some(50.00),
        receipt_merchant: Some("Target".to_string()),
        content_hash: Some("auto_match_test_hash".to_string()),
    };
    db.create_receipt_full(&receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Run auto-match
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/receipts/auto-match")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["checked"].as_i64().unwrap(), 1);
    assert_eq!(json["matched"].as_i64().unwrap(), 1);
}

// ========== Reprocessing Tests ==========

#[tokio::test]
async fn test_reprocess_transaction_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/99999/reprocess")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_reprocess_transaction_no_ollama() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create an account and transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "NETFLIX.COM".to_string(),
        amount: -15.99,
        category: None,
        import_hash: "hash123".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    db.insert_transaction(account_id, &tx).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/1/reprocess")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed with an error in the response (Ollama not configured)
    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert_eq!(json["success"], false);
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("Ollama not configured"));
}

#[tokio::test]
async fn test_bulk_reprocess_no_ollama() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "transaction_ids": [1, 2, 3]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ollama/reprocess")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should fail with bad request (Ollama not configured)
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json = get_body_json(response).await;
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("Ollama not configured"));
}

#[tokio::test]
async fn test_bulk_reprocess_invalid_json() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ollama/reprocess")
                .header("content-type", "application/json")
                .body(Body::from("not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json = get_body_json(response).await;
    assert!(json["error"].as_str().unwrap().contains("Invalid JSON"));
}

#[tokio::test]
async fn test_ollama_stats() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ollama/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total_calls").is_some());
    assert!(json.get("success_rate").is_some());
    assert!(json.get("avg_latency_ms").is_some());
}

#[tokio::test]
async fn test_ollama_stats_with_period() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ollama/stats?period=last-30-days")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("period_start").is_some());
    assert!(json.get("period_end").is_some());
}

#[tokio::test]
async fn test_ollama_calls() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ollama/calls")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.is_array());
}

#[tokio::test]
async fn test_ollama_calls_with_limit() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ollama/calls?limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.is_array());
}

#[tokio::test]
async fn test_ollama_health() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ollama/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("available").is_some());
    assert!(json.get("recent_error_rate").is_some());
}

#[tokio::test]
async fn test_ollama_recommendation() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/ollama/recommendation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("stats_summary").is_some());
    assert!(json.get("recommendations").is_some());
    assert!(json.get("should_switch").is_some());
}

// --- Report entity/card_member filtering tests ---

#[tokio::test]
async fn test_report_spending_with_entity_filter() {
    let app = setup_test_app();

    // Test that entity_id filter doesn't cause errors (even with no data)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?entity_id=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total").is_some());
}

#[tokio::test]
async fn test_report_spending_with_card_member_filter() {
    let app = setup_test_app();

    // Test that card_member filter doesn't cause errors
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?card_member=John%20Doe")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total").is_some());
}

#[tokio::test]
async fn test_report_trends_with_entity_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends?entity_id=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("data").is_some());
}

#[tokio::test]
async fn test_report_trends_with_card_member_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends?card_member=Jane%20Smith")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("data").is_some());
}

#[tokio::test]
async fn test_report_merchants_with_entity_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/merchants?entity_id=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("merchants").is_some());
}

#[tokio::test]
async fn test_report_merchants_with_card_member_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/merchants?card_member=Bob%20Wilson")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("merchants").is_some());
}

#[tokio::test]
async fn test_report_spending_with_both_filters() {
    let app = setup_test_app();

    // Test combined entity_id and card_member filters
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?entity_id=1&card_member=Test%20User")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("total").is_some());
}

// ========== Security Tests ==========

#[tokio::test]
async fn test_auth_empty_header() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: true,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Empty string header should be rejected (defense in depth)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags")
                .header("cf-access-authenticated-user-email", "")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Empty header should be rejected even if Cloudflare wouldn't send one
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_whitespace_only_header() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: true,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Whitespace-only header should be rejected (defense in depth)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tags")
                .header("cf-access-authenticated-user-email", "   ")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Whitespace-only should be treated same as empty/missing
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_sql_injection_in_query_params() {
    let app = setup_test_app();

    // URL-encoded SQL injection attempt in search parameter
    // Note: Special chars must be URL-encoded
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?search=DROP%20TABLE%20transactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return OK (empty results), not crash or affect DB
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_nonexistent_resource_returns_404() {
    let app = setup_test_app();

    // Request non-existent account
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/accounts/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_negative_id_returns_404() {
    let app = setup_test_app();

    // Negative ID should be parsed but not found
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions/-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pagination_limits_enforced() {
    let app = setup_test_app();

    // Request more than MAX_PAGE_LIMIT
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?limit=10000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed but limit is clamped
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_malformed_json_returns_400() {
    let app = setup_test_app();

    // Send malformed JSON
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from("{invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_unicode_in_request_body() {
    let app = setup_test_app();

    // Create account with unicode name
    let body = serde_json::json!({
        "name": "测试账户 🏦 Security Test",
        "bank": "chase"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Either CREATED (201) for new account or OK (200) for upsert is acceptable
    assert!(response.status() == StatusCode::CREATED || response.status() == StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json
        .get("name")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("测试账户"));
}

#[tokio::test]
async fn test_very_long_string_in_body() {
    let app = setup_test_app();

    // Very long name (10000 chars) - unique to avoid upsert
    let long_name: String = format!("SecurityTest_{}", "A".repeat(10000));
    let body = serde_json::json!({
        "name": long_name,
        "bank": "chase"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Either CREATED (201) for new account or OK (200) for upsert is acceptable
    assert!(response.status() == StatusCode::CREATED || response.status() == StatusCode::OK);
}

#[tokio::test]
async fn test_error_response_no_stack_trace() {
    let app = setup_test_app();

    // Trigger an error (non-existent resource)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/accounts/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = get_body_json(response).await;

    // Error response should have "error" field but no stack trace
    assert!(json.get("error").is_some());
    let error_msg = json.get("error").unwrap().as_str().unwrap();

    // Should not contain stack trace indicators
    assert!(!error_msg.contains("at "));
    assert!(!error_msg.contains("src/"));
    assert!(!error_msg.contains("panic"));
    assert!(!error_msg.contains("thread"));
}

// ========== Export API Tests ==========

#[tokio::test]
async fn test_export_transactions_csv() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/transactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
}

#[tokio::test]
async fn test_export_transactions_json() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/transactions?format=json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
}

#[tokio::test]
async fn test_export_transactions_with_date_range() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/transactions?from=2024-01-01&to=2024-12-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_export_transactions_invalid_format() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/transactions?format=xml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_export_transactions_invalid_date() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/transactions?from=not-a-date")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_export_full() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/full")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );

    let json = get_body_json(response).await;
    assert!(json.get("metadata").is_some());
    assert!(json.get("accounts").is_some());
    assert!(json.get("transactions").is_some());
    assert!(json.get("tags").is_some());
}

#[tokio::test]
async fn test_import_full() {
    let app = setup_test_app();

    // Create a minimal valid backup (fields must match FullBackup struct)
    let backup = serde_json::json!({
        "metadata": {
            "version": "1.0",
            "created_at": "2024-01-01T00:00:00Z",
            "total_records": 0
        },
        "accounts": [],
        "locations": [],
        "entities": [],
        "tags": [],
        "merchant_aliases": [],
        "trips": [],
        "tag_rules": [],
        "subscriptions": [],
        "price_history": [],
        "transactions": [],
        "transaction_tags": [],
        "transaction_splits": [],
        "split_tags": [],
        "receipts": [],
        "alerts": [],
        "mileage_logs": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/full?clear=true")
                .header("content-type", "application/json")
                .body(Body::from(backup.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("success").unwrap().as_bool().unwrap());
    assert!(json.get("stats").is_some());
}

#[tokio::test]
async fn test_import_full_invalid_json() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/full")
                .header("content-type", "application/json")
                .body(Body::from("not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========== Detection API Tests ==========

#[tokio::test]
async fn test_run_detection_empty_db() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("subscriptions_found").is_some());
    assert!(json.get("zombies_detected").is_some());
}

#[tokio::test]
async fn test_run_detection_zombies_only() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect?zombies_only=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_run_detection_duplicates_only() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect?duplicates_only=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_run_detection_price_increases_only() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/detect?price_increases_only=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Subscription API Tests ==========

#[tokio::test]
async fn test_list_subscriptions_with_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/subscriptions?account_id=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    // Just verify we got a valid array response - empty is fine
    assert!(json.as_array().is_some());
}

#[tokio::test]
async fn test_exclude_subscription_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subscriptions/99999/exclude")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_unexclude_subscription_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subscriptions/99999/unexclude")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ========== Transaction Filtering Tests ==========

#[tokio::test]
async fn test_list_transactions_untagged() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?untagged=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    assert!(json.get("transactions").is_some());
    assert!(json.get("total").is_some());
}

#[tokio::test]
async fn test_list_transactions_with_all_filters() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?account_id=1&entity_id=1&search=test&period=this-month&sort=amount&order=desc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_transactions_with_tag_and_date_filter() {
    // This test verifies that tag filtering works correctly with date filtering
    // (regression test for parameter ordering bug where CTE params were misaligned)
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    // Create a transaction dated today
    let today = chrono::Utc::now().date_naive();
    let tx = hone_core::models::NewTransaction {
        date: today,
        description: "TEST MERCHANT".to_string(),
        amount: -50.0,
        category: None,
        import_hash: "test_hash_tag_date".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    // Get a root tag (Dining)
    let dining_tag = db
        .get_tag_by_path("Dining")
        .unwrap()
        .expect("Dining tag should exist");

    // Tag the transaction
    db.add_transaction_tag(tx_id, dining_tag.id, TagSource::Manual, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Query with both tag filter AND date filter (this-month)
    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!(
                    "/api/transactions?tag_ids={}&period=this-month",
                    dining_tag.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_body_json(response).await;
    let transactions = json["transactions"]
        .as_array()
        .expect("transactions should be array");

    // Should find the transaction we created (it's tagged with Dining and dated today)
    assert_eq!(
        transactions.len(),
        1,
        "Should find exactly 1 transaction with tag+date filter"
    );
    assert_eq!(transactions[0]["id"], tx_id);
}

// ========== Account API Tests (additional coverage) ==========

#[tokio::test]
async fn test_get_account() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/accounts/{}", account_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["name"], "Test Account");
    assert_eq!(json["bank"], "chase");
}

#[tokio::test]
async fn test_get_account_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/accounts/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_account() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db.upsert_account("Old Name", Bank::Chase, None).unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "name": "New Name",
        "bank": "amex"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/accounts/{}", account_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["name"], "New Name");
    assert_eq!(json["bank"], "amex");
}

#[tokio::test]
async fn test_update_account_not_found() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "name": "New Name",
        "bank": "chase"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/accounts/99999")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_account_invalid_bank() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "name": "Test",
        "bank": "invalid_bank"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&format!("/api/accounts/{}", account_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_account() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db.upsert_account("To Delete", Bank::Chase, None).unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/accounts/{}", account_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_delete_account_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/accounts/99999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_account_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let entity = NewEntity {
        name: "John".to_string(),
        entity_type: EntityType::Person,
        icon: None,
        color: None,
    };
    let entity_id = db.create_entity(&entity).unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "entity_id": entity_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/accounts/{}/entity", account_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_update_account_entity_remove() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "entity_id": null
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/accounts/{}/entity", account_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_update_account_entity_not_found() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "entity_id": 1
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/accounts/99999/entity")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_account_entity_invalid_entity() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "entity_id": 99999
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/accounts/{}/entity", account_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ========== Backup API Tests ==========

#[tokio::test]
async fn test_list_backups() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/backup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json.as_array().is_some());
}

// ========== Receipt Link/Unlink Tests ==========

#[tokio::test]
async fn test_unlink_receipt_success() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create account and transaction
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let tx_id = db
        .insert_transaction(
            account_id,
            &hone_core::models::NewTransaction {
                date: chrono::NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "Test TX".to_string(),
                amount: -50.0,
                category: None,
                import_hash: "testhash".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap()
        .unwrap();

    // Create a linked receipt
    let receipt_id = db.create_receipt(tx_id, None).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/receipts/{}/unlink", receipt_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["transaction_id"].is_null());
}

#[tokio::test]
async fn test_unlink_receipt_not_linked() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a receipt without transaction
    let new_receipt = hone_core::models::NewReceipt {
        transaction_id: None,
        image_path: Some("/tmp/test.jpg".to_string()),
        image_data: None,
        status: hone_core::models::ReceiptStatus::Pending,
        role: hone_core::models::ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: None,
        receipt_merchant: None,
        content_hash: None,
    };
    let receipt_id = db.create_receipt_full(&new_receipt).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/receipts/{}/unlink", receipt_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_unlink_receipt_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/receipts/99999/unlink")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ========== Subscription API Tests (Extended) ==========

#[tokio::test]
async fn test_list_subscriptions_by_account() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    // Create a subscription for this account
    db.upsert_subscription("Netflix", Some(account_id), Some(15.99), None, None, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/api/subscriptions?account_id={}", account_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["merchant"], "Netflix");
}

#[tokio::test]
async fn test_cancel_subscription_returns_success() {
    // Note: cancel_subscription doesn't validate existence, returns success even for missing IDs
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/subscriptions/99999/cancel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Handler doesn't validate existence - returns 200 with success=true
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_exclude_subscription() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let sub_id = db
        .upsert_subscription("Costco", None, Some(120.0), None, None, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/subscriptions/{}/exclude", sub_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_unexclude_subscription_success() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let sub_id = db
        .upsert_subscription("Costco", None, Some(120.0), None, None, None)
        .unwrap();
    db.exclude_subscription(sub_id).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/subscriptions/{}/unexclude", sub_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

// ========== Alert API Tests (Extended) ==========

#[tokio::test]
async fn test_list_alerts_with_dismissed() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create an active alert
    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO alerts (type, message, dismissed) VALUES ('zombie', 'Test alert', 0)",
        [],
    )
    .unwrap();
    // Create a dismissed alert
    conn.execute(
        "INSERT INTO alerts (type, message, dismissed) VALUES ('zombie', 'Dismissed alert', 1)",
        [],
    )
    .unwrap();
    drop(conn);

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Without dismissed
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/alerts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json.as_array().unwrap().len(), 1);

    // With dismissed
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/alerts?include_dismissed=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_dismiss_alert_returns_success() {
    // Note: dismiss_alert doesn't validate existence - returns success even for missing IDs
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/alerts/99999/dismiss")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Handler doesn't validate existence - uses UPDATE which succeeds even if no rows affected
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_restore_alert_returns_success() {
    // Note: restore_alert doesn't validate existence - returns success even for missing IDs
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/alerts/99999/restore")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Handler doesn't validate existence
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

// ========== Transaction Archive Tests ==========

#[tokio::test]
async fn test_archive_transaction() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let tx_id = db
        .insert_transaction(
            account_id,
            &hone_core::models::NewTransaction {
                date: chrono::NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "Test TX".to_string(),
                amount: -50.0,
                category: None,
                import_hash: "testhash".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap()
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/transactions/{}/archive", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_unarchive_transaction() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let tx_id = db
        .insert_transaction(
            account_id,
            &hone_core::models::NewTransaction {
                date: chrono::NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "Test TX".to_string(),
                amount: -50.0,
                category: None,
                import_hash: "testhash".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap()
        .unwrap();

    db.archive_transaction(tx_id).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/transactions/{}/unarchive", tx_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_archive_transaction_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/99999/archive")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_unarchive_transaction_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/99999/unarchive")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ========== Backup API Tests ==========

fn setup_test_app_with_backup_dir(backup_dir: PathBuf) -> Router {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    // Insert some test data for backup
    db.upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    create_router_with_options(db, None, config, Some(backup_dir))
}

#[tokio::test]
async fn test_list_backups_empty() {
    let temp_dir = TempDir::new().unwrap();
    let app = setup_test_app_with_backup_dir(temp_dir.path().to_path_buf());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/backup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_create_backup() {
    let temp_dir = TempDir::new().unwrap();
    let app = setup_test_app_with_backup_dir(temp_dir.path().to_path_buf());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup")
                .header("content-type", "application/json")
                .body(Body::from(r#"{}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    // Format is "hone-2024-01-15-120000.db.gz"
    let name = json["name"].as_str().unwrap();
    assert!(
        name.starts_with("hone-"),
        "Name should start with 'hone-': {}",
        name
    );
    assert!(
        name.ends_with(".db.gz"),
        "Name should end with '.db.gz': {}",
        name
    );
    assert!(json["path"].as_str().is_some());
    assert!(json["size"].as_u64().unwrap() > 0);
    assert_eq!(json["accounts"].as_i64().unwrap(), 1);
    assert_eq!(json["transactions"].as_i64().unwrap(), 0);
    // LocalDestination assumes encrypted: true for hone backup files
    assert!(json["encrypted"].as_bool().is_some());
    assert!(json["compressed"].as_bool().unwrap());
}

#[tokio::test]
async fn test_create_backup_with_name() {
    let temp_dir = TempDir::new().unwrap();
    let app = setup_test_app_with_backup_dir(temp_dir.path().to_path_buf());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name": "hone-custom-backup.db.gz"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let json = get_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "Response body: {:?}", json);
    assert_eq!(json["name"].as_str().unwrap(), "hone-custom-backup.db.gz");
}

#[tokio::test]
async fn test_list_backups_after_create() {
    let temp_dir = TempDir::new().unwrap();
    let backup_dir = temp_dir.path().to_path_buf();

    // Create first backup
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    db.upsert_account("Test Account", Bank::Chase, None)
        .unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app =
        create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name": "hone-backup1.db.gz"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Create second backup
    let app =
        create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name": "hone-backup2.db.gz"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // List backups
    let app = create_router_with_options(db, None, config, Some(backup_dir));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/backup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let backups = json.as_array().unwrap();
    assert_eq!(backups.len(), 2);

    // Verify backup names are present (order may vary)
    let names: Vec<&str> = backups
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"hone-backup1.db.gz"));
    assert!(names.contains(&"hone-backup2.db.gz"));
}

#[tokio::test]
async fn test_prune_backups_default() {
    let temp_dir = TempDir::new().unwrap();
    let backup_dir = temp_dir.path().to_path_buf();

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };

    // Create 10 backups
    for i in 0..10 {
        let app =
            create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/backup")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"name": "hone-backup{:02}.db.gz"}}"#,
                        i
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Prune with default keep=7
    let app =
        create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup/prune")
                .header("content-type", "application/json")
                .body(Body::from(r#"{}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["deleted_count"].as_u64().unwrap(), 3);
    assert_eq!(json["retained_count"].as_u64().unwrap(), 7);
    assert!(json["bytes_freed"].as_u64().unwrap() > 0);
    assert_eq!(json["deleted_names"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_prune_backups_custom_keep() {
    let temp_dir = TempDir::new().unwrap();
    let backup_dir = temp_dir.path().to_path_buf();

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };

    // Create 5 backups
    for i in 0..5 {
        let app =
            create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/backup")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"name": "hone-backup{:02}.db.gz"}}"#,
                        i
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Prune keeping only 2
    let app =
        create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup/prune")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"keep": 2}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["deleted_count"].as_u64().unwrap(), 3);
    assert_eq!(json["retained_count"].as_u64().unwrap(), 2);

    // Verify only 2 remain
    let app = create_router_with_options(db, None, config, Some(backup_dir));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/backup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = get_body_json(response).await;
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_prune_backups_nothing_to_prune() {
    let temp_dir = TempDir::new().unwrap();
    let backup_dir = temp_dir.path().to_path_buf();

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };

    // Create only 3 backups
    for i in 0..3 {
        let app =
            create_router_with_options(db.clone(), None, config.clone(), Some(backup_dir.clone()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/backup")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"name": "hone-backup{:02}.db.gz"}}"#,
                        i
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Prune with default keep=7 (should prune nothing)
    let app = create_router_with_options(db, None, config, Some(backup_dir));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backup/prune")
                .header("content-type", "application/json")
                .body(Body::from(r#"{}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["deleted_count"].as_u64().unwrap(), 0);
    assert_eq!(json["retained_count"].as_u64().unwrap(), 3);
    assert_eq!(json["bytes_freed"].as_u64().unwrap(), 0);
}

// ========== Import CSV (JSON API) Tests ==========

#[tokio::test]
async fn test_import_csv_json_success() {
    use base64::Engine;

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Chase CSV format
    let csv_data = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo\n\
        01/15/2024,01/16/2024,AMAZON MARKETPLACE,-99.99,Sale,-99.99,\n\
        01/14/2024,01/15/2024,STARBUCKS,Coffee Shop,Sale,-5.50,";

    let csv_base64 = base64::engine::general_purpose::STANDARD.encode(csv_data);

    let body = serde_json::json!({
        "account_id": account_id,
        "csv_data": csv_base64
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["imported"].as_u64().unwrap(), 2);
    assert_eq!(json["skipped"].as_u64().unwrap(), 0);
    assert_eq!(json["account_name"].as_str().unwrap(), "Test Account");
    assert_eq!(json["bank"].as_str().unwrap(), "chase");
}

#[tokio::test]
async fn test_import_csv_json_skips_duplicates() {
    use base64::Engine;

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };

    // Chase CSV format
    let csv_data = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo\n\
        01/15/2024,01/16/2024,AMAZON MARKETPLACE,-99.99,Sale,-99.99,";

    let csv_base64 = base64::engine::general_purpose::STANDARD.encode(csv_data);

    let body = serde_json::json!({
        "account_id": account_id,
        "csv_data": csv_base64
    });

    // First import
    let app = create_router(db.clone(), None, config.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["imported"].as_u64().unwrap(), 1);

    // Second import - same data should be skipped
    let app = create_router(db, None, config);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["imported"].as_u64().unwrap(), 0);
    assert_eq!(json["skipped"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn test_import_csv_json_account_not_found() {
    use base64::Engine;

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let csv_data = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo\n\
        01/15/2024,01/16/2024,TEST,-10.00,Sale,-10.00,";

    let csv_base64 = base64::engine::general_purpose::STANDARD.encode(csv_data);

    let body = serde_json::json!({
        "account_id": 99999,
        "csv_data": csv_base64
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_import_csv_json_invalid_base64() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let body = serde_json::json!({
        "account_id": account_id,
        "csv_data": "not-valid-base64!!!"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_import_csv_json_with_amex_format() {
    use base64::Engine;

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db.upsert_account("Amex Card", Bank::Amex, None).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Amex CSV format
    let csv_data = "Date,Description,Amount\n\
        01/15/24,\"AMAZON MARKETPLACE\",99.99\n\
        01/14/24,\"STARBUCKS\",5.50";

    let csv_base64 = base64::engine::general_purpose::STANDARD.encode(csv_data);

    let body = serde_json::json!({
        "account_id": account_id,
        "csv_data": csv_base64
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["imported"].as_u64().unwrap(), 2);
    assert_eq!(json["bank"].as_str().unwrap(), "amex");
}

#[tokio::test]
async fn test_import_csv_json_runs_detection() {
    use base64::Engine;

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Create recurring transactions that should be detected as a subscription
    let csv_data = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo\n\
        01/15/2024,01/16/2024,NETFLIX,-15.99,Sale,-15.99,\n\
        12/15/2023,12/16/2023,NETFLIX,-15.99,Sale,-15.99,\n\
        11/15/2023,11/16/2023,NETFLIX,-15.99,Sale,-15.99,\n\
        10/15/2023,10/16/2023,NETFLIX,-15.99,Sale,-15.99,";

    let csv_base64 = base64::engine::general_purpose::STANDARD.encode(csv_data);

    let body = serde_json::json!({
        "account_id": account_id,
        "csv_data": csv_base64
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["imported"].as_u64().unwrap(), 4);
    // Import now returns immediately before async processing
    // Detection results are initially 0 and update asynchronously
    assert_eq!(json["subscriptions_found"].as_u64().unwrap(), 0);
    // Session ID should be returned for tracking progress
    assert!(json["import_session_id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_import_csv_json_tags_transactions() {
    use base64::Engine;

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Transaction that should get auto-tagged
    let csv_data = "Transaction Date,Post Date,Description,Category,Type,Amount,Memo\n\
        01/15/2024,01/16/2024,SHELL GAS STATION,-45.00,Sale,-45.00,";

    let csv_base64 = base64::engine::general_purpose::STANDARD.encode(csv_data);

    let body = serde_json::json!({
        "account_id": account_id,
        "csv_data": csv_base64
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/json")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["imported"].as_u64().unwrap(), 1);
    // Import now returns immediately before async tagging
    // Tagging happens in background, initial response shows 0
    assert_eq!(json["transactions_tagged"].as_i64().unwrap(), 0);
    // Session ID should be returned for tracking progress
    assert!(json["import_session_id"].as_i64().unwrap() > 0);
}

// ========== Additional Alert Tests ==========

#[tokio::test]
async fn test_dismiss_alert_exclude() {
    use hone_core::models::{NewTransaction, SubscriptionStatus};

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create account
    let account_id = db
        .upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    // Create transactions for a subscription pattern
    let dates = vec!["2024-01-15", "2024-02-15", "2024-03-15", "2024-04-15"];
    for (i, date_str) in dates.iter().enumerate() {
        let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap();
        let tx = NewTransaction {
            date,
            description: "NETFLIX".to_string(),
            amount: -15.99,
            category: None,
            import_hash: format!("netflix-hash-{}", i),
            card_member: None,
            original_data: None,
            import_format: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
    }

    // Run detection to create subscription and potentially alerts
    let detector = hone_core::detect::WasteDetector::new(&db);
    let _ = detector.detect_all().await.unwrap();

    // Get alerts
    let alerts = db.list_alerts(true).unwrap();

    // Find a zombie alert with a subscription_id
    let alert = alerts.iter().find(|a| a.subscription_id.is_some());

    if let Some(alert) = alert {
        let config = ServerConfig {
            require_auth: false,
            allowed_origins: vec![],
            ..Default::default()
        };
        let app = create_router(db.clone(), None, config);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/api/alerts/{}/dismiss-exclude", alert.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = get_body_json(response).await;
        assert_eq!(json["success"].as_bool().unwrap(), true);

        // Verify alert is dismissed
        let updated_alerts = db.list_alerts(false).unwrap();
        assert!(!updated_alerts.iter().any(|a| a.id == alert.id));

        // Verify subscription is excluded
        if let Some(sub_id) = alert.subscription_id {
            let sub = db.get_subscription(sub_id).unwrap();
            if let Some(s) = sub {
                assert_eq!(s.status, SubscriptionStatus::Excluded);
            }
        }
    }
}

// ========== Additional Report Tests ==========

#[tokio::test]
async fn test_report_spending_with_expansion() {
    let app = setup_test_app();

    // Request spending report with expansion - verify it returns valid response
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?period=all&expand=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    // SpendingSummary has total field
    assert!(json["total"].as_f64().is_some());
}

#[tokio::test]
async fn test_report_spending_with_tag_filter() {
    let app = setup_test_app();

    // Request spending with tag filter using a seeded root tag (Groceries)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?period=all&tag=Groceries")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["total"].as_f64().is_some());
}

#[tokio::test]
async fn test_report_spending_entity_filter_empty() {
    let app = setup_test_app();

    // Request spending with entity filter on non-existent entity
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/spending?period=all&entity_id=999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["total"].as_f64().is_some());
}

#[tokio::test]
async fn test_report_trends_weekly_granularity() {
    let app = setup_test_app();

    // Request trends with weekly granularity
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends?granularity=weekly&period=last-30-days")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    // TrendsReport has data array
    assert!(json["data"].as_array().is_some());
}

#[tokio::test]
async fn test_report_trends_bad_granularity() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends?granularity=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_report_merchants_custom_limit() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/merchants?limit=5&period=all")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    // MerchantsReport has merchants array
    assert!(json["merchants"].as_array().is_some());
}

// ========== Additional Tests for Coverage ==========

#[tokio::test]
async fn test_report_by_entity_custom_dates() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-entity?from=2024-01-01&to=2024-12-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["entities"].as_array().is_some());
}

#[tokio::test]
async fn test_report_by_location_custom_dates() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/by-location?from=2024-01-01&to=2024-12-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json.as_array().is_some());
}

#[tokio::test]
async fn test_report_spending_all_periods() {
    let app = setup_test_app();

    // Test all period presets - some may not be tested elsewhere
    let periods = [
        "this-month",
        "last-month",
        "this-year",
        "last-year",
        "last-30-days",
        "last-90-days",
        "last-12-months",
    ];

    for period in periods {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&format!("/api/reports/spending?period={}", period))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Failed for period: {}",
            period
        );
    }
}

#[tokio::test]
async fn test_transactions_card_member_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?card_member=TEST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_transactions_multiple_filters() {
    let app = setup_test_app();

    // Test multiple filters combined
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/transactions?from=2024-01-01&to=2024-12-31&limit=10&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_export_transactions_all_params() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/transactions?from=2024-01-01&to=2024-12-31&tag_ids=1,2,3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_export_full_with_data() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Add some test data
    let _account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/export/full")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["accounts"].is_array());
    assert!(!json["accounts"].as_array().unwrap().is_empty());
    assert!(json["tags"].is_array());
}

#[tokio::test]
async fn test_report_trends_tag_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/trends?tag=Groceries&period=all")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["data"].as_array().is_some());
}

#[tokio::test]
async fn test_report_merchants_tag_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/reports/merchants?tag=Groceries&period=all")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["merchants"].as_array().is_some());
}

#[tokio::test]
async fn test_subscriptions_account_filter() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/subscriptions?account_id=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ========== Feedback API Tests ==========

#[tokio::test]
async fn test_list_feedback_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/feedback")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let feedback = json.as_array().unwrap();
    assert!(feedback.is_empty());
}

#[tokio::test]
async fn test_create_feedback() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "feedback_type": "helpful",
        "target_type": "alert",
        "target_id": 1,
        "reason": "Great insight!"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/feedback")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["id"].as_i64().unwrap() > 0);
    assert_eq!(json["feedback"]["feedback_type"], "helpful");
    assert_eq!(json["feedback"]["target_type"], "alert");
    assert_eq!(json["feedback"]["reason"], "Great insight!");
}

#[tokio::test]
async fn test_create_feedback_invalid_type() {
    let app = setup_test_app();

    let body = serde_json::json!({
        "feedback_type": "invalid_type",
        "target_type": "alert"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/feedback")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_feedback_stats() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/feedback/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert!(json["total_feedback"].as_i64().is_some());
    assert!(json["helpful_count"].as_i64().is_some());
    assert!(json["not_helpful_count"].as_i64().is_some());
}

#[tokio::test]
async fn test_feedback_revert() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create feedback directly in DB
    use hone_core::models::{FeedbackTargetType, FeedbackType, NewUserFeedback};
    let feedback_id = db
        .create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::Helpful,
            target_type: FeedbackTargetType::Alert,
            target_id: Some(1),
            original_value: None,
            corrected_value: None,
            reason: None,
            context: None,
        })
        .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Revert the feedback
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/feedback/{}/revert", feedback_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_feedback_filter_by_type() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create different types of feedback
    use hone_core::models::{FeedbackTargetType, FeedbackType, NewUserFeedback};
    db.create_feedback(&NewUserFeedback {
        feedback_type: FeedbackType::Helpful,
        target_type: FeedbackTargetType::Alert,
        target_id: Some(1),
        original_value: None,
        corrected_value: None,
        reason: None,
        context: None,
    })
    .unwrap();
    db.create_feedback(&NewUserFeedback {
        feedback_type: FeedbackType::NotHelpful,
        target_type: FeedbackTargetType::Explanation,
        target_id: Some(2),
        original_value: None,
        corrected_value: None,
        reason: None,
        context: None,
    })
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db, None, config);

    // Filter by feedback_type
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/feedback?feedback_type=helpful")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let feedback = json.as_array().unwrap();
    assert_eq!(feedback.len(), 1);
    assert_eq!(feedback[0]["feedback_type"], "helpful");
}

// =============================================================================
// Bulk Tag Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_add_tags() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create test transactions
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx1 = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "AMAZON".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "bulk_tag_test_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx2 = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
        description: "WALMART".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "bulk_tag_test_2".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id1 = db.insert_transaction(account_id, &tx1).unwrap().unwrap();
    let tx_id2 = db.insert_transaction(account_id, &tx2).unwrap().unwrap();

    let tags = db.list_tags().unwrap();
    let shopping_tag = tags.iter().find(|t| t.name == "Shopping").unwrap();
    let groceries_tag = tags.iter().find(|t| t.name == "Groceries").unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // Bulk add tags
    let body = serde_json::json!({
        "transaction_ids": [tx_id1, tx_id2],
        "tag_ids": [shopping_tag.id, groceries_tag.id]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/bulk-tags")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["processed"], 2); // Number of transactions
    assert_eq!(json["success_count"], 4); // 2 transactions x 2 tags
    assert_eq!(json["failed_count"], 0);

    // Verify tags were added
    let tx1_tags = db.get_transaction_tags_with_details(tx_id1).unwrap();
    let tx2_tags = db.get_transaction_tags_with_details(tx_id2).unwrap();
    assert_eq!(tx1_tags.len(), 2);
    assert_eq!(tx2_tags.len(), 2);

    // Verify source is Manual
    assert!(tx1_tags
        .iter()
        .all(|t| t.source == hone_core::models::TagSource::Manual));
}

#[tokio::test]
async fn test_bulk_add_tags_idempotent() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "AMAZON".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "bulk_tag_idempotent".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let tags = db.list_tags().unwrap();
    let shopping_tag = tags.iter().find(|t| t.name == "Shopping").unwrap();

    // Add tag first
    db.add_transaction_tag(
        tx_id,
        shopping_tag.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // Try to add the same tag again via bulk
    let body = serde_json::json!({
        "transaction_ids": [tx_id],
        "tag_ids": [shopping_tag.id]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions/bulk-tags")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    // Should succeed silently (idempotent)
    assert_eq!(json["processed"], 1);

    // Verify still only one tag
    let tx_tags = db.get_transaction_tags_with_details(tx_id).unwrap();
    assert_eq!(tx_tags.len(), 1);
}

#[tokio::test]
async fn test_bulk_remove_tags() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create test transactions with tags
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx1 = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "AMAZON".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "bulk_remove_test_1".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx2 = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
        description: "WALMART".to_string(),
        amount: -50.00,
        category: None,
        import_hash: "bulk_remove_test_2".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id1 = db.insert_transaction(account_id, &tx1).unwrap().unwrap();
    let tx_id2 = db.insert_transaction(account_id, &tx2).unwrap().unwrap();

    let tags = db.list_tags().unwrap();
    let shopping_tag = tags.iter().find(|t| t.name == "Shopping").unwrap();

    // Add tags to transactions
    db.add_transaction_tag(
        tx_id1,
        shopping_tag.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();
    db.add_transaction_tag(
        tx_id2,
        shopping_tag.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // Bulk remove tags
    let body = serde_json::json!({
        "transaction_ids": [tx_id1, tx_id2],
        "tag_ids": [shopping_tag.id]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/transactions/bulk-tags")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["processed"], 2);
    assert_eq!(json["success_count"], 2);
    assert_eq!(json["failed_count"], 0);

    // Verify tags were removed
    let tx1_tags = db.get_transaction_tags_with_details(tx_id1).unwrap();
    let tx2_tags = db.get_transaction_tags_with_details(tx_id2).unwrap();
    assert!(tx1_tags.is_empty());
    assert!(tx2_tags.is_empty());
}

#[tokio::test]
async fn test_bulk_remove_tags_nonexistent() {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a transaction without tags
    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let tx = hone_core::models::NewTransaction {
        date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        description: "AMAZON".to_string(),
        amount: -100.00,
        category: None,
        import_hash: "bulk_remove_nonexistent".to_string(),
        original_data: None,
        import_format: None,
        card_member: None,
        payment_method: None,
    };
    let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

    let tags = db.list_tags().unwrap();
    let shopping_tag = tags.iter().find(|t| t.name == "Shopping").unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // Try to remove a tag that doesn't exist on transaction
    let body = serde_json::json!({
        "transaction_ids": [tx_id],
        "tag_ids": [shopping_tag.id]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/transactions/bulk-tags")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed silently
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    assert_eq!(json["processed"], 1);
}

// ========== Insights API Tests ==========

#[tokio::test]
async fn test_get_top_insights_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/insights?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert!(insights.is_empty());
}

#[tokio::test]
async fn test_list_all_insights_empty() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/insights/all")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert!(insights.is_empty());
}

#[tokio::test]
async fn test_refresh_insights() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/insights/refresh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    // Should have run successfully with count of generated insights
    assert!(json.get("count").is_some());
}

#[tokio::test]
async fn test_dismiss_insight_not_found() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/insights/99999/dismiss")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_snooze_insight_not_found() {
    let app = setup_test_app();

    let body = serde_json::json!({ "days": 7 });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/insights/99999/snooze")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_insight_lifecycle() {
    use hone_core::insights::{Finding, InsightType, Severity};

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a test insight directly in the database
    let finding = Finding::new(
        InsightType::SavingsOpportunity,
        "test:lifecycle:1",
        Severity::Warning,
        "Test Insight",
        "This is a test insight for API testing",
    );
    let insight_id = db.upsert_insight_finding(&finding).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // 1. Get insights - should have 1
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/insights?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0]["id"], insight_id);
    assert_eq!(insights[0]["status"], "active");

    // 2. Dismiss the insight
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/insights/{}/dismiss", insight_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 3. Get insights - should be empty (dismissed are filtered)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/insights?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert!(insights.is_empty());

    // 4. Check all insights with status filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/insights/all?status=dismissed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0]["status"], "dismissed");

    // 5. Restore the insight
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/insights/{}/restore", insight_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 6. Get insights - should have 1 again
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/insights?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0]["status"], "active");
}

#[tokio::test]
async fn test_snooze_insight() {
    use hone_core::insights::{Finding, InsightType, Severity};

    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();

    // Create a test insight
    let finding = Finding::new(
        InsightType::ExpenseForecaster,
        "test:snooze:1",
        Severity::Info,
        "Upcoming Expense",
        "Test forecast insight",
    );
    let insight_id = db.upsert_insight_finding(&finding).unwrap();

    let config = ServerConfig {
        require_auth: false,
        allowed_origins: vec![],
        ..Default::default()
    };
    let app = create_router(db.clone(), None, config);

    // Snooze for 7 days
    let body = serde_json::json!({ "days": 7 });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/insights/{}/snooze", insight_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check it's now snoozed and not in top insights
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/insights?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert!(insights.is_empty());

    // Check snoozed filter
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/insights/all?status=snoozed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = get_body_json(response).await;
    let insights = json.as_array().unwrap();
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0]["status"], "snoozed");
    assert!(insights[0]["snoozed_until"].is_string());
}

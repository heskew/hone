//! CLI command tests
//!
//! This module contains all tests for the CLI commands.

use chrono::Datelike;
use hone_core::db::Database;
use hone_core::models::{Bank, PatternType, TagSource};

use crate::commands::{self, truncate};

fn setup_test_db() -> Database {
    let db = Database::in_memory().unwrap();
    db.seed_root_tags().unwrap();
    db
}

/// Create a test account and transaction, returning (account_id, tx_id)
fn create_test_transaction(db: &Database, description: &str, amount: f64) -> (i64, i64) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
    let conn = db.conn().unwrap();
    let hash = format!(
        "hash_{}_{}",
        description,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (?1, '2024-01-01', ?2, ?3, ?4)",
        rusqlite::params![account_id, description, amount, hash],
    ).unwrap();
    let tx_id = conn.last_insert_rowid();
    (account_id, tx_id)
}

// ========== Tags Command Tests ==========

#[test]
fn test_cmd_tags_list() {
    let db = setup_test_db();
    let result = commands::cmd_tags_list(&db);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_tags_add_root() {
    let db = setup_test_db();
    let result = commands::cmd_tags_add(&db, "CustomTag", None, None);
    assert!(result.is_ok());

    let tag = db.resolve_tag("CustomTag").unwrap();
    assert!(tag.is_some());
    assert_eq!(tag.unwrap().name, "CustomTag");
}

#[test]
fn test_cmd_tags_add_child() {
    let db = setup_test_db();
    // Use a child name that isn't already seeded
    let result = commands::cmd_tags_add(
        &db,
        "Transport.TestChild",
        Some("#ff0000"),
        Some("TEST_PATTERN"),
    );
    assert!(result.is_ok());

    let tag = db.get_tag_by_path("Transport.TestChild").unwrap();
    assert!(tag.is_some());
    let tag = tag.unwrap();
    assert_eq!(tag.name, "TestChild");
    assert_eq!(tag.color.as_deref(), Some("#ff0000"));
    assert_eq!(tag.auto_patterns.as_deref(), Some("TEST_PATTERN"));
}

#[test]
fn test_cmd_tags_add_invalid_parent() {
    let db = setup_test_db();
    let result = commands::cmd_tags_add(&db, "NonExistent.Child", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_cmd_tags_rename() {
    let db = setup_test_db();
    commands::cmd_tags_add(&db, "OldName", None, None).unwrap();

    let result = commands::cmd_tags_rename(&db, "OldName", "NewName");
    assert!(result.is_ok());

    let old = db.resolve_tag("OldName").unwrap();
    assert!(old.is_none());
    let new = db.resolve_tag("NewName").unwrap();
    assert!(new.is_some());
}

#[test]
fn test_cmd_tags_move() {
    let db = setup_test_db();
    commands::cmd_tags_add(&db, "Movable", None, None).unwrap();

    let result = commands::cmd_tags_move(&db, "Movable", "Transport");
    assert!(result.is_ok());

    let tag = db.get_tag_by_path("Transport.Movable").unwrap();
    assert!(tag.is_some());
}

#[test]
fn test_cmd_tags_move_to_root() {
    let db = setup_test_db();
    commands::cmd_tags_add(&db, "Transport.Temp", None, None).unwrap();

    let result = commands::cmd_tags_move(&db, "Transport.Temp", "root");
    assert!(result.is_ok());

    let tag = db.resolve_tag("Temp").unwrap();
    assert!(tag.is_some());
    assert!(tag.unwrap().parent_id.is_none());
}

#[test]
fn test_cmd_tags_delete_empty() {
    let db = setup_test_db();
    commands::cmd_tags_add(&db, "ToDelete", None, None).unwrap();

    let result = commands::cmd_tags_delete(&db, "ToDelete", false, false);
    assert!(result.is_ok());

    let tag = db.resolve_tag("ToDelete").unwrap();
    assert!(tag.is_none());
}

#[test]
fn test_cmd_tags_delete_with_transactions_requires_force() {
    let db = setup_test_db();

    let (_, tx_id) = create_test_transaction(&db, "TEST TX", -50.0);

    let groceries = db.resolve_tag("Groceries").unwrap().unwrap();
    db.add_transaction_tag(tx_id, groceries.id, TagSource::Manual, None)
        .unwrap();

    let result = commands::cmd_tags_delete(&db, "Groceries", false, false);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("has 1 transactions"));

    let result = commands::cmd_tags_delete(&db, "Groceries", true, false);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_tags_merge() {
    let db = setup_test_db();

    commands::cmd_tags_add(&db, "TagA", None, None).unwrap();
    commands::cmd_tags_add(&db, "TagB", None, None).unwrap();

    let (_, tx_id) = create_test_transaction(&db, "TEST", -10.0);
    let tag_a = db.resolve_tag("TagA").unwrap().unwrap();
    let tag_b = db.resolve_tag("TagB").unwrap().unwrap();
    db.add_transaction_tag(tx_id, tag_a.id, TagSource::Manual, None)
        .unwrap();

    let result = commands::cmd_tags_merge(&db, "TagA", "TagB");
    assert!(result.is_ok());

    assert!(db.resolve_tag("TagA").unwrap().is_none());

    let tags = db.get_transaction_tags(tx_id).unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag_id, tag_b.id);
}

// ========== Rules Command Tests ==========

#[test]
fn test_cmd_rules_list_empty() {
    let db = setup_test_db();
    let result = commands::cmd_rules_list(&db);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_rules_add() {
    let db = setup_test_db();
    let result =
        commands::cmd_rules_add(&db, "Groceries", "WHOLE FOODS|TRADER JOE", "contains", 10);
    assert!(result.is_ok());

    let rules = db.list_tag_rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].rule.pattern, "WHOLE FOODS|TRADER JOE");
    assert_eq!(rules[0].rule.priority, 10);
}

#[test]
fn test_cmd_rules_add_regex() {
    let db = setup_test_db();
    let result = commands::cmd_rules_add(&db, "Transport", r"UBER|LYFT.*RIDE", "regex", 5);
    assert!(result.is_ok());

    let rules = db.list_tag_rules().unwrap();
    assert_eq!(rules[0].rule.pattern_type, PatternType::Regex);
}

#[test]
fn test_cmd_rules_add_invalid_tag() {
    let db = setup_test_db();
    let result = commands::cmd_rules_add(&db, "NonExistent", "PATTERN", "contains", 0);
    assert!(result.is_err());
}

#[test]
fn test_cmd_rules_add_invalid_pattern_type() {
    let db = setup_test_db();
    let result = commands::cmd_rules_add(&db, "Groceries", "PATTERN", "invalid", 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("valid types"));
}

#[test]
fn test_cmd_rules_delete() {
    let db = setup_test_db();

    commands::cmd_rules_add(&db, "Groceries", "TEST", "contains", 0).unwrap();
    let rules = db.list_tag_rules().unwrap();
    let rule_id = rules[0].rule.id;

    let result = commands::cmd_rules_delete(&db, rule_id);
    assert!(result.is_ok());

    let rules = db.list_tag_rules().unwrap();
    assert!(rules.is_empty());
}

#[test]
fn test_cmd_rules_test() {
    let db = setup_test_db();

    commands::cmd_rules_add(&db, "Groceries", "WHOLE FOODS", "contains", 10).unwrap();
    commands::cmd_rules_add(&db, "Dining", "RESTAURANT", "contains", 5).unwrap();

    let result = commands::cmd_rules_test(&db, "WHOLE FOODS MARKET #123");
    assert!(result.is_ok());

    let result = commands::cmd_rules_test(&db, "RANDOM MERCHANT");
    assert!(result.is_ok());
}

// ========== Transaction Tagging Tests ==========

#[test]
fn test_resolve_tag_by_name() {
    let db = setup_test_db();
    let tag = commands::resolve_tag_arg(&db, "Groceries");
    assert!(tag.is_ok());
    assert_eq!(tag.unwrap().name, "Groceries");
}

#[test]
fn test_resolve_tag_by_path() {
    let db = setup_test_db();
    // Transport.Gas is now seeded, so we can just look it up directly
    let tag = commands::resolve_tag_arg(&db, "Transport.Gas");
    assert!(tag.is_ok());
    assert_eq!(tag.unwrap().name, "Gas");
}

#[test]
fn test_resolve_tag_not_found() {
    let db = setup_test_db();
    let tag = commands::resolve_tag_arg(&db, "NonExistent");
    assert!(tag.is_err());
    assert!(tag.unwrap_err().to_string().contains("not found"));
}

// ========== Report Tests ==========

#[test]
fn test_cmd_report_by_tag_empty() {
    let db = setup_test_db();
    let result = commands::cmd_report_by_tag(&db, None, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_by_tag_with_data() {
    let db = setup_test_db();

    let (_, tx_id) = create_test_transaction(&db, "WHOLE FOODS", -75.50);
    let groceries = db.resolve_tag("Groceries").unwrap().unwrap();
    db.add_transaction_tag(tx_id, groceries.id, TagSource::Manual, None)
        .unwrap();

    let result = commands::cmd_report_by_tag(&db, None, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_with_depth_filter() {
    let db = setup_test_db();

    // Transport.Gas is now seeded, just need to add the grandchild
    commands::cmd_tags_add(&db, "Transport.Gas.Premium", None, None).unwrap();

    let result = commands::cmd_report_by_tag(&db, Some(0), None, None);
    assert!(result.is_ok());

    let result = commands::cmd_report_by_tag(&db, Some(1), None, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_with_date_range() {
    let db = setup_test_db();

    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1);
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31);

    let result = commands::cmd_report_by_tag(&db, None, from, to);
    assert!(result.is_ok());
}

// ========== Helper Function Tests ==========

#[test]
fn test_truncate() {
    assert_eq!(truncate("short", 10), "short");
    assert_eq!(truncate("a long string that exceeds", 10), "a long ..."); // 7 chars + "..."
    assert_eq!(truncate("exact", 5), "exact");
    assert_eq!(truncate("exactly", 7), "exactly");
    assert_eq!(truncate("toolong", 6), "too...");
}

// ========== Dashboard Tests ==========

#[test]
fn test_cmd_dashboard_empty() {
    let db = setup_test_db();
    let stats = db.get_dashboard_stats().unwrap();
    assert_eq!(stats.total_accounts, 0);
    assert_eq!(stats.total_transactions, 0);
}

// ========== New Report Command Tests ==========

#[test]
fn test_cmd_report_spending_empty() {
    let db = setup_test_db();
    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result = commands::cmd_report_spending(&db, from, to, None, false);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_spending_with_data() {
    let db = setup_test_db();

    let (_, tx_id) = create_test_transaction(&db, "SAFEWAY GROCERY", -85.50);
    let groceries = db.resolve_tag("Groceries").unwrap().unwrap();
    db.add_transaction_tag(tx_id, groceries.id, TagSource::Manual, None)
        .unwrap();

    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result = commands::cmd_report_spending(&db, from, to, None, false);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_spending_with_tag_filter() {
    let db = setup_test_db();

    let (_, tx_id) = create_test_transaction(&db, "UBER EATS", -25.00);
    let dining = db.resolve_tag("Dining").unwrap().unwrap();
    db.add_transaction_tag(tx_id, dining.id, TagSource::Manual, None)
        .unwrap();

    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result = commands::cmd_report_spending(&db, from, to, Some("Dining"), false);
    if let Err(ref e) = result {
        eprintln!("Error: {:?}", e);
    }
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_trends_empty() {
    let db = setup_test_db();
    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result =
        commands::cmd_report_trends(&db, from, to, hone_core::models::Granularity::Monthly, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_trends_with_data() {
    let db = setup_test_db();

    create_test_transaction(&db, "AMAZON PRIME", -14.99);

    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result =
        commands::cmd_report_trends(&db, from, to, hone_core::models::Granularity::Monthly, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_trends_weekly() {
    let db = setup_test_db();
    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 3, 31).unwrap();

    let result =
        commands::cmd_report_trends(&db, from, to, hone_core::models::Granularity::Weekly, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_merchants_empty() {
    let db = setup_test_db();
    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result = commands::cmd_report_merchants(&db, from, to, 10, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_merchants_with_data() {
    let db = setup_test_db();

    create_test_transaction(&db, "STARBUCKS", -5.50);
    create_test_transaction(&db, "STARBUCKS", -6.00);
    create_test_transaction(&db, "CHIPOTLE", -12.00);

    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

    let result = commands::cmd_report_merchants(&db, from, to, 10, None);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_subscriptions_empty() {
    let db = setup_test_db();
    let result = commands::cmd_report_subscriptions(&db);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_report_savings_empty() {
    let db = setup_test_db();
    let result = commands::cmd_report_savings(&db);
    assert!(result.is_ok());
}

#[test]
fn test_resolve_period_this_month() {
    let (from, to) = commands::resolve_period("this-month", None, None).unwrap();
    let today = chrono::Utc::now().date_naive();
    assert_eq!(from.month(), today.month());
    assert_eq!(to.month(), today.month());
}

#[test]
fn test_resolve_period_last_month() {
    let (from, to) = commands::resolve_period("last-month", None, None).unwrap();
    let today = chrono::Utc::now().date_naive();
    let last_month = if today.month() == 1 {
        12
    } else {
        today.month() - 1
    };
    assert_eq!(from.month(), last_month);
    assert_eq!(to.month(), last_month);
}

#[test]
fn test_resolve_period_last_30_days() {
    let (from, to) = commands::resolve_period("last-30-days", None, None).unwrap();
    let diff = to.signed_duration_since(from).num_days();
    assert_eq!(diff, 30);
}

#[test]
fn test_resolve_period_last_90_days() {
    let (from, to) = commands::resolve_period("last-90-days", None, None).unwrap();
    let diff = to.signed_duration_since(from).num_days();
    assert_eq!(diff, 90);
}

#[test]
fn test_resolve_period_this_year() {
    let (from, _to) = commands::resolve_period("this-year", None, None).unwrap();
    let today = chrono::Utc::now().date_naive();
    assert_eq!(from.year(), today.year());
    assert_eq!(from.month(), 1);
    assert_eq!(from.day(), 1);
}

#[test]
fn test_resolve_period_all_time() {
    let (from, to) = commands::resolve_period("all", None, None).unwrap();
    assert_eq!(from.year(), 2000);
    let today = chrono::Utc::now().date_naive();
    assert_eq!(to, today);
}

#[test]
fn test_cmd_subscriptions_cancel() {
    let db = setup_test_db();

    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
         VALUES ('Netflix', 15.99, 'monthly', '2024-01-01', '2024-06-01', 'active')",
        [],
    )
    .unwrap();
    drop(conn);

    let result = commands::cmd_subscriptions_cancel(&db, "Netflix", None);
    if let Err(ref e) = result {
        eprintln!("Error: {:?}", e);
    }
    assert!(result.is_ok());

    let conn = db.conn().unwrap();
    let cancelled: String = conn
        .query_row(
            "SELECT status FROM subscriptions WHERE merchant = 'Netflix'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cancelled, "cancelled");
}

#[test]
fn test_cmd_subscriptions_cancel_not_found() {
    let db = setup_test_db();

    let result = commands::cmd_subscriptions_cancel(&db, "NonExistent", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ========== Backup/Restore Tests ==========

#[test]
fn test_cmd_backup_create() {
    use hone_core::backup::LocalDestination;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let backup_dir = dir.path().join("backups");

    // Create and populate a database
    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    db.upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    // Create backup
    let result = commands::cmd_backup_create(&db, None, Some(backup_dir.clone()));
    assert!(result.is_ok());

    // Verify backup exists
    let destination = LocalDestination::new(&backup_dir).unwrap();
    let backups = Database::list_backups(&destination).unwrap();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].name.starts_with("hone-"));
    assert!(backups[0].name.ends_with(".db.gz"));
}

#[test]
fn test_cmd_backup_list_empty() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let backup_dir = dir.path().join("backups");

    // List should work even with no backups
    let result = commands::cmd_backup_list(Some(backup_dir));
    assert!(result.is_ok());
}

#[test]
fn test_cmd_backup_restore() {
    use hone_core::backup::LocalDestination;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let backup_dir = dir.path().join("backups");
    let restored_path = dir.path().join("restored.db");

    // Create and populate a database
    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    db.upsert_account("Test Account", Bank::Chase, None)
        .unwrap();

    // Create backup
    commands::cmd_backup_create(&db, None, Some(backup_dir.clone())).unwrap();
    drop(db);

    // Get backup name
    let destination = LocalDestination::new(&backup_dir).unwrap();
    let backups = Database::list_backups(&destination).unwrap();
    let backup_name = &backups[0].name;

    // Restore
    let result =
        commands::cmd_backup_restore(&restored_path, backup_name, Some(backup_dir), false, true);
    assert!(result.is_ok());

    // Verify restored database
    let restored_db = Database::new_unencrypted(restored_path.to_str().unwrap()).unwrap();
    let stats = restored_db.get_dashboard_stats().unwrap();
    assert_eq!(stats.total_accounts, 1);
}

#[test]
fn test_cmd_backup_prune() {
    use hone_core::backup::LocalDestination;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let backup_dir = dir.path().join("backups");

    // Create a database
    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();

    // Create multiple backups with different names
    for i in 1..=5 {
        let name = format!("hone-2024-01-{:02}-120000.db.gz", i);
        commands::cmd_backup_create(&db, Some(&name), Some(backup_dir.clone())).unwrap();
    }

    // Verify we have 5 backups
    let destination = LocalDestination::new(&backup_dir).unwrap();
    assert_eq!(Database::list_backups(&destination).unwrap().len(), 5);

    // Prune to keep 2
    let result = commands::cmd_backup_prune(2, Some(backup_dir.clone()), true);
    assert!(result.is_ok());

    // Verify we have 2 backups
    assert_eq!(Database::list_backups(&destination).unwrap().len(), 2);
}

// ========== Entity Command Tests ==========

#[test]
fn test_cmd_entities_list_empty() {
    let db = setup_test_db();
    let result = commands::cmd_entities_list(&db, false);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_entities_add_person() {
    let db = setup_test_db();
    let result = commands::cmd_entities_add(&db, "John", "person", Some("👤"), Some("#ff0000"));
    assert!(result.is_ok());

    let entities = db.list_entities(false).unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "John");
    assert_eq!(
        entities[0].entity_type,
        hone_core::models::EntityType::Person
    );
    assert_eq!(entities[0].icon.as_deref(), Some("👤"));
    assert_eq!(entities[0].color.as_deref(), Some("#ff0000"));
}

#[test]
fn test_cmd_entities_add_pet() {
    let db = setup_test_db();
    let result = commands::cmd_entities_add(&db, "Rex", "pet", Some("🐕"), None);
    assert!(result.is_ok());

    let entities = db.list_entities(false).unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "Rex");
    assert_eq!(entities[0].entity_type, hone_core::models::EntityType::Pet);
}

#[test]
fn test_cmd_entities_add_vehicle() {
    let db = setup_test_db();
    let result = commands::cmd_entities_add(&db, "Honda Civic", "vehicle", Some("🚗"), None);
    assert!(result.is_ok());

    let entities = db.list_entities(false).unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(
        entities[0].entity_type,
        hone_core::models::EntityType::Vehicle
    );
}

#[test]
fn test_cmd_entities_add_property() {
    let db = setup_test_db();
    let result = commands::cmd_entities_add(&db, "Lake House", "property", Some("🏠"), None);
    assert!(result.is_ok());

    let entities = db.list_entities(false).unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(
        entities[0].entity_type,
        hone_core::models::EntityType::Property
    );
}

#[test]
fn test_cmd_entities_add_invalid_type() {
    let db = setup_test_db();
    let result = commands::cmd_entities_add(&db, "Test", "invalid_type", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("valid types"));
}

#[test]
fn test_cmd_entities_list_with_data() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    commands::cmd_entities_add(&db, "Rex", "pet", None, None).unwrap();

    let result = commands::cmd_entities_list(&db, false);
    assert!(result.is_ok());

    let entities = db.list_entities(false).unwrap();
    assert_eq!(entities.len(), 2);
}

#[test]
fn test_cmd_entities_list_type() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    commands::cmd_entities_add(&db, "Rex", "pet", None, None).unwrap();
    commands::cmd_entities_add(&db, "Sarah", "person", None, None).unwrap();

    let result = commands::cmd_entities_list_type(&db, "person");
    assert!(result.is_ok());

    let people = db
        .list_entities_by_type(hone_core::models::EntityType::Person)
        .unwrap();
    assert_eq!(people.len(), 2);
}

#[test]
fn test_cmd_entities_list_type_invalid() {
    let db = setup_test_db();
    let result = commands::cmd_entities_list_type(&db, "invalid");
    assert!(result.is_err());
}

#[test]
fn test_cmd_entities_update() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    let entities = db.list_entities(false).unwrap();
    let entity_id = entities[0].id;

    let result =
        commands::cmd_entities_update(&db, entity_id, Some("Johnny"), Some("👨"), Some("#0000ff"));
    assert!(result.is_ok());

    let updated = db.get_entity(entity_id).unwrap().unwrap();
    assert_eq!(updated.name, "Johnny");
    assert_eq!(updated.icon.as_deref(), Some("👨"));
    assert_eq!(updated.color.as_deref(), Some("#0000ff"));
}

#[test]
fn test_cmd_entities_update_not_found() {
    let db = setup_test_db();
    let result = commands::cmd_entities_update(&db, 99999, Some("Test"), None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_cmd_entities_archive() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    let entities = db.list_entities(false).unwrap();
    let entity_id = entities[0].id;

    let result = commands::cmd_entities_archive(&db, entity_id);
    assert!(result.is_ok());

    // Should not show in default list
    let active = db.list_entities(false).unwrap();
    assert!(active.is_empty());

    // Should show with include_archived
    let all = db.list_entities(true).unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].archived);
}

#[test]
fn test_cmd_entities_archive_not_found() {
    let db = setup_test_db();
    let result = commands::cmd_entities_archive(&db, 99999);
    assert!(result.is_err());
}

#[test]
fn test_cmd_entities_unarchive() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    let entities = db.list_entities(false).unwrap();
    let entity_id = entities[0].id;

    // Archive then unarchive
    commands::cmd_entities_archive(&db, entity_id).unwrap();
    let result = commands::cmd_entities_unarchive(&db, entity_id);
    assert!(result.is_ok());

    let active = db.list_entities(false).unwrap();
    assert_eq!(active.len(), 1);
    assert!(!active[0].archived);
}

#[test]
fn test_cmd_entities_delete() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    let entities = db.list_entities(false).unwrap();
    let entity_id = entities[0].id;

    let result = commands::cmd_entities_delete(&db, entity_id, false);
    assert!(result.is_ok());

    let remaining = db.list_entities(true).unwrap();
    assert!(remaining.is_empty());
}

#[test]
fn test_cmd_entities_delete_not_found() {
    let db = setup_test_db();
    let result = commands::cmd_entities_delete(&db, 99999, false);
    assert!(result.is_err());
}

#[test]
fn test_resolve_entity_by_id() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();
    let entities = db.list_entities(false).unwrap();
    let entity_id = entities[0].id;

    let result = commands::resolve_entity_arg(&db, &entity_id.to_string());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "John");
}

#[test]
fn test_resolve_entity_by_name() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John Smith", "person", None, None).unwrap();

    let result = commands::resolve_entity_arg(&db, "John Smith");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "John Smith");
}

#[test]
fn test_resolve_entity_by_name_case_insensitive() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "John", "person", None, None).unwrap();

    let result = commands::resolve_entity_arg(&db, "JOHN");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "John");
}

#[test]
fn test_resolve_entity_not_found() {
    let db = setup_test_db();
    let result = commands::resolve_entity_arg(&db, "NonExistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_cmd_entities_list_show_archived() {
    let db = setup_test_db();
    commands::cmd_entities_add(&db, "Active", "person", None, None).unwrap();
    commands::cmd_entities_add(&db, "Archived", "person", None, None).unwrap();
    let entities = db.list_entities(false).unwrap();
    let archived_id = entities.iter().find(|e| e.name == "Archived").unwrap().id;
    db.archive_entity(archived_id).unwrap();

    // Without archived flag
    let result = commands::cmd_entities_list(&db, false);
    assert!(result.is_ok());

    // With archived flag
    let result = commands::cmd_entities_list(&db, true);
    assert!(result.is_ok());
}

// ========== Core Command Tests ==========

#[test]
fn test_cmd_accounts_empty() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    drop(db);

    let result = commands::cmd_accounts(&db_path, true);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_accounts_with_data() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    db.upsert_account("Chase Checking", Bank::Chase, None)
        .unwrap();
    db.upsert_account("Amex Card", Bank::Amex, None).unwrap();
    drop(db);

    let result = commands::cmd_accounts(&db_path, true);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_transactions_empty() {
    let db = setup_test_db();
    let result = commands::cmd_transactions_list(&db, 10);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_transactions_with_data() {
    let db = setup_test_db();
    let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

    // Insert some transactions
    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (?1, '2024-01-15', 'STARBUCKS', -5.50, 'hash1')",
        rusqlite::params![account_id],
    ).unwrap();
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (?1, '2024-01-16', 'PAYCHECK', 2500.00, 'hash2')",
        rusqlite::params![account_id],
    ).unwrap();
    drop(conn);

    let result = commands::cmd_transactions_list(&db, 10);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_transactions_archive_unarchive() {
    let db = setup_test_db();
    let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

    // Insert a transaction
    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (?1, '2024-01-15', 'STARBUCKS', -5.50, 'hash1')",
        rusqlite::params![account_id],
    ).unwrap();
    drop(conn);

    // Get transaction ID
    let txs = db.list_transactions(None, 10, 0).unwrap();
    assert_eq!(txs.len(), 1);
    let tx_id = txs[0].id;
    assert!(!txs[0].archived);

    // Archive it
    let result = commands::cmd_transactions_archive(&db, tx_id);
    assert!(result.is_ok());

    // Verify it's archived
    let txs = db.list_transactions(None, 10, 0).unwrap();
    assert_eq!(txs.len(), 0); // Not in regular list

    let archived = db.list_archived_transactions(10, 0).unwrap();
    assert_eq!(archived.len(), 1);
    assert!(archived[0].archived);

    // Unarchive it
    let result = commands::cmd_transactions_unarchive(&db, tx_id);
    assert!(result.is_ok());

    // Verify it's back
    let txs = db.list_transactions(None, 10, 0).unwrap();
    assert_eq!(txs.len(), 1);
    assert!(!txs[0].archived);
}

#[test]
fn test_cmd_subscriptions_list_empty() {
    let db = setup_test_db();
    let result = commands::cmd_subscriptions_list(&db);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_subscriptions_list_with_data() {
    let db = setup_test_db();

    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
         VALUES ('Netflix', 15.99, 'monthly', '2024-01-01', '2024-06-01', 'active')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
         VALUES ('Spotify', 9.99, 'monthly', '2023-06-01', '2024-06-01', 'zombie')",
        [],
    )
    .unwrap();
    drop(conn);

    let result = commands::cmd_subscriptions_list(&db);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_alerts_empty() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    drop(db);

    let result = commands::cmd_alerts(&db_path, false, true);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_alerts_with_data() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();

    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO alerts (type, subscription_id, message, dismissed)
         VALUES ('zombie', NULL, 'Potential zombie subscription', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO alerts (type, subscription_id, message, dismissed)
         VALUES ('price_increase', NULL, 'Price increased 10%', 1)",
        [],
    )
    .unwrap();
    drop(conn);
    drop(db);

    // Without dismissed
    let result = commands::cmd_alerts(&db_path, false, true);
    assert!(result.is_ok());

    // With dismissed
    let result = commands::cmd_alerts(&db_path, true, true);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cmd_detect_all() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    drop(db);

    let result = commands::cmd_detect(&db_path, "all", true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cmd_detect_zombies() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    drop(db);

    let result = commands::cmd_detect(&db_path, "zombies", true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cmd_detect_increases() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    drop(db);

    let result = commands::cmd_detect(&db_path, "increases", true).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cmd_detect_duplicates() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    drop(db);

    let result = commands::cmd_detect(&db_path, "duplicates", true).await;
    assert!(result.is_ok());
}

#[test]
fn test_cmd_status() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Status on non-existent db
    let result = commands::cmd_status(&db_path, true);
    assert!(result.is_ok());

    // Create database
    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    db.seed_root_tags().unwrap();
    db.upsert_account("Test", Bank::Chase, None).unwrap();
    drop(db);

    // Status on existing db
    let result = commands::cmd_status(&db_path, true);
    assert!(result.is_ok());
}

#[test]
fn test_cmd_init() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let result = commands::cmd_init(&db_path, true);
    assert!(result.is_ok());

    // Verify database was created
    assert!(db_path.exists());

    // Verify tags were seeded
    let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
    let tags = db.list_tags().unwrap();
    assert!(!tags.is_empty());
}

#[test]
fn test_cmd_subscriptions_cancel_with_date() {
    let db = setup_test_db();

    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
         VALUES ('Gym', 50.00, 'monthly', '2024-01-01', '2024-06-01', 'active')",
        [],
    )
    .unwrap();
    drop(conn);

    let result = commands::cmd_subscriptions_cancel(&db, "Gym", Some("2024-06-15"));
    assert!(result.is_ok());

    let conn = db.conn().unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM subscriptions WHERE merchant = 'Gym'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let cancelled_at: Option<String> = conn
        .query_row(
            "SELECT cancelled_at FROM subscriptions WHERE merchant = 'Gym'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "cancelled");
    assert!(cancelled_at.is_some());
}

#[test]
fn test_cmd_subscriptions_cancel_invalid_date() {
    let db = setup_test_db();

    let conn = db.conn().unwrap();
    conn.execute(
        "INSERT INTO subscriptions (merchant, amount, frequency, first_seen, last_seen, status)
         VALUES ('Service', 10.00, 'monthly', '2024-01-01', '2024-06-01', 'active')",
        [],
    )
    .unwrap();
    drop(conn);

    let result = commands::cmd_subscriptions_cancel(&db, "Service", Some("invalid-date"));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid --date format"));
}

#[test]
fn test_open_db_unencrypted() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Create unencrypted
    let result = commands::open_db(&db_path, true);
    assert!(result.is_ok());

    // Open again unencrypted
    let result = commands::open_db(&db_path, true);
    assert!(result.is_ok());
}

// ========== Receipts Command Tests ==========

#[test]
fn test_cmd_receipts_list_empty() {
    let db = setup_test_db();
    let result = commands::cmd_receipts_list(&db, "pending");
    assert!(result.is_ok());
}

#[test]
fn test_cmd_receipts_list_with_data() {
    use hone_core::models::{NewReceipt, ReceiptRole, ReceiptStatus};

    let db = setup_test_db();

    // Create some pending receipts
    let receipt = NewReceipt {
        transaction_id: None,
        image_path: Some("/receipts/test.jpg".to_string()),
        image_data: None,
        status: ReceiptStatus::Pending,
        role: ReceiptRole::Primary,
        receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
        receipt_total: Some(87.43),
        receipt_merchant: Some("Target".to_string()),
        content_hash: Some("test_hash_1".to_string()),
    };
    db.create_receipt_full(&receipt).unwrap();

    let result = commands::cmd_receipts_list(&db, "pending");
    assert!(result.is_ok());
}

#[test]
fn test_cmd_receipts_list_invalid_status() {
    let db = setup_test_db();
    let result = commands::cmd_receipts_list(&db, "invalid_status");
    assert!(result.is_err());
}

#[test]
fn test_cmd_receipts_match() {
    use hone_core::models::{NewReceipt, ReceiptRole, ReceiptStatus};

    let db = setup_test_db();

    // Create transaction
    let (_account_id, tx_id) = create_test_transaction(&db, "TARGET", -87.43);

    // Create pending receipt
    let receipt = NewReceipt {
        transaction_id: None,
        image_path: Some("/receipts/target.jpg".to_string()),
        image_data: None,
        status: ReceiptStatus::Pending,
        role: ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(87.43),
        receipt_merchant: Some("Target".to_string()),
        content_hash: Some("match_test_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    // Match them
    let result = commands::cmd_receipts_match(&db, receipt_id, tx_id);
    assert!(result.is_ok());

    // Verify receipt is now matched
    let updated = db.get_receipt(receipt_id).unwrap().unwrap();
    assert_eq!(updated.status, ReceiptStatus::Matched);
    assert_eq!(updated.transaction_id, Some(tx_id));
}

#[test]
fn test_cmd_receipts_match_not_found() {
    let db = setup_test_db();
    let result = commands::cmd_receipts_match(&db, 999, 999);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_cmd_receipts_status() {
    use hone_core::models::{NewReceipt, ReceiptRole, ReceiptStatus};

    let db = setup_test_db();

    let receipt = NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: ReceiptStatus::Pending,
        role: ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("status_test_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    // Update status
    let result = commands::cmd_receipts_status(&db, receipt_id, "manual_review");
    assert!(result.is_ok());

    // Verify
    let updated = db.get_receipt(receipt_id).unwrap().unwrap();
    assert_eq!(updated.status, ReceiptStatus::ManualReview);
}

#[test]
fn test_cmd_receipts_status_invalid() {
    use hone_core::models::{NewReceipt, ReceiptRole, ReceiptStatus};

    let db = setup_test_db();

    let receipt = NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: ReceiptStatus::Pending,
        role: ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("invalid_status_test_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    let result = commands::cmd_receipts_status(&db, receipt_id, "invalid_status");
    assert!(result.is_err());
}

#[test]
fn test_cmd_receipts_dismiss() {
    use hone_core::models::{NewReceipt, ReceiptRole, ReceiptStatus};

    let db = setup_test_db();

    let receipt = NewReceipt {
        transaction_id: None,
        image_path: None,
        image_data: None,
        status: ReceiptStatus::Pending,
        role: ReceiptRole::Primary,
        receipt_date: None,
        receipt_total: Some(50.00),
        receipt_merchant: None,
        content_hash: Some("dismiss_test_hash".to_string()),
    };
    let receipt_id = db.create_receipt_full(&receipt).unwrap();

    // Dismiss
    let result = commands::cmd_receipts_dismiss(&db, receipt_id);
    assert!(result.is_ok());

    // Verify deleted
    let deleted = db.get_receipt(receipt_id).unwrap();
    assert!(deleted.is_none());
}

#[test]
fn test_cmd_receipts_dismiss_not_found() {
    let db = setup_test_db();
    let result = commands::cmd_receipts_dismiss(&db, 999);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ========== Export/Import Tests ==========

#[test]
fn test_export_transactions_csv_empty() {
    let db = setup_test_db();
    let csv = db
        .export_transactions_csv(&hone_core::export::TransactionExportOptions::default())
        .unwrap();

    // Should have header only
    assert!(csv.starts_with("date,description,amount"));
    assert_eq!(csv.lines().count(), 1); // Header only
}

#[test]
fn test_export_transactions_csv_with_data() {
    let db = setup_test_db();

    // Create some transactions
    let (_, _tx_id1) = create_test_transaction(&db, "STARBUCKS", -5.50);
    let (_, _tx_id2) = create_test_transaction(&db, "AMAZON", -25.00);

    let csv = db
        .export_transactions_csv(&hone_core::export::TransactionExportOptions::default())
        .unwrap();

    // Should have header + 2 data rows
    assert_eq!(csv.lines().count(), 3);
    assert!(csv.contains("STARBUCKS"));
    assert!(csv.contains("AMAZON"));
}

#[test]
fn test_export_full_backup() {
    let db = setup_test_db();

    // Create some data
    let (account_id, tx_id) = create_test_transaction(&db, "TEST MERCHANT", -100.00);

    // Tag the transaction
    let groceries = db.resolve_tag("Groceries").unwrap().unwrap();
    db.add_transaction_tag(
        tx_id,
        groceries.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();

    // Export full backup
    let backup = db.export_full_backup().unwrap();

    // Verify metadata
    assert!(!backup.metadata.version.is_empty());

    // Verify data
    assert!(backup.accounts.iter().any(|a| a.id == account_id));
    assert!(backup.transactions.iter().any(|t| t.id == tx_id));
    assert!(backup.tags.len() >= 15); // Seeded root tags
    assert!(backup
        .transaction_tags
        .iter()
        .any(|tt| tt.transaction_id == tx_id));
}

#[test]
fn test_export_import_round_trip() {
    // Create source database with data
    let db = setup_test_db();
    let (_, tx_id) = create_test_transaction(&db, "NETFLIX", -15.99);
    let groceries = db.resolve_tag("Groceries").unwrap().unwrap();
    db.add_transaction_tag(
        tx_id,
        groceries.id,
        hone_core::models::TagSource::Manual,
        None,
    )
    .unwrap();

    // Export full backup
    let backup = db.export_full_backup().unwrap();

    // Create new empty database
    let target_db = Database::in_memory().unwrap();

    // Import the backup (with clear=true to ensure clean state)
    let stats = target_db.import_full_backup(&backup, true).unwrap();

    // Verify import stats
    assert_eq!(stats.accounts, 1);
    assert_eq!(stats.transactions, 1);
    assert!(stats.tags >= 15);
    assert_eq!(stats.transaction_tags, 1);

    // Verify data was actually imported
    let transactions = target_db.list_transactions(None, 10, 0).unwrap();
    assert_eq!(transactions.len(), 1);
    assert!(transactions[0].description.contains("NETFLIX"));

    // Verify tags were imported
    let tags = target_db.list_tags().unwrap();
    assert!(tags.len() >= 15);

    // Verify transaction tag was imported
    let tx_tags = target_db.get_transaction_tags(transactions[0].id).unwrap();
    assert_eq!(tx_tags.len(), 1);
}

#[test]
fn test_cmd_export_transactions() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let output_path = dir.path().join("export.csv");

    let db = setup_test_db();
    create_test_transaction(&db, "TEST EXPORT", -50.00);

    // Export to file
    let result =
        commands::cmd_export_transactions(&db, Some(output_path.clone()), None, None, None, false);
    assert!(result.is_ok());

    // Verify file was created
    assert!(output_path.exists());

    // Verify contents
    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("TEST EXPORT"));
}

#[test]
fn test_cmd_export_full() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let output_path = dir.path().join("backup.json");

    let db = setup_test_db();
    create_test_transaction(&db, "BACKUP TEST", -75.00);

    // Export full backup
    let result = commands::cmd_export_full(&db, &output_path);
    assert!(result.is_ok());

    // Verify file was created
    assert!(output_path.exists());

    // Verify it's valid JSON
    let contents = std::fs::read_to_string(&output_path).unwrap();
    let backup: hone_core::export::FullBackup = serde_json::from_str(&contents).unwrap();
    assert!(!backup.metadata.version.is_empty());
    assert!(backup
        .transactions
        .iter()
        .any(|t| t.description.contains("BACKUP TEST")));
}

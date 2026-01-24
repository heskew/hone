//! Database tests

use super::*;
use crate::models::*;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn test_in_memory_db() {
        let db = Database::in_memory().unwrap();
        let accounts = db.list_accounts().unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn test_account_crud() {
        let db = Database::in_memory().unwrap();

        let id = db
            .upsert_account("My Chase Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        assert!(id > 0);

        // Upsert same account returns same ID
        let id2 = db
            .upsert_account("My Chase Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        assert_eq!(id, id2);

        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "My Chase Card");
    }

    #[test]
    fn test_tags_schema_exists() {
        let db = Database::in_memory().unwrap();
        let conn = db.conn().unwrap();

        // Verify tags table exists with expected columns
        let result: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tags') WHERE name IN ('id', 'name', 'parent_id', 'color', 'icon', 'auto_patterns', 'created_at')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(result, 7, "tags table should have 7 expected columns");

        // Verify transaction_tags table exists
        let result: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transaction_tags') WHERE name IN ('transaction_id', 'tag_id', 'source', 'confidence', 'created_at')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            result, 5,
            "transaction_tags table should have 5 expected columns"
        );

        // Verify tag_rules table exists
        let result: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tag_rules') WHERE name IN ('id', 'tag_id', 'pattern', 'pattern_type', 'priority', 'created_at')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(result, 6, "tag_rules table should have 6 expected columns");
    }

    #[test]
    fn test_tags_table_constraints() {
        let db = Database::in_memory().unwrap();
        let conn = db.conn().unwrap();

        // Insert a root tag
        conn.execute(
            "INSERT INTO tags (name, parent_id) VALUES ('Transport', NULL)",
            [],
        )
        .unwrap();

        // Insert a child tag
        conn.execute("INSERT INTO tags (name, parent_id) VALUES ('Gas', 1)", [])
            .unwrap();

        // Verify unique constraint (same name, same parent should fail)
        let result = conn.execute("INSERT INTO tags (name, parent_id) VALUES ('Gas', 1)", []);
        assert!(
            result.is_err(),
            "Duplicate tag name under same parent should fail"
        );

        // Same name with different parent should succeed
        conn.execute(
            "INSERT INTO tags (name, parent_id) VALUES ('Gas', NULL)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn test_transaction_tags_cascade_delete() {
        let db = Database::in_memory().unwrap();
        let conn = db.conn().unwrap();

        // Create account and transaction (inline to avoid second pool connection)
        conn.execute(
            "INSERT INTO accounts (name, bank, account_type) VALUES ('Test Account', 'chase', 'checking')",
            [],
        )
        .unwrap();
        let account_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (?, '2024-01-01', 'Test', -10.0, 'hash1')",
            params![account_id],
        )
        .unwrap();
        let tx_id: i64 = conn.last_insert_rowid();

        // Create a tag
        conn.execute(
            "INSERT INTO tags (name, parent_id) VALUES ('Test Tag', NULL)",
            [],
        )
        .unwrap();
        let tag_id: i64 = conn.last_insert_rowid();

        // Link transaction to tag
        conn.execute(
            "INSERT INTO transaction_tags (transaction_id, tag_id, source) VALUES (?, ?, 'manual')",
            params![tx_id, tag_id],
        )
        .unwrap();

        // Verify link exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transaction_tags WHERE transaction_id = ?",
                params![tx_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Delete the tag - should cascade delete the link
        conn.execute("DELETE FROM tags WHERE id = ?", params![tag_id])
            .unwrap();

        // Verify link was deleted
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transaction_tags WHERE transaction_id = ?",
                params![tx_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "Deleting tag should cascade delete transaction_tags"
        );
    }

    #[test]
    fn test_tag_rules_cascade_delete() {
        let db = Database::in_memory().unwrap();
        let conn = db.conn().unwrap();

        // Create a tag
        conn.execute(
            "INSERT INTO tags (name, parent_id) VALUES ('Gas', NULL)",
            [],
        )
        .unwrap();
        let tag_id: i64 = conn.last_insert_rowid();

        // Create a rule for that tag
        conn.execute(
            "INSERT INTO tag_rules (tag_id, pattern, pattern_type, priority) VALUES (?, 'SHELL|CHEVRON', 'contains', 10)",
            params![tag_id],
        )
        .unwrap();

        // Verify rule exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tag_rules WHERE tag_id = ?",
                params![tag_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Delete the tag - should cascade delete the rule
        conn.execute("DELETE FROM tags WHERE id = ?", params![tag_id])
            .unwrap();

        // Verify rule was deleted
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tag_rules", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "Deleting tag should cascade delete tag_rules");
    }

    #[test]
    fn test_seed_root_tags() {
        let db = Database::in_memory().unwrap();

        db.seed_root_tags().unwrap();

        let tags = db.list_root_tags().unwrap();
        assert_eq!(tags.len(), 17, "Should have 17 root tags");

        // Verify some expected tags
        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(tag_names.contains(&"Income"));
        assert!(tag_names.contains(&"Groceries"));
        assert!(tag_names.contains(&"Entertainment"));
        assert!(tag_names.contains(&"Other"));

        // Verify subscription categories were created
        let categories = db.get_subscription_categories().unwrap();
        assert_eq!(categories.len(), 7, "Should have 7 subscription categories");
        let cat_names: Vec<&str> = categories.iter().map(|t| t.name.as_str()).collect();
        assert!(cat_names.contains(&"Streaming"));
        assert!(cat_names.contains(&"Music"));
        assert!(cat_names.contains(&"CloudStorage"));
        assert!(cat_names.contains(&"News"));
        assert!(cat_names.contains(&"Fitness"));
        assert!(cat_names.contains(&"Gaming"));
        assert!(cat_names.contains(&"Software"));

        // Verify idempotency - running again shouldn't create duplicates
        db.seed_root_tags().unwrap();
        let tags_again = db.list_root_tags().unwrap();
        assert_eq!(tags_again.len(), 17, "Should still have 17 root tags");
        let categories_again = db.get_subscription_categories().unwrap();
        assert_eq!(
            categories_again.len(),
            7,
            "Should still have 7 subscription categories"
        );
    }

    #[test]
    fn test_tag_crud() {
        let db = Database::in_memory().unwrap();

        // Create root tag
        let transport_id = db
            .create_tag("Transport", None, Some("#ef4444"), None, Some("GAS|UBER"))
            .unwrap();
        assert!(transport_id > 0);

        // Create child tag
        let gas_id = db
            .create_tag("Gas", Some(transport_id), None, None, Some("SHELL|CHEVRON"))
            .unwrap();
        assert!(gas_id > 0);

        // Get by ID
        let tag = db.get_tag(transport_id).unwrap().unwrap();
        assert_eq!(tag.name, "Transport");
        assert_eq!(tag.color, Some("#ef4444".to_string()));

        // Get by path
        let tag = db.get_tag_by_path("Transport.Gas").unwrap().unwrap();
        assert_eq!(tag.id, gas_id);
        assert_eq!(tag.parent_id, Some(transport_id));

        // Update tag
        db.update_tag(
            transport_id,
            Some("Transportation"),
            None,
            Some(Some("#dc2626")),
            None,
            None,
        )
        .unwrap();
        let updated = db.get_tag(transport_id).unwrap().unwrap();
        assert_eq!(updated.name, "Transportation");
        assert_eq!(updated.color, Some("#dc2626".to_string()));

        // List children
        let children = db.get_tag_children(transport_id).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "Gas");
    }

    #[test]
    fn test_tag_path_resolution() {
        let db = Database::in_memory().unwrap();

        // Create hierarchy (release connection between calls)
        let transport_id = db.create_tag("Transport", None, None, None, None).unwrap();
        db.create_tag("Gas", Some(transport_id), None, None, None)
            .unwrap();

        // Simple name resolution
        let tag = db.resolve_tag("Transport").unwrap().unwrap();
        assert_eq!(tag.name, "Transport");

        // Path resolution
        let tag = db.resolve_tag("Transport.Gas").unwrap().unwrap();
        assert_eq!(tag.name, "Gas");

        // Non-existent
        let tag = db.resolve_tag("NonExistent").unwrap();
        assert!(tag.is_none());
    }

    #[test]
    fn test_tag_ambiguity() {
        let db = Database::in_memory().unwrap();

        // Create two tags with same name at different levels
        let transport_id = db.create_tag("Transport", None, None, None, None).unwrap();
        db.create_tag("Gas", None, None, None, None).unwrap();
        db.create_tag("Gas", Some(transport_id), None, None, None)
            .unwrap();

        // Check ambiguity
        assert!(db.is_tag_name_ambiguous("Gas").unwrap());
        assert!(!db.is_tag_name_ambiguous("Transport").unwrap());

        // Resolve should fail for ambiguous name
        let result = db.resolve_tag("Gas");
        assert!(result.is_err());

        // But path should work
        let tag = db.resolve_tag("Transport.Gas").unwrap().unwrap();
        assert_eq!(tag.parent_id, Some(transport_id));
    }

    #[test]
    fn test_tag_tree() {
        let db = Database::in_memory().unwrap();

        // Create hierarchy
        let transport_id = db.create_tag("Transport", None, None, None, None).unwrap();
        let _gas_id = db
            .create_tag("Gas", Some(transport_id), None, None, None)
            .unwrap();
        let _rideshare_id = db
            .create_tag("Rideshare", Some(transport_id), None, None, None)
            .unwrap();
        let _groceries_id = db.create_tag("Groceries", None, None, None, None).unwrap();

        let tree = db.get_tag_tree().unwrap();
        assert_eq!(tree.len(), 2); // Two root tags

        // Find Transport in tree
        let transport = tree.iter().find(|t| t.tag.name == "Transport").unwrap();
        assert_eq!(transport.depth, 0);
        assert_eq!(transport.path, "Transport");
        assert_eq!(transport.children.len(), 2);

        // Check child paths
        let gas = transport
            .children
            .iter()
            .find(|t| t.tag.name == "Gas")
            .unwrap();
        assert_eq!(gas.depth, 1);
        assert_eq!(gas.path, "Transport.Gas");
    }

    #[test]
    fn test_tag_delete_with_reparent() {
        let db = Database::in_memory().unwrap();

        // Create hierarchy: Transport -> Gas
        let transport_id = db.create_tag("Transport", None, None, None, None).unwrap();
        let gas_id = db
            .create_tag("Gas", Some(transport_id), None, None, None)
            .unwrap();

        // Create transaction and tag it (use scope to release conn)
        let tx_id = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'Test', -50.0, 'hash1')",
                [],
            )
            .unwrap();
            conn.last_insert_rowid()
        };

        // Tag transaction with Gas
        db.add_transaction_tag(tx_id, gas_id, TagSource::Manual, None)
            .unwrap();

        // Delete Gas with reparent
        let result = db.delete_tag(gas_id, true).unwrap();
        assert_eq!(result.transactions_moved, 1);
        assert_eq!(result.children_affected, 0);

        // Transaction should now be tagged with Transport
        let tags = db.get_transaction_tags(tx_id).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag_id, transport_id);
    }

    #[test]
    fn test_merge_tags() {
        let db = Database::in_memory().unwrap();

        // Create two tags
        let source_id = db.create_tag("OldTag", None, None, None, None).unwrap();
        let target_id = db.create_tag("NewTag", None, None, None, None).unwrap();

        // Create transaction and tag it with source (use scope to release conn)
        let tx_id = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'Test', -50.0, 'hash1')",
                [],
            )
            .unwrap();
            conn.last_insert_rowid()
        };

        db.add_transaction_tag(tx_id, source_id, TagSource::Manual, None)
            .unwrap();

        // Merge source into target
        let moved = db.merge_tags(source_id, target_id).unwrap();
        assert_eq!(moved, 1);

        // Source should be deleted
        assert!(db.get_tag(source_id).unwrap().is_none());

        // Transaction should now be tagged with target
        let tags = db.get_transaction_tags(tx_id).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag_id, target_id);
    }

    #[test]
    fn test_transaction_tags() {
        let db = Database::in_memory().unwrap();

        // Setup: create account and transaction (use scope to release conn)
        let tx_id = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'SHELL OIL', -45.0, 'hash1')",
                [],
            )
            .unwrap();
            conn.last_insert_rowid()
        };

        let transport_id = db.create_tag("Transport", None, None, None, None).unwrap();
        let gas_id = db
            .create_tag("Gas", Some(transport_id), None, None, None)
            .unwrap();

        // Add tags
        db.add_transaction_tag(tx_id, transport_id, TagSource::Pattern, Some(0.9))
            .unwrap();
        db.add_transaction_tag(tx_id, gas_id, TagSource::Manual, None)
            .unwrap();

        // Get transaction tags
        let tags = db.get_transaction_tags(tx_id).unwrap();
        assert_eq!(tags.len(), 2);

        // Get transactions by tag
        let txs = db.get_transactions_by_tag(gas_id, false).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].id, tx_id);

        // Get with descendants
        let txs = db.get_transactions_by_tag(transport_id, true).unwrap();
        assert_eq!(txs.len(), 1); // Still 1 because same transaction

        // Count
        let count = db.count_transactions_by_tag(gas_id).unwrap();
        assert_eq!(count, 1);

        // Remove tag
        db.remove_transaction_tag(tx_id, gas_id).unwrap();
        let tags = db.get_transaction_tags(tx_id).unwrap();
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn test_untagged_transactions() {
        let db = Database::in_memory().unwrap();

        // Create account and transactions (use scope to release conn)
        let tx1 = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'Tagged', -10.0, 'hash1')",
                [],
            )
            .unwrap();
            let tx1 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-02', 'Untagged', -20.0, 'hash2')",
                [],
            )
            .unwrap();
            tx1
        };

        // Tag first transaction
        let tag_id = db.create_tag("Test", None, None, None, None).unwrap();
        db.add_transaction_tag(tx1, tag_id, TagSource::Manual, None)
            .unwrap();

        // Get untagged
        let untagged = db.get_untagged_transactions(100).unwrap();
        assert_eq!(untagged.len(), 1);
        assert_eq!(untagged[0].description, "Untagged");
    }

    #[test]
    fn test_tag_rules() {
        let db = Database::in_memory().unwrap();

        // Create tag and rules
        let gas_id = db.create_tag("Gas", None, None, None, None).unwrap();

        let rule1_id = db
            .create_tag_rule(gas_id, "SHELL|CHEVRON", PatternType::Contains, 10)
            .unwrap();
        let rule2_id = db
            .create_tag_rule(gas_id, "^EXXON.*", PatternType::Regex, 5)
            .unwrap();

        assert!(rule1_id > 0);
        assert!(rule2_id > 0);

        // List all rules
        let rules = db.list_tag_rules().unwrap();
        assert_eq!(rules.len(), 2);
        // Should be ordered by priority DESC
        assert_eq!(rules[0].rule.priority, 10);
        assert_eq!(rules[1].rule.priority, 5);

        // Get rules for specific tag
        let tag_rules = db.get_tag_rules(gas_id).unwrap();
        assert_eq!(tag_rules.len(), 2);

        // Delete rule
        db.delete_tag_rule(rule1_id).unwrap();
        let rules = db.list_tag_rules().unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_spending_by_tag() {
        let db = Database::in_memory().unwrap();

        // Create tags
        let transport_id = db.create_tag("Transport", None, None, None, None).unwrap();
        let gas_id = db
            .create_tag("Gas", Some(transport_id), None, None, None)
            .unwrap();

        // Create transactions (use scope to release conn)
        let (tx1, tx2) = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'Shell', -50.0, 'hash1')",
                [],
            )
            .unwrap();
            let tx1 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-02', 'Uber', -25.0, 'hash2')",
                [],
            )
            .unwrap();
            let tx2 = conn.last_insert_rowid();
            (tx1, tx2)
        };

        // Tag transactions
        db.add_transaction_tag(tx1, gas_id, TagSource::Manual, None)
            .unwrap();
        db.add_transaction_tag(tx2, transport_id, TagSource::Manual, None)
            .unwrap();

        // Get spending report
        let spending = db.get_spending_by_tag(None, None).unwrap();
        assert_eq!(spending.len(), 1); // Only Transport (root)

        let transport_spending = &spending[0];
        assert_eq!(transport_spending.tag_name, "Transport");
        assert_eq!(transport_spending.direct_amount, 25.0); // Uber only
        assert_eq!(transport_spending.total_amount, 75.0); // Uber + Gas
    }

    // ========== Report Tests ==========

    #[test]
    fn test_spending_summary() {
        use chrono::NaiveDate;

        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create test data using seeded tags
        let dining_tag = db.resolve_tag("Dining").unwrap().unwrap();
        let transport_tag = db.resolve_tag("Transport").unwrap().unwrap();

        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();

            // Dining transactions
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-15', 'Restaurant', -50.0, 'hash1')",
                [],
            )
            .unwrap();
            let tx1 = conn.last_insert_rowid();

            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-20', 'Another Restaurant', -100.0, 'hash2')",
                [],
            )
            .unwrap();
            let tx2 = conn.last_insert_rowid();

            // Transport transaction
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-25', 'Uber', -30.0, 'hash3')",
                [],
            )
            .unwrap();
            let tx3 = conn.last_insert_rowid();

            // Untagged transaction
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-10', 'Unknown', -20.0, 'hash4')",
                [],
            )
            .unwrap();

            // Tag transactions
            conn.execute(
                "INSERT INTO transaction_tags (transaction_id, tag_id, source) VALUES (?, ?, 'manual')",
                params![tx1, dining_tag.id],
            ).unwrap();
            conn.execute(
                "INSERT INTO transaction_tags (transaction_id, tag_id, source) VALUES (?, ?, 'manual')",
                params![tx2, dining_tag.id],
            ).unwrap();
            conn.execute(
                "INSERT INTO transaction_tags (transaction_id, tag_id, source) VALUES (?, ?, 'manual')",
                params![tx3, transport_tag.id],
            ).unwrap();
        }

        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();

        let summary = db
            .get_spending_summary(from, to, None, false, None, None)
            .unwrap();

        assert_eq!(summary.total, 200.0); // 50 + 100 + 30 + 20
        assert_eq!(summary.untagged.amount, 20.0);
        assert_eq!(summary.untagged.transaction_count, 1);
        assert!(summary.categories.len() >= 2); // At least Dining and Transport
    }

    #[test]
    fn test_spending_trends() {
        use chrono::NaiveDate;

        let db = Database::in_memory().unwrap();

        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();

            // Transactions across multiple months
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-15', 'Jan purchase', -100.0, 'hash1')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-02-15', 'Feb purchase', -150.0, 'hash2')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-03-15', 'Mar purchase', -200.0, 'hash3')",
                [],
            )
            .unwrap();
        }

        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap();

        let report = db
            .get_spending_trends(from, to, Granularity::Monthly, None, None, None)
            .unwrap();

        assert_eq!(report.data.len(), 3);
        assert_eq!(report.data[0].period, "2024-01");
        assert_eq!(report.data[0].amount, 100.0);
        assert_eq!(report.data[1].period, "2024-02");
        assert_eq!(report.data[1].amount, 150.0);
        assert_eq!(report.data[2].period, "2024-03");
        assert_eq!(report.data[2].amount, 200.0);
    }

    #[test]
    fn test_top_merchants() {
        use chrono::NaiveDate;

        let db = Database::in_memory().unwrap();

        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();

            // Multiple transactions at different merchants
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, merchant_normalized, import_hash) VALUES (1, '2024-01-15', 'Amazon Purchase', -300.0, 'AMAZON', 'hash1')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, merchant_normalized, import_hash) VALUES (1, '2024-01-16', 'Walmart', -200.0, 'WALMART', 'hash2')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, merchant_normalized, import_hash) VALUES (1, '2024-01-17', 'Target', -100.0, 'TARGET', 'hash3')",
                [],
            )
            .unwrap();
        }

        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();

        let report = db
            .get_top_merchants(from, to, 10, None, None, None)
            .unwrap();

        assert_eq!(report.merchants.len(), 3);
        // Should be sorted by amount descending
        assert_eq!(report.merchants[0].merchant, "AMAZON");
        assert_eq!(report.merchants[0].amount, 300.0);
        assert_eq!(report.merchants[1].merchant, "WALMART");
        assert_eq!(report.merchants[2].merchant, "TARGET");
    }

    #[test]
    fn test_subscription_cancel_and_savings() {
        use chrono::NaiveDate;

        let db = Database::in_memory().unwrap();

        // Create a subscription and also seed alerts table to avoid issues
        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO subscriptions (merchant, amount, frequency, status, first_seen, last_seen) VALUES ('Netflix', 15.99, 'monthly', 'active', '2023-01-01', '2024-01-01')",
                [],
            )
            .unwrap();
        }

        // Cancel the subscription
        let cancel_date = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        db.cancel_subscription(1, Some(cancel_date)).unwrap();

        // Check savings report (this doesn't rely on alerts table)
        let savings = db.get_savings_report().unwrap();
        assert_eq!(savings.cancelled_count, 1);
        assert_eq!(savings.total_monthly_saved, 15.99);
        assert!(savings.total_savings > 0.0); // Depends on current date
    }

    #[test]
    fn test_find_subscription_by_merchant_or_id() {
        let db = Database::in_memory().unwrap();

        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO subscriptions (merchant, amount, frequency, status) VALUES ('Netflix Streaming', 15.99, 'monthly', 'active')",
                [],
            )
            .unwrap();
        }

        // Find by ID
        let id = db.find_subscription_by_merchant_or_id("1").unwrap();
        assert_eq!(id, Some(1));

        // Find by partial merchant name
        let id = db.find_subscription_by_merchant_or_id("netflix").unwrap();
        assert_eq!(id, Some(1));

        // Not found
        let id = db.find_subscription_by_merchant_or_id("spotify").unwrap();
        assert!(id.is_none());
    }

    #[test]
    fn test_encrypted_database() {
        use std::fs;

        let test_path = "/tmp/hone_test_encrypted.db";

        // Clean up any existing test file
        let _ = fs::remove_file(test_path);

        // Create an encrypted database
        {
            let db = Database::new_with_key(test_path, Some("test-passphrase")).unwrap();

            // Insert some data
            db.upsert_account("Test Account", Bank::Chase, None)
                .unwrap();
            db.seed_root_tags().unwrap();

            let accounts = db.list_accounts().unwrap();
            assert_eq!(accounts.len(), 1);
        }

        // Verify we can open it with the same key
        {
            let db = Database::new_with_key(test_path, Some("test-passphrase")).unwrap();
            let accounts = db.list_accounts().unwrap();
            assert_eq!(accounts.len(), 1);
        }

        // Verify opening without key fails (file is actually encrypted)
        {
            let result = Database::new_with_key(test_path, None);
            assert!(
                result.is_err(),
                "Should fail to open encrypted db without key"
            );
        }

        // Verify opening with wrong key fails
        {
            let result = Database::new_with_key(test_path, Some("wrong-passphrase"));
            assert!(
                result.is_err(),
                "Should fail to open encrypted db with wrong key"
            );
        }

        // Clean up
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_key_derivation_is_deterministic() {
        let key1 = derive_key("my-secret").unwrap();
        let key2 = derive_key("my-secret").unwrap();
        assert_eq!(key1, key2);

        // Different passphrase = different key
        let key3 = derive_key("other-secret").unwrap();
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_encryption_required_by_default() {
        use std::env;
        use std::fs;

        let test_path = "/tmp/hone_test_encryption_required.db";

        // Clean up any existing test file
        let _ = fs::remove_file(test_path);

        // Ensure HONE_DB_KEY is not set for this test
        env::remove_var(DB_KEY_ENV);

        // Database::new() should fail without HONE_DB_KEY
        let result = Database::new(test_path);
        assert!(
            result.is_err(),
            "Database::new() should fail without HONE_DB_KEY"
        );

        let err_msg = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(
            err_msg.contains("encryption required") || err_msg.contains(DB_KEY_ENV),
            "Error should mention encryption requirement: {}",
            err_msg
        );

        // new_unencrypted() should succeed
        let result = Database::new_unencrypted(test_path);
        assert!(result.is_ok(), "new_unencrypted() should succeed");

        // Clean up
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_encrypted_vs_unencrypted_incompatible() {
        use std::fs;

        let test_path = "/tmp/hone_test_encrypted_vs_unencrypted.db";

        // Clean up any existing test file
        let _ = fs::remove_file(test_path);

        // Create an encrypted database with explicit key
        {
            let db = Database::new_with_key(test_path, Some("test-secret-key")).unwrap();
            db.upsert_account("Test", Bank::Chase, None).unwrap();
        }

        // Try to open with unencrypted - should fail because DB is encrypted
        let result = Database::new_unencrypted(test_path);
        assert!(
            result.is_err(),
            "Should fail to open encrypted db without key"
        );

        // Clean up
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_unencrypted_database_roundtrip() {
        use std::fs;

        let test_path = "/tmp/hone_test_unencrypted.db";

        // Clean up any existing test file
        let _ = fs::remove_file(test_path);

        // Create unencrypted database
        {
            let db = Database::new_unencrypted(test_path).unwrap();
            db.upsert_account("Test Account", Bank::Chase, None)
                .unwrap();
            db.seed_root_tags().unwrap();

            let accounts = db.list_accounts().unwrap();
            assert_eq!(accounts.len(), 1);
        }

        // Reopen unencrypted database
        {
            let db = Database::new_unencrypted(test_path).unwrap();
            let accounts = db.list_accounts().unwrap();
            assert_eq!(accounts.len(), 1);
        }

        // Clean up
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_entity_crud() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create entity
        let entity_id = db
            .create_entity(&crate::models::NewEntity {
                name: "John Doe".to_string(),
                entity_type: crate::models::EntityType::Person,
                icon: Some("👤".to_string()),
                color: Some("#FF0000".to_string()),
            })
            .unwrap();
        assert!(entity_id > 0);

        // Get entity
        let entity = db.get_entity(entity_id).unwrap().unwrap();
        assert_eq!(entity.name, "John Doe");
        assert_eq!(entity.entity_type, crate::models::EntityType::Person);
        assert_eq!(entity.icon, Some("👤".to_string()));
        assert!(!entity.archived);

        // List entities
        let entities = db.list_entities(false).unwrap();
        assert_eq!(entities.len(), 1);

        // Archive entity
        db.archive_entity(entity_id).unwrap();
        let entities = db.list_entities(false).unwrap();
        assert_eq!(entities.len(), 0);

        // Include archived
        let entities = db.list_entities(true).unwrap();
        assert_eq!(entities.len(), 1);
        assert!(entities[0].archived);

        // Unarchive entity
        db.unarchive_entity(entity_id).unwrap();
        let entity = db.get_entity(entity_id).unwrap().unwrap();
        assert!(!entity.archived);

        // Delete entity
        db.delete_entity(entity_id).unwrap();
        let entity = db.get_entity(entity_id).unwrap();
        assert!(entity.is_none());
    }

    #[test]
    fn test_entity_types() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create entities of different types
        let person_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Alice".to_string(),
                entity_type: crate::models::EntityType::Person,
                icon: None,
                color: None,
            })
            .unwrap();

        let vehicle_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Honda Civic".to_string(),
                entity_type: crate::models::EntityType::Vehicle,
                icon: None,
                color: None,
            })
            .unwrap();

        let pet_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Fluffy".to_string(),
                entity_type: crate::models::EntityType::Pet,
                icon: None,
                color: None,
            })
            .unwrap();

        let property_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Beach House".to_string(),
                entity_type: crate::models::EntityType::Property,
                icon: None,
                color: None,
            })
            .unwrap();

        // List by type
        let people = db
            .list_entities_by_type(crate::models::EntityType::Person)
            .unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].id, person_id);

        let vehicles = db
            .list_entities_by_type(crate::models::EntityType::Vehicle)
            .unwrap();
        assert_eq!(vehicles.len(), 1);
        assert_eq!(vehicles[0].id, vehicle_id);

        let pets = db
            .list_entities_by_type(crate::models::EntityType::Pet)
            .unwrap();
        assert_eq!(pets.len(), 1);
        assert_eq!(pets[0].id, pet_id);

        let properties = db
            .list_entities_by_type(crate::models::EntityType::Property)
            .unwrap();
        assert_eq!(properties.len(), 1);
        assert_eq!(properties[0].id, property_id);
    }

    #[test]
    fn test_location_crud() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create location
        let location_id = db
            .create_location(&crate::models::NewLocation {
                name: Some("Office".to_string()),
                address: Some("123 Main St".to_string()),
                city: Some("Austin".to_string()),
                state: Some("TX".to_string()),
                country: Some("USA".to_string()),
                latitude: None,
                longitude: None,
                location_type: Some(crate::models::LocationType::Work),
            })
            .unwrap();
        assert!(location_id > 0);

        // Get location
        let location = db.get_location(location_id).unwrap().unwrap();
        assert_eq!(location.name, Some("Office".to_string()));
        assert_eq!(location.address, Some("123 Main St".to_string()));
        assert_eq!(
            location.location_type,
            Some(crate::models::LocationType::Work)
        );

        // List locations
        let locations = db.list_locations().unwrap();
        assert_eq!(locations.len(), 1);

        // Delete location
        db.delete_location(location_id).unwrap();
        let location = db.get_location(location_id).unwrap();
        assert!(location.is_none());
    }

    #[test]
    fn test_trip_crud() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create trip
        let trip_id = db
            .create_trip(&crate::models::NewTrip {
                name: "Paris Vacation".to_string(),
                description: Some("Summer trip to Paris".to_string()),
                start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
                location_id: None,
                budget: Some(5000.0),
            })
            .unwrap();
        assert!(trip_id > 0);

        // Get trip
        let trip = db.get_trip(trip_id).unwrap().unwrap();
        assert_eq!(trip.name, "Paris Vacation");
        assert_eq!(trip.budget, Some(5000.0));
        assert!(!trip.archived);

        // List trips
        let trips = db.list_trips(false).unwrap();
        assert_eq!(trips.len(), 1);

        // Archive trip
        db.archive_trip(trip_id).unwrap();
        let trips = db.list_trips(false).unwrap();
        assert_eq!(trips.len(), 0);

        let trips = db.list_trips(true).unwrap();
        assert_eq!(trips.len(), 1);
        assert!(trips[0].archived);

        // Delete trip
        db.delete_trip(trip_id).unwrap();
        let trip = db.get_trip(trip_id).unwrap();
        assert!(trip.is_none());
    }

    #[test]
    fn test_mileage_log_crud() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create vehicle entity first
        let vehicle_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Toyota Camry".to_string(),
                entity_type: crate::models::EntityType::Vehicle,
                icon: None,
                color: None,
            })
            .unwrap();

        // Create mileage log
        let log_id = db
            .create_mileage_log(&crate::models::NewMileageLog {
                entity_id: vehicle_id,
                date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                odometer: 25000.0,
                note: Some("Start of year".to_string()),
            })
            .unwrap();
        assert!(log_id > 0);

        // Get mileage log
        let log = db.get_mileage_log(log_id).unwrap().unwrap();
        assert_eq!(log.entity_id, vehicle_id);
        assert_eq!(log.odometer, 25000.0);
        assert_eq!(log.note, Some("Start of year".to_string()));

        // List logs for vehicle
        let logs = db.get_mileage_logs(vehicle_id).unwrap();
        assert_eq!(logs.len(), 1);

        // Add another log and check total miles
        db.create_mileage_log(&crate::models::NewMileageLog {
            entity_id: vehicle_id,
            date: chrono::NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            odometer: 35000.0,
            note: Some("Mid year".to_string()),
        })
        .unwrap();

        let total = db.get_vehicle_total_miles(vehicle_id).unwrap();
        assert_eq!(total, Some(10000.0));

        // Delete mileage log
        db.delete_mileage_log(log_id).unwrap();
        let log = db.get_mileage_log(log_id).unwrap();
        assert!(log.is_none());
    }

    #[test]
    fn test_transaction_splits() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create account and transaction
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Target Purchase".to_string(),
                    amount: -150.00,
                    category: None,
                    import_hash: "hash_target_purchase".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Create entity
        let entity_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Kids".to_string(),
                entity_type: crate::models::EntityType::Person,
                icon: None,
                color: None,
            })
            .unwrap();

        // Create splits
        let split1_id = db
            .create_split(&crate::models::NewTransactionSplit {
                transaction_id: tx_id,
                amount: 100.00,
                description: Some("Groceries".to_string()),
                entity_id: None,
                purchaser_id: None,
                split_type: crate::models::SplitType::Item,
            })
            .unwrap();

        let _split2_id = db
            .create_split(&crate::models::NewTransactionSplit {
                transaction_id: tx_id,
                amount: 50.00,
                description: Some("Toys".to_string()),
                entity_id: Some(entity_id),
                purchaser_id: None,
                split_type: crate::models::SplitType::Item,
            })
            .unwrap();

        // Get splits for transaction
        let splits = db.get_splits_for_transaction(tx_id).unwrap();
        assert_eq!(splits.len(), 2);

        // Get split by id
        let split = db.get_split_by_id(split1_id).unwrap().unwrap();
        assert_eq!(split.amount, 100.00);
        assert_eq!(split.description, Some("Groceries".to_string()));

        // Get splits with details
        let splits_detail = db.get_splits_with_details(tx_id).unwrap();
        assert_eq!(splits_detail.len(), 2);

        // Count splits by entity
        let count = db.count_splits_by_entity(entity_id).unwrap();
        assert_eq!(count, 1);

        // Delete split
        db.delete_split(split1_id).unwrap();
        let splits = db.get_splits_for_transaction(tx_id).unwrap();
        assert_eq!(splits.len(), 1);

        // Delete all splits for transaction
        db.delete_splits_for_transaction(tx_id).unwrap();
        let splits = db.get_splits_for_transaction(tx_id).unwrap();
        assert_eq!(splits.len(), 0);
    }

    #[test]
    fn test_split_tags() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create account, transaction, and split
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Purchase".to_string(),
                    amount: -50.00,
                    category: None,
                    import_hash: "hash_purchase".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        let split_id = db
            .create_split(&crate::models::NewTransactionSplit {
                transaction_id: tx_id,
                amount: 50.00,
                description: Some("Item".to_string()),
                entity_id: None,
                purchaser_id: None,
                split_type: crate::models::SplitType::Item,
            })
            .unwrap();

        // Get a tag
        let tags = db.list_tags().unwrap();
        let tag = tags.first().unwrap();

        // Add tag to split
        db.add_split_tag(split_id, tag.id, crate::models::TagSource::Manual, None)
            .unwrap();

        // Remove tag from split
        db.remove_split_tag(split_id, tag.id).unwrap();
    }

    #[test]
    fn test_trip_transactions() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create trip
        let trip_id = db
            .create_trip(&crate::models::NewTrip {
                name: "Business Trip".to_string(),
                description: None,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
                end_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 3, 5).unwrap()),
                location_id: None,
                budget: None,
            })
            .unwrap();

        // Create account and transactions
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx1_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 3, 2).unwrap(),
                    description: "Hotel".to_string(),
                    amount: -200.00,
                    category: None,
                    import_hash: "hash_hotel".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        let tx2_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 3, 3).unwrap(),
                    description: "Dinner".to_string(),
                    amount: -50.00,
                    category: None,
                    import_hash: "hash_dinner".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Assign transactions to trip
        db.assign_transaction_to_trip(tx1_id, Some(trip_id))
            .unwrap();
        db.assign_transaction_to_trip(tx2_id, Some(trip_id))
            .unwrap();

        // Get trip transactions
        let transactions = db.get_trip_transactions(trip_id).unwrap();
        assert_eq!(transactions.len(), 2);

        // Get trip spending
        let (total, count) = db.get_trip_spending(trip_id).unwrap();
        assert_eq!(total, 250.00);
        assert_eq!(count, 2);

        // Unassign transaction from trip
        db.assign_transaction_to_trip(tx1_id, None).unwrap();
        let transactions = db.get_trip_transactions(trip_id).unwrap();
        assert_eq!(transactions.len(), 1);
    }

    #[test]
    fn test_receipt_crud() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create account and transaction
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Grocery Store".to_string(),
                    amount: -75.00,
                    category: None,
                    import_hash: "hash_grocery_store".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Create receipt
        let receipt_id = db
            .create_receipt(tx_id, Some("/path/to/receipt.jpg"))
            .unwrap();
        assert!(receipt_id > 0);

        // Get receipt
        let receipt = db.get_receipt(receipt_id).unwrap().unwrap();
        assert_eq!(receipt.transaction_id, Some(tx_id));
        assert_eq!(receipt.image_path, Some("/path/to/receipt.jpg".to_string()));
        assert!(receipt.parsed_json.is_none());
        assert_eq!(receipt.status, ReceiptStatus::Matched);
        assert_eq!(receipt.role, ReceiptRole::Primary);

        // Update receipt with parsed data
        db.update_receipt_parsed(receipt_id, r#"{"items":[]}"#)
            .unwrap();
        let receipt = db.get_receipt(receipt_id).unwrap().unwrap();
        assert_eq!(receipt.parsed_json, Some(r#"{"items":[]}"#.to_string()));

        // Get receipts for transaction
        let receipts = db.get_receipts_for_transaction(tx_id).unwrap();
        assert_eq!(receipts.len(), 1);

        // Delete receipt
        db.delete_receipt(receipt_id).unwrap();
        let receipt = db.get_receipt(receipt_id).unwrap();
        assert!(receipt.is_none());
    }

    #[test]
    fn test_spending_by_location() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create location
        let location_id = db
            .create_location(&crate::models::NewLocation {
                name: Some("Target".to_string()),
                address: None,
                city: Some("Austin".to_string()),
                state: Some("TX".to_string()),
                country: Some("USA".to_string()),
                latitude: None,
                longitude: None,
                location_type: Some(crate::models::LocationType::Store),
            })
            .unwrap();

        // Create account and transactions with location
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Target Purchase".to_string(),
                    amount: -100.00,
                    category: None,
                    import_hash: "hash_target_location".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Associate transaction with location
        db.update_transaction_location(tx_id, Some(location_id), None)
            .unwrap();

        // Get spending by location
        let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let spending = db.get_spending_by_location(from, to).unwrap();
        assert!(!spending.is_empty());
        assert_eq!(spending[0].location_id, location_id);
        assert_eq!(spending[0].total_spent, 100.0);
    }

    #[test]
    fn test_spending_by_entity() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create entity
        let entity_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Kids".to_string(),
                entity_type: crate::models::EntityType::Person,
                icon: None,
                color: None,
            })
            .unwrap();

        // Create account and transactions with entity
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Toy Store".to_string(),
                    amount: -50.00,
                    category: None,
                    import_hash: "hash_toy_store".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Create split with entity
        db.create_split(&crate::models::NewTransactionSplit {
            transaction_id: tx_id,
            amount: 50.00,
            description: None,
            entity_id: Some(entity_id),
            purchaser_id: None,
            split_type: crate::models::SplitType::Item,
        })
        .unwrap();

        // Get spending by entity
        let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let spending = db.get_spending_by_entity(from, to).unwrap();
        assert!(!spending.is_empty());
        assert_eq!(spending[0].0.id, entity_id);
    }

    #[test]
    fn test_update_entity() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create entity
        let entity_id = db
            .create_entity(&crate::models::NewEntity {
                name: "Old Name".to_string(),
                entity_type: crate::models::EntityType::Person,
                icon: None,
                color: None,
            })
            .unwrap();

        // Update entity using raw SQL since update_entity may not exist
        let conn = db.conn().unwrap();
        conn.execute(
            "UPDATE entities SET name = ? WHERE id = ?",
            rusqlite::params!["New Name", entity_id],
        )
        .unwrap();

        // Verify update
        let entity = db.get_entity(entity_id).unwrap().unwrap();
        assert_eq!(entity.name, "New Name");
    }

    #[test]
    fn test_update_trip() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create trip
        let trip_id = db
            .create_trip(&crate::models::NewTrip {
                name: "Original Trip".to_string(),
                description: None,
                start_date: None,
                end_date: None,
                location_id: None,
                budget: None,
            })
            .unwrap();

        // Update trip using raw SQL
        let conn = db.conn().unwrap();
        conn.execute(
            "UPDATE trips SET name = ?, budget = ? WHERE id = ?",
            rusqlite::params!["Updated Trip", 1000.0, trip_id],
        )
        .unwrap();

        // Verify update
        let trip = db.get_trip(trip_id).unwrap().unwrap();
        assert_eq!(trip.name, "Updated Trip");
        assert_eq!(trip.budget, Some(1000.0));
    }

    // ========== Receipt Workflow Tests ==========

    #[test]
    fn test_create_receipt_full() {
        let db = Database::in_memory().unwrap();

        // Create pending receipt without transaction (receipt-first workflow)
        let new_receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/receipts/target_2024.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            receipt_total: Some(87.43),
            receipt_merchant: Some("Target".to_string()),
            content_hash: Some("abc123hash".to_string()),
        };

        let receipt_id = db.create_receipt_full(&new_receipt).unwrap();
        assert!(receipt_id > 0);

        // Verify receipt was created with all fields
        let receipt = db.get_receipt(receipt_id).unwrap().unwrap();
        assert!(receipt.transaction_id.is_none());
        assert_eq!(
            receipt.image_path,
            Some("/receipts/target_2024.jpg".to_string())
        );
        assert_eq!(receipt.status, ReceiptStatus::Pending);
        assert_eq!(receipt.role, ReceiptRole::Primary);
        assert_eq!(
            receipt.receipt_date,
            Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
        assert_eq!(receipt.receipt_total, Some(87.43));
        assert_eq!(receipt.receipt_merchant, Some("Target".to_string()));
        assert_eq!(receipt.content_hash, Some("abc123hash".to_string()));
    }

    #[test]
    fn test_receipt_deduplication_by_hash() {
        let db = Database::in_memory().unwrap();

        let hash = "unique_content_hash_12345";

        // Create first receipt with hash
        let receipt1 = NewReceipt {
            transaction_id: None,
            image_path: Some("/receipts/first.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: Some(50.00),
            receipt_merchant: Some("Store A".to_string()),
            content_hash: Some(hash.to_string()),
        };
        let id1 = db.create_receipt_full(&receipt1).unwrap();

        // Look up by hash
        let found = db.get_receipt_by_hash(hash).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id1);

        // Look up non-existent hash
        let not_found = db.get_receipt_by_hash("nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_pending_receipts() {
        let db = Database::in_memory().unwrap();

        // Create receipts with different statuses
        let pending1 = NewReceipt {
            transaction_id: None,
            image_path: None,
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 10).unwrap()),
            receipt_total: Some(25.00),
            receipt_merchant: Some("Merchant A".to_string()),
            content_hash: Some("hash1".to_string()),
        };

        let pending2 = NewReceipt {
            transaction_id: None,
            image_path: None,
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 12).unwrap()),
            receipt_total: Some(75.00),
            receipt_merchant: Some("Merchant B".to_string()),
            content_hash: Some("hash2".to_string()),
        };

        let matched = NewReceipt {
            transaction_id: None,
            image_path: None,
            image_data: None,
            status: ReceiptStatus::Matched,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: Some(100.00),
            receipt_merchant: Some("Merchant C".to_string()),
            content_hash: Some("hash3".to_string()),
        };

        db.create_receipt_full(&pending1).unwrap();
        db.create_receipt_full(&pending2).unwrap();
        db.create_receipt_full(&matched).unwrap();

        // Get pending receipts
        let pending = db.get_pending_receipts().unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().all(|r| r.status == ReceiptStatus::Pending));
    }

    #[test]
    fn test_get_receipts_by_status() {
        let db = Database::in_memory().unwrap();

        // Create receipts with various statuses
        let statuses = [
            ReceiptStatus::Pending,
            ReceiptStatus::Pending,
            ReceiptStatus::ManualReview,
            ReceiptStatus::Orphaned,
        ];

        for (i, status) in statuses.iter().enumerate() {
            let receipt = NewReceipt {
                transaction_id: None,
                image_path: None,
                image_data: None,
                status: *status,
                role: ReceiptRole::Primary,
                receipt_date: None,
                receipt_total: Some((i + 1) as f64 * 10.0),
                receipt_merchant: None,
                content_hash: Some(format!("status_hash_{}", i)),
            };
            db.create_receipt_full(&receipt).unwrap();
        }

        // Query by status
        assert_eq!(
            db.get_receipts_by_status(ReceiptStatus::Pending)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            db.get_receipts_by_status(ReceiptStatus::ManualReview)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.get_receipts_by_status(ReceiptStatus::Orphaned)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.get_receipts_by_status(ReceiptStatus::Matched)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn test_update_receipt_parsed_data() {
        let db = Database::in_memory().unwrap();

        // Create receipt without parsed data
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/receipts/unparsed.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: None,
            receipt_merchant: None,
            content_hash: Some("parse_test_hash".to_string()),
        };
        let id = db.create_receipt_full(&receipt).unwrap();

        // Verify initially unparsed
        let unparsed = db.get_receipt(id).unwrap().unwrap();
        assert!(unparsed.parsed_json.is_none());
        assert!(unparsed.parsed_at.is_none());
        assert!(unparsed.receipt_merchant.is_none());
        assert!(unparsed.receipt_date.is_none());
        assert!(unparsed.receipt_total.is_none());

        // Update with parsed data (simulating LLM extraction)
        let parsed_json = r#"{"merchant":"Costco","items":[{"name":"Paper Towels","amount":15.99}],"total":47.82}"#;
        let parsed_date = chrono::NaiveDate::from_ymd_opt(2024, 1, 20).unwrap();
        db.update_receipt_parsed_data(
            id,
            parsed_json,
            Some("Costco"),
            Some(parsed_date),
            Some(47.82),
        )
        .unwrap();

        // Verify all fields updated
        let parsed = db.get_receipt(id).unwrap().unwrap();
        assert_eq!(parsed.parsed_json, Some(parsed_json.to_string()));
        assert!(parsed.parsed_at.is_some());
        assert_eq!(parsed.receipt_merchant, Some("Costco".to_string()));
        assert_eq!(parsed.receipt_date, Some(parsed_date));
        assert_eq!(parsed.receipt_total, Some(47.82));
    }

    #[test]
    fn test_update_receipt_status() {
        let db = Database::in_memory().unwrap();

        // Create pending receipt
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: None,
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: Some(100.00),
            receipt_merchant: Some("Test Store".to_string()),
            content_hash: Some("status_update_hash".to_string()),
        };
        let id = db.create_receipt_full(&receipt).unwrap();

        // Verify initial status
        let r = db.get_receipt(id).unwrap().unwrap();
        assert_eq!(r.status, ReceiptStatus::Pending);

        // Update to manual review
        db.update_receipt_status(id, ReceiptStatus::ManualReview)
            .unwrap();
        let r = db.get_receipt(id).unwrap().unwrap();
        assert_eq!(r.status, ReceiptStatus::ManualReview);

        // Update to orphaned
        db.update_receipt_status(id, ReceiptStatus::Orphaned)
            .unwrap();
        let r = db.get_receipt(id).unwrap().unwrap();
        assert_eq!(r.status, ReceiptStatus::Orphaned);
    }

    #[test]
    fn test_link_receipt_to_transaction() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create transaction
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "TARGET #1234 AUSTIN TX".to_string(),
                    amount: -87.43,
                    category: None,
                    import_hash: "target_import_hash".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Create pending receipt (receipt-first workflow)
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/receipts/target.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            receipt_total: Some(87.43),
            receipt_merchant: Some("Target".to_string()),
            content_hash: Some("link_test_hash".to_string()),
        };
        let receipt_id = db.create_receipt_full(&receipt).unwrap();

        // Verify receipt is pending and unlinked
        let r = db.get_receipt(receipt_id).unwrap().unwrap();
        assert_eq!(r.status, ReceiptStatus::Pending);
        assert!(r.transaction_id.is_none());

        // Link receipt to transaction
        db.link_receipt_to_transaction(receipt_id, tx_id).unwrap();

        // Verify receipt is now matched and linked
        let r = db.get_receipt(receipt_id).unwrap().unwrap();
        assert_eq!(r.status, ReceiptStatus::Matched);
        assert_eq!(r.transaction_id, Some(tx_id));

        // Verify receipt appears in transaction's receipts
        let tx_receipts = db.get_receipts_for_transaction(tx_id).unwrap();
        assert_eq!(tx_receipts.len(), 1);
        assert_eq!(tx_receipts[0].id, receipt_id);
    }

    #[test]
    fn test_transaction_source_and_expected_amount() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create account
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();

        // Insert a regular transaction (source defaults to 'import')
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Regular Purchase".to_string(),
                    amount: -50.00,
                    category: None,
                    import_hash: "source_test_hash".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Get transaction and verify default source
        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert_eq!(tx.source, TransactionSource::Import);
        assert!(tx.expected_amount.is_none());

        // Update transaction to set source and expected_amount (tip scenario)
        let conn = db.conn().unwrap();
        conn.execute(
            "UPDATE transactions SET source = 'receipt', expected_amount = 42.50 WHERE id = ?",
            params![tx_id],
        )
        .unwrap();
        drop(conn);

        // Verify updated fields
        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert_eq!(tx.source, TransactionSource::Receipt);
        assert_eq!(tx.expected_amount, Some(42.50));
    }

    #[test]
    fn test_receipt_roles() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create transaction
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    description: "Restaurant".to_string(),
                    amount: -75.00,
                    category: None,
                    import_hash: "roles_test_hash".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Create primary receipt (itemized bill)
        let primary = NewReceipt {
            transaction_id: Some(tx_id),
            image_path: Some("/receipts/itemized_bill.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Matched,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: Some(65.00),
            receipt_merchant: Some("Restaurant".to_string()),
            content_hash: Some("primary_hash".to_string()),
        };

        // Create supplementary receipt (signed credit card slip)
        let supplementary = NewReceipt {
            transaction_id: Some(tx_id),
            image_path: Some("/receipts/cc_slip.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Matched,
            role: ReceiptRole::Supplementary,
            receipt_date: None,
            receipt_total: Some(75.00), // Includes tip
            receipt_merchant: None,
            content_hash: Some("supplementary_hash".to_string()),
        };

        db.create_receipt_full(&primary).unwrap();
        db.create_receipt_full(&supplementary).unwrap();

        // Get receipts for transaction - should be ordered by role (primary first)
        let receipts = db.get_receipts_for_transaction(tx_id).unwrap();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].role, ReceiptRole::Primary);
        assert_eq!(receipts[1].role, ReceiptRole::Supplementary);
    }

    #[test]
    fn test_receipt_with_image_data() {
        let db = Database::in_memory().unwrap();

        // Create receipt with embedded image data (small receipt scenario)
        let image_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]; // Fake JPEG header
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: None,
            image_data: Some(image_bytes.clone()),
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: Some(15.00),
            receipt_merchant: Some("Coffee Shop".to_string()),
            content_hash: Some("image_data_hash".to_string()),
        };

        let id = db.create_receipt_full(&receipt).unwrap();

        // Note: image_data is stored but not retrieved by row_to_receipt
        // This is intentional - we don't want to load large blobs unnecessarily
        let r = db.get_receipt(id).unwrap().unwrap();
        assert!(r.image_path.is_none()); // No file path, data is in DB
        assert_eq!(r.receipt_merchant, Some("Coffee Shop".to_string()));
    }

    #[test]
    fn test_receipt_auto_matching_exact_match() {
        let db = Database::in_memory().unwrap();

        // Create account and transaction
        let account_id = db
            .upsert_account("Test Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        let tx = NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "STARBUCKS STORE 12345".to_string(),
            amount: -5.75,
            category: None,
            import_hash: "tx_hash_1".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
        db.update_merchant_normalized(1, "Starbucks").unwrap();

        // Create a pending receipt with exact matching data
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/path/to/receipt.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
            receipt_total: Some(5.75),
            receipt_merchant: Some("Starbucks".to_string()),
            content_hash: Some("hash_1".to_string()),
        };
        let receipt_id = db.create_receipt_full(&receipt).unwrap();

        // Find matching transactions
        let r = db.get_receipt(receipt_id).unwrap().unwrap();
        let candidates = db.find_matching_transactions(&r).unwrap();

        assert!(!candidates.is_empty());
        assert!(
            candidates[0].score >= 0.85,
            "Exact match should have high score"
        );
        assert_eq!(candidates[0].match_factors.amount_diff, 0.0);
        assert_eq!(candidates[0].match_factors.days_diff, 0);
    }

    #[test]
    fn test_receipt_auto_matching_with_tip() {
        let db = Database::in_memory().unwrap();

        // Create account and transaction (includes tip)
        let account_id = db
            .upsert_account("Test Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        let tx = NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "RESTAURANT XYZ".to_string(),
            amount: -55.00, // Bill + tip
            category: None,
            import_hash: "tx_hash_2".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();

        // Create receipt without tip
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/path/to/receipt.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
            receipt_total: Some(45.00), // Before tip
            receipt_merchant: Some("Restaurant XYZ".to_string()),
            content_hash: Some("hash_2".to_string()),
        };
        let receipt_id = db.create_receipt_full(&receipt).unwrap();

        // Find matching transactions - should still match within tolerance
        let r = db.get_receipt(receipt_id).unwrap().unwrap();
        let candidates = db.find_matching_transactions(&r).unwrap();

        assert!(!candidates.is_empty());
        // $10 tip on $45 is ~22%, just over 20% tolerance, but within $5 fixed
        // Actually $10 > $5 fixed, so may not match. Let's check:
        assert_eq!(candidates[0].match_factors.amount_diff, 10.0);
    }

    #[test]
    fn test_receipt_auto_matching_date_window() {
        let db = Database::in_memory().unwrap();

        // Create account and transaction
        let account_id = db
            .upsert_account("Test Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        let tx = NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 12).unwrap(), // 2 days after receipt
            description: "AMAZON PURCHASE".to_string(),
            amount: -25.00,
            category: None,
            import_hash: "tx_hash_3".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();

        // Create receipt dated 2 days before transaction
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/path/to/receipt.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
            receipt_total: Some(25.00),
            receipt_merchant: Some("Amazon".to_string()),
            content_hash: Some("hash_3".to_string()),
        };
        let receipt_id = db.create_receipt_full(&receipt).unwrap();

        // Should find match within 3-day window
        let r = db.get_receipt(receipt_id).unwrap().unwrap();
        let candidates = db.find_matching_transactions(&r).unwrap();

        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].match_factors.days_diff, 2);
        assert!(candidates[0].match_factors.date_score >= 0.5); // 2 days = 0.5 score
    }

    #[test]
    fn test_receipt_auto_match_function() {
        let db = Database::in_memory().unwrap();

        // Create account and transaction
        let account_id = db
            .upsert_account("Test Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        let tx = NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "TARGET STORE 789".to_string(),
            amount: -50.00,
            category: None,
            import_hash: "tx_hash_4".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();
        db.update_merchant_normalized(1, "Target").unwrap();

        // Create a pending receipt with high-confidence match
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/path/to/receipt.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
            receipt_total: Some(50.00),
            receipt_merchant: Some("Target".to_string()),
            content_hash: Some("hash_4".to_string()),
        };
        db.create_receipt_full(&receipt).unwrap();

        // Run auto-match
        let (matched, checked) = db.auto_match_receipts().unwrap();

        assert_eq!(checked, 1, "Should have checked 1 pending receipt");
        assert_eq!(matched, 1, "Should have matched the receipt");

        // Verify receipt is now matched
        let receipts = db.get_pending_receipts().unwrap();
        assert!(receipts.is_empty(), "No pending receipts should remain");
    }

    #[test]
    fn test_receipt_no_match_without_data() {
        let db = Database::in_memory().unwrap();

        // Create receipt without date or amount
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/path/to/receipt.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: None,
            receipt_total: None,
            receipt_merchant: Some("Some Store".to_string()),
            content_hash: Some("hash_5".to_string()),
        };
        let receipt_id = db.create_receipt_full(&receipt).unwrap();

        // Should return empty candidates without date/amount
        let r = db.get_receipt(receipt_id).unwrap().unwrap();
        let candidates = db.find_matching_transactions(&r).unwrap();

        assert!(candidates.is_empty(), "Cannot match without date or amount");
    }

    #[test]
    fn test_receipt_match_candidates_api() {
        let db = Database::in_memory().unwrap();

        // Create account and transaction
        let account_id = db
            .upsert_account("Test Card", Bank::Chase, Some(AccountType::Credit))
            .unwrap();
        let tx = NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "GROCERY STORE ABC".to_string(),
            amount: -75.50,
            category: None,
            import_hash: "tx_hash_5".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();

        // Create a pending receipt
        let receipt = NewReceipt {
            transaction_id: None,
            image_path: Some("/path/to/receipt.jpg".to_string()),
            image_data: None,
            status: ReceiptStatus::Pending,
            role: ReceiptRole::Primary,
            receipt_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()),
            receipt_total: Some(75.50),
            receipt_merchant: Some("Grocery Store".to_string()),
            content_hash: Some("hash_6".to_string()),
        };
        let receipt_id = db.create_receipt_full(&receipt).unwrap();

        // Use the API function
        let candidates = db.get_receipt_match_candidates(receipt_id).unwrap();

        assert!(!candidates.is_empty());
        assert!(candidates[0].score > 0.5);
    }
}

/// Security-focused tests for input validation and injection prevention
#[cfg(test)]
mod security_tests {
    use super::*;
    use chrono::NaiveDate;

    // ========== SQL Injection Prevention Tests ==========

    #[test]
    fn test_sql_injection_in_account_name() {
        let db = Database::in_memory().unwrap();

        // Attempt SQL injection in account name
        let malicious_name = "'; DROP TABLE accounts; --";
        let result = db.upsert_account(malicious_name, Bank::Chase, None);
        assert!(result.is_ok());

        // Verify database is intact and name was stored literally
        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, malicious_name);
    }

    #[test]
    fn test_sql_injection_in_transaction_description() {
        let db = Database::in_memory().unwrap();
        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Attempt SQL injection in transaction description
        let malicious_desc = "PURCHASE'; DELETE FROM transactions WHERE '1'='1";
        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            description: malicious_desc.to_string(),
            amount: -50.00,
            category: None,
            import_hash: "hash_injection_test".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };

        let result = db.insert_transaction(account_id, &tx);
        assert!(result.is_ok());

        // Verify description stored literally, no injection occurred
        let transactions = db.list_transactions(None, 100, 0).unwrap();
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].description, malicious_desc);
    }

    #[test]
    fn test_sql_injection_in_tag_name() {
        let db = Database::in_memory().unwrap();

        // Attempt SQL injection in tag name
        let malicious_tag = "Food'; UPDATE tags SET name='HACKED' WHERE '1'='1";
        let result = db.create_tag(malicious_tag, None, None, None, None);
        assert!(result.is_ok());

        // Verify tag created with literal name
        let tags = db.list_tags().unwrap();
        let found = tags.iter().find(|t| t.name == malicious_tag);
        assert!(found.is_some());
    }

    #[test]
    fn test_sql_injection_in_tag_rule_pattern() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let tags = db.list_tags().unwrap();
        let tag_id = tags[0].id;

        // Attempt SQL injection in pattern
        let malicious_pattern = "test%'; DROP TABLE tag_rules; --";
        let result = db.create_tag_rule(tag_id, malicious_pattern, PatternType::Contains, 0);
        assert!(result.is_ok());

        // Verify rule exists with literal pattern
        let rules = db.list_tag_rules().unwrap();
        let found = rules.iter().find(|r| r.rule.pattern == malicious_pattern);
        assert!(found.is_some());
    }

    #[test]
    fn test_sql_injection_in_entity_name() {
        let db = Database::in_memory().unwrap();

        let malicious_name = "Alice'; DELETE FROM entities; --";
        let entity = NewEntity {
            name: malicious_name.to_string(),
            entity_type: EntityType::Person,
            icon: None,
            color: None,
        };
        let result = db.create_entity(&entity);
        assert!(result.is_ok());

        let entities = db.list_entities(false).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, malicious_name);
    }

    #[test]
    fn test_sql_injection_in_search_query() {
        let db = Database::in_memory().unwrap();
        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Create a normal transaction
        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            description: "Normal purchase".to_string(),
            amount: -25.00,
            category: None,
            import_hash: "hash_search_test".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();

        // Attempt SQL injection in search
        let malicious_search = "'; DROP TABLE transactions; --";
        let results = db
            .search_transactions(None, Some(malicious_search), 100, 0)
            .unwrap();

        // Search should return empty (no match), but DB should be intact
        assert!(results.is_empty());

        // Verify table still exists and has data
        let all_transactions = db.list_transactions(None, 100, 0).unwrap();
        assert_eq!(all_transactions.len(), 1);
    }

    // ========== Boundary Condition Tests ==========

    #[test]
    fn test_extreme_date_values() {
        let db = Database::in_memory().unwrap();
        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Test very old date
        let tx1 = NewTransaction {
            date: NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
            description: "Historical".to_string(),
            amount: -10.00,
            category: None,
            import_hash: "hash_old_date".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx1).is_ok());

        // Test far future date
        let tx2 = NewTransaction {
            date: NaiveDate::from_ymd_opt(2100, 12, 31).unwrap(),
            description: "Future".to_string(),
            amount: -20.00,
            category: None,
            import_hash: "hash_future_date".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx2).is_ok());

        // Verify both stored correctly
        let transactions = db.list_transactions(None, 100, 0).unwrap();
        assert_eq!(transactions.len(), 2);
    }

    #[test]
    fn test_extreme_amount_values() {
        let db = Database::in_memory().unwrap();
        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Test very large amount
        let tx1 = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Large purchase".to_string(),
            amount: -999_999_999.99,
            category: None,
            import_hash: "hash_large".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx1).is_ok());

        // Test very small amount (fraction of a cent)
        let tx2 = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            description: "Tiny purchase".to_string(),
            amount: -0.001,
            category: None,
            import_hash: "hash_tiny".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx2).is_ok());

        // Test zero amount
        let tx3 = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(),
            description: "Zero".to_string(),
            amount: 0.0,
            category: None,
            import_hash: "hash_zero".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx3).is_ok());

        let transactions = db.list_transactions(None, 100, 0).unwrap();
        assert_eq!(transactions.len(), 3);
    }

    #[test]
    fn test_unicode_and_special_characters() {
        let db = Database::in_memory().unwrap();

        // Unicode in account name
        let unicode_name = "测试账户 🏦 Тест";
        let account_id = db.upsert_account(unicode_name, Bank::Chase, None).unwrap();

        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts[0].name, unicode_name);

        // Special characters in description
        let special_desc = "Purchase <script>alert('xss')</script> & \"quotes\" 'apostrophes'";
        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            description: special_desc.to_string(),
            amount: -50.00,
            category: None,
            import_hash: "hash_unicode".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();

        let transactions = db.list_transactions(None, 100, 0).unwrap();
        assert_eq!(transactions[0].description, special_desc);
    }

    #[test]
    fn test_empty_and_whitespace_strings() {
        let db = Database::in_memory().unwrap();
        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Empty description should work (DB doesn't restrict)
        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            description: "".to_string(),
            amount: -10.00,
            category: None,
            import_hash: "hash_empty".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx).is_ok());

        // Whitespace-only description
        let tx2 = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 16).unwrap(),
            description: "   \t\n  ".to_string(),
            amount: -20.00,
            category: None,
            import_hash: "hash_whitespace".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        assert!(db.insert_transaction(account_id, &tx2).is_ok());
    }

    #[test]
    fn test_very_long_strings() {
        let db = Database::in_memory().unwrap();

        // Very long account name (1000 chars)
        let long_name: String = "A".repeat(1000);
        let result = db.upsert_account(&long_name, Bank::Chase, None);
        assert!(result.is_ok());

        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts[0].name.len(), 1000);
    }

    #[test]
    fn test_nonexistent_id_operations() {
        let db = Database::in_memory().unwrap();

        // Operations on non-existent IDs should fail gracefully
        let result = db.get_account(99999);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let result = db.get_transaction(99999);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Delete non-existent should not error (affects 0 rows)
        let result = db.delete_account(99999);
        assert!(result.is_ok());
    }

    #[test]
    fn test_negative_ids() {
        let db = Database::in_memory().unwrap();

        // Negative IDs should be handled gracefully
        let result = db.get_account(-1);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let result = db.get_transaction(-999);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ========== Transaction Atomicity Tests ==========

    #[test]
    fn test_delete_account_atomicity() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create account with transactions and tags
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();

        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            description: "Test transaction".to_string(),
            amount: -50.00,
            category: None,
            import_hash: "hash_atomicity".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        let tx_id = db.insert_transaction(account_id, &tx).unwrap().unwrap();

        // Add a tag to the transaction
        let tags = db.list_tags().unwrap();
        db.add_transaction_tag(tx_id, tags[0].id, TagSource::Manual, None)
            .unwrap();

        // Delete account (should be atomic)
        let result = db.delete_account(account_id);
        assert!(result.is_ok());

        // Verify everything was deleted
        let accounts = db.list_accounts().unwrap();
        assert!(accounts.is_empty());

        let transactions = db.list_transactions(None, 100, 0).unwrap();
        assert!(transactions.is_empty());
    }

    // ========== Date Range Query Tests ==========

    #[test]
    fn test_spending_by_tag_date_filtering_with_parameterized_queries() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Test that date filtering with parameterized queries works without errors
        // The key security check is that dates are not string-interpolated into SQL

        // Test with various date ranges - should not crash or error
        let result = db.get_spending_by_tag(
            Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()),
        );
        assert!(result.is_ok());

        // Test with only from date
        let result =
            db.get_spending_by_tag(Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()), None);
        assert!(result.is_ok());

        // Test with only to date
        let result =
            db.get_spending_by_tag(None, Some(NaiveDate::from_ymd_opt(2025, 6, 30).unwrap()));
        assert!(result.is_ok());

        // Test with no date filter
        let result = db.get_spending_by_tag(None, None);
        assert!(result.is_ok());

        // Test with edge case dates
        let result = db.get_spending_by_tag(
            Some(NaiveDate::from_ymd_opt(1900, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2100, 12, 31).unwrap()),
        );
        assert!(result.is_ok());
    }

    // ========== Subscription Tests ==========

    #[test]
    fn test_upsert_subscription_new() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        let sub_id = db
            .upsert_subscription(
                "Netflix",
                Some(account_id),
                Some(15.99),
                Some(Frequency::Monthly),
                Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                Some(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()),
            )
            .unwrap();

        assert!(sub_id > 0);

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.merchant, "Netflix");
        assert_eq!(sub.account_id, Some(account_id));
        assert_eq!(sub.amount, Some(15.99));
        assert_eq!(sub.frequency, Some(Frequency::Monthly));
        assert_eq!(sub.status, SubscriptionStatus::Active);
    }

    #[test]
    fn test_upsert_subscription_update_existing() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        // Create initial subscription
        let sub_id1 = db
            .upsert_subscription(
                "Netflix",
                Some(account_id),
                Some(15.99),
                Some(Frequency::Monthly),
                Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                Some(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()),
            )
            .unwrap();

        // Update with new amount and last_seen
        let sub_id2 = db
            .upsert_subscription(
                "Netflix",
                Some(account_id),
                Some(17.99),
                Some(Frequency::Monthly),
                Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap()),
            )
            .unwrap();

        // Should return the same ID
        assert_eq!(sub_id1, sub_id2);

        // Should have updated amount and last_seen
        let sub = db.get_subscription(sub_id1).unwrap().unwrap();
        assert_eq!(sub.amount, Some(17.99));
        assert_eq!(
            sub.last_seen,
            Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap())
        );
    }

    #[test]
    fn test_upsert_subscription_no_account() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create subscription without account_id
        let sub_id = db
            .upsert_subscription(
                "Spotify",
                None,
                Some(9.99),
                Some(Frequency::Monthly),
                None,
                None,
            )
            .unwrap();

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.merchant, "Spotify");
        assert_eq!(sub.account_id, None);
    }

    #[test]
    fn test_list_subscriptions_all() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account1 = db.upsert_account("Chase", Bank::Chase, None).unwrap();
        let account2 = db.upsert_account("Amex", Bank::Amex, None).unwrap();

        db.upsert_subscription("Netflix", Some(account1), Some(15.99), None, None, None)
            .unwrap();
        db.upsert_subscription("Spotify", Some(account2), Some(9.99), None, None, None)
            .unwrap();
        db.upsert_subscription("Hulu", None, Some(7.99), None, None, None)
            .unwrap();

        let all_subs = db.list_subscriptions(None).unwrap();
        assert_eq!(all_subs.len(), 3);
    }

    #[test]
    fn test_list_subscriptions_by_account() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account1 = db.upsert_account("Chase", Bank::Chase, None).unwrap();
        let account2 = db.upsert_account("Amex", Bank::Amex, None).unwrap();

        db.upsert_subscription("Netflix", Some(account1), Some(15.99), None, None, None)
            .unwrap();
        db.upsert_subscription("Disney+", Some(account1), Some(12.99), None, None, None)
            .unwrap();
        db.upsert_subscription("Spotify", Some(account2), Some(9.99), None, None, None)
            .unwrap();

        let account1_subs = db.list_subscriptions(Some(account1)).unwrap();
        assert_eq!(account1_subs.len(), 2);

        let account2_subs = db.list_subscriptions(Some(account2)).unwrap();
        assert_eq!(account2_subs.len(), 1);
        assert_eq!(account2_subs[0].merchant, "Spotify");
    }

    #[test]
    fn test_get_subscription_not_found() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let result = db.get_subscription(999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_subscription_status() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let sub_id = db
            .upsert_subscription("Netflix", None, Some(15.99), None, None, None)
            .unwrap();

        // Update to zombie
        db.update_subscription_status(sub_id, SubscriptionStatus::Zombie)
            .unwrap();
        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Zombie);

        // Update to cancelled
        db.update_subscription_status(sub_id, SubscriptionStatus::Cancelled)
            .unwrap();
        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Cancelled);
    }

    #[test]
    fn test_acknowledge_subscription() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let sub_id = db
            .upsert_subscription("Netflix", None, Some(15.99), None, None, None)
            .unwrap();

        // Mark as zombie first
        db.update_subscription_status(sub_id, SubscriptionStatus::Zombie)
            .unwrap();

        // Acknowledge - should set to active and user_acknowledged=true
        db.acknowledge_subscription(sub_id).unwrap();

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Active);
        assert!(sub.user_acknowledged);
    }

    #[test]
    fn test_reactivate_subscription() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let sub_id = db
            .upsert_subscription("Netflix", None, Some(15.99), None, None, None)
            .unwrap();

        // Cancel subscription
        db.cancel_subscription(sub_id, Some(NaiveDate::from_ymd_opt(2024, 5, 1).unwrap()))
            .unwrap();

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Cancelled);

        // Reactivate with new charge
        let new_date = NaiveDate::from_ymd_opt(2024, 7, 1).unwrap();
        db.reactivate_subscription(sub_id, new_date, 17.99).unwrap();

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Active);
        assert_eq!(sub.amount, Some(17.99));
        assert_eq!(sub.last_seen, Some(new_date));
        assert!(sub.user_acknowledged); // Should be acknowledged after reactivation
    }

    #[test]
    fn test_exclude_subscription() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let sub_id = db
            .upsert_subscription("Costco", None, Some(120.0), None, None, None)
            .unwrap();

        db.exclude_subscription(sub_id).unwrap();

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Excluded);

        // Should also add to merchant cache with user override
        let cache_result = db.get_merchant_subscription_cache("COSTCO").unwrap();
        assert_eq!(cache_result, Some(false)); // Not a subscription

        // Verify it's marked as user override
        assert!(db.has_merchant_user_override("Costco").unwrap());
    }

    #[test]
    fn test_unexclude_subscription() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let sub_id = db
            .upsert_subscription("Costco", None, Some(120.0), None, None, None)
            .unwrap();

        // Exclude first
        db.exclude_subscription(sub_id).unwrap();
        assert_eq!(
            db.get_subscription(sub_id).unwrap().unwrap().status,
            SubscriptionStatus::Excluded
        );

        // Unexclude
        db.unexclude_subscription(sub_id).unwrap();

        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Active);

        // User override should be removed from cache
        assert!(!db.has_merchant_user_override("Costco").unwrap());
    }

    // ========== Merchant Subscription Cache Tests ==========

    #[test]
    fn test_merchant_subscription_cache_not_cached() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let result = db
            .get_merchant_subscription_cache("UNKNOWN_MERCHANT")
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_subscription_classification() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Cache as subscription
        db.cache_subscription_classification("Netflix", true, Some(0.95))
            .unwrap();

        let result = db.get_merchant_subscription_cache("NETFLIX").unwrap();
        assert_eq!(result, Some(true));

        // Cache as retail
        db.cache_subscription_classification("Target", false, Some(0.99))
            .unwrap();

        let result = db.get_merchant_subscription_cache("TARGET").unwrap();
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_cache_subscription_classification_update() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Initial cache as subscription
        db.cache_subscription_classification("SomeStore", true, Some(0.6))
            .unwrap();

        // Update to retail (higher confidence)
        db.cache_subscription_classification("SomeStore", false, Some(0.9))
            .unwrap();

        let result = db.get_merchant_subscription_cache("SOMESTORE").unwrap();
        assert_eq!(result, Some(false)); // Should be updated
    }

    #[test]
    fn test_user_override_prevents_ollama_cache_update() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create subscription and exclude it (creates user override)
        let sub_id = db
            .upsert_subscription("Costco", None, Some(120.0), None, None, None)
            .unwrap();
        db.exclude_subscription(sub_id).unwrap();

        // Verify user override exists
        assert!(db.has_merchant_user_override("Costco").unwrap());

        // Try to update via Ollama cache - should not override user decision
        db.cache_subscription_classification("Costco", true, Some(0.99))
            .unwrap();

        // Should still be marked as not-subscription (user override preserved)
        let result = db.get_merchant_subscription_cache("COSTCO").unwrap();
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_has_merchant_user_override_false() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Cache via Ollama (not user override)
        db.cache_subscription_classification("Netflix", true, Some(0.95))
            .unwrap();

        assert!(!db.has_merchant_user_override("Netflix").unwrap());
    }

    #[test]
    fn test_subscription_frequency_parsing() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Weekly
        let sub_id = db
            .upsert_subscription(
                "Weekly Service",
                None,
                Some(5.0),
                Some(Frequency::Weekly),
                None,
                None,
            )
            .unwrap();
        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.frequency, Some(Frequency::Weekly));

        // Monthly
        let sub_id = db
            .upsert_subscription(
                "Monthly Service",
                None,
                Some(10.0),
                Some(Frequency::Monthly),
                None,
                None,
            )
            .unwrap();
        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.frequency, Some(Frequency::Monthly));

        // Yearly
        let sub_id = db
            .upsert_subscription(
                "Yearly Service",
                None,
                Some(100.0),
                Some(Frequency::Yearly),
                None,
                None,
            )
            .unwrap();
        let sub = db.get_subscription(sub_id).unwrap().unwrap();
        assert_eq!(sub.frequency, Some(Frequency::Yearly));
    }

    // ========== Transaction Tests ==========

    #[test]
    fn test_insert_transaction_new() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
            description: "WALMART".to_string(),
            amount: -45.99,
            category: None,
            import_hash: "hash123".to_string(),
            original_data: None,
            import_format: Some("chase_csv".to_string()),
            card_member: None,
            payment_method: None,
        };

        let result = db.insert_transaction(account_id, &tx).unwrap();
        assert!(result.is_some());

        let tx_id = result.unwrap();
        let fetched = db.get_transaction(tx_id).unwrap().unwrap();
        assert_eq!(fetched.description, "WALMART");
        assert_eq!(fetched.amount, -45.99);
    }

    #[test]
    fn test_insert_transaction_duplicate() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        let tx = NewTransaction {
            date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
            description: "WALMART".to_string(),
            amount: -45.99,
            category: None,
            import_hash: "hash123".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };

        // Insert first time
        let result1 = db.insert_transaction(account_id, &tx).unwrap();
        assert!(result1.is_some());

        // Insert again with same hash - should be rejected
        let result2 = db.insert_transaction(account_id, &tx).unwrap();
        assert!(result2.is_none());

        // Only one transaction should exist
        let count = db.count_transactions().unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_search_transactions_by_account() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account1 = db.upsert_account("Chase", Bank::Chase, None).unwrap();
        let account2 = db.upsert_account("Amex", Bank::Amex, None).unwrap();

        db.insert_transaction(
            account1,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "CHASE TX".to_string(),
                amount: -10.0,
                category: None,
                import_hash: "h1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            account2,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                description: "AMEX TX".to_string(),
                amount: -20.0,
                category: None,
                import_hash: "h2".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        // Search by account 1
        let txs = db
            .search_transactions(Some(account1), None, 100, 0)
            .unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].description, "CHASE TX");

        // Search all
        let txs = db.search_transactions(None, None, 100, 0).unwrap();
        assert_eq!(txs.len(), 2);
    }

    #[test]
    fn test_search_transactions_by_description() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "AMAZON MARKETPLACE".to_string(),
                amount: -50.0,
                category: None,
                import_hash: "h1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                description: "WALMART STORE".to_string(),
                amount: -30.0,
                category: None,
                import_hash: "h2".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        // Search for "amazon"
        let txs = db
            .search_transactions(None, Some("amazon"), 100, 0)
            .unwrap();
        assert_eq!(txs.len(), 1);
        assert!(txs[0].description.contains("AMAZON"));

        // Case-insensitive search
        let txs = db
            .search_transactions(None, Some("WALMART"), 100, 0)
            .unwrap();
        assert_eq!(txs.len(), 1);
    }

    #[test]
    fn test_search_transactions_with_date_range() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        // Insert transactions with different dates
        for day in 1..=5 {
            db.insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, day).unwrap(),
                    description: format!("TX Day {}", day),
                    amount: -(day as f64) * 10.0,
                    category: None,
                    import_hash: format!("h{}", day),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap();
        }

        let from_date = NaiveDate::from_ymd_opt(2024, 6, 2).unwrap();
        let to_date = NaiveDate::from_ymd_opt(2024, 6, 4).unwrap();

        let txs = db
            .search_transactions_with_tags_and_dates(
                None,
                None,
                None,
                Some((from_date, to_date)),
                100,
                0,
            )
            .unwrap();

        assert_eq!(txs.len(), 3); // Days 2, 3, 4
    }

    #[test]
    fn test_search_transactions_with_sort() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "Small".to_string(),
                amount: -10.0,
                category: None,
                import_hash: "h1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                description: "Large".to_string(),
                amount: -100.0,
                category: None,
                import_hash: "h2".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        // Sort by amount ascending
        let txs = db
            .search_transactions_with_tags_dates_and_sort(
                None,
                None,
                None,
                None,
                Some("amount"),
                Some("asc"),
                false,
                100,
                0,
            )
            .unwrap();
        assert_eq!(txs[0].description, "Large"); // -100 is less than -10
        assert_eq!(txs[1].description, "Small");

        // Sort by amount descending
        let txs = db
            .search_transactions_with_tags_dates_and_sort(
                None,
                None,
                None,
                None,
                Some("amount"),
                Some("desc"),
                false,
                100,
                0,
            )
            .unwrap();
        assert_eq!(txs[0].description, "Small"); // -10 is greater than -100
    }

    #[test]
    fn test_archive_unarchive_transaction() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                    description: "Test TX".to_string(),
                    amount: -50.0,
                    category: None,
                    import_hash: "h1".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Initially not archived
        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert!(!tx.archived);

        // Archive it
        db.archive_transaction(tx_id).unwrap();
        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert!(tx.archived);

        // Should not appear in regular search
        let txs = db.search_transactions(None, None, 100, 0).unwrap();
        assert!(txs.is_empty());

        // Should appear in archived list
        let archived = db.list_archived_transactions(100, 0).unwrap();
        assert_eq!(archived.len(), 1);

        // Count archived
        let count = db.count_archived_transactions().unwrap();
        assert_eq!(count, 1);

        // Unarchive
        db.unarchive_transaction(tx_id).unwrap();
        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert!(!tx.archived);

        // Should appear in regular search again
        let txs = db.search_transactions(None, None, 100, 0).unwrap();
        assert_eq!(txs.len(), 1);
    }

    #[test]
    fn test_update_merchant_normalized() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                    description: "TRADER JOE'S #456 SAN FRANCISCO".to_string(),
                    amount: -75.0,
                    category: None,
                    import_hash: "h1".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Initially no normalized name
        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert!(tx.merchant_normalized.is_none());

        // Update with normalized name
        db.update_merchant_normalized(tx_id, "Trader Joe's")
            .unwrap();

        let tx = db.get_transaction(tx_id).unwrap().unwrap();
        assert_eq!(tx.merchant_normalized, Some("Trader Joe's".to_string()));
    }

    #[test]
    fn test_get_unnormalized_transactions() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        // Create two transactions
        let tx_id1 = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                    description: "TX1".to_string(),
                    amount: -10.0,
                    category: None,
                    import_hash: "h1".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                description: "TX2".to_string(),
                amount: -20.0,
                category: None,
                import_hash: "h2".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        // Both should be unnormalized
        let unnormalized = db.get_unnormalized_transactions(100).unwrap();
        assert_eq!(unnormalized.len(), 2);

        // Normalize one
        db.update_merchant_normalized(tx_id1, "TX1 Normalized")
            .unwrap();

        // Only one should be unnormalized now
        let unnormalized = db.get_unnormalized_transactions(100).unwrap();
        assert_eq!(unnormalized.len(), 1);
        assert_eq!(unnormalized[0].description, "TX2");
    }

    #[test]
    fn test_count_transactions_with_filters() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account1 = db.upsert_account("Chase", Bank::Chase, None).unwrap();
        let account2 = db.upsert_account("Amex", Bank::Amex, None).unwrap();

        // Insert 3 transactions: 2 in account1, 1 in account2
        for i in 1..=3 {
            let account_id = if i <= 2 { account1 } else { account2 };
            db.insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, i as u32).unwrap(),
                    description: format!("TX{}", i),
                    amount: -(i as f64) * 10.0,
                    category: None,
                    import_hash: format!("h{}", i),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap();
        }

        // Count all
        let count = db.count_transactions().unwrap();
        assert_eq!(count, 3);

        // Count by account
        let count = db.count_transactions_search(Some(account1), None).unwrap();
        assert_eq!(count, 2);

        // Count by search
        let count = db.count_transactions_search(None, Some("TX1")).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_get_transaction_not_found() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let result = db.get_transaction(99999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_search_transactions_by_card_member() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Amex", Bank::Amex, None).unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                description: "TX1".to_string(),
                amount: -10.0,
                category: None,
                import_hash: "h1".to_string(),
                original_data: None,
                import_format: Some("amex_csv".to_string()),
                card_member: Some("JOHN DOE".to_string()),
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                description: "TX2".to_string(),
                amount: -20.0,
                category: None,
                import_hash: "h2".to_string(),
                original_data: None,
                import_format: Some("amex_csv".to_string()),
                card_member: Some("JANE DOE".to_string()),
                payment_method: None,
            },
        )
        .unwrap();

        // Search by card member
        let txs = db
            .search_transactions_full(
                None,
                None,
                Some("JOHN DOE"),
                None,
                None,
                false,
                None,
                None,
                None,
                false,
                100,
                0,
            )
            .unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].description, "TX1");

        // Count by card member
        let count = db
            .count_transactions_full(None, None, Some("JANE DOE"), None, None, false, None)
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_search_transactions_pagination() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        // Insert 10 transactions
        for i in 1..=10 {
            db.insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, i as u32).unwrap(),
                    description: format!("TX{}", i),
                    amount: -(i as f64),
                    category: None,
                    import_hash: format!("h{}", i),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap();
        }

        // Get first page (limit 3)
        let page1 = db.search_transactions(None, None, 3, 0).unwrap();
        assert_eq!(page1.len(), 3);

        // Get second page (limit 3, offset 3)
        let page2 = db.search_transactions(None, None, 3, 3).unwrap();
        assert_eq!(page2.len(), 3);

        // Verify they're different
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[test]
    fn test_clear_auto_tags_for_transactions() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Create a transaction
        let tx_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                    description: "Test purchase".to_string(),
                    amount: -50.0,
                    category: None,
                    import_hash: "clear_tags_test".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        let dining_id = db.get_tag_by_path("Dining").unwrap().unwrap().id;
        let shopping_id = db.get_tag_by_path("Shopping").unwrap().unwrap().id;

        // Add a manual tag
        db.add_transaction_tag(tx_id, dining_id, TagSource::Manual, None)
            .unwrap();

        // Add an auto tag (pattern)
        db.add_transaction_tag(tx_id, shopping_id, TagSource::Pattern, None)
            .unwrap();

        // Verify both tags exist
        let tags = db.get_transaction_tags(tx_id).unwrap();
        assert_eq!(tags.len(), 2);

        // Clear auto tags
        let cleared = db.clear_auto_tags_for_transactions(&[tx_id]).unwrap();
        assert_eq!(cleared, 1); // Only the pattern tag should be removed

        // Verify manual tag remains
        let tags_after = db.get_transaction_tags(tx_id).unwrap();
        assert_eq!(tags_after.len(), 1);
        assert_eq!(tags_after[0].tag_id, dining_id);
        assert_eq!(tags_after[0].source, TagSource::Manual);
    }

    #[test]
    fn test_clear_auto_tags_preserves_all_manual_tags() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Create two transactions
        let tx1_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                    description: "TX1".to_string(),
                    amount: -10.0,
                    category: None,
                    import_hash: "clear_test_1".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        let tx2_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                    description: "TX2".to_string(),
                    amount: -20.0,
                    category: None,
                    import_hash: "clear_test_2".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        let dining_id = db.get_tag_by_path("Dining").unwrap().unwrap().id;
        let shopping_id = db.get_tag_by_path("Shopping").unwrap().unwrap().id;

        // TX1: manual dining, ollama shopping
        db.add_transaction_tag(tx1_id, dining_id, TagSource::Manual, None)
            .unwrap();
        db.add_transaction_tag(tx1_id, shopping_id, TagSource::Ollama, None)
            .unwrap();

        // TX2: manual shopping, rule dining
        db.add_transaction_tag(tx2_id, shopping_id, TagSource::Manual, None)
            .unwrap();
        db.add_transaction_tag(tx2_id, dining_id, TagSource::Rule, None)
            .unwrap();

        // Clear auto tags for both
        let cleared = db
            .clear_auto_tags_for_transactions(&[tx1_id, tx2_id])
            .unwrap();
        assert_eq!(cleared, 2); // ollama from tx1, rule from tx2

        // Verify manual tags remain
        let tx1_tags = db.get_transaction_tags(tx1_id).unwrap();
        assert_eq!(tx1_tags.len(), 1);
        assert_eq!(tx1_tags[0].source, TagSource::Manual);

        let tx2_tags = db.get_transaction_tags(tx2_id).unwrap();
        assert_eq!(tx2_tags.len(), 1);
        assert_eq!(tx2_tags[0].source, TagSource::Manual);
    }

    #[test]
    fn test_clear_auto_tags_empty_list() {
        let db = Database::in_memory().unwrap();

        // Should return 0 for empty list
        let cleared = db.clear_auto_tags_for_transactions(&[]).unwrap();
        assert_eq!(cleared, 0);
    }

    #[test]
    fn test_clear_merchant_normalized_for_transactions() {
        let db = Database::in_memory().unwrap();

        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();

        // Create transactions with normalized merchants
        let tx1_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                    description: "STARBUCKS #123".to_string(),
                    amount: -5.0,
                    category: None,
                    import_hash: "merchant_test_1".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        let tx2_id = db
            .insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                    description: "TRADER JOE'S #456".to_string(),
                    amount: -50.0,
                    category: None,
                    import_hash: "merchant_test_2".to_string(),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap()
            .unwrap();

        // Set normalized names
        db.update_merchant_normalized(tx1_id, "Starbucks").unwrap();
        db.update_merchant_normalized(tx2_id, "Trader Joe's")
            .unwrap();

        // Verify they're set
        let tx1 = db.get_transaction(tx1_id).unwrap().unwrap();
        assert_eq!(tx1.merchant_normalized, Some("Starbucks".to_string()));

        let tx2 = db.get_transaction(tx2_id).unwrap().unwrap();
        assert_eq!(tx2.merchant_normalized, Some("Trader Joe's".to_string()));

        // Clear normalized names
        let cleared = db
            .clear_merchant_normalized_for_transactions(&[tx1_id, tx2_id])
            .unwrap();
        assert_eq!(cleared, 2);

        // Verify they're cleared
        let tx1_after = db.get_transaction(tx1_id).unwrap().unwrap();
        assert!(tx1_after.merchant_normalized.is_none());

        let tx2_after = db.get_transaction(tx2_id).unwrap().unwrap();
        assert!(tx2_after.merchant_normalized.is_none());
    }

    #[test]
    fn test_clear_merchant_normalized_empty_list() {
        let db = Database::in_memory().unwrap();

        // Should return 0 for empty list
        let cleared = db.clear_merchant_normalized_for_transactions(&[]).unwrap();
        assert_eq!(cleared, 0);
    }
}

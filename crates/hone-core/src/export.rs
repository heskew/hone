//! Export functionality for transactions and full database backups
//!
//! Supports:
//! - Transaction CSV export with filtering (date range, tags)
//! - Full JSON backup export/import with all database tables

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::Result;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

/// Options for transaction export
#[derive(Debug, Clone, Default)]
pub struct TransactionExportOptions {
    /// Start date filter (inclusive)
    pub from: Option<NaiveDate>,
    /// End date filter (inclusive)
    pub to: Option<NaiveDate>,
    /// Filter to transactions with any of these tag IDs
    pub tag_ids: Option<Vec<i64>>,
    /// Include child tags when filtering
    pub include_children: bool,
}

/// A transaction with its associated data for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionExport {
    pub id: i64,
    pub account_id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category: Option<String>,
    pub merchant_normalized: Option<String>,
    pub import_hash: String,
    pub purchase_location_id: Option<i64>,
    pub vendor_location_id: Option<i64>,
    pub trip_id: Option<i64>,
    pub source: String,
    pub expected_amount: Option<f64>,
    pub created_at: String,
    /// Account name for CSV export convenience
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
    /// Comma-separated tag names for CSV export
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
}

/// A transaction tag entry for backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionTagEntry {
    pub transaction_id: i64,
    pub tag_id: i64,
    pub source: String,
    pub confidence: Option<f64>,
    pub created_at: String,
}

/// A split tag entry for backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitTagEntry {
    pub split_id: i64,
    pub tag_id: i64,
    pub source: String,
    pub confidence: Option<f64>,
    pub created_at: String,
}

/// Price history entry for backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHistoryEntry {
    pub id: i64,
    pub subscription_id: i64,
    pub amount: f64,
    pub detected_at: String,
}

/// Subscription export with cancellation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionExport {
    pub id: i64,
    pub merchant: String,
    pub amount: Option<f64>,
    pub frequency: Option<String>,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub status: String,
    pub user_acknowledged: bool,
    pub cancelled_at: Option<String>,
    pub cancelled_monthly_amount: Option<f64>,
    pub created_at: String,
}

/// Alert export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertExport {
    pub id: i64,
    pub alert_type: String,
    pub subscription_id: Option<i64>,
    pub message: Option<String>,
    pub dismissed: bool,
    pub created_at: String,
}

/// Entity export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExport {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub archived: bool,
    pub created_at: String,
}

/// Location export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationExport {
    pub id: i64,
    pub name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_type: Option<String>,
    pub created_at: String,
}

/// Tag export (with parent reference for hierarchy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagExport {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub auto_patterns: Option<String>,
    pub created_at: String,
}

/// Tag rule export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRuleExport {
    pub id: i64,
    pub tag_id: i64,
    pub pattern: String,
    pub pattern_type: String,
    pub priority: i32,
    pub created_at: String,
}

/// Account export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountExport {
    pub id: i64,
    pub name: String,
    pub bank: String,
    pub account_type: Option<String>,
    pub created_at: String,
}

/// Trip export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripExport {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location_id: Option<i64>,
    pub budget: Option<f64>,
    pub archived: bool,
    pub created_at: String,
}

/// Mileage log export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MileageLogExport {
    pub id: i64,
    pub entity_id: i64,
    pub date: String,
    pub odometer: f64,
    pub note: Option<String>,
    pub created_at: String,
}

/// Transaction split export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSplitExport {
    pub id: i64,
    pub transaction_id: i64,
    pub amount: f64,
    pub description: Option<String>,
    pub split_type: String,
    pub entity_id: Option<i64>,
    pub purchaser_id: Option<i64>,
    pub created_at: String,
}

/// Receipt export (without image data for JSON backup - just metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptExport {
    pub id: i64,
    pub transaction_id: Option<i64>,
    pub image_path: Option<String>,
    pub parsed_json: Option<String>,
    pub parsed_at: Option<String>,
    pub status: String,
    pub role: String,
    pub receipt_date: Option<String>,
    pub receipt_total: Option<f64>,
    pub receipt_merchant: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: String,
}

/// Merchant alias export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantAliasExport {
    pub id: i64,
    pub receipt_name: String,
    pub canonical_name: String,
    pub bank: Option<String>,
    pub confidence: f64,
    pub created_at: String,
}

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Application version that created the backup
    pub version: String,
    /// When the backup was created
    pub created_at: String,
    /// Total number of records in backup
    pub total_records: i64,
}

/// Full database backup structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullBackup {
    pub metadata: BackupMetadata,
    pub accounts: Vec<AccountExport>,
    pub locations: Vec<LocationExport>,
    pub entities: Vec<EntityExport>,
    pub tags: Vec<TagExport>,
    pub merchant_aliases: Vec<MerchantAliasExport>,
    pub trips: Vec<TripExport>,
    pub tag_rules: Vec<TagRuleExport>,
    pub subscriptions: Vec<SubscriptionExport>,
    pub price_history: Vec<PriceHistoryEntry>,
    pub transactions: Vec<TransactionExport>,
    pub transaction_tags: Vec<TransactionTagEntry>,
    pub transaction_splits: Vec<TransactionSplitExport>,
    pub split_tags: Vec<SplitTagEntry>,
    pub receipts: Vec<ReceiptExport>,
    pub alerts: Vec<AlertExport>,
    pub mileage_logs: Vec<MileageLogExport>,
}

/// Import statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStats {
    pub accounts: i64,
    pub locations: i64,
    pub entities: i64,
    pub tags: i64,
    pub merchant_aliases: i64,
    pub trips: i64,
    pub tag_rules: i64,
    pub subscriptions: i64,
    pub price_history: i64,
    pub transactions: i64,
    pub transaction_tags: i64,
    pub transaction_splits: i64,
    pub split_tags: i64,
    pub receipts: i64,
    pub alerts: i64,
    pub mileage_logs: i64,
}

impl Database {
    /// Export transactions to CSV format
    pub fn export_transactions_csv(&self, opts: &TransactionExportOptions) -> Result<String> {
        let transactions = self.export_transactions(opts)?;

        let mut csv = String::from("date,description,amount,category,merchant,account,tags\n");

        for tx in transactions {
            // Escape CSV fields that might contain commas or quotes
            let description = escape_csv_field(&tx.description);
            let category = tx.category.as_deref().unwrap_or("");
            let merchant = tx.merchant_normalized.as_deref().unwrap_or("");
            let account = tx.account_name.as_deref().unwrap_or("");
            let tags = tx.tags.as_deref().unwrap_or("");

            csv.push_str(&format!(
                "{},{},{:.2},{},{},{},{}\n",
                tx.date,
                description,
                tx.amount,
                escape_csv_field(category),
                escape_csv_field(merchant),
                escape_csv_field(account),
                escape_csv_field(tags)
            ));
        }

        Ok(csv)
    }

    /// Export transactions with filtering
    pub fn export_transactions(
        &self,
        opts: &TransactionExportOptions,
    ) -> Result<Vec<TransactionExport>> {
        use rusqlite::params;
        let conn = self.conn()?;

        // Build the query based on options
        let mut sql = String::from(
            r#"
            SELECT
                t.id, t.account_id, t.date, t.description, t.amount, t.category,
                t.merchant_normalized, t.import_hash, t.purchase_location_id,
                t.vendor_location_id, t.trip_id, t.source, t.expected_amount, t.archived, t.created_at,
                a.name as account_name
            FROM transactions t
            LEFT JOIN accounts a ON a.id = t.account_id
            WHERE t.archived = 0
            "#,
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(from) = &opts.from {
            sql.push_str(&format!(" AND t.date >= ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(from.to_string()));
        }

        if let Some(to) = &opts.to {
            sql.push_str(&format!(" AND t.date <= ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(to.to_string()));
        }

        if let Some(tag_ids) = &opts.tag_ids {
            if !tag_ids.is_empty() {
                if opts.include_children {
                    // Include child tags recursively
                    let placeholders: Vec<String> = (0..tag_ids.len())
                        .map(|i| format!("?{}", params_vec.len() + i + 1))
                        .collect();
                    sql.push_str(&format!(
                        r#" AND t.id IN (
                            SELECT DISTINCT tt.transaction_id
                            FROM transaction_tags tt
                            JOIN (
                                WITH RECURSIVE tag_tree AS (
                                    SELECT id FROM tags WHERE id IN ({})
                                    UNION ALL
                                    SELECT t.id FROM tags t JOIN tag_tree tr ON t.parent_id = tr.id
                                )
                                SELECT id FROM tag_tree
                            ) child_tags ON tt.tag_id = child_tags.id
                        )"#,
                        placeholders.join(",")
                    ));
                } else {
                    let placeholders: Vec<String> = (0..tag_ids.len())
                        .map(|i| format!("?{}", params_vec.len() + i + 1))
                        .collect();
                    sql.push_str(&format!(
                        " AND t.id IN (SELECT transaction_id FROM transaction_tags WHERE tag_id IN ({}))",
                        placeholders.join(",")
                    ));
                }
                for id in tag_ids {
                    params_vec.push(Box::new(*id));
                }
            }
        }

        sql.push_str(" ORDER BY t.date DESC");

        // Convert params to references
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let transactions: Vec<TransactionExport> = stmt
            .query_map(params_refs.as_slice(), |row| {
                let id: i64 = row.get(0)?;
                Ok(TransactionExport {
                    id,
                    account_id: row.get(1)?,
                    date: row.get(2)?,
                    description: row.get(3)?,
                    amount: row.get(4)?,
                    category: row.get(5)?,
                    merchant_normalized: row.get(6)?,
                    import_hash: row.get(7)?,
                    purchase_location_id: row.get(8)?,
                    vendor_location_id: row.get(9)?,
                    trip_id: row.get(10)?,
                    source: row
                        .get::<_, Option<String>>(11)?
                        .unwrap_or_else(|| "import".to_string()),
                    expected_amount: row.get(12)?,
                    // Skip archived at index 13
                    created_at: row.get(14)?,
                    account_name: row.get(15)?,
                    tags: None, // Will be filled in below
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Get tags for each transaction
        let mut result = Vec::with_capacity(transactions.len());
        for mut tx in transactions {
            let tags: Vec<String> = conn
                .prepare(
                    r#"
                    SELECT t.name
                    FROM transaction_tags tt
                    JOIN tags t ON t.id = tt.tag_id
                    WHERE tt.transaction_id = ?1
                    "#,
                )?
                .query_map(params![tx.id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            tx.tags = if tags.is_empty() {
                None
            } else {
                Some(tags.join(","))
            };
            result.push(tx);
        }

        Ok(result)
    }

    /// Export full database backup
    pub fn export_full_backup(&self) -> Result<FullBackup> {
        let conn = self.conn()?;

        // Export accounts
        let accounts = self.export_all_accounts(&conn)?;

        // Export locations
        let locations = self.export_all_locations(&conn)?;

        // Export entities
        let entities = self.export_all_entities(&conn)?;

        // Export tags (ordered for proper import - parents first)
        let tags = self.export_all_tags(&conn)?;

        // Export merchant aliases
        let merchant_aliases = self.export_all_merchant_aliases(&conn)?;

        // Export trips
        let trips = self.export_all_trips(&conn)?;

        // Export tag rules
        let tag_rules = self.export_all_tag_rules(&conn)?;

        // Export subscriptions
        let subscriptions = self.export_all_subscriptions(&conn)?;

        // Export price history
        let price_history = self.export_all_price_history(&conn)?;

        // Export transactions
        let transactions = self.export_transactions(&TransactionExportOptions::default())?;

        // Export transaction tags
        let transaction_tags = self.export_all_transaction_tags(&conn)?;

        // Export transaction splits
        let transaction_splits = self.export_all_transaction_splits(&conn)?;

        // Export split tags
        let split_tags = self.export_all_split_tags(&conn)?;

        // Export receipts
        let receipts = self.export_all_receipts(&conn)?;

        // Export alerts
        let alerts = self.export_all_alerts(&conn)?;

        // Export mileage logs
        let mileage_logs = self.export_all_mileage_logs(&conn)?;

        let total_records = accounts.len()
            + locations.len()
            + entities.len()
            + tags.len()
            + merchant_aliases.len()
            + trips.len()
            + tag_rules.len()
            + subscriptions.len()
            + price_history.len()
            + transactions.len()
            + transaction_tags.len()
            + transaction_splits.len()
            + split_tags.len()
            + receipts.len()
            + alerts.len()
            + mileage_logs.len();

        Ok(FullBackup {
            metadata: BackupMetadata {
                version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: Utc::now().to_rfc3339(),
                total_records: total_records as i64,
            },
            accounts,
            locations,
            entities,
            tags,
            merchant_aliases,
            trips,
            tag_rules,
            subscriptions,
            price_history,
            transactions,
            transaction_tags,
            transaction_splits,
            split_tags,
            receipts,
            alerts,
            mileage_logs,
        })
    }

    // Helper functions for exporting each table

    fn export_all_accounts(&self, conn: &crate::db::DbConn) -> Result<Vec<AccountExport>> {
        let mut stmt = conn
            .prepare("SELECT id, name, bank, account_type, created_at FROM accounts ORDER BY id")?;

        let rows = stmt.query_map([], |row| {
            Ok(AccountExport {
                id: row.get(0)?,
                name: row.get(1)?,
                bank: row.get(2)?,
                account_type: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_locations(&self, conn: &crate::db::DbConn) -> Result<Vec<LocationExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, address, city, state, country, latitude, longitude, location_type, created_at FROM locations ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(LocationExport {
                id: row.get(0)?,
                name: row.get(1)?,
                address: row.get(2)?,
                city: row.get(3)?,
                state: row.get(4)?,
                country: row
                    .get::<_, Option<String>>(5)?
                    .unwrap_or_else(|| "US".to_string()),
                latitude: row.get(6)?,
                longitude: row.get(7)?,
                location_type: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_entities(&self, conn: &crate::db::DbConn) -> Result<Vec<EntityExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, type, icon, color, archived, created_at FROM entities ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(EntityExport {
                id: row.get(0)?,
                name: row.get(1)?,
                entity_type: row.get(2)?,
                icon: row.get(3)?,
                color: row.get(4)?,
                archived: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_tags(&self, conn: &crate::db::DbConn) -> Result<Vec<TagExport>> {
        // Order by parent_id NULLS FIRST to ensure parents come before children
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, color, icon, auto_patterns, created_at FROM tags ORDER BY COALESCE(parent_id, 0), id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TagExport {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                color: row.get(3)?,
                icon: row.get(4)?,
                auto_patterns: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_merchant_aliases(
        &self,
        conn: &crate::db::DbConn,
    ) -> Result<Vec<MerchantAliasExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, receipt_name, canonical_name, bank, confidence, created_at FROM merchant_aliases ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(MerchantAliasExport {
                id: row.get(0)?,
                receipt_name: row.get(1)?,
                canonical_name: row.get(2)?,
                bank: row.get(3)?,
                confidence: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_trips(&self, conn: &crate::db::DbConn) -> Result<Vec<TripExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, description, start_date, end_date, location_id, budget, archived, created_at FROM trips ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TripExport {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                start_date: row.get(3)?,
                end_date: row.get(4)?,
                location_id: row.get(5)?,
                budget: row.get(6)?,
                archived: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_tag_rules(&self, conn: &crate::db::DbConn) -> Result<Vec<TagRuleExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, tag_id, pattern, pattern_type, priority, created_at FROM tag_rules ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TagRuleExport {
                id: row.get(0)?,
                tag_id: row.get(1)?,
                pattern: row.get(2)?,
                pattern_type: row.get(3)?,
                priority: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_subscriptions(
        &self,
        conn: &crate::db::DbConn,
    ) -> Result<Vec<SubscriptionExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, merchant, amount, frequency, first_seen, last_seen, status, user_acknowledged, cancelled_at, cancelled_monthly_amount, created_at FROM subscriptions ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SubscriptionExport {
                id: row.get(0)?,
                merchant: row.get(1)?,
                amount: row.get(2)?,
                frequency: row.get(3)?,
                first_seen: row.get(4)?,
                last_seen: row.get(5)?,
                status: row.get(6)?,
                user_acknowledged: row.get(7)?,
                cancelled_at: row.get(8)?,
                cancelled_monthly_amount: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_price_history(&self, conn: &crate::db::DbConn) -> Result<Vec<PriceHistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, subscription_id, amount, detected_at FROM price_history ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PriceHistoryEntry {
                id: row.get(0)?,
                subscription_id: row.get(1)?,
                amount: row.get(2)?,
                detected_at: row.get(3)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_transaction_tags(
        &self,
        conn: &crate::db::DbConn,
    ) -> Result<Vec<TransactionTagEntry>> {
        let mut stmt = conn.prepare(
            "SELECT transaction_id, tag_id, source, confidence, created_at FROM transaction_tags ORDER BY transaction_id, tag_id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TransactionTagEntry {
                transaction_id: row.get(0)?,
                tag_id: row.get(1)?,
                source: row.get(2)?,
                confidence: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_transaction_splits(
        &self,
        conn: &crate::db::DbConn,
    ) -> Result<Vec<TransactionSplitExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, amount, description, split_type, entity_id, purchaser_id, created_at FROM transaction_splits ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TransactionSplitExport {
                id: row.get(0)?,
                transaction_id: row.get(1)?,
                amount: row.get(2)?,
                description: row.get(3)?,
                split_type: row.get(4)?,
                entity_id: row.get(5)?,
                purchaser_id: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_split_tags(&self, conn: &crate::db::DbConn) -> Result<Vec<SplitTagEntry>> {
        let mut stmt = conn.prepare(
            "SELECT split_id, tag_id, source, confidence, created_at FROM split_tags ORDER BY split_id, tag_id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SplitTagEntry {
                split_id: row.get(0)?,
                tag_id: row.get(1)?,
                source: row.get(2)?,
                confidence: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_receipts(&self, conn: &crate::db::DbConn) -> Result<Vec<ReceiptExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, image_path, parsed_json, parsed_at, status, role, receipt_date, receipt_total, receipt_merchant, content_hash, created_at FROM receipts ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ReceiptExport {
                id: row.get(0)?,
                transaction_id: row.get(1)?,
                image_path: row.get(2)?,
                parsed_json: row.get(3)?,
                parsed_at: row.get(4)?,
                status: row.get(5)?,
                role: row.get(6)?,
                receipt_date: row.get(7)?,
                receipt_total: row.get(8)?,
                receipt_merchant: row.get(9)?,
                content_hash: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_alerts(&self, conn: &crate::db::DbConn) -> Result<Vec<AlertExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, type, subscription_id, message, dismissed, created_at FROM alerts ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(AlertExport {
                id: row.get(0)?,
                alert_type: row.get(1)?,
                subscription_id: row.get(2)?,
                message: row.get(3)?,
                dismissed: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn export_all_mileage_logs(&self, conn: &crate::db::DbConn) -> Result<Vec<MileageLogExport>> {
        let mut stmt = conn.prepare(
            "SELECT id, entity_id, date, odometer, note, created_at FROM mileage_logs ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(MileageLogExport {
                id: row.get(0)?,
                entity_id: row.get(1)?,
                date: row.get(2)?,
                odometer: row.get(3)?,
                note: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Import a full backup, restoring all data
    ///
    /// This clears existing data if `clear_existing` is true, then imports
    /// all data from the backup in dependency order.
    pub fn import_full_backup(
        &self,
        backup: &FullBackup,
        clear_existing: bool,
    ) -> Result<ImportStats> {
        use rusqlite::params;

        if clear_existing {
            self.soft_reset()?;
            // Also clear configuration tables
            let conn = self.conn()?;
            conn.execute_batch(
                r#"
                DELETE FROM tag_rules;
                DELETE FROM tags;
                DELETE FROM entities;
                DELETE FROM locations;
                DELETE FROM trips;
                DELETE FROM accounts;
                DELETE FROM merchant_aliases;
                "#,
            )?;
        }

        let conn = self.conn()?;

        let mut stats = ImportStats {
            accounts: 0,
            locations: 0,
            entities: 0,
            tags: 0,
            merchant_aliases: 0,
            trips: 0,
            tag_rules: 0,
            subscriptions: 0,
            price_history: 0,
            transactions: 0,
            transaction_tags: 0,
            transaction_splits: 0,
            split_tags: 0,
            receipts: 0,
            alerts: 0,
            mileage_logs: 0,
        };

        // 1. Import accounts (independent)
        for account in &backup.accounts {
            conn.execute(
                "INSERT INTO accounts (id, name, bank, account_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![account.id, account.name, account.bank, account.account_type, account.created_at],
            )?;
            stats.accounts += 1;
        }

        // 2. Import locations (independent)
        for loc in &backup.locations {
            conn.execute(
                "INSERT INTO locations (id, name, address, city, state, country, latitude, longitude, location_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![loc.id, loc.name, loc.address, loc.city, loc.state, loc.country, loc.latitude, loc.longitude, loc.location_type, loc.created_at],
            )?;
            stats.locations += 1;
        }

        // 3. Import entities (independent)
        for entity in &backup.entities {
            conn.execute(
                "INSERT INTO entities (id, name, type, icon, color, archived, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![entity.id, entity.name, entity.entity_type, entity.icon, entity.color, entity.archived, entity.created_at],
            )?;
            stats.entities += 1;
        }

        // 4. Import tags (self-referential - ordered so parents come first)
        for tag in &backup.tags {
            conn.execute(
                "INSERT INTO tags (id, name, parent_id, color, icon, auto_patterns, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![tag.id, tag.name, tag.parent_id, tag.color, tag.icon, tag.auto_patterns, tag.created_at],
            )?;
            stats.tags += 1;
        }

        // 5. Import merchant aliases (independent)
        for alias in &backup.merchant_aliases {
            conn.execute(
                "INSERT INTO merchant_aliases (id, receipt_name, canonical_name, bank, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![alias.id, alias.receipt_name, alias.canonical_name, alias.bank, alias.confidence, alias.created_at],
            )?;
            stats.merchant_aliases += 1;
        }

        // 6. Import trips (depends on locations)
        for trip in &backup.trips {
            conn.execute(
                "INSERT INTO trips (id, name, description, start_date, end_date, location_id, budget, archived, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![trip.id, trip.name, trip.description, trip.start_date, trip.end_date, trip.location_id, trip.budget, trip.archived, trip.created_at],
            )?;
            stats.trips += 1;
        }

        // 7. Import tag rules (depends on tags)
        for rule in &backup.tag_rules {
            conn.execute(
                "INSERT INTO tag_rules (id, tag_id, pattern, pattern_type, priority, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![rule.id, rule.tag_id, rule.pattern, rule.pattern_type, rule.priority, rule.created_at],
            )?;
            stats.tag_rules += 1;
        }

        // 8. Import subscriptions (independent)
        for sub in &backup.subscriptions {
            conn.execute(
                "INSERT INTO subscriptions (id, merchant, amount, frequency, first_seen, last_seen, status, user_acknowledged, cancelled_at, cancelled_monthly_amount, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![sub.id, sub.merchant, sub.amount, sub.frequency, sub.first_seen, sub.last_seen, sub.status, sub.user_acknowledged, sub.cancelled_at, sub.cancelled_monthly_amount, sub.created_at],
            )?;
            stats.subscriptions += 1;
        }

        // 9. Import price history (depends on subscriptions)
        for ph in &backup.price_history {
            conn.execute(
                "INSERT INTO price_history (id, subscription_id, amount, detected_at) VALUES (?1, ?2, ?3, ?4)",
                params![ph.id, ph.subscription_id, ph.amount, ph.detected_at],
            )?;
            stats.price_history += 1;
        }

        // 10. Import transactions (depends on accounts, locations, trips)
        for tx in &backup.transactions {
            conn.execute(
                "INSERT INTO transactions (id, account_id, date, description, amount, category, merchant_normalized, import_hash, purchase_location_id, vendor_location_id, trip_id, source, expected_amount, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![tx.id, tx.account_id, tx.date, tx.description, tx.amount, tx.category, tx.merchant_normalized, tx.import_hash, tx.purchase_location_id, tx.vendor_location_id, tx.trip_id, tx.source, tx.expected_amount, tx.created_at],
            )?;
            stats.transactions += 1;
        }

        // 11. Import transaction tags (depends on transactions, tags)
        for tt in &backup.transaction_tags {
            conn.execute(
                "INSERT INTO transaction_tags (transaction_id, tag_id, source, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![tt.transaction_id, tt.tag_id, tt.source, tt.confidence, tt.created_at],
            )?;
            stats.transaction_tags += 1;
        }

        // 12. Import transaction splits (depends on transactions, entities)
        for split in &backup.transaction_splits {
            conn.execute(
                "INSERT INTO transaction_splits (id, transaction_id, amount, description, split_type, entity_id, purchaser_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![split.id, split.transaction_id, split.amount, split.description, split.split_type, split.entity_id, split.purchaser_id, split.created_at],
            )?;
            stats.transaction_splits += 1;
        }

        // 13. Import split tags (depends on splits, tags)
        for st in &backup.split_tags {
            conn.execute(
                "INSERT INTO split_tags (split_id, tag_id, source, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![st.split_id, st.tag_id, st.source, st.confidence, st.created_at],
            )?;
            stats.split_tags += 1;
        }

        // 14. Import receipts (depends on transactions)
        for receipt in &backup.receipts {
            conn.execute(
                "INSERT INTO receipts (id, transaction_id, image_path, parsed_json, parsed_at, status, role, receipt_date, receipt_total, receipt_merchant, content_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![receipt.id, receipt.transaction_id, receipt.image_path, receipt.parsed_json, receipt.parsed_at, receipt.status, receipt.role, receipt.receipt_date, receipt.receipt_total, receipt.receipt_merchant, receipt.content_hash, receipt.created_at],
            )?;
            stats.receipts += 1;
        }

        // 15. Import alerts (depends on subscriptions)
        for alert in &backup.alerts {
            conn.execute(
                "INSERT INTO alerts (id, type, subscription_id, message, dismissed, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![alert.id, alert.alert_type, alert.subscription_id, alert.message, alert.dismissed, alert.created_at],
            )?;
            stats.alerts += 1;
        }

        // 16. Import mileage logs (depends on entities)
        for log in &backup.mileage_logs {
            conn.execute(
                "INSERT INTO mileage_logs (id, entity_id, date, odometer, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![log.id, log.entity_id, log.date, log.odometer, log.note, log.created_at],
            )?;
            stats.mileage_logs += 1;
        }

        Ok(stats)
    }
}

/// Escape a field for CSV output
fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{Bank, NewTransaction};

    #[test]
    fn test_escape_csv_field() {
        assert_eq!(escape_csv_field("simple"), "simple");
        assert_eq!(escape_csv_field("with,comma"), "\"with,comma\"");
        assert_eq!(escape_csv_field("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(escape_csv_field("with\nnewline"), "\"with\nnewline\"");
    }

    #[test]
    fn test_export_options_default() {
        let opts = TransactionExportOptions::default();
        assert!(opts.from.is_none());
        assert!(opts.to.is_none());
        assert!(opts.tag_ids.is_none());
        assert!(!opts.include_children);
    }

    #[test]
    fn test_full_backup_serialization() {
        let backup = FullBackup {
            metadata: BackupMetadata {
                version: "0.1.0".to_string(),
                created_at: "2024-01-15T10:30:00Z".to_string(),
                total_records: 0,
            },
            accounts: vec![],
            locations: vec![],
            entities: vec![],
            tags: vec![],
            merchant_aliases: vec![],
            trips: vec![],
            tag_rules: vec![],
            subscriptions: vec![],
            price_history: vec![],
            transactions: vec![],
            transaction_tags: vec![],
            transaction_splits: vec![],
            split_tags: vec![],
            receipts: vec![],
            alerts: vec![],
            mileage_logs: vec![],
        };

        let json = serde_json::to_string(&backup).unwrap();
        assert!(json.contains("\"version\":\"0.1.0\""));

        let parsed: FullBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.metadata.version, "0.1.0");
    }

    #[test]
    fn test_export_transactions_empty() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let transactions = db
            .export_transactions(&TransactionExportOptions::default())
            .unwrap();
        assert!(transactions.is_empty());
    }

    #[test]
    fn test_export_transactions_basic() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
                description: "WALMART".to_string(),
                amount: -45.99,
                category: None,
                import_hash: "hash1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        let transactions = db
            .export_transactions(&TransactionExportOptions::default())
            .unwrap();
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].description, "WALMART");
        assert_eq!(transactions[0].amount, -45.99);
        assert_eq!(transactions[0].account_name, Some("Chase".to_string()));
    }

    #[test]
    fn test_export_transactions_with_date_filter() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        // Insert transactions on different days
        for day in [10, 15, 20] {
            db.insert_transaction(
                account_id,
                &NewTransaction {
                    date: NaiveDate::from_ymd_opt(2024, 6, day).unwrap(),
                    description: format!("TX Day {}", day),
                    amount: -(day as f64),
                    category: None,
                    import_hash: format!("hash{}", day),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap();
        }

        // Export with date range filter
        let opts = TransactionExportOptions {
            from: Some(NaiveDate::from_ymd_opt(2024, 6, 12).unwrap()),
            to: Some(NaiveDate::from_ymd_opt(2024, 6, 18).unwrap()),
            tag_ids: None,
            include_children: false,
        };

        let transactions = db.export_transactions(&opts).unwrap();
        assert_eq!(transactions.len(), 1); // Only day 15
        assert!(transactions[0].description.contains("15"));
    }

    #[test]
    fn test_export_transactions_csv() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
                description: "WALMART".to_string(),
                amount: -45.99,
                category: None,
                import_hash: "hash1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        let csv = db
            .export_transactions_csv(&TransactionExportOptions::default())
            .unwrap();
        assert!(csv.contains("date,description,amount,category,merchant,account,tags\n"));
        assert!(csv.contains("2024-06-15"));
        assert!(csv.contains("WALMART"));
        assert!(csv.contains("-45.99"));
        assert!(csv.contains("Chase"));
    }

    #[test]
    fn test_export_full_backup_empty_db() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let backup = db.export_full_backup().unwrap();
        assert!(!backup.metadata.version.is_empty());
        assert!(backup.accounts.is_empty());
        assert!(backup.transactions.is_empty());
        // Tags should be seeded
        assert!(!backup.tags.is_empty());
    }

    #[test]
    fn test_export_full_backup_with_data() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create some data
        let account_id = db.upsert_account("Chase", Bank::Chase, None).unwrap();

        db.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
                description: "WALMART".to_string(),
                amount: -45.99,
                category: None,
                import_hash: "hash1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        let backup = db.export_full_backup().unwrap();
        assert_eq!(backup.accounts.len(), 1);
        assert_eq!(backup.accounts[0].name, "Chase");
        assert_eq!(backup.transactions.len(), 1);
        assert_eq!(backup.transactions[0].description, "WALMART");
    }

    #[test]
    fn test_import_full_backup() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create backup data
        let backup = FullBackup {
            metadata: BackupMetadata {
                version: "0.1.0".to_string(),
                created_at: Utc::now().to_rfc3339(),
                total_records: 2,
            },
            accounts: vec![AccountExport {
                id: 1,
                name: "Imported Account".to_string(),
                bank: "chase".to_string(),
                account_type: None,
                created_at: Utc::now().to_rfc3339(),
            }],
            locations: vec![],
            entities: vec![],
            tags: vec![],
            merchant_aliases: vec![],
            trips: vec![],
            tag_rules: vec![],
            subscriptions: vec![],
            price_history: vec![],
            transactions: vec![],
            transaction_tags: vec![],
            transaction_splits: vec![],
            split_tags: vec![],
            receipts: vec![],
            alerts: vec![],
            mileage_logs: vec![],
        };

        let stats = db.import_full_backup(&backup, true).unwrap();
        assert_eq!(stats.accounts, 1);

        // Verify the account was imported
        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "Imported Account");
    }

    #[test]
    fn test_import_full_backup_clears_existing() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create existing data
        db.upsert_account("Existing Account", Bank::Chase, None)
            .unwrap();

        // Import with clear_existing = true
        let backup = FullBackup {
            metadata: BackupMetadata {
                version: "0.1.0".to_string(),
                created_at: Utc::now().to_rfc3339(),
                total_records: 1,
            },
            accounts: vec![AccountExport {
                id: 1,
                name: "New Account".to_string(),
                bank: "amex".to_string(),
                account_type: None,
                created_at: Utc::now().to_rfc3339(),
            }],
            locations: vec![],
            entities: vec![],
            tags: vec![],
            merchant_aliases: vec![],
            trips: vec![],
            tag_rules: vec![],
            subscriptions: vec![],
            price_history: vec![],
            transactions: vec![],
            transaction_tags: vec![],
            transaction_splits: vec![],
            split_tags: vec![],
            receipts: vec![],
            alerts: vec![],
            mileage_logs: vec![],
        };

        let stats = db.import_full_backup(&backup, true).unwrap();
        assert_eq!(stats.accounts, 1);

        // Only the new account should exist
        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "New Account");
    }

    #[test]
    fn test_export_import_roundtrip() {
        let db1 = Database::in_memory().unwrap();
        db1.seed_root_tags().unwrap();

        // Create data in first db
        let account_id = db1
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();
        db1.insert_transaction(
            account_id,
            &NewTransaction {
                date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
                description: "ROUNDTRIP TEST".to_string(),
                amount: -99.99,
                category: None,
                import_hash: "roundtrip_hash".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        // Export
        let backup = db1.export_full_backup().unwrap();

        // Import into fresh db
        let db2 = Database::in_memory().unwrap();
        let stats = db2.import_full_backup(&backup, false).unwrap();

        assert!(stats.accounts > 0);
        assert!(stats.transactions > 0);
        assert!(stats.tags > 0);

        // Verify data
        let accounts = db2.list_accounts().unwrap();
        assert!(accounts.iter().any(|a| a.name == "Test Account"));

        let txs = db2
            .search_transactions(None, Some("ROUNDTRIP"), 100, 0)
            .unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, -99.99);
    }
}

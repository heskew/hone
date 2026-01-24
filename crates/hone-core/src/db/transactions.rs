//! Transaction operations

use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension};

use super::transaction_filter::TransactionFilter;
use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::{NewTransaction, Transaction};

/// Result of inserting a transaction
#[derive(Debug, Clone)]
pub enum TransactionInsertResult {
    /// Transaction was inserted successfully, contains new transaction ID
    Inserted(i64),
    /// Transaction was a duplicate, contains existing transaction ID
    Duplicate(i64),
}

impl Database {
    /// Insert a transaction (skips duplicates based on import_hash)
    pub fn insert_transaction(&self, account_id: i64, tx: &NewTransaction) -> Result<Option<i64>> {
        let conn = self.conn()?;

        // Check for duplicate
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM transactions WHERE import_hash = ?",
                params![tx.import_hash],
                |row| row.get(0),
            )
            .ok();

        if existing.is_some() {
            return Ok(None); // Duplicate, skip
        }

        conn.execute(
            r#"
            INSERT INTO transactions (account_id, date, description, amount, category, import_hash, original_data, import_format, card_member, payment_method)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                account_id,
                tx.date.to_string(),
                tx.description,
                tx.amount,
                tx.category,
                tx.import_hash,
                tx.original_data,
                tx.import_format,
                tx.card_member,
                tx.payment_method.map(|p| p.as_str()),
            ],
        )?;

        Ok(Some(conn.last_insert_rowid()))
    }

    /// Insert a transaction with session tracking
    ///
    /// Unlike `insert_transaction`, this returns detailed information about whether
    /// the transaction was inserted or was a duplicate (and which existing transaction).
    /// Also sets the import_session_id on the transaction if inserted.
    pub fn insert_transaction_with_session(
        &self,
        account_id: i64,
        tx: &NewTransaction,
        import_session_id: i64,
    ) -> Result<TransactionInsertResult> {
        let conn = self.conn()?;

        // Check for duplicate
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM transactions WHERE import_hash = ?",
                params![tx.import_hash],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(existing_id) = existing {
            return Ok(TransactionInsertResult::Duplicate(existing_id));
        }

        conn.execute(
            r#"
            INSERT INTO transactions (account_id, date, description, amount, category, import_hash, original_data, import_format, card_member, payment_method, import_session_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                account_id,
                tx.date.to_string(),
                tx.description,
                tx.amount,
                tx.category,
                tx.import_hash,
                tx.original_data,
                tx.import_format,
                tx.card_member,
                tx.payment_method.map(|p| p.as_str()),
                import_session_id,
            ],
        )?;

        Ok(TransactionInsertResult::Inserted(conn.last_insert_rowid()))
    }

    /// List transactions with optional filters
    pub fn list_transactions(
        &self,
        account_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        self.search_transactions(account_id, None, limit, offset)
    }

    /// Search transactions with optional filters
    pub fn search_transactions(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        self.search_transactions_with_tags(account_id, search, None, limit, offset)
    }

    /// Search transactions with optional filters including tag filtering
    /// When tag_ids is provided, returns transactions that have ANY of the specified tags
    /// (or their descendant tags in the hierarchy)
    pub fn search_transactions_with_tags(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;

        // Build dynamic WHERE clause
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(aid) = account_id {
            conditions.push("t.account_id = ?".to_string());
            params.push(Box::new(aid));
        }

        if let Some(q) = search {
            if !q.trim().is_empty() {
                // Search in description and merchant_normalized (case-insensitive)
                conditions.push("(t.description LIKE ? COLLATE NOCASE OR t.merchant_normalized LIKE ? COLLATE NOCASE)".to_string());
                let pattern = format!("%{}%", q.trim());
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
            }
        }

        // Tag filtering with hierarchy support
        let tag_cte = if let Some(ids) = tag_ids {
            if !ids.is_empty() {
                // Build placeholders for tag IDs
                let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
                for id in ids {
                    params.push(Box::new(*id));
                }

                // CTE to get all descendant tags for the selected tags
                let cte = format!(
                    r#"WITH RECURSIVE tag_tree AS (
                        SELECT id FROM tags WHERE id IN ({})
                        UNION ALL
                        SELECT t.id FROM tags t
                        INNER JOIN tag_tree tt ON t.parent_id = tt.id
                    )"#,
                    placeholders.join(", ")
                );

                conditions.push("t.id IN (SELECT transaction_id FROM transaction_tags WHERE tag_id IN (SELECT id FROM tag_tree))".to_string());
                Some(cte)
            } else {
                None
            }
        } else {
            None
        };

        // Always filter out archived transactions (this method doesn't expose include_archived)
        conditions.push("t.archived = 0".to_string());

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = if let Some(cte) = tag_cte {
            format!(
                r#"
                {}
                SELECT t.id, t.account_id, t.date, t.description, t.amount, t.category, t.merchant_normalized,
                       t.import_hash, t.purchase_location_id, t.vendor_location_id, t.trip_id,
                       t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
                FROM transactions t
                {}
                ORDER BY t.date DESC, t.id DESC
                LIMIT ? OFFSET ?
                "#,
                cte, where_clause
            )
        } else {
            format!(
                r#"
                SELECT t.id, t.account_id, t.date, t.description, t.amount, t.category, t.merchant_normalized,
                       t.import_hash, t.purchase_location_id, t.vendor_location_id, t.trip_id,
                       t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
                FROM transactions t
                {}
                ORDER BY t.date DESC, t.id DESC
                LIMIT ? OFFSET ?
                "#,
                where_clause
            )
        };

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let transactions = stmt
            .query_map(params_refs.as_slice(), |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Search transactions with optional filters including tag filtering and date range
    pub fn search_transactions_with_tags_and_dates(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
        date_range: Option<(NaiveDate, NaiveDate)>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        self.search_transactions_with_tags_dates_and_sort(
            account_id, search, tag_ids, date_range, None, None,
            false, // exclude archived by default
            limit, offset,
        )
    }

    /// Search transactions with optional filters including tag filtering, date range, and sort
    /// When include_archived is false (default), archived transactions are excluded
    /// entity_id filters by account owner (via accounts.entity_id)
    pub fn search_transactions_with_tags_dates_and_sort(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
        date_range: Option<(NaiveDate, NaiveDate)>,
        sort_field: Option<&str>,
        sort_order: Option<&str>,
        include_archived: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        self.search_transactions_full(
            account_id,
            None,
            None,
            search,
            tag_ids,
            false, // untagged
            date_range,
            sort_field,
            sort_order,
            include_archived,
            limit,
            offset,
        )
    }

    /// Full transaction search with all filter options
    /// entity_id filters by account owner (via accounts.entity_id)
    /// card_member filters by cardholder name (exact match, case-insensitive)
    /// untagged: when true, only returns transactions with no tags
    pub fn search_transactions_full(
        &self,
        account_id: Option<i64>,
        entity_id: Option<i64>,
        card_member: Option<&str>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
        untagged: bool,
        date_range: Option<(NaiveDate, NaiveDate)>,
        sort_field: Option<&str>,
        sort_order: Option<&str>,
        include_archived: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;

        // Build filter using the builder
        let filter = TransactionFilter::new()
            .account_id(account_id)
            .entity_id(entity_id)
            .card_member(card_member)
            .search(search)
            .tag_ids(tag_ids)
            .untagged(untagged)
            .date_range(date_range)
            .sort_field(sort_field)
            .sort_order(sort_order)
            .include_archived(include_archived)
            .build();

        // Build SELECT query
        let sql = if let Some(ref cte) = filter.cte {
            format!(
                r#"
                {}
                SELECT t.id, t.account_id, t.date, t.description, t.amount, t.category, t.merchant_normalized,
                       t.import_hash, t.purchase_location_id, t.vendor_location_id, t.trip_id,
                       t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
                FROM transactions t
                {}
                {}
                {}
                LIMIT ? OFFSET ?
                "#,
                cte, filter.join_clause, filter.where_clause, filter.order_clause
            )
        } else {
            format!(
                r#"
                SELECT t.id, t.account_id, t.date, t.description, t.amount, t.category, t.merchant_normalized,
                       t.import_hash, t.purchase_location_id, t.vendor_location_id, t.trip_id,
                       t.source, t.expected_amount, t.archived, t.original_data, t.import_format, t.card_member, t.payment_method, t.created_at
                FROM transactions t
                {}
                {}
                {}
                LIMIT ? OFFSET ?
                "#,
                filter.join_clause, filter.where_clause, filter.order_clause
            )
        };

        // Add pagination params
        let mut params = filter.into_params();
        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let transactions = stmt
            .query_map(params_refs.as_slice(), |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Count transactions matching search criteria
    pub fn count_transactions_search(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
    ) -> Result<i64> {
        self.count_transactions_search_with_tags(account_id, search, None)
    }

    /// Count transactions matching search criteria including tag filtering
    pub fn count_transactions_search_with_tags(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Build dynamic WHERE clause
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(aid) = account_id {
            conditions.push("t.account_id = ?".to_string());
            params.push(Box::new(aid));
        }

        if let Some(q) = search {
            if !q.trim().is_empty() {
                conditions.push("(t.description LIKE ? COLLATE NOCASE OR t.merchant_normalized LIKE ? COLLATE NOCASE)".to_string());
                let pattern = format!("%{}%", q.trim());
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
            }
        }

        // Always exclude archived transactions for count
        conditions.push("t.archived = 0".to_string());

        // Tag filtering with hierarchy support
        let tag_cte = if let Some(ids) = tag_ids {
            if !ids.is_empty() {
                let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
                for id in ids {
                    params.push(Box::new(*id));
                }

                let cte = format!(
                    r#"WITH RECURSIVE tag_tree AS (
                        SELECT id FROM tags WHERE id IN ({})
                        UNION ALL
                        SELECT t.id FROM tags t
                        INNER JOIN tag_tree tt ON t.parent_id = tt.id
                    )"#,
                    placeholders.join(", ")
                );

                conditions.push("t.id IN (SELECT transaction_id FROM transaction_tags WHERE tag_id IN (SELECT id FROM tag_tree))".to_string());
                Some(cte)
            } else {
                None
            }
        } else {
            None
        };

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = if let Some(cte) = tag_cte {
            format!(
                "{} SELECT COUNT(*) FROM transactions t {}",
                cte, where_clause
            )
        } else {
            format!("SELECT COUNT(*) FROM transactions t {}", where_clause)
        };

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = stmt.query_row(params_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    /// Count transactions matching search criteria including tag filtering and date range
    pub fn count_transactions_search_with_tags_and_dates(
        &self,
        account_id: Option<i64>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
        date_range: Option<(NaiveDate, NaiveDate)>,
    ) -> Result<i64> {
        self.count_transactions_full(account_id, None, None, search, tag_ids, false, date_range)
    }

    /// Count transactions matching all filter criteria including entity and card_member filtering
    /// untagged: when true, only counts transactions with no tags
    pub fn count_transactions_full(
        &self,
        account_id: Option<i64>,
        entity_id: Option<i64>,
        card_member: Option<&str>,
        search: Option<&str>,
        tag_ids: Option<&[i64]>,
        untagged: bool,
        date_range: Option<(NaiveDate, NaiveDate)>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Build filter using the builder (count always excludes archived)
        let filter = TransactionFilter::new()
            .account_id(account_id)
            .entity_id(entity_id)
            .card_member(card_member)
            .search(search)
            .tag_ids(tag_ids)
            .untagged(untagged)
            .date_range(date_range)
            .include_archived(false)
            .build();

        // Build COUNT query
        let sql = filter.build_count_query();

        let mut stmt = conn.prepare(&sql)?;
        let params_refs = filter.params_refs();

        let count: i64 = stmt.query_row(params_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    /// Helper to convert a row to Transaction
    /// Column order: id, account_id, date, description, amount, category, merchant_normalized,
    ///               import_hash, purchase_location_id, vendor_location_id, trip_id,
    ///               source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
    pub(crate) fn row_to_transaction(row: &rusqlite::Row) -> rusqlite::Result<Transaction> {
        let date_str: String = row.get(2)?;
        let source_str: Option<String> = row.get(11)?;
        let archived_int: i64 = row.get(13)?;
        let payment_method_str: Option<String> = row.get(17)?;
        let created_at_str: String = row.get(18)?;
        Ok(Transaction {
            id: row.get(0)?,
            account_id: row.get(1)?,
            date: chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").unwrap_or_default(),
            description: row.get(3)?,
            amount: row.get(4)?,
            category: row.get(5)?,
            merchant_normalized: row.get(6)?,
            import_hash: row.get(7)?,
            purchase_location_id: row.get(8)?,
            vendor_location_id: row.get(9)?,
            trip_id: row.get(10)?,
            source: source_str.and_then(|s| s.parse().ok()).unwrap_or_default(),
            expected_amount: row.get(12)?,
            archived: archived_int != 0,
            original_data: row.get(14)?,
            import_format: row.get(15)?,
            card_member: row.get(16)?,
            payment_method: payment_method_str.and_then(|s| s.parse().ok()),
            created_at: parse_datetime(&created_at_str),
        })
    }

    /// Count total transactions
    pub fn count_transactions(&self) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get a single transaction by ID
    pub fn get_transaction(&self, id: i64) -> Result<Option<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, date, description, amount, category, merchant_normalized,
                    import_hash, purchase_location_id, vendor_location_id, trip_id,
                    source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
             FROM transactions WHERE id = ?",
        )?;

        let transaction = stmt
            .query_row(params![id], |row| Self::row_to_transaction(row))
            .optional()?;

        Ok(transaction)
    }

    /// Update the normalized merchant name for a transaction
    pub fn update_merchant_normalized(&self, id: i64, merchant_normalized: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE transactions SET merchant_normalized = ? WHERE id = ?",
            params![merchant_normalized, id],
        )?;
        Ok(())
    }

    /// Get transactions that don't have a normalized merchant name yet
    pub fn get_unnormalized_transactions(&self, limit: i64) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, date, description, amount, category, merchant_normalized,
                   import_hash, purchase_location_id, vendor_location_id, trip_id,
                   source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
            FROM transactions
            WHERE merchant_normalized IS NULL AND archived = 0
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )?;

        let transactions = stmt
            .query_map(params![limit], |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Archive a transaction (hide from reports and lists)
    pub fn archive_transaction(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE transactions SET archived = 1 WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Unarchive a transaction (restore to reports and lists)
    pub fn unarchive_transaction(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE transactions SET archived = 0 WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Count archived transactions
    pub fn count_archived_transactions(&self) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE archived = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// List archived transactions
    pub fn list_archived_transactions(&self, limit: i64, offset: i64) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, date, description, amount, category, merchant_normalized,
                   import_hash, purchase_location_id, vendor_location_id, trip_id,
                   source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
            FROM transactions
            WHERE archived = 1
            ORDER BY date DESC, id DESC
            LIMIT ? OFFSET ?
            "#,
        )?;

        let transactions = stmt
            .query_map(params![limit, offset], |row| Self::row_to_transaction(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    // ============================================
    // Merchant Name Cache (Learning from Corrections)
    // ============================================

    /// Look up a cached merchant name for a description
    pub fn get_cached_merchant_name(&self, description: &str) -> Result<Option<String>> {
        let conn = self.conn()?;
        let result: Option<String> = conn
            .query_row(
                r#"
                SELECT merchant_name FROM merchant_name_cache
                WHERE description = ?
                "#,
                params![description],
                |row| row.get(0),
            )
            .ok();

        // Increment hit count if found
        if result.is_some() {
            let _ = conn.execute(
                r#"
                UPDATE merchant_name_cache
                SET hit_count = hit_count + 1, updated_at = CURRENT_TIMESTAMP
                WHERE description = ?
                "#,
                params![description],
            );
        }

        Ok(result)
    }

    /// Cache a merchant name mapping (from user correction or Ollama)
    pub fn cache_merchant_name(
        &self,
        description: &str,
        merchant_name: &str,
        source: &str,
        confidence: f64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            INSERT INTO merchant_name_cache (description, merchant_name, source, confidence)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(description) DO UPDATE SET
                merchant_name = excluded.merchant_name,
                source = excluded.source,
                confidence = excluded.confidence,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![description, merchant_name, source, confidence],
        )?;
        Ok(())
    }

    /// Update merchant name for a transaction and cache for future learning
    ///
    /// When user edits a merchant name:
    /// 1. Update the transaction's merchant_normalized
    /// 2. Cache the mapping for future transactions with same description
    /// 3. Update any other transactions with the same description
    pub fn update_merchant_name_with_learning(
        &self,
        transaction_id: i64,
        merchant_name: &str,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Get the transaction's description
        let description: String = conn.query_row(
            "SELECT description FROM transactions WHERE id = ?",
            params![transaction_id],
            |row| row.get(0),
        )?;

        // Update this transaction
        conn.execute(
            "UPDATE transactions SET merchant_normalized = ? WHERE id = ?",
            params![merchant_name, transaction_id],
        )?;

        // Cache the mapping for future imports (user correction = confidence 1.0)
        self.cache_merchant_name(&description, merchant_name, "user", 1.0)?;

        // Update all other transactions with the same description
        let updated: i64 = conn
            .query_row(
                r#"
                UPDATE transactions
                SET merchant_normalized = ?
                WHERE description = ? AND id != ?
                RETURNING (SELECT changes())
                "#,
                params![merchant_name, description, transaction_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(updated + 1) // Return total updated including the original
    }

    /// Clear merchant_normalized for a set of transactions (to allow re-normalization)
    /// Returns the number of transactions updated
    pub fn clear_merchant_normalized_for_transactions(
        &self,
        transaction_ids: &[i64],
    ) -> Result<usize> {
        if transaction_ids.is_empty() {
            return Ok(0);
        }

        let conn = self.conn()?;

        // Build placeholders for IN clause
        let placeholders: Vec<String> = transaction_ids.iter().map(|_| "?".to_string()).collect();
        let placeholders_str = placeholders.join(", ");

        let sql = format!(
            "UPDATE transactions SET merchant_normalized = NULL WHERE id IN ({})",
            placeholders_str
        );

        let mut stmt = conn.prepare(&sql)?;

        // Convert transaction_ids to params
        let params: Vec<&dyn rusqlite::ToSql> = transaction_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let updated = stmt.execute(params.as_slice())?;

        tracing::info!("Cleared merchant_normalized for {} transactions", updated);

        Ok(updated)
    }

    /// Get merchant name cache statistics
    pub fn get_merchant_cache_stats(&self) -> Result<MerchantCacheStats> {
        let conn = self.conn()?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM merchant_name_cache", [], |row| {
            row.get(0)
        })?;

        let by_user: i64 = conn.query_row(
            "SELECT COUNT(*) FROM merchant_name_cache WHERE source = 'user'",
            [],
            |row| row.get(0),
        )?;

        let by_ollama: i64 = conn.query_row(
            "SELECT COUNT(*) FROM merchant_name_cache WHERE source = 'ollama'",
            [],
            |row| row.get(0),
        )?;

        let total_hits: i64 = conn.query_row(
            "SELECT COALESCE(SUM(hit_count), 0) FROM merchant_name_cache",
            [],
            |row| row.get(0),
        )?;

        Ok(MerchantCacheStats {
            total_entries: total,
            user_corrections: by_user,
            ollama_learned: by_ollama,
            total_hits,
        })
    }
}

/// Statistics for the merchant name cache
#[derive(Debug, Clone, serde::Serialize)]
pub struct MerchantCacheStats {
    pub total_entries: i64,
    pub user_corrections: i64,
    pub ollama_learned: i64,
    pub total_hits: i64,
}

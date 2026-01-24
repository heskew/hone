//! Import history operations

use chrono::NaiveDate;
use rusqlite::params;

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::{
    Bank, ImportSession, ImportSessionWithAccount, ImportStatus, ImportTaggingBreakdown,
    MerchantChange, NewImportSession, NewReprocessRun, ReprocessRun, ReprocessRunStatus,
    ReprocessRunSummary, ReprocessRunWithComparison, ReprocessSnapshot, RunComparison,
    SkippedTransaction, TagChange, TagDifference, TaggingBreakdownDiff, Transaction,
};

impl Database {
    /// Create a new import session
    pub fn create_import_session(&self, session: &NewImportSession) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            INSERT INTO import_sessions (account_id, filename, file_size_bytes, bank, user_email, ollama_model)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            params![
                session.account_id,
                session.filename,
                session.file_size_bytes,
                session.bank.as_str(),
                session.user_email,
                session.ollama_model,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Update import session with final results
    pub fn update_import_session_results(
        &self,
        session_id: i64,
        imported: i64,
        skipped: i64,
        tagging: &ImportTaggingBreakdown,
        subscriptions_found: i64,
        zombies_detected: i64,
        price_increases_detected: i64,
        duplicates_detected: i64,
        receipts_matched: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            UPDATE import_sessions SET
                imported_count = ?,
                skipped_count = ?,
                tagged_by_learned = ?,
                tagged_by_rule = ?,
                tagged_by_pattern = ?,
                tagged_by_ollama = ?,
                tagged_by_bank_category = ?,
                tagged_fallback = ?,
                subscriptions_found = ?,
                zombies_detected = ?,
                price_increases_detected = ?,
                duplicates_detected = ?,
                receipts_matched = ?
            WHERE id = ?
            "#,
            params![
                imported,
                skipped,
                tagging.by_learned,
                tagging.by_rule,
                tagging.by_pattern,
                tagging.by_ollama,
                tagging.by_bank_category,
                tagging.fallback,
                subscriptions_found,
                zombies_detected,
                price_increases_detected,
                duplicates_detected,
                receipts_matched,
                session_id,
            ],
        )?;
        Ok(())
    }

    /// Update just the tagging breakdown (called after tagging phase completes)
    pub fn update_import_session_tagging(
        &self,
        session_id: i64,
        tagging: &ImportTaggingBreakdown,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            UPDATE import_sessions SET
                tagged_by_learned = ?,
                tagged_by_rule = ?,
                tagged_by_pattern = ?,
                tagged_by_ollama = ?,
                tagged_by_bank_category = ?,
                tagged_fallback = ?
            WHERE id = ?
            "#,
            params![
                tagging.by_learned,
                tagging.by_rule,
                tagging.by_pattern,
                tagging.by_ollama,
                tagging.by_bank_category,
                tagging.fallback,
                session_id,
            ],
        )?;
        Ok(())
    }

    /// Record a skipped (duplicate) transaction
    pub fn record_skipped_transaction(
        &self,
        session_id: i64,
        date: NaiveDate,
        description: &str,
        amount: f64,
        import_hash: &str,
        existing_tx_id: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            INSERT INTO import_skipped_transactions
                (import_session_id, date, description, amount, import_hash, existing_transaction_id)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            params![
                session_id,
                date.to_string(),
                description,
                amount,
                import_hash,
                existing_tx_id
            ],
        )?;
        Ok(())
    }

    /// List import sessions with optional account filter
    pub fn list_import_sessions(
        &self,
        account_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ImportSessionWithAccount>> {
        let conn = self.conn()?;

        let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(acc_id) = account_id
        {
            (
                r#"
                SELECT s.id, s.account_id, s.filename, s.file_size_bytes, s.bank,
                       s.imported_count, s.skipped_count,
                       s.tagged_by_learned, s.tagged_by_rule, s.tagged_by_pattern, s.tagged_by_ollama,
                       s.tagged_by_bank_category, s.tagged_fallback,
                       s.subscriptions_found, s.zombies_detected, s.price_increases_detected,
                       s.duplicates_detected, s.receipts_matched,
                       s.user_email, s.ollama_model,
                       s.status, s.processing_phase, s.processing_current, s.processing_total, s.processing_error,
                       s.tagging_duration_ms, s.normalizing_duration_ms, s.matching_duration_ms,
                       s.detecting_duration_ms, s.total_duration_ms,
                       s.created_at,
                       a.name as account_name
                FROM import_sessions s
                JOIN accounts a ON s.account_id = a.id
                WHERE s.account_id = ?
                ORDER BY s.created_at DESC
                LIMIT ? OFFSET ?
                "#,
                vec![
                    Box::new(acc_id) as Box<dyn rusqlite::ToSql>,
                    Box::new(limit),
                    Box::new(offset),
                ],
            )
        } else {
            (
                r#"
                SELECT s.id, s.account_id, s.filename, s.file_size_bytes, s.bank,
                       s.imported_count, s.skipped_count,
                       s.tagged_by_learned, s.tagged_by_rule, s.tagged_by_pattern, s.tagged_by_ollama,
                       s.tagged_by_bank_category, s.tagged_fallback,
                       s.subscriptions_found, s.zombies_detected, s.price_increases_detected,
                       s.duplicates_detected, s.receipts_matched,
                       s.user_email, s.ollama_model,
                       s.status, s.processing_phase, s.processing_current, s.processing_total, s.processing_error,
                       s.tagging_duration_ms, s.normalizing_duration_ms, s.matching_duration_ms,
                       s.detecting_duration_ms, s.total_duration_ms,
                       s.created_at,
                       a.name as account_name
                FROM import_sessions s
                JOIN accounts a ON s.account_id = a.id
                ORDER BY s.created_at DESC
                LIMIT ? OFFSET ?
                "#,
                vec![
                    Box::new(limit) as Box<dyn rusqlite::ToSql>,
                    Box::new(offset),
                ],
            )
        };

        let mut stmt = conn.prepare(sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let sessions = stmt
            .query_map(params_refs.as_slice(), |row| {
                Self::map_import_session_row(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Count import sessions with optional account filter
    pub fn count_import_sessions(&self, account_id: Option<i64>) -> Result<i64> {
        let conn = self.conn()?;

        let count: i64 = if let Some(acc_id) = account_id {
            conn.query_row(
                "SELECT COUNT(*) FROM import_sessions WHERE account_id = ?",
                params![acc_id],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM import_sessions", [], |row| row.get(0))?
        };

        Ok(count)
    }

    /// Get a single import session by ID
    pub fn get_import_session(&self, id: i64) -> Result<Option<ImportSessionWithAccount>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            r#"
            SELECT s.id, s.account_id, s.filename, s.file_size_bytes, s.bank,
                   s.imported_count, s.skipped_count,
                   s.tagged_by_learned, s.tagged_by_rule, s.tagged_by_pattern, s.tagged_by_ollama,
                   s.tagged_by_bank_category, s.tagged_fallback,
                   s.subscriptions_found, s.zombies_detected, s.price_increases_detected,
                   s.duplicates_detected, s.receipts_matched,
                   s.user_email, s.ollama_model,
                   s.status, s.processing_phase, s.processing_current, s.processing_total, s.processing_error,
                   s.tagging_duration_ms, s.normalizing_duration_ms, s.matching_duration_ms,
                   s.detecting_duration_ms, s.total_duration_ms,
                   s.created_at,
                   a.name as account_name
            FROM import_sessions s
            JOIN accounts a ON s.account_id = a.id
            WHERE s.id = ?
            "#,
            params![id],
            |row| Self::map_import_session_row(row),
        );

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get transactions from an import session
    pub fn get_import_session_transactions(
        &self,
        session_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, date, description, amount, category, merchant_normalized,
                   import_hash, purchase_location_id, vendor_location_id, trip_id, source,
                   expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
            FROM transactions
            WHERE import_session_id = ?
            ORDER BY date DESC
            LIMIT ? OFFSET ?
            "#,
        )?;

        let transactions = stmt
            .query_map(params![session_id, limit, offset], |row| {
                let date_str: String = row.get(2)?;
                let source_str: String = row.get(11)?;
                let payment_method_str: Option<String> = row.get(17)?;
                let created_at_str: String = row.get(18)?;

                Ok(Transaction {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    date: chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    description: row.get(3)?,
                    amount: row.get(4)?,
                    category: row.get(5)?,
                    merchant_normalized: row.get(6)?,
                    import_hash: row.get(7)?,
                    purchase_location_id: row.get(8)?,
                    vendor_location_id: row.get(9)?,
                    trip_id: row.get(10)?,
                    source: source_str.parse().unwrap_or_default(),
                    expected_amount: row.get(12)?,
                    archived: row.get(13)?,
                    original_data: row.get(14)?,
                    import_format: row.get(15)?,
                    card_member: row.get(16)?,
                    payment_method: payment_method_str.and_then(|s| s.parse().ok()),
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(transactions)
    }

    /// Count transactions in an import session
    pub fn count_import_session_transactions(&self, session_id: i64) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE import_session_id = ?",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get skipped transactions for an import session
    pub fn get_skipped_transactions(&self, session_id: i64) -> Result<Vec<SkippedTransaction>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, import_session_id, date, description, amount, import_hash,
                   existing_transaction_id, created_at
            FROM import_skipped_transactions
            WHERE import_session_id = ?
            ORDER BY date DESC
            "#,
        )?;

        let skipped = stmt
            .query_map(params![session_id], |row| {
                let date_str: String = row.get(2)?;
                let created_at_str: String = row.get(7)?;

                Ok(SkippedTransaction {
                    id: row.get(0)?,
                    import_session_id: row.get(1)?,
                    date: chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    description: row.get(3)?,
                    amount: row.get(4)?,
                    import_hash: row.get(5)?,
                    existing_transaction_id: row.get(6)?,
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(skipped)
    }

    /// Helper to map a row to ImportSessionWithAccount
    fn map_import_session_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<ImportSessionWithAccount> {
        let bank_str: String = row.get(4)?;
        let status_str: Option<String> = row.get(20)?;
        let created_at_str: String = row.get(30)?;

        Ok(ImportSessionWithAccount {
            session: ImportSession {
                id: row.get(0)?,
                account_id: row.get(1)?,
                filename: row.get(2)?,
                file_size_bytes: row.get(3)?,
                // Bank should always be valid since it comes from a valid account,
                // but default to Chase if somehow corrupt
                bank: bank_str.parse().unwrap_or(Bank::Chase),
                imported_count: row.get(5)?,
                skipped_count: row.get(6)?,
                tagged_by_learned: row.get(7)?,
                tagged_by_rule: row.get(8)?,
                tagged_by_pattern: row.get(9)?,
                tagged_by_ollama: row.get(10)?,
                tagged_by_bank_category: row.get(11)?,
                tagged_fallback: row.get(12)?,
                subscriptions_found: row.get(13)?,
                zombies_detected: row.get(14)?,
                price_increases_detected: row.get(15)?,
                duplicates_detected: row.get(16)?,
                receipts_matched: row.get(17)?,
                user_email: row.get(18)?,
                ollama_model: row.get(19)?,
                status: status_str
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(ImportStatus::Pending),
                processing_phase: row.get(21)?,
                processing_current: row.get::<_, Option<i64>>(22)?.unwrap_or(0),
                processing_total: row.get::<_, Option<i64>>(23)?.unwrap_or(0),
                processing_error: row.get(24)?,
                tagging_duration_ms: row.get(25)?,
                normalizing_duration_ms: row.get(26)?,
                matching_duration_ms: row.get(27)?,
                detecting_duration_ms: row.get(28)?,
                total_duration_ms: row.get(29)?,
                created_at: parse_datetime(&created_at_str),
            },
            account_name: row.get(31)?,
        })
    }

    /// Update import session status
    pub fn update_import_status(&self, session_id: i64, status: ImportStatus) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE import_sessions SET status = ? WHERE id = ?",
            params![status.as_str(), session_id],
        )?;
        Ok(())
    }

    /// Update import session processing progress
    pub fn update_import_progress(
        &self,
        session_id: i64,
        phase: &str,
        current: i64,
        total: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"UPDATE import_sessions SET
                status = 'processing',
                processing_phase = ?,
                processing_current = ?,
                processing_total = ?
            WHERE id = ?"#,
            params![phase, current, total, session_id],
        )?;
        Ok(())
    }

    /// Mark import session as completed
    pub fn mark_import_completed(&self, session_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"UPDATE import_sessions SET
                status = 'completed',
                processing_phase = NULL,
                processing_current = 0,
                processing_total = 0
            WHERE id = ?"#,
            params![session_id],
        )?;
        Ok(())
    }

    /// Mark import session as failed
    pub fn mark_import_failed(&self, session_id: i64, error: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"UPDATE import_sessions SET
                status = 'failed',
                processing_error = ?
            WHERE id = ?"#,
            params![error, session_id],
        )?;
        Ok(())
    }

    /// Recover any import sessions that were left in 'processing' state
    /// (e.g., due to server restart mid-import). Marks them as failed.
    /// Returns the number of sessions recovered.
    pub fn recover_stuck_imports(&self) -> Result<i64> {
        let conn = self.conn()?;
        let count = conn.execute(
            r#"UPDATE import_sessions SET
                status = 'failed',
                processing_error = 'Server restarted during import. Please re-import the file.'
            WHERE status = 'processing'"#,
            [],
        )?;
        Ok(count as i64)
    }

    /// Cancel an import session that is currently processing
    /// Returns true if the session was cancelled, false if it wasn't in processing state
    pub fn cancel_import(&self, session_id: i64) -> Result<bool> {
        let conn = self.conn()?;
        let count = conn.execute(
            r#"UPDATE import_sessions SET
                status = 'cancelled',
                processing_error = 'Import cancelled by user'
            WHERE id = ? AND status = 'processing'"#,
            params![session_id],
        )?;
        Ok(count > 0)
    }

    /// Update a single phase duration
    pub fn update_import_phase_duration(
        &self,
        session_id: i64,
        phase: &str,
        duration_ms: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        let column = match phase {
            "tagging" => "tagging_duration_ms",
            "normalizing" => "normalizing_duration_ms",
            "matching_receipts" => "matching_duration_ms",
            "detecting" => "detecting_duration_ms",
            _ => return Ok(()), // Ignore unknown phases
        };
        conn.execute(
            &format!("UPDATE import_sessions SET {} = ? WHERE id = ?", column),
            params![duration_ms, session_id],
        )?;
        Ok(())
    }

    /// Update total import duration
    pub fn update_import_total_duration(
        &self,
        session_id: i64,
        total_duration_ms: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE import_sessions SET total_duration_ms = ? WHERE id = ?",
            params![total_duration_ms, session_id],
        )?;
        Ok(())
    }

    /// Capture current state of an import session for before/after comparison
    pub fn capture_reprocess_snapshot(
        &self,
        session_id: i64,
    ) -> Result<crate::models::ReprocessSnapshot> {
        use crate::models::{ReprocessSnapshot, TransactionTagSnapshot};

        let conn = self.conn()?;

        // Get current tagging breakdown from import session
        let tagging: ImportTaggingBreakdown = conn.query_row(
            r#"SELECT tagged_by_learned, tagged_by_rule, tagged_by_pattern, tagged_by_ollama,
                      tagged_by_bank_category, tagged_fallback
               FROM import_sessions WHERE id = ?"#,
            params![session_id],
            |row| {
                Ok(ImportTaggingBreakdown {
                    by_learned: row.get(0)?,
                    by_rule: row.get(1)?,
                    by_pattern: row.get(2)?,
                    by_ollama: row.get(3)?,
                    by_bank_category: row.get(4)?,
                    fallback: row.get(5)?,
                })
            },
        )?;

        // Get detection results
        let (
            subscriptions_found,
            zombies_detected,
            price_increases_detected,
            duplicates_detected,
            receipts_matched,
        ): (i64, i64, i64, i64, i64) = conn.query_row(
            r#"SELECT subscriptions_found, zombies_detected, price_increases_detected,
                      duplicates_detected, receipts_matched
               FROM import_sessions WHERE id = ?"#,
            params![session_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;

        // Get sample of transactions with their tags (first 100)
        let mut stmt = conn.prepare(
            r#"SELECT t.id, t.description, t.merchant_normalized
               FROM transactions t
               WHERE t.import_session_id = ?
               ORDER BY t.date DESC
               LIMIT 100"#,
        )?;

        let tx_rows: Vec<(i64, String, Option<String>)> = stmt
            .query_map(params![session_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // For each transaction, get its tags
        let mut sample_transactions = Vec::new();
        for (tx_id, description, merchant_normalized) in tx_rows {
            let tags: Vec<String> = conn
                .prepare("SELECT tag.name FROM transaction_tags tt JOIN tags tag ON tt.tag_id = tag.id WHERE tt.transaction_id = ?")?
                .query_map(params![tx_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            sample_transactions.push(TransactionTagSnapshot {
                id: tx_id,
                description,
                merchant_normalized,
                tags,
            });
        }

        Ok(ReprocessSnapshot {
            tagging_breakdown: tagging,
            subscriptions_found,
            zombies_detected,
            price_increases_detected,
            duplicates_detected,
            receipts_matched,
            sample_transactions,
        })
    }

    /// Store a reprocess snapshot (before or after), optionally linked to a run
    pub fn store_reprocess_snapshot(
        &self,
        session_id: i64,
        snapshot_type: &str,
        snapshot: &ReprocessSnapshot,
        run_id: Option<i64>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        let tagging_json = serde_json::to_string(&snapshot.tagging_breakdown)
            .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

        let detection_json = serde_json::json!({
            "subscriptions_found": snapshot.subscriptions_found,
            "zombies_detected": snapshot.zombies_detected,
            "price_increases_detected": snapshot.price_increases_detected,
            "duplicates_detected": snapshot.duplicates_detected,
            "receipts_matched": snapshot.receipts_matched,
        })
        .to_string();

        let sample_json = serde_json::to_string(&snapshot.sample_transactions)
            .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

        conn.execute(
            r#"INSERT INTO reprocess_snapshots
               (import_session_id, reprocess_run_id, snapshot_type, tagging_breakdown, detection_results, sample_transactions)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            params![session_id, run_id, snapshot_type, tagging_json, detection_json, sample_json],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get reprocess comparison (before and after snapshots with computed diff)
    pub fn get_reprocess_comparison(
        &self,
        session_id: i64,
    ) -> Result<Option<crate::models::ReprocessComparison>> {
        use crate::models::{
            MerchantChange, ReprocessComparison, ReprocessSnapshot, TagChange,
            TransactionTagSnapshot,
        };

        let conn = self.conn()?;

        // Get both snapshots
        let mut stmt = conn.prepare(
            r#"SELECT snapshot_type, tagging_breakdown, detection_results, sample_transactions
               FROM reprocess_snapshots
               WHERE import_session_id = ?
               ORDER BY created_at ASC"#,
        )?;

        let snapshots: Vec<(String, String, String, Option<String>)> = stmt
            .query_map(params![session_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Need both before and after
        let before_row = snapshots.iter().find(|(t, _, _, _)| t == "before");
        let after_row = snapshots.iter().find(|(t, _, _, _)| t == "after");

        let (before_row, after_row) = match (before_row, after_row) {
            (Some(b), Some(a)) => (b, a),
            _ => return Ok(None),
        };

        // Parse snapshots
        let parse_snapshot = |tagging_json: &str,
                              detection_json: &str,
                              sample_json: &Option<String>|
         -> Result<ReprocessSnapshot> {
            let tagging: ImportTaggingBreakdown = serde_json::from_str(tagging_json)
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

            let detection: serde_json::Value = serde_json::from_str(detection_json)
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

            let sample_transactions: Vec<TransactionTagSnapshot> = sample_json
                .as_ref()
                .map(|s| serde_json::from_str(s))
                .transpose()
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?
                .unwrap_or_default();

            Ok(ReprocessSnapshot {
                tagging_breakdown: tagging,
                subscriptions_found: detection["subscriptions_found"].as_i64().unwrap_or(0),
                zombies_detected: detection["zombies_detected"].as_i64().unwrap_or(0),
                price_increases_detected: detection["price_increases_detected"]
                    .as_i64()
                    .unwrap_or(0),
                duplicates_detected: detection["duplicates_detected"].as_i64().unwrap_or(0),
                receipts_matched: detection["receipts_matched"].as_i64().unwrap_or(0),
                sample_transactions,
            })
        };

        let before = parse_snapshot(&before_row.1, &before_row.2, &before_row.3)?;
        let after = parse_snapshot(&after_row.1, &after_row.2, &after_row.3)?;

        // Compute tag changes by comparing sample transactions
        let mut tag_changes = Vec::new();
        let mut merchant_changes = Vec::new();

        for after_tx in &after.sample_transactions {
            if let Some(before_tx) = before
                .sample_transactions
                .iter()
                .find(|t| t.id == after_tx.id)
            {
                // Check for tag changes
                let mut before_tags = before_tx.tags.clone();
                let mut after_tags = after_tx.tags.clone();
                before_tags.sort();
                after_tags.sort();

                if before_tags != after_tags {
                    tag_changes.push(TagChange {
                        transaction_id: after_tx.id,
                        description: after_tx.description.clone(),
                        before_tags: before_tx.tags.clone(),
                        after_tags: after_tx.tags.clone(),
                    });
                }

                // Check for merchant name changes
                if before_tx.merchant_normalized != after_tx.merchant_normalized {
                    merchant_changes.push(MerchantChange {
                        transaction_id: after_tx.id,
                        description: after_tx.description.clone(),
                        before_merchant: before_tx.merchant_normalized.clone(),
                        after_merchant: after_tx.merchant_normalized.clone(),
                    });
                }
            }
        }

        Ok(Some(ReprocessComparison {
            before,
            after,
            tag_changes,
            merchant_changes,
        }))
    }

    /// Delete reprocess snapshots for a session that are NOT linked to any run
    /// (legacy snapshots from before run tracking was added)
    pub fn delete_orphan_reprocess_snapshots(&self, session_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM reprocess_snapshots WHERE import_session_id = ? AND reprocess_run_id IS NULL",
            params![session_id],
        )?;
        Ok(())
    }

    // ========================================================================
    // Reprocess Run Management
    // ========================================================================

    /// Create a new reprocess run and return its ID
    pub fn create_reprocess_run(&self, new_run: &NewReprocessRun) -> Result<i64> {
        let conn = self.conn()?;

        // Get next run number for this session
        let run_number: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(run_number), 0) + 1 FROM reprocess_runs WHERE import_session_id = ?",
                params![new_run.import_session_id],
                |row| row.get(0),
            )
            .unwrap_or(1);

        conn.execute(
            r#"INSERT INTO reprocess_runs
               (import_session_id, run_number, ollama_model, status, initiated_by, reason)
               VALUES (?, ?, ?, 'running', ?, ?)"#,
            params![
                new_run.import_session_id,
                run_number,
                new_run.ollama_model,
                new_run.initiated_by,
                new_run.reason,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Mark a reprocess run as completed
    pub fn complete_reprocess_run(&self, run_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE reprocess_runs SET status = 'completed', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![run_id],
        )?;
        Ok(())
    }

    /// Mark a reprocess run as failed
    pub fn fail_reprocess_run(&self, run_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE reprocess_runs SET status = 'failed', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![run_id],
        )?;
        Ok(())
    }

    /// Recover any reprocess runs that were left in 'running' state
    /// (e.g., due to server restart mid-reprocess). Marks them as failed.
    /// Returns the number of runs recovered.
    pub fn recover_stuck_reprocess_runs(&self) -> Result<i64> {
        let conn = self.conn()?;
        let count = conn.execute(
            r#"UPDATE reprocess_runs SET
                status = 'failed',
                completed_at = CURRENT_TIMESTAMP
            WHERE status = 'running'"#,
            [],
        )?;
        Ok(count as i64)
    }

    /// Get a single reprocess run by ID
    pub fn get_reprocess_run(&self, run_id: i64) -> Result<Option<ReprocessRun>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            r#"SELECT id, import_session_id, run_number, ollama_model, status,
                      initiated_by, reason, started_at, completed_at, created_at
               FROM reprocess_runs WHERE id = ?"#,
            params![run_id],
            |row| Self::map_reprocess_run_row(row),
        );

        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all reprocess runs for an import session
    pub fn list_reprocess_runs(&self, session_id: i64) -> Result<Vec<ReprocessRunSummary>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"SELECT r.id, r.run_number, r.ollama_model, r.status, r.initiated_by,
                      r.started_at, r.completed_at,
                      (SELECT COUNT(*) FROM reprocess_snapshots s
                       WHERE s.reprocess_run_id = r.id AND s.snapshot_type = 'after') as has_after
               FROM reprocess_runs r
               WHERE r.import_session_id = ?
               ORDER BY r.run_number DESC"#,
        )?;

        let runs = stmt
            .query_map(params![session_id], |row| {
                let status_str: String = row.get(3)?;
                let started_str: String = row.get(5)?;
                let completed_str: Option<String> = row.get(6)?;

                Ok(ReprocessRunSummary {
                    id: row.get(0)?,
                    run_number: row.get(1)?,
                    ollama_model: row.get(2)?,
                    status: status_str.parse().unwrap_or(ReprocessRunStatus::Running),
                    initiated_by: row.get(4)?,
                    started_at: parse_datetime(&started_str),
                    completed_at: completed_str.map(|s| parse_datetime(&s)),
                    // These will be computed separately
                    tags_changed: 0,
                    merchants_changed: 0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // For each run, compute change counts from snapshots
        let mut enriched_runs = Vec::new();
        for mut run in runs {
            if let Ok(Some(comparison)) = self.get_reprocess_comparison_for_run(run.id) {
                run.tags_changed = comparison.tag_changes.len() as i64;
                run.merchants_changed = comparison.merchant_changes.len() as i64;
            }
            enriched_runs.push(run);
        }

        Ok(enriched_runs)
    }

    /// Get the latest reprocess run for a session
    pub fn get_latest_reprocess_run(&self, session_id: i64) -> Result<Option<ReprocessRun>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            r#"SELECT id, import_session_id, run_number, ollama_model, status,
                      initiated_by, reason, started_at, completed_at, created_at
               FROM reprocess_runs
               WHERE import_session_id = ?
               ORDER BY run_number DESC
               LIMIT 1"#,
            params![session_id],
            |row| Self::map_reprocess_run_row(row),
        );

        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get reprocess comparison for a specific run
    pub fn get_reprocess_comparison_for_run(
        &self,
        run_id: i64,
    ) -> Result<Option<crate::models::ReprocessComparison>> {
        use crate::models::{ReprocessComparison, TransactionTagSnapshot};

        let conn = self.conn()?;

        // Get both snapshots for this run
        let mut stmt = conn.prepare(
            r#"SELECT snapshot_type, tagging_breakdown, detection_results, sample_transactions
               FROM reprocess_snapshots
               WHERE reprocess_run_id = ?
               ORDER BY created_at ASC"#,
        )?;

        let snapshots: Vec<(String, String, String, Option<String>)> = stmt
            .query_map(params![run_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Need both before and after
        let before_row = snapshots.iter().find(|(t, _, _, _)| t == "before");
        let after_row = snapshots.iter().find(|(t, _, _, _)| t == "after");

        let (before_row, after_row) = match (before_row, after_row) {
            (Some(b), Some(a)) => (b, a),
            _ => return Ok(None),
        };

        // Parse snapshots
        let parse_snapshot = |tagging_json: &str,
                              detection_json: &str,
                              sample_json: &Option<String>|
         -> Result<ReprocessSnapshot> {
            let tagging: ImportTaggingBreakdown = serde_json::from_str(tagging_json)
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

            let detection: serde_json::Value = serde_json::from_str(detection_json)
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

            let sample_transactions: Vec<TransactionTagSnapshot> = sample_json
                .as_ref()
                .map(|s| serde_json::from_str(s))
                .transpose()
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?
                .unwrap_or_default();

            Ok(ReprocessSnapshot {
                tagging_breakdown: tagging,
                subscriptions_found: detection["subscriptions_found"].as_i64().unwrap_or(0),
                zombies_detected: detection["zombies_detected"].as_i64().unwrap_or(0),
                price_increases_detected: detection["price_increases_detected"]
                    .as_i64()
                    .unwrap_or(0),
                duplicates_detected: detection["duplicates_detected"].as_i64().unwrap_or(0),
                receipts_matched: detection["receipts_matched"].as_i64().unwrap_or(0),
                sample_transactions,
            })
        };

        let before = parse_snapshot(&before_row.1, &before_row.2, &before_row.3)?;
        let after = parse_snapshot(&after_row.1, &after_row.2, &after_row.3)?;

        // Compute changes
        let (tag_changes, merchant_changes) = Self::compute_snapshot_changes(&before, &after);

        Ok(Some(ReprocessComparison {
            before,
            after,
            tag_changes,
            merchant_changes,
        }))
    }

    /// Get a reprocess run with its comparison data
    pub fn get_reprocess_run_with_comparison(
        &self,
        run_id: i64,
    ) -> Result<Option<ReprocessRunWithComparison>> {
        let run = match self.get_reprocess_run(run_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let comparison = self.get_reprocess_comparison_for_run(run_id)?;

        Ok(Some(ReprocessRunWithComparison {
            run,
            before: comparison.as_ref().map(|c| c.before.clone()),
            after: comparison.as_ref().map(|c| c.after.clone()),
            tag_changes: comparison.as_ref().map(|c| c.tag_changes.clone()),
            merchant_changes: comparison.as_ref().map(|c| c.merchant_changes.clone()),
        }))
    }

    /// Compare two specific runs
    pub fn compare_runs(&self, run_a_id: i64, run_b_id: i64) -> Result<Option<RunComparison>> {
        use crate::models::{DetectionResultsDiff, MerchantDifference};

        // Get both runs
        let run_a = match self.get_reprocess_run(run_a_id)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let run_b = match self.get_reprocess_run(run_b_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        // Get after snapshots from both runs
        let snapshot_a = self.get_run_after_snapshot(run_a_id)?;
        let snapshot_b = self.get_run_after_snapshot(run_b_id)?;

        let (snapshot_a, snapshot_b) = match (snapshot_a, snapshot_b) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(None),
        };

        // Compute tagging breakdown diff
        let tagging_diff = TaggingBreakdownDiff {
            learned_diff: snapshot_b.tagging_breakdown.by_learned
                - snapshot_a.tagging_breakdown.by_learned,
            rule_diff: snapshot_b.tagging_breakdown.by_rule - snapshot_a.tagging_breakdown.by_rule,
            pattern_diff: snapshot_b.tagging_breakdown.by_pattern
                - snapshot_a.tagging_breakdown.by_pattern,
            ollama_diff: snapshot_b.tagging_breakdown.by_ollama
                - snapshot_a.tagging_breakdown.by_ollama,
            bank_category_diff: snapshot_b.tagging_breakdown.by_bank_category
                - snapshot_a.tagging_breakdown.by_bank_category,
            fallback_diff: snapshot_b.tagging_breakdown.fallback
                - snapshot_a.tagging_breakdown.fallback,
        };

        // Compute detection results diff
        let detection_diff = DetectionResultsDiff {
            subscriptions_diff: snapshot_b.subscriptions_found - snapshot_a.subscriptions_found,
            zombies_diff: snapshot_b.zombies_detected - snapshot_a.zombies_detected,
            price_increases_diff: snapshot_b.price_increases_detected
                - snapshot_a.price_increases_detected,
            duplicates_diff: snapshot_b.duplicates_detected - snapshot_a.duplicates_detected,
            receipts_matched_diff: snapshot_b.receipts_matched - snapshot_a.receipts_matched,
        };

        // Compute tag and merchant differences
        let mut tag_differences = Vec::new();
        let mut merchant_differences = Vec::new();

        for tx_b in &snapshot_b.sample_transactions {
            if let Some(tx_a) = snapshot_a
                .sample_transactions
                .iter()
                .find(|t| t.id == tx_b.id)
            {
                let mut tags_a = tx_a.tags.clone();
                let mut tags_b = tx_b.tags.clone();
                tags_a.sort();
                tags_b.sort();

                if tags_a != tags_b {
                    tag_differences.push(TagDifference {
                        transaction_id: tx_b.id,
                        description: tx_b.description.clone(),
                        run_a_tags: tx_a.tags.clone(),
                        run_b_tags: tx_b.tags.clone(),
                    });
                }

                if tx_a.merchant_normalized != tx_b.merchant_normalized {
                    merchant_differences.push(MerchantDifference {
                        transaction_id: tx_b.id,
                        description: tx_b.description.clone(),
                        run_a_merchant: tx_a.merchant_normalized.clone(),
                        run_b_merchant: tx_b.merchant_normalized.clone(),
                    });
                }
            }
        }

        // Build run summaries
        let run_a_comparison = self.get_reprocess_comparison_for_run(run_a_id)?;
        let run_b_comparison = self.get_reprocess_comparison_for_run(run_b_id)?;

        let run_a_summary = ReprocessRunSummary {
            id: run_a.id,
            run_number: run_a.run_number,
            ollama_model: run_a.ollama_model,
            status: run_a.status,
            initiated_by: run_a.initiated_by,
            started_at: run_a.started_at,
            completed_at: run_a.completed_at,
            tags_changed: run_a_comparison
                .as_ref()
                .map(|c| c.tag_changes.len() as i64)
                .unwrap_or(0),
            merchants_changed: run_a_comparison
                .as_ref()
                .map(|c| c.merchant_changes.len() as i64)
                .unwrap_or(0),
        };

        let run_b_summary = ReprocessRunSummary {
            id: run_b.id,
            run_number: run_b.run_number,
            ollama_model: run_b.ollama_model,
            status: run_b.status,
            initiated_by: run_b.initiated_by,
            started_at: run_b.started_at,
            completed_at: run_b.completed_at,
            tags_changed: run_b_comparison
                .as_ref()
                .map(|c| c.tag_changes.len() as i64)
                .unwrap_or(0),
            merchants_changed: run_b_comparison
                .as_ref()
                .map(|c| c.merchant_changes.len() as i64)
                .unwrap_or(0),
        };

        Ok(Some(RunComparison {
            run_a: run_a_summary,
            run_b: run_b_summary,
            tagging_diff,
            detection_diff,
            tag_differences,
            merchant_differences,
        }))
    }

    /// Get the "after" snapshot for a run
    fn get_run_after_snapshot(&self, run_id: i64) -> Result<Option<ReprocessSnapshot>> {
        use crate::models::TransactionTagSnapshot;

        let conn = self.conn()?;

        let result = conn.query_row(
            r#"SELECT tagging_breakdown, detection_results, sample_transactions
               FROM reprocess_snapshots
               WHERE reprocess_run_id = ? AND snapshot_type = 'after'"#,
            params![run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        );

        let (tagging_json, detection_json, sample_json) = match result {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let tagging: ImportTaggingBreakdown = serde_json::from_str(&tagging_json)
            .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

        let detection: serde_json::Value = serde_json::from_str(&detection_json)
            .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?;

        let sample_transactions: Vec<TransactionTagSnapshot> = sample_json
            .as_ref()
            .map(|s| serde_json::from_str(s))
            .transpose()
            .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?
            .unwrap_or_default();

        Ok(Some(ReprocessSnapshot {
            tagging_breakdown: tagging,
            subscriptions_found: detection["subscriptions_found"].as_i64().unwrap_or(0),
            zombies_detected: detection["zombies_detected"].as_i64().unwrap_or(0),
            price_increases_detected: detection["price_increases_detected"].as_i64().unwrap_or(0),
            duplicates_detected: detection["duplicates_detected"].as_i64().unwrap_or(0),
            receipts_matched: detection["receipts_matched"].as_i64().unwrap_or(0),
            sample_transactions,
        }))
    }

    /// Helper to compute tag and merchant changes between two snapshots
    fn compute_snapshot_changes(
        before: &ReprocessSnapshot,
        after: &ReprocessSnapshot,
    ) -> (Vec<TagChange>, Vec<MerchantChange>) {
        let mut tag_changes = Vec::new();
        let mut merchant_changes = Vec::new();

        for after_tx in &after.sample_transactions {
            if let Some(before_tx) = before
                .sample_transactions
                .iter()
                .find(|t| t.id == after_tx.id)
            {
                let mut before_tags = before_tx.tags.clone();
                let mut after_tags = after_tx.tags.clone();
                before_tags.sort();
                after_tags.sort();

                if before_tags != after_tags {
                    tag_changes.push(TagChange {
                        transaction_id: after_tx.id,
                        description: after_tx.description.clone(),
                        before_tags: before_tx.tags.clone(),
                        after_tags: after_tx.tags.clone(),
                    });
                }

                if before_tx.merchant_normalized != after_tx.merchant_normalized {
                    merchant_changes.push(MerchantChange {
                        transaction_id: after_tx.id,
                        description: after_tx.description.clone(),
                        before_merchant: before_tx.merchant_normalized.clone(),
                        after_merchant: after_tx.merchant_normalized.clone(),
                    });
                }
            }
        }

        (tag_changes, merchant_changes)
    }

    /// Helper to map a row to ReprocessRun
    fn map_reprocess_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReprocessRun> {
        let status_str: String = row.get(4)?;
        let started_str: String = row.get(7)?;
        let completed_str: Option<String> = row.get(8)?;
        let created_str: String = row.get(9)?;

        Ok(ReprocessRun {
            id: row.get(0)?,
            import_session_id: row.get(1)?,
            run_number: row.get(2)?,
            ollama_model: row.get(3)?,
            status: status_str.parse().unwrap_or(ReprocessRunStatus::Running),
            initiated_by: row.get(5)?,
            reason: row.get(6)?,
            started_at: parse_datetime(&started_str),
            completed_at: completed_str.map(|s| parse_datetime(&s)),
            created_at: parse_datetime(&created_str),
        })
    }

    /// Get a snapshot representing the initial import state (before any reprocessing)
    ///
    /// This creates a "virtual" snapshot from the import session's original data,
    /// allowing comparison of any reprocess run back to the original baseline.
    pub fn get_initial_import_snapshot(
        &self,
        session_id: i64,
    ) -> Result<Option<ReprocessSnapshot>> {
        use crate::models::TransactionTagSnapshot;

        // Get the import session
        let session = match self.get_import_session(session_id)? {
            Some(s) => s.session,
            None => return Ok(None),
        };

        // Get sample transactions with their current tags
        // We use the first "before" snapshot if it exists, otherwise get current state
        let conn = self.conn()?;

        // Try to get the "before" snapshot from the first run
        let first_run_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM reprocess_runs WHERE import_session_id = ? ORDER BY run_number ASC LIMIT 1",
                params![session_id],
                |row| row.get(0),
            )
            .ok();

        let sample_transactions: Vec<TransactionTagSnapshot> = if let Some(run_id) = first_run_id {
            // Get sample_transactions from the first run's "before" snapshot
            let result: Option<String> = conn
                .query_row(
                    "SELECT sample_transactions FROM reprocess_snapshots WHERE reprocess_run_id = ? AND snapshot_type = 'before'",
                    params![run_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            result
                .as_ref()
                .map(|s| serde_json::from_str(s))
                .transpose()
                .map_err(|e| crate::error::Error::InvalidData(e.to_string()))?
                .unwrap_or_default()
        } else {
            // No reprocess runs yet, get current transaction state
            // (This shouldn't happen in normal flow since we're comparing runs)
            Vec::new()
        };

        Ok(Some(ReprocessSnapshot {
            tagging_breakdown: ImportTaggingBreakdown {
                by_learned: session.tagged_by_learned,
                by_rule: session.tagged_by_rule,
                by_pattern: session.tagged_by_pattern,
                by_ollama: session.tagged_by_ollama,
                by_bank_category: session.tagged_by_bank_category,
                fallback: session.tagged_fallback,
            },
            subscriptions_found: session.subscriptions_found,
            zombies_detected: session.zombies_detected,
            price_increases_detected: session.price_increases_detected,
            duplicates_detected: session.duplicates_detected,
            receipts_matched: session.receipts_matched,
            sample_transactions,
        }))
    }

    /// Compare a reprocess run to the initial import state
    ///
    /// This allows users to see how much a run improved/changed things compared
    /// to the original import, not just compared to the previous run.
    pub fn compare_run_to_initial(
        &self,
        session_id: i64,
        run_id: i64,
    ) -> Result<Option<RunComparison>> {
        use crate::models::{DetectionResultsDiff, MerchantDifference};

        // Get the run
        let run = match self.get_reprocess_run(run_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        if run.import_session_id != session_id {
            return Ok(None);
        }

        // Get initial import snapshot
        let snapshot_initial = match self.get_initial_import_snapshot(session_id)? {
            Some(s) => s,
            None => return Ok(None),
        };

        // Get run's after snapshot
        let snapshot_run = match self.get_run_after_snapshot(run_id)? {
            Some(s) => s,
            None => return Ok(None),
        };

        // Compute diffs
        let tagging_diff = TaggingBreakdownDiff {
            learned_diff: snapshot_run.tagging_breakdown.by_learned
                - snapshot_initial.tagging_breakdown.by_learned,
            rule_diff: snapshot_run.tagging_breakdown.by_rule
                - snapshot_initial.tagging_breakdown.by_rule,
            pattern_diff: snapshot_run.tagging_breakdown.by_pattern
                - snapshot_initial.tagging_breakdown.by_pattern,
            ollama_diff: snapshot_run.tagging_breakdown.by_ollama
                - snapshot_initial.tagging_breakdown.by_ollama,
            bank_category_diff: snapshot_run.tagging_breakdown.by_bank_category
                - snapshot_initial.tagging_breakdown.by_bank_category,
            fallback_diff: snapshot_run.tagging_breakdown.fallback
                - snapshot_initial.tagging_breakdown.fallback,
        };

        let detection_diff = DetectionResultsDiff {
            subscriptions_diff: snapshot_run.subscriptions_found
                - snapshot_initial.subscriptions_found,
            zombies_diff: snapshot_run.zombies_detected - snapshot_initial.zombies_detected,
            price_increases_diff: snapshot_run.price_increases_detected
                - snapshot_initial.price_increases_detected,
            duplicates_diff: snapshot_run.duplicates_detected
                - snapshot_initial.duplicates_detected,
            receipts_matched_diff: snapshot_run.receipts_matched
                - snapshot_initial.receipts_matched,
        };

        // Compute tag and merchant differences
        let mut tag_differences = Vec::new();
        let mut merchant_differences = Vec::new();

        for tx_run in &snapshot_run.sample_transactions {
            if let Some(tx_initial) = snapshot_initial
                .sample_transactions
                .iter()
                .find(|t| t.id == tx_run.id)
            {
                let mut tags_initial = tx_initial.tags.clone();
                let mut tags_run = tx_run.tags.clone();
                tags_initial.sort();
                tags_run.sort();

                if tags_initial != tags_run {
                    tag_differences.push(TagDifference {
                        transaction_id: tx_run.id,
                        description: tx_run.description.clone(),
                        run_a_tags: tx_initial.tags.clone(),
                        run_b_tags: tx_run.tags.clone(),
                    });
                }

                if tx_initial.merchant_normalized != tx_run.merchant_normalized {
                    merchant_differences.push(MerchantDifference {
                        transaction_id: tx_run.id,
                        description: tx_run.description.clone(),
                        run_a_merchant: tx_initial.merchant_normalized.clone(),
                        run_b_merchant: tx_run.merchant_normalized.clone(),
                    });
                }
            }
        }

        // Get the import session for initial state info
        let session = self.get_import_session(session_id)?.unwrap();

        // Build run summaries - initial is run_a (id=0, run_number=0)
        let initial_summary = ReprocessRunSummary {
            id: 0,
            run_number: 0,
            ollama_model: session.session.ollama_model.clone(),
            status: ReprocessRunStatus::Completed,
            initiated_by: session.session.user_email.clone(),
            started_at: session.session.created_at,
            completed_at: Some(session.session.created_at),
            tags_changed: 0,
            merchants_changed: 0,
        };

        let run_comparison = self.get_reprocess_comparison_for_run(run_id)?;
        let run_summary = ReprocessRunSummary {
            id: run.id,
            run_number: run.run_number,
            ollama_model: run.ollama_model,
            status: run.status,
            initiated_by: run.initiated_by,
            started_at: run.started_at,
            completed_at: run.completed_at,
            tags_changed: run_comparison
                .as_ref()
                .map(|c| c.tag_changes.len() as i64)
                .unwrap_or(0),
            merchants_changed: run_comparison
                .as_ref()
                .map(|c| c.merchant_changes.len() as i64)
                .unwrap_or(0),
        };

        Ok(Some(RunComparison {
            run_a: initial_summary,
            run_b: run_summary,
            tagging_diff,
            detection_diff,
            tag_differences,
            merchant_differences,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Bank;

    fn setup_test_db() -> Database {
        let db = Database::in_memory().unwrap();
        // Create a test account
        db.conn()
            .unwrap()
            .execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test Account', 'chase')",
                [],
            )
            .unwrap();
        db
    }

    #[test]
    fn test_create_import_session() {
        let db = setup_test_db();

        let session = NewImportSession {
            account_id: 1,
            filename: Some("test.csv".to_string()),
            file_size_bytes: Some(1024),
            bank: Bank::Chase,
            user_email: Some("test@example.com".to_string()),
            ollama_model: Some("gemma3".to_string()),
        };

        let id = db.create_import_session(&session).unwrap();
        assert_eq!(id, 1);

        let loaded = db.get_import_session(id).unwrap().unwrap();
        assert_eq!(loaded.session.filename.as_deref(), Some("test.csv"));
        assert_eq!(loaded.session.bank, Bank::Chase);
        assert_eq!(loaded.session.ollama_model.as_deref(), Some("gemma3"));
        assert_eq!(loaded.account_name, "Test Account");
    }

    #[test]
    fn test_update_import_session_results() {
        let db = setup_test_db();

        let session = NewImportSession {
            account_id: 1,
            filename: None,
            file_size_bytes: None,
            bank: Bank::Chase,
            user_email: None,
            ollama_model: None,
        };

        let id = db.create_import_session(&session).unwrap();

        let tagging = ImportTaggingBreakdown {
            by_learned: 4,
            by_rule: 5,
            by_pattern: 10,
            by_ollama: 3,
            by_bank_category: 2,
            fallback: 1,
        };

        db.update_import_session_results(id, 20, 5, &tagging, 3, 1, 0, 2, 1)
            .unwrap();

        let loaded = db.get_import_session(id).unwrap().unwrap();
        assert_eq!(loaded.session.imported_count, 20);
        assert_eq!(loaded.session.skipped_count, 5);
        assert_eq!(loaded.session.tagged_by_rule, 5);
        assert_eq!(loaded.session.tagged_by_pattern, 10);
        assert_eq!(loaded.session.subscriptions_found, 3);
    }

    #[test]
    fn test_record_and_get_skipped_transactions() {
        let db = setup_test_db();

        // Create a test transaction to reference
        db.conn()
            .unwrap()
            .execute(
                r#"INSERT INTO transactions (account_id, date, description, amount, import_hash)
                   VALUES (1, '2024-01-15', 'EXISTING TX', -29.99, 'existing123')"#,
                [],
            )
            .unwrap();
        let existing_tx_id: i64 = db
            .conn()
            .unwrap()
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .unwrap();

        let session = NewImportSession {
            account_id: 1,
            filename: None,
            file_size_bytes: None,
            bank: Bank::Chase,
            user_email: None,
            ollama_model: None,
        };

        let session_id = db.create_import_session(&session).unwrap();

        db.record_skipped_transaction(
            session_id,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            "AMAZON PURCHASE",
            -29.99,
            "abc123",
            Some(existing_tx_id),
        )
        .unwrap();

        db.record_skipped_transaction(
            session_id,
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
            "STARBUCKS",
            -5.50,
            "def456",
            None,
        )
        .unwrap();

        let skipped = db.get_skipped_transactions(session_id).unwrap();
        assert_eq!(skipped.len(), 2);
        assert_eq!(skipped[0].description, "STARBUCKS"); // Ordered by date DESC
        assert_eq!(skipped[1].description, "AMAZON PURCHASE");
        assert_eq!(skipped[1].existing_transaction_id, Some(existing_tx_id));
    }

    #[test]
    fn test_list_import_sessions() {
        let db = setup_test_db();

        // Create multiple sessions
        for i in 0..3 {
            let session = NewImportSession {
                account_id: 1,
                filename: Some(format!("file{}.csv", i)),
                file_size_bytes: None,
                bank: Bank::Chase,
                user_email: None,
                ollama_model: None,
            };
            db.create_import_session(&session).unwrap();
        }

        let sessions = db.list_import_sessions(None, 10, 0).unwrap();
        assert_eq!(sessions.len(), 3);

        // Test pagination
        let sessions = db.list_import_sessions(None, 2, 0).unwrap();
        assert_eq!(sessions.len(), 2);

        let sessions = db.list_import_sessions(None, 2, 2).unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_count_import_sessions() {
        let db = setup_test_db();

        assert_eq!(db.count_import_sessions(None).unwrap(), 0);

        let session = NewImportSession {
            account_id: 1,
            filename: None,
            file_size_bytes: None,
            bank: Bank::Chase,
            user_email: None,
            ollama_model: None,
        };
        db.create_import_session(&session).unwrap();
        db.create_import_session(&session).unwrap();

        assert_eq!(db.count_import_sessions(None).unwrap(), 2);
        assert_eq!(db.count_import_sessions(Some(1)).unwrap(), 2);
        assert_eq!(db.count_import_sessions(Some(999)).unwrap(), 0);
    }
}

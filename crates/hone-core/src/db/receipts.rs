//! Receipt operations

use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension};

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::*;

/// Matching configuration constants
const DATE_WINDOW_DAYS: i64 = 3; // How many days to search around receipt date
const AMOUNT_TOLERANCE_PERCENT: f64 = 0.20; // 20% tolerance for tips/tax
const AMOUNT_TOLERANCE_FIXED: f64 = 5.0; // Or $5 fixed tolerance
const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.85; // Auto-match threshold

impl Database {
    /// Create a receipt (new version with all fields)
    pub fn create_receipt_full(&self, receipt: &NewReceipt) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO receipts (transaction_id, image_path, image_data, status, role,
             receipt_date, receipt_total, receipt_merchant, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                receipt.transaction_id,
                receipt.image_path,
                receipt.image_data,
                receipt.status.as_str(),
                receipt.role.as_str(),
                receipt.receipt_date.map(|d| d.to_string()),
                receipt.receipt_total,
                receipt.receipt_merchant,
                receipt.content_hash,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Create a receipt for a transaction (legacy compatibility)
    pub fn create_receipt(&self, transaction_id: i64, image_path: Option<&str>) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO receipts (transaction_id, image_path, status, role) VALUES (?, ?, 'matched', 'primary')",
            params![transaction_id, image_path],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get receipt by ID
    pub fn get_receipt(&self, id: i64) -> Result<Option<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, image_path, parsed_json, parsed_at,
                    status, role, receipt_date, receipt_total, receipt_merchant,
                    content_hash, created_at
             FROM receipts WHERE id = ?",
        )?;

        let receipt = stmt
            .query_row(params![id], |row| Self::row_to_receipt(row))
            .optional()?;

        Ok(receipt)
    }

    /// Get receipt by content hash (for deduplication)
    pub fn get_receipt_by_hash(&self, content_hash: &str) -> Result<Option<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, image_path, parsed_json, parsed_at,
                    status, role, receipt_date, receipt_total, receipt_merchant,
                    content_hash, created_at
             FROM receipts WHERE content_hash = ?",
        )?;

        let receipt = stmt
            .query_row(params![content_hash], |row| Self::row_to_receipt(row))
            .optional()?;

        Ok(receipt)
    }

    /// Get receipts for a transaction
    pub fn get_receipts_for_transaction(&self, transaction_id: i64) -> Result<Vec<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, image_path, parsed_json, parsed_at,
                    status, role, receipt_date, receipt_total, receipt_merchant,
                    content_hash, created_at
             FROM receipts WHERE transaction_id = ? ORDER BY role ASC, created_at DESC",
        )?;

        let receipts = stmt
            .query_map(params![transaction_id], |row| Self::row_to_receipt(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(receipts)
    }

    /// Get pending receipts (awaiting transaction match)
    pub fn get_pending_receipts(&self) -> Result<Vec<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, image_path, parsed_json, parsed_at,
                    status, role, receipt_date, receipt_total, receipt_merchant,
                    content_hash, created_at
             FROM receipts WHERE status = 'pending' ORDER BY created_at DESC",
        )?;

        let receipts = stmt
            .query_map([], |row| Self::row_to_receipt(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(receipts)
    }

    /// Get receipts by status
    pub fn get_receipts_by_status(&self, status: ReceiptStatus) -> Result<Vec<Receipt>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, image_path, parsed_json, parsed_at,
                    status, role, receipt_date, receipt_total, receipt_merchant,
                    content_hash, created_at
             FROM receipts WHERE status = ? ORDER BY created_at DESC",
        )?;

        let receipts = stmt
            .query_map(params![status.as_str()], |row| Self::row_to_receipt(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(receipts)
    }

    /// Helper to convert a row to Receipt
    fn row_to_receipt(row: &rusqlite::Row) -> rusqlite::Result<Receipt> {
        let created_at_str: String = row.get(11)?;
        let parsed_at_str: Option<String> = row.get(4)?;
        let status_str: String = row.get(5)?;
        let role_str: String = row.get(6)?;
        let receipt_date_str: Option<String> = row.get(7)?;

        Ok(Receipt {
            id: row.get(0)?,
            transaction_id: row.get(1)?,
            image_path: row.get(2)?,
            parsed_json: row.get(3)?,
            parsed_at: parsed_at_str.map(|s| parse_datetime(&s)),
            status: status_str.parse().unwrap_or_default(),
            role: role_str.parse().unwrap_or_default(),
            receipt_date: receipt_date_str
                .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            receipt_total: row.get(8)?,
            receipt_merchant: row.get(9)?,
            content_hash: row.get(10)?,
            created_at: parse_datetime(&created_at_str),
        })
    }

    /// Update receipt with parsed JSON from LLM
    pub fn update_receipt_parsed(&self, id: i64, parsed_json: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE receipts SET parsed_json = ?, parsed_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![parsed_json, id],
        )?;
        Ok(())
    }

    /// Update receipt parsed data (merchant, date, total from LLM)
    pub fn update_receipt_parsed_data(
        &self,
        id: i64,
        parsed_json: &str,
        merchant: Option<&str>,
        date: Option<NaiveDate>,
        total: Option<f64>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE receipts SET parsed_json = ?, parsed_at = CURRENT_TIMESTAMP,
             receipt_merchant = ?, receipt_date = ?, receipt_total = ?
             WHERE id = ?",
            params![
                parsed_json,
                merchant,
                date.map(|d| d.to_string()),
                total,
                id
            ],
        )?;
        Ok(())
    }

    /// Update receipt status
    pub fn update_receipt_status(&self, id: i64, status: ReceiptStatus) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE receipts SET status = ? WHERE id = ?",
            params![status.as_str(), id],
        )?;
        Ok(())
    }

    /// Link receipt to transaction
    pub fn link_receipt_to_transaction(&self, receipt_id: i64, transaction_id: i64) -> Result<()> {
        let mut conn = self.conn()?;

        // Get receipt total to update transaction's expected_amount
        let receipt_total: Option<f64> = conn
            .query_row(
                "SELECT receipt_total FROM receipts WHERE id = ?",
                params![receipt_id],
                |row| row.get(0),
            )
            .optional()?;

        let tx = conn.transaction()?;

        tx.execute(
            "UPDATE receipts SET transaction_id = ?, status = 'matched' WHERE id = ?",
            params![transaction_id, receipt_id],
        )?;

        if let Some(total) = receipt_total {
            tx.execute(
                "UPDATE transactions SET expected_amount = ? WHERE id = ?",
                params![total, transaction_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Unlink receipt from transaction (sets to orphaned)
    pub fn unlink_receipt(&self, receipt_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE receipts SET transaction_id = NULL, status = 'orphaned' WHERE id = ?",
            params![receipt_id],
        )?;
        Ok(())
    }

    /// Delete a receipt
    pub fn delete_receipt(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM receipts WHERE id = ?", params![id])?;
        Ok(())
    }

    // ========== Auto-Matching Functions ==========

    /// Find candidate transactions that could match a receipt
    /// Returns candidates sorted by match score (highest first)
    pub fn find_matching_transactions(
        &self,
        receipt: &Receipt,
    ) -> Result<Vec<ReceiptMatchCandidate>> {
        // Need at least a date or amount to search
        if receipt.receipt_date.is_none() && receipt.receipt_total.is_none() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;

        // Build query with date window if we have a date
        let (sql, date_params): (String, Vec<String>) = if let Some(date) = receipt.receipt_date {
            let from_date = date - chrono::Duration::days(DATE_WINDOW_DAYS);
            let to_date = date + chrono::Duration::days(DATE_WINDOW_DAYS);
            (
                format!(
                    "SELECT id, account_id, date, description, amount, category, merchant_normalized,
                            import_hash, purchase_location_id, vendor_location_id, trip_id,
                            source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
                     FROM transactions
                     WHERE date >= ? AND date <= ? AND archived = 0
                     ORDER BY date DESC
                     LIMIT 100"
                ),
                vec![from_date.to_string(), to_date.to_string()],
            )
        } else {
            // No date, search recent transactions
            (
                "SELECT id, account_id, date, description, amount, category, merchant_normalized,
                        import_hash, purchase_location_id, vendor_location_id, trip_id,
                        source, expected_amount, archived, original_data, import_format, card_member, payment_method, created_at
                 FROM transactions
                 WHERE archived = 0
                 ORDER BY date DESC
                 LIMIT 100"
                    .to_string(),
                vec![],
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let transactions: Vec<Transaction> = if date_params.is_empty() {
            stmt.query_map([], |row| Self::row_to_transaction(row))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![date_params[0], date_params[1]], |row| {
                Self::row_to_transaction(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };

        // Score each transaction against the receipt
        let mut candidates: Vec<ReceiptMatchCandidate> = transactions
            .into_iter()
            .filter_map(|tx| {
                let match_factors = self.compute_match_factors(receipt, &tx);
                let score = self.compute_match_score(&match_factors);

                // Filter out very low scores
                if score < 0.3 {
                    return None;
                }

                Some(ReceiptMatchCandidate {
                    transaction: tx,
                    score,
                    match_factors,
                })
            })
            .collect();

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return top candidates
        candidates.truncate(10);
        Ok(candidates)
    }

    /// Compute individual match factors for a receipt-transaction pair
    fn compute_match_factors(&self, receipt: &Receipt, transaction: &Transaction) -> MatchFactors {
        // Amount comparison
        let (amount_score, amount_diff) = if let Some(receipt_total) = receipt.receipt_total {
            let tx_amount = transaction.amount.abs(); // Transaction amounts may be negative
            let receipt_amount = receipt_total.abs();
            let diff = (tx_amount - receipt_amount).abs();

            // Score based on how close the amounts are
            // Perfect match = 1.0, within tolerance = 0.5-0.9, way off = 0.0
            let tolerance = (receipt_amount * AMOUNT_TOLERANCE_PERCENT).max(AMOUNT_TOLERANCE_FIXED);
            let score = if diff < 0.01 {
                1.0 // Exact match
            } else if diff <= tolerance {
                0.9 - (diff / tolerance) * 0.4 // 0.5-0.9 range
            } else {
                0.0
            };
            (score, diff)
        } else {
            (0.5, 0.0) // Unknown amount, neutral score
        };

        // Date comparison
        let (date_score, days_diff) = if let Some(receipt_date) = receipt.receipt_date {
            let days = (transaction.date - receipt_date).num_days().abs();
            let score = if days == 0 {
                1.0
            } else if days <= DATE_WINDOW_DAYS {
                1.0 - (days as f64 / (DATE_WINDOW_DAYS as f64 + 1.0))
            } else {
                0.0
            };
            (score, days)
        } else {
            (0.5, 0) // Unknown date, neutral score
        };

        // Merchant comparison
        let merchant_score = self.compute_merchant_similarity(
            receipt.receipt_merchant.as_deref(),
            transaction.merchant_normalized.as_deref(),
            &transaction.description,
        );

        MatchFactors {
            amount_score,
            date_score,
            merchant_score,
            amount_diff,
            days_diff,
            ollama_evaluation: None,
        }
    }

    /// Compute overall match score from individual factors
    fn compute_match_score(&self, factors: &MatchFactors) -> f64 {
        // Weighted average with amount being most important
        let amount_weight = 0.5;
        let date_weight = 0.3;
        let merchant_weight = 0.2;

        factors.amount_score * amount_weight
            + factors.date_score * date_weight
            + factors.merchant_score * merchant_weight
    }

    /// Compute merchant name similarity (0.0-1.0)
    fn compute_merchant_similarity(
        &self,
        receipt_merchant: Option<&str>,
        normalized_merchant: Option<&str>,
        description: &str,
    ) -> f64 {
        let receipt_name = match receipt_merchant {
            Some(name) if !name.is_empty() => name.to_lowercase(),
            _ => return 0.5, // Unknown, neutral score
        };

        // Check against normalized merchant name first (best comparison)
        if let Some(normalized) = normalized_merchant {
            let normalized_lower = normalized.to_lowercase();
            if normalized_lower == receipt_name {
                return 1.0; // Exact match
            }
            if normalized_lower.contains(&receipt_name) || receipt_name.contains(&normalized_lower)
            {
                return 0.9; // Substring match
            }
            // Check for significant word overlap
            if self.words_overlap(&receipt_name, &normalized_lower) {
                return 0.7;
            }
        }

        // Fall back to description comparison
        let desc_lower = description.to_lowercase();
        if desc_lower.contains(&receipt_name) || receipt_name.contains(&desc_lower) {
            return 0.6;
        }
        if self.words_overlap(&receipt_name, &desc_lower) {
            return 0.4;
        }

        0.0 // No match
    }

    /// Check if two strings have significant word overlap
    fn words_overlap(&self, a: &str, b: &str) -> bool {
        let words_a: std::collections::HashSet<&str> = a
            .split_whitespace()
            .filter(|w| w.len() > 2) // Skip short words
            .collect();
        let words_b: std::collections::HashSet<&str> =
            b.split_whitespace().filter(|w| w.len() > 2).collect();

        if words_a.is_empty() || words_b.is_empty() {
            return false;
        }

        let overlap = words_a.intersection(&words_b).count();
        overlap > 0
    }

    /// Auto-match pending receipts to transactions
    /// Returns (matched_count, receipts_checked)
    pub fn auto_match_receipts(&self) -> Result<(usize, usize)> {
        let pending = self.get_pending_receipts()?;
        let mut matched = 0;

        for receipt in &pending {
            let candidates = self.find_matching_transactions(receipt)?;

            // Auto-link if we have a high-confidence single match
            if let Some(best) = candidates.first() {
                if best.score >= HIGH_CONFIDENCE_THRESHOLD {
                    // Check that this transaction doesn't already have a receipt
                    let existing = self.get_receipts_for_transaction(best.transaction.id)?;
                    if existing.is_empty() {
                        self.link_receipt_to_transaction(receipt.id, best.transaction.id)?;
                        matched += 1;
                    }
                }
            }
        }

        Ok((matched, pending.len()))
    }

    /// Get match candidates for a specific receipt (for UI)
    pub fn get_receipt_match_candidates(
        &self,
        receipt_id: i64,
    ) -> Result<Vec<ReceiptMatchCandidate>> {
        let receipt = self
            .get_receipt(receipt_id)?
            .ok_or_else(|| crate::error::Error::NotFound("Receipt not found".to_string()))?;

        self.find_matching_transactions(&receipt)
    }
}

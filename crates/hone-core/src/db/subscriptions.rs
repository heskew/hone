//! Subscription operations

use chrono::NaiveDate;
use rusqlite::params;

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::{Frequency, Subscription, SubscriptionStatus};

impl Database {
    /// Upsert a subscription by merchant name and account
    pub fn upsert_subscription(
        &self,
        merchant: &str,
        account_id: Option<i64>,
        amount: Option<f64>,
        frequency: Option<Frequency>,
        first_seen: Option<NaiveDate>,
        last_seen: Option<NaiveDate>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Check if exists (unique by merchant + account_id)
        let existing: Option<i64> = if let Some(acc_id) = account_id {
            conn.query_row(
                "SELECT id FROM subscriptions WHERE merchant = ? AND account_id = ?",
                params![merchant, acc_id],
                |row| row.get(0),
            )
            .ok()
        } else {
            conn.query_row(
                "SELECT id FROM subscriptions WHERE merchant = ? AND account_id IS NULL",
                params![merchant],
                |row| row.get(0),
            )
            .ok()
        };

        if let Some(id) = existing {
            // Update last_seen and amount if provided
            if let (Some(amt), Some(ls)) = (amount, last_seen) {
                conn.execute(
                    "UPDATE subscriptions SET amount = ?, last_seen = ? WHERE id = ?",
                    params![amt, ls.to_string(), id],
                )?;
            }
            return Ok(id);
        }

        conn.execute(
            r#"
            INSERT INTO subscriptions (merchant, account_id, amount, frequency, first_seen, last_seen, status)
            VALUES (?, ?, ?, ?, ?, ?, 'active')
            "#,
            params![
                merchant,
                account_id,
                amount,
                frequency.map(|f| f.as_str()),
                first_seen.map(|d| d.to_string()),
                last_seen.map(|d| d.to_string()),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// List all subscriptions, optionally filtered by account
    pub fn list_subscriptions(&self, account_id: Option<i64>) -> Result<Vec<Subscription>> {
        let conn = self.conn()?;

        let (query, params_vec): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(acc_id) =
            account_id
        {
            (
                r#"
                SELECT id, merchant, account_id, amount, frequency, first_seen, last_seen, status, user_acknowledged, acknowledged_at, created_at
                FROM subscriptions
                WHERE account_id = ?
                ORDER BY last_seen DESC NULLS LAST
                "#
                .to_string(),
                vec![Box::new(acc_id)],
            )
        } else {
            (
                r#"
                SELECT id, merchant, account_id, amount, frequency, first_seen, last_seen, status, user_acknowledged, acknowledged_at, created_at
                FROM subscriptions
                ORDER BY last_seen DESC NULLS LAST
                "#
                .to_string(),
                vec![],
            )
        };

        let mut stmt = conn.prepare(&query)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let subscriptions = stmt
            .query_map(params_refs.as_slice(), |row| {
                let freq_str: Option<String> = row.get(4)?;
                let status_str: String = row.get(7)?;
                let first_seen_str: Option<String> = row.get(5)?;
                let last_seen_str: Option<String> = row.get(6)?;
                let acknowledged_at_str: Option<String> = row.get(9)?;
                let created_at_str: String = row.get(10)?;

                Ok(Subscription {
                    id: row.get(0)?,
                    merchant: row.get(1)?,
                    account_id: row.get(2)?,
                    amount: row.get(3)?,
                    frequency: freq_str.and_then(|s| match s.as_str() {
                        "weekly" => Some(Frequency::Weekly),
                        "monthly" => Some(Frequency::Monthly),
                        "yearly" => Some(Frequency::Yearly),
                        _ => None,
                    }),
                    first_seen: first_seen_str
                        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    last_seen: last_seen_str
                        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    status: match status_str.as_str() {
                        "cancelled" => SubscriptionStatus::Cancelled,
                        "zombie" => SubscriptionStatus::Zombie,
                        "excluded" => SubscriptionStatus::Excluded,
                        _ => SubscriptionStatus::Active,
                    },
                    user_acknowledged: row.get(8)?,
                    acknowledged_at: acknowledged_at_str.map(|s| parse_datetime(&s)),
                    created_at: parse_datetime(&created_at_str),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(subscriptions)
    }

    /// Update subscription status
    pub fn update_subscription_status(&self, id: i64, status: SubscriptionStatus) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE subscriptions SET status = ? WHERE id = ?",
            params![status.as_str(), id],
        )?;
        Ok(())
    }

    /// Acknowledge a subscription (mark as known)
    ///
    /// Sets `user_acknowledged = TRUE` and records the current timestamp in `acknowledged_at`.
    /// This timestamp is used to detect stale acknowledgments - subscriptions that were
    /// acknowledged more than 90 days ago may be flagged as zombies again.
    pub fn acknowledge_subscription(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE subscriptions SET user_acknowledged = TRUE, acknowledged_at = CURRENT_TIMESTAMP, status = 'active' WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Reactivate a cancelled subscription (when a new charge is detected)
    ///
    /// Sets user_acknowledged=true since we just saw a charge, indicating active use.
    /// Also updates acknowledged_at to the current timestamp.
    /// This prevents immediate zombie detection after reactivation.
    pub fn reactivate_subscription(
        &self,
        id: i64,
        new_last_seen: NaiveDate,
        amount: f64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            r#"
            UPDATE subscriptions
            SET status = 'active',
                last_seen = ?,
                amount = ?,
                cancelled_at = NULL,
                cancelled_monthly_amount = NULL,
                user_acknowledged = TRUE,
                acknowledged_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
            params![new_last_seen.to_string(), amount, id],
        )?;
        Ok(())
    }

    /// Exclude a subscription from detection (user marked as "not a subscription")
    ///
    /// Also updates the merchant subscription cache to remember this choice.
    pub fn exclude_subscription(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        // Get the merchant name first
        let merchant: String = conn.query_row(
            "SELECT merchant FROM subscriptions WHERE id = ?",
            params![id],
            |row| row.get(0),
        )?;

        // Update subscription status
        conn.execute(
            "UPDATE subscriptions SET status = 'excluded' WHERE id = ?",
            params![id],
        )?;

        // Update merchant cache with user override
        conn.execute(
            r#"
            INSERT INTO merchant_subscription_cache (merchant_pattern, is_subscription, confidence, source)
            VALUES (?, FALSE, 1.0, 'user_override')
            ON CONFLICT(merchant_pattern) DO UPDATE SET
                is_subscription = FALSE,
                confidence = 1.0,
                source = 'user_override'
            "#,
            params![merchant.to_uppercase()],
        )?;

        Ok(())
    }

    /// Unexclude a subscription (re-enable detection)
    ///
    /// Sets status back to active and removes the user override from cache.
    pub fn unexclude_subscription(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        // Get the merchant name first
        let merchant: String = conn.query_row(
            "SELECT merchant FROM subscriptions WHERE id = ?",
            params![id],
            |row| row.get(0),
        )?;

        // Update subscription status back to active
        conn.execute(
            "UPDATE subscriptions SET status = 'active' WHERE id = ?",
            params![id],
        )?;

        // Remove user override from cache (or set it to subscription)
        conn.execute(
            "DELETE FROM merchant_subscription_cache WHERE merchant_pattern = ? AND source = 'user_override'",
            params![merchant.to_uppercase()],
        )?;

        Ok(())
    }

    /// Delete a subscription by ID
    ///
    /// Use this to remove false positive subscriptions. Also clears any cached
    /// subscription classification for this merchant.
    pub fn delete_subscription(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        // Get merchant name first for cache cleanup
        let merchant: String = conn.query_row(
            "SELECT merchant FROM subscriptions WHERE id = ?",
            params![id],
            |row| row.get(0),
        )?;

        // Delete the subscription
        conn.execute("DELETE FROM subscriptions WHERE id = ?", params![id])?;

        // Clear any cached classification for this merchant
        conn.execute(
            "DELETE FROM merchant_subscription_cache WHERE merchant_pattern = ?",
            params![merchant.to_uppercase()],
        )?;

        // Delete any alerts associated with this subscription
        conn.execute("DELETE FROM alerts WHERE subscription_id = ?", params![id])?;

        Ok(())
    }

    /// Get subscription by ID
    pub fn get_subscription(&self, id: i64) -> Result<Option<Subscription>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            r#"
            SELECT id, merchant, account_id, amount, frequency, first_seen, last_seen, status, user_acknowledged, acknowledged_at, created_at
            FROM subscriptions
            WHERE id = ?
            "#,
            params![id],
            |row| {
                let freq_str: Option<String> = row.get(4)?;
                let status_str: String = row.get(7)?;
                let first_seen_str: Option<String> = row.get(5)?;
                let last_seen_str: Option<String> = row.get(6)?;
                let acknowledged_at_str: Option<String> = row.get(9)?;
                let created_at_str: String = row.get(10)?;

                Ok(Subscription {
                    id: row.get(0)?,
                    merchant: row.get(1)?,
                    account_id: row.get(2)?,
                    amount: row.get(3)?,
                    frequency: freq_str.and_then(|s| match s.as_str() {
                        "weekly" => Some(Frequency::Weekly),
                        "monthly" => Some(Frequency::Monthly),
                        "yearly" => Some(Frequency::Yearly),
                        _ => None,
                    }),
                    first_seen: first_seen_str
                        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    last_seen: last_seen_str
                        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    status: match status_str.as_str() {
                        "cancelled" => SubscriptionStatus::Cancelled,
                        "zombie" => SubscriptionStatus::Zombie,
                        "excluded" => SubscriptionStatus::Excluded,
                        _ => SubscriptionStatus::Active,
                    },
                    user_acknowledged: row.get(8)?,
                    acknowledged_at: acknowledged_at_str.map(|s| parse_datetime(&s)),
                    created_at: parse_datetime(&created_at_str),
                })
            },
        );

        match result {
            Ok(sub) => Ok(Some(sub)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // ========== Merchant Subscription Cache ==========

    /// Check if a merchant is cached as a subscription or retail
    ///
    /// Returns Some(true) if subscription, Some(false) if retail, None if not cached
    pub fn get_merchant_subscription_cache(&self, merchant: &str) -> Result<Option<bool>> {
        let conn = self.conn()?;

        let result: std::result::Result<bool, _> = conn.query_row(
            "SELECT is_subscription FROM merchant_subscription_cache WHERE merchant_pattern = ?",
            params![merchant.to_uppercase()],
            |row| row.get(0),
        );

        match result {
            Ok(is_sub) => Ok(Some(is_sub)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Cache an Ollama subscription classification result
    pub fn cache_subscription_classification(
        &self,
        merchant: &str,
        is_subscription: bool,
        confidence: Option<f64>,
    ) -> Result<()> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT INTO merchant_subscription_cache (merchant_pattern, is_subscription, confidence, source)
            VALUES (?, ?, ?, 'ollama')
            ON CONFLICT(merchant_pattern) DO UPDATE SET
                is_subscription = excluded.is_subscription,
                confidence = excluded.confidence
            WHERE source != 'user_override'
            "#,
            params![merchant.to_uppercase(), is_subscription, confidence],
        )?;

        Ok(())
    }

    /// Check if merchant has a user override in the cache
    pub fn has_merchant_user_override(&self, merchant: &str) -> Result<bool> {
        let conn = self.conn()?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM merchant_subscription_cache WHERE merchant_pattern = ? AND source = 'user_override'",
            params![merchant.to_uppercase()],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }
}

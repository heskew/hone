//! Alert, dashboard, and audit log operations

use chrono::NaiveDate;
use rusqlite::params;

use super::{parse_datetime, AuditEntry, Database};
use crate::error::Result;
use crate::models::{
    Alert, AlertType, DashboardStats, Frequency, SpendingAnomalyData, SpendingChangeExplanation,
    Subscription, SubscriptionStatus,
};
use crate::ollama::DuplicateAnalysis;

impl Database {
    /// Create an alert
    pub fn create_alert(
        &self,
        alert_type: AlertType,
        subscription_id: Option<i64>,
        message: Option<&str>,
    ) -> Result<i64> {
        self.create_alert_with_analysis(alert_type, subscription_id, message, None)
    }

    /// Create an alert with optional Ollama analysis (for duplicate alerts)
    pub fn create_alert_with_analysis(
        &self,
        alert_type: AlertType,
        subscription_id: Option<i64>,
        message: Option<&str>,
        analysis: Option<&DuplicateAnalysis>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        // Check for existing undismissed alert of same type for same subscription
        if let Some(sub_id) = subscription_id {
            if let Ok(id) = conn.query_row(
                "SELECT id FROM alerts WHERE type = ? AND subscription_id = ? AND dismissed = FALSE",
                params![alert_type.as_str(), sub_id],
                |row| row.get(0),
            ) {
                return Ok(id);
            }
        }

        let analysis_json = analysis.map(|a| serde_json::to_string(a).ok()).flatten();

        conn.execute(
            "INSERT INTO alerts (type, subscription_id, message, ollama_analysis) VALUES (?, ?, ?, ?)",
            params![alert_type.as_str(), subscription_id, message, analysis_json],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Create a spending anomaly alert
    pub fn create_spending_anomaly_alert(&self, data: &SpendingAnomalyData) -> Result<i64> {
        let conn = self.conn()?;

        // Check for existing undismissed spending_anomaly alert for the same tag
        if let Ok(id) = conn.query_row(
            r#"
            SELECT id FROM alerts
            WHERE type = 'spending_anomaly'
            AND dismissed = FALSE
            AND json_extract(spending_anomaly_data, '$.tag_id') = ?
            "#,
            params![data.tag_id],
            |row| row.get::<_, i64>(0),
        ) {
            // Update existing alert with new data
            let data_json = serde_json::to_string(data).ok();
            let message = format!(
                "{} spending {} by {:.0}%",
                data.tag_name,
                if data.percent_change > 0.0 {
                    "increased"
                } else {
                    "decreased"
                },
                data.percent_change.abs()
            );
            conn.execute(
                "UPDATE alerts SET spending_anomaly_data = ?, message = ? WHERE id = ?",
                params![data_json, message, id],
            )?;
            return Ok(id);
        }

        // Create new alert
        let data_json = serde_json::to_string(data).ok();
        let message = format!(
            "{} spending {} by {:.0}%",
            data.tag_name,
            if data.percent_change > 0.0 {
                "increased"
            } else {
                "decreased"
            },
            data.percent_change.abs()
        );

        conn.execute(
            "INSERT INTO alerts (type, message, spending_anomaly_data) VALUES (?, ?, ?)",
            params!["spending_anomaly", message, data_json],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Update the spending analysis (explanation) for an existing spending anomaly alert
    pub fn update_spending_analysis(
        &self,
        alert_id: i64,
        explanation: &SpendingChangeExplanation,
    ) -> Result<()> {
        let conn = self.conn()?;

        // Get current spending_anomaly_data
        let current_json: Option<String> = conn.query_row(
            "SELECT spending_anomaly_data FROM alerts WHERE id = ?",
            params![alert_id],
            |row| row.get(0),
        )?;

        // Parse, update, and save back
        if let Some(json) = current_json {
            if let Ok(mut data) = serde_json::from_str::<SpendingAnomalyData>(&json) {
                data.explanation = Some(explanation.clone());
                let updated_json = serde_json::to_string(&data).ok();
                conn.execute(
                    "UPDATE alerts SET spending_anomaly_data = ? WHERE id = ?",
                    params![updated_json, alert_id],
                )?;
            }
        }

        Ok(())
    }

    /// List alerts (optionally filtered by dismissed status)
    pub fn list_alerts(&self, include_dismissed: bool) -> Result<Vec<Alert>> {
        let conn = self.conn()?;

        let sql = if include_dismissed {
            r#"
            SELECT a.id, a.type, a.subscription_id, a.message, a.dismissed, a.created_at, a.ollama_analysis, a.spending_anomaly_data,
                   s.id, s.merchant, s.account_id, s.amount, s.frequency, s.first_seen, s.last_seen, s.status, s.user_acknowledged, s.acknowledged_at, s.created_at
            FROM alerts a
            LEFT JOIN subscriptions s ON a.subscription_id = s.id
            ORDER BY a.created_at DESC
            "#
        } else {
            r#"
            SELECT a.id, a.type, a.subscription_id, a.message, a.dismissed, a.created_at, a.ollama_analysis, a.spending_anomaly_data,
                   s.id, s.merchant, s.account_id, s.amount, s.frequency, s.first_seen, s.last_seen, s.status, s.user_acknowledged, s.acknowledged_at, s.created_at
            FROM alerts a
            LEFT JOIN subscriptions s ON a.subscription_id = s.id
            WHERE a.dismissed = FALSE
            ORDER BY a.created_at DESC
            "#
        };

        let mut stmt = conn.prepare(sql)?;

        let alerts = stmt
            .query_map([], |row| {
                let type_str: String = row.get(1)?;
                let alert_created_at_str: String = row.get(5)?;
                let analysis_json: Option<String> = row.get(6)?;
                let spending_anomaly_json: Option<String> = row.get(7)?;

                // Parse subscription if present
                // Columns: 8=s.id, 9=s.merchant, 10=s.account_id, 11=s.amount, 12=s.frequency,
                //          13=s.first_seen, 14=s.last_seen, 15=s.status, 16=s.user_acknowledged, 17=s.acknowledged_at, 18=s.created_at
                let subscription: Option<Subscription> = row.get::<_, Option<i64>>(8)?.map(|_| {
                    let freq_str: Option<String> = row.get(12).ok().flatten();
                    let status_str: String = row.get(15).unwrap_or_else(|_| "active".to_string());
                    let first_seen_str: Option<String> = row.get(13).ok().flatten();
                    let last_seen_str: Option<String> = row.get(14).ok().flatten();
                    let acknowledged_at_str: Option<String> = row.get(17).ok().flatten();
                    let sub_created_at_str: String = row.get(18).unwrap_or_default();

                    Subscription {
                        id: row.get(8).unwrap_or(0),
                        merchant: row.get(9).unwrap_or_default(),
                        account_id: row.get(10).ok().flatten(),
                        amount: row.get(11).ok().flatten(),
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
                        user_acknowledged: row.get(16).unwrap_or(false),
                        acknowledged_at: acknowledged_at_str.map(|s| parse_datetime(&s)),
                        created_at: parse_datetime(&sub_created_at_str),
                    }
                });

                // Parse ollama_analysis JSON
                let ollama_analysis: Option<DuplicateAnalysis> =
                    analysis_json.and_then(|json| serde_json::from_str(&json).ok());

                // Parse spending_anomaly_data JSON
                let spending_anomaly: Option<SpendingAnomalyData> =
                    spending_anomaly_json.and_then(|json| serde_json::from_str(&json).ok());

                Ok(Alert {
                    id: row.get(0)?,
                    alert_type: match type_str.as_str() {
                        "price_increase" => AlertType::PriceIncrease,
                        "duplicate" => AlertType::Duplicate,
                        "resume" => AlertType::Resume,
                        "spending_anomaly" => AlertType::SpendingAnomaly,
                        "tip_discrepancy" => AlertType::TipDiscrepancy,
                        _ => AlertType::Zombie,
                    },
                    subscription_id: row.get(2)?,
                    message: row.get(3)?,
                    dismissed: row.get(4)?,
                    created_at: parse_datetime(&alert_created_at_str),
                    ollama_analysis,
                    spending_anomaly,
                    subscription,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(alerts)
    }

    /// Get a single alert by ID
    pub fn get_alert(&self, id: i64) -> Result<Alert> {
        let conn = self.conn()?;

        conn.query_row(
            r#"
            SELECT a.id, a.type, a.subscription_id, a.message, a.dismissed, a.created_at, a.ollama_analysis, a.spending_anomaly_data
            FROM alerts a
            WHERE a.id = ?
            "#,
            params![id],
            |row| {
                let type_str: String = row.get(1)?;
                let alert_created_at_str: String = row.get(5)?;
                let analysis_json: Option<String> = row.get(6)?;
                let spending_anomaly_json: Option<String> = row.get(7)?;

                // Parse ollama_analysis JSON
                let ollama_analysis: Option<DuplicateAnalysis> =
                    analysis_json.and_then(|json| serde_json::from_str(&json).ok());

                // Parse spending_anomaly_data JSON
                let spending_anomaly: Option<SpendingAnomalyData> =
                    spending_anomaly_json.and_then(|json| serde_json::from_str(&json).ok());

                Ok(Alert {
                    id: row.get(0)?,
                    alert_type: match type_str.as_str() {
                        "price_increase" => AlertType::PriceIncrease,
                        "duplicate" => AlertType::Duplicate,
                        "resume" => AlertType::Resume,
                        "spending_anomaly" => AlertType::SpendingAnomaly,
                        "tip_discrepancy" => AlertType::TipDiscrepancy,
                        _ => AlertType::Zombie,
                    },
                    subscription_id: row.get(2)?,
                    message: row.get(3)?,
                    dismissed: row.get(4)?,
                    created_at: parse_datetime(&alert_created_at_str),
                    ollama_analysis,
                    spending_anomaly,
                    subscription: None, // Don't load subscription for simple get
                })
            },
        )
        .map_err(|e| e.into())
    }

    /// Count active (undismissed) alerts
    pub fn count_active_alerts(&self) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM alerts WHERE dismissed = FALSE",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Dismiss an alert
    pub fn dismiss_alert(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE alerts SET dismissed = TRUE WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Restore (undismiss) an alert
    pub fn restore_alert(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE alerts SET dismissed = FALSE WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Get dashboard statistics
    pub fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        let conn = self.conn()?;

        let total_transactions: i64 =
            conn.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?;

        let total_accounts: i64 =
            conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;

        let active_subscriptions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subscriptions WHERE status = 'active'",
            [],
            |row| row.get(0),
        )?;

        let monthly_subscription_cost: f64 = conn
            .query_row(
                r#"
                SELECT COALESCE(SUM(
                    CASE frequency
                        WHEN 'weekly' THEN amount * 4.33
                        WHEN 'monthly' THEN amount
                        WHEN 'yearly' THEN amount / 12
                        ELSE amount
                    END
                ), 0)
                FROM subscriptions
                WHERE status = 'active' AND amount IS NOT NULL
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let active_alerts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM alerts WHERE dismissed = FALSE",
            [],
            |row| row.get(0),
        )?;

        // Potential savings from zombie subscriptions
        let potential_monthly_savings: f64 = conn
            .query_row(
                r#"
                SELECT COALESCE(SUM(
                    CASE s.frequency
                        WHEN 'weekly' THEN s.amount * 4.33
                        WHEN 'monthly' THEN s.amount
                        WHEN 'yearly' THEN s.amount / 12
                        ELSE s.amount
                    END
                ), 0)
                FROM alerts a
                JOIN subscriptions s ON a.subscription_id = s.id
                WHERE a.type = 'zombie' AND a.dismissed = FALSE AND s.amount IS NOT NULL
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        // Count transactions without tags
        let untagged_transactions: i64 = conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM transactions t
            WHERE NOT EXISTS (
                SELECT 1 FROM transaction_tags tt WHERE tt.transaction_id = t.id
            )
            "#,
            [],
            |row| row.get(0),
        )?;

        Ok(DashboardStats {
            total_transactions,
            total_accounts,
            active_subscriptions,
            monthly_subscription_cost,
            active_alerts,
            potential_monthly_savings,
            recent_imports: vec![], // TODO: Track imports separately
            untagged_transactions,
        })
    }

    /// Log an audit event
    pub fn log_audit(
        &self,
        user_email: &str,
        action: &str,
        entity_type: Option<&str>,
        entity_id: Option<i64>,
        details: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT INTO audit_log (user_email, action, entity_type, entity_id, details)
            VALUES (?, ?, ?, ?, ?)
            "#,
            params![user_email, action, entity_type, entity_id, details],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// List audit log entries
    pub fn list_audit_log(&self, limit: i64) -> Result<Vec<AuditEntry>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, timestamp, user_email, action, entity_type, entity_id, details
            FROM audit_log
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )?;

        let entries = stmt
            .query_map(params![limit], |row| {
                let timestamp_str: String = row.get(1)?;
                Ok(AuditEntry {
                    id: row.get(0)?,
                    timestamp: timestamp_str,
                    user_email: row.get(2)?,
                    action: row.get(3)?,
                    entity_type: row.get(4)?,
                    entity_id: row.get(5)?,
                    details: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }
}

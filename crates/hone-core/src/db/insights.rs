//! Insight finding database operations

use chrono::{Duration, Utc};
use rusqlite::params;

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::insights::{Finding, InsightFinding, InsightStatus, InsightType, Severity};

impl Database {
    /// Upsert an insight finding by its unique key
    ///
    /// If a finding with the same (insight_type, finding_key) exists, updates
    /// last_detected_at and refreshes the data. Otherwise inserts a new record.
    pub fn upsert_insight_finding(&self, finding: &Finding) -> Result<i64> {
        let conn = self.conn()?;

        let data_json = serde_json::to_string(&finding.data)?;
        let insight_type = finding.insight_type.as_str();
        let severity = finding.severity.as_str();
        let detected_at = finding.detected_at.format("%Y-%m-%d %H:%M:%S").to_string();

        // Try to update existing
        let updated = conn.execute(
            r#"
            UPDATE insight_findings
            SET last_detected_at = ?,
                severity = ?,
                title = ?,
                summary = ?,
                detail = ?,
                data = ?,
                status = CASE WHEN status = 'snoozed' AND snoozed_until < datetime('now') THEN 'active' ELSE status END
            WHERE insight_type = ? AND finding_key = ?
            "#,
            params![
                detected_at,
                severity,
                finding.title,
                finding.summary,
                finding.detail,
                data_json,
                insight_type,
                finding.key
            ],
        )?;

        if updated > 0 {
            // Get the existing id
            let id: i64 = conn.query_row(
                "SELECT id FROM insight_findings WHERE insight_type = ? AND finding_key = ?",
                params![insight_type, finding.key],
                |row| row.get(0),
            )?;
            return Ok(id);
        }

        // Insert new
        conn.execute(
            r#"
            INSERT INTO insight_findings (
                insight_type, finding_key, severity, title, summary, detail, data,
                first_detected_at, last_detected_at, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active')
            "#,
            params![
                insight_type,
                finding.key,
                severity,
                finding.title,
                finding.summary,
                finding.detail,
                data_json,
                detected_at,
                detected_at
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// List insight findings with optional status filter
    pub fn list_insight_findings(
        &self,
        status: Option<InsightStatus>,
    ) -> Result<Vec<InsightFinding>> {
        let conn = self.conn()?;

        let findings = if let Some(s) = status {
            let sql = r#"
                SELECT id, insight_type, finding_key, severity, title, summary, detail, data,
                       first_detected_at, last_detected_at, status, snoozed_until, user_feedback
                FROM insight_findings
                WHERE status = ?
                ORDER BY
                    CASE severity
                        WHEN 'alert' THEN 1
                        WHEN 'warning' THEN 2
                        WHEN 'attention' THEN 3
                        ELSE 4
                    END,
                    last_detected_at DESC
            "#;
            let mut stmt = conn.prepare(sql)?;
            let rows =
                stmt.query_map(params![s.as_str()], |row| self.row_to_insight_finding(row))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let sql = r#"
                SELECT id, insight_type, finding_key, severity, title, summary, detail, data,
                       first_detected_at, last_detected_at, status, snoozed_until, user_feedback
                FROM insight_findings
                ORDER BY
                    CASE severity
                        WHEN 'alert' THEN 1
                        WHEN 'warning' THEN 2
                        WHEN 'attention' THEN 3
                        ELSE 4
                    END,
                    last_detected_at DESC
            "#;
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| self.row_to_insight_finding(row))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok(findings)
    }

    /// Get top N active insights for dashboard display
    pub fn get_top_insights(&self, limit: usize) -> Result<Vec<InsightFinding>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, insight_type, finding_key, severity, title, summary, detail, data,
                   first_detected_at, last_detected_at, status, snoozed_until, user_feedback
            FROM insight_findings
            WHERE status = 'active'
               OR (status = 'snoozed' AND snoozed_until < datetime('now'))
            ORDER BY
                CASE severity
                    WHEN 'alert' THEN 1
                    WHEN 'warning' THEN 2
                    WHEN 'attention' THEN 3
                    ELSE 4
                END,
                last_detected_at DESC
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            self.row_to_insight_finding(row)
        })?;

        let findings: std::result::Result<Vec<_>, _> = rows.collect();
        Ok(findings?)
    }

    /// Get a single insight finding by ID
    pub fn get_insight_finding(&self, id: i64) -> Result<Option<InsightFinding>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            r#"
            SELECT id, insight_type, finding_key, severity, title, summary, detail, data,
                   first_detected_at, last_detected_at, status, snoozed_until, user_feedback
            FROM insight_findings
            WHERE id = ?
            "#,
            params![id],
            |row| self.row_to_insight_finding(row),
        );

        match result {
            Ok(finding) => Ok(Some(finding)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Dismiss an insight finding
    pub fn dismiss_insight(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE insight_findings SET status = 'dismissed' WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Snooze an insight finding for N days
    pub fn snooze_insight(&self, id: i64, days: u32) -> Result<()> {
        let conn = self.conn()?;
        let snooze_until = (Utc::now() + Duration::days(days as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        conn.execute(
            "UPDATE insight_findings SET status = 'snoozed', snoozed_until = ? WHERE id = ?",
            params![snooze_until, id],
        )?;
        Ok(())
    }

    /// Restore a dismissed or snoozed insight to active
    pub fn restore_insight(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE insight_findings SET status = 'active', snoozed_until = NULL WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    /// Set user feedback on an insight
    pub fn set_insight_feedback(&self, id: i64, feedback: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE insight_findings SET user_feedback = ? WHERE id = ?",
            params![feedback, id],
        )?;
        Ok(())
    }

    /// Count active insights
    pub fn count_active_insights(&self) -> Result<i64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            r#"
            SELECT COUNT(*) FROM insight_findings
            WHERE status = 'active'
               OR (status = 'snoozed' AND snoozed_until < datetime('now'))
            "#,
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Delete all insights (for testing/reset)
    pub fn delete_all_insights(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM insight_findings", [])?;
        Ok(())
    }

    /// Helper to convert a row to InsightFinding
    fn row_to_insight_finding(&self, row: &rusqlite::Row) -> rusqlite::Result<InsightFinding> {
        let insight_type_str: String = row.get(1)?;
        let severity_str: String = row.get(3)?;
        let status_str: String = row.get(10)?;
        let data_json: String = row.get(7)?;
        let first_detected_str: String = row.get(8)?;
        let last_detected_str: String = row.get(9)?;
        let snoozed_until_str: Option<String> = row.get(11)?;

        Ok(InsightFinding {
            id: row.get(0)?,
            insight_type: insight_type_str
                .parse()
                .unwrap_or(InsightType::SavingsOpportunity),
            finding_key: row.get(2)?,
            severity: severity_str.parse().unwrap_or(Severity::Info),
            title: row.get(4)?,
            summary: row.get(5)?,
            detail: row.get(6)?,
            data: serde_json::from_str(&data_json).unwrap_or_default(),
            first_detected_at: parse_datetime(&first_detected_str),
            last_detected_at: parse_datetime(&last_detected_str),
            status: status_str.parse().unwrap_or(InsightStatus::Active),
            snoozed_until: snoozed_until_str.map(|s| parse_datetime(&s)),
            user_feedback: row.get(12)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::Finding;

    #[test]
    fn test_upsert_insight_finding() {
        let db = Database::in_memory().unwrap();

        let finding = Finding::new(
            InsightType::SavingsOpportunity,
            "test:key:1",
            Severity::Warning,
            "Test Title",
            "Test summary",
        )
        .with_detail("Test detail");

        let id1 = db.upsert_insight_finding(&finding).unwrap();
        assert!(id1 > 0);

        // Upsert again should update, not create new
        let id2 = db.upsert_insight_finding(&finding).unwrap();
        assert_eq!(id1, id2);

        // Verify only one record
        let findings = db.list_insight_findings(None).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].title, "Test Title");
    }

    #[test]
    fn test_dismiss_and_restore() {
        let db = Database::in_memory().unwrap();

        let finding = Finding::new(
            InsightType::ExpenseForecaster,
            "test:dismiss",
            Severity::Info,
            "Dismiss Test",
            "Test",
        );

        let id = db.upsert_insight_finding(&finding).unwrap();

        // Should be active
        let f = db.get_insight_finding(id).unwrap().unwrap();
        assert_eq!(f.status, InsightStatus::Active);

        // Dismiss
        db.dismiss_insight(id).unwrap();
        let f = db.get_insight_finding(id).unwrap().unwrap();
        assert_eq!(f.status, InsightStatus::Dismissed);

        // Restore
        db.restore_insight(id).unwrap();
        let f = db.get_insight_finding(id).unwrap().unwrap();
        assert_eq!(f.status, InsightStatus::Active);
    }

    #[test]
    fn test_snooze_insight() {
        let db = Database::in_memory().unwrap();

        let finding = Finding::new(
            InsightType::SpendingExplainer,
            "test:snooze",
            Severity::Attention,
            "Snooze Test",
            "Test",
        );

        let id = db.upsert_insight_finding(&finding).unwrap();

        // Snooze for 7 days
        db.snooze_insight(id, 7).unwrap();

        let f = db.get_insight_finding(id).unwrap().unwrap();
        assert_eq!(f.status, InsightStatus::Snoozed);
        assert!(f.snoozed_until.is_some());
    }

    #[test]
    fn test_get_top_insights() {
        let db = Database::in_memory().unwrap();

        // Create insights with different severities
        for (i, severity) in [Severity::Info, Severity::Warning, Severity::Alert]
            .iter()
            .enumerate()
        {
            let finding = Finding::new(
                InsightType::SavingsOpportunity,
                format!("test:top:{}", i),
                *severity,
                format!("Finding {}", i),
                "Test",
            );
            db.upsert_insight_finding(&finding).unwrap();
        }

        let top = db.get_top_insights(2).unwrap();
        assert_eq!(top.len(), 2);
        // Alert should be first
        assert_eq!(top[0].severity, Severity::Alert);
        // Warning should be second
        assert_eq!(top[1].severity, Severity::Warning);
    }

    #[test]
    fn test_count_active_insights() {
        let db = Database::in_memory().unwrap();

        // Create 3 insights
        for i in 0..3 {
            let finding = Finding::new(
                InsightType::SavingsOpportunity,
                format!("test:count:{}", i),
                Severity::Info,
                format!("Finding {}", i),
                "Test",
            );
            db.upsert_insight_finding(&finding).unwrap();
        }

        assert_eq!(db.count_active_insights().unwrap(), 3);

        // Dismiss one
        db.dismiss_insight(1).unwrap();
        assert_eq!(db.count_active_insights().unwrap(), 2);
    }
}

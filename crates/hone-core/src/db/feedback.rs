//! User feedback operations

use rusqlite::params;

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::{
    FeedbackContext, FeedbackStats, FeedbackTargetStats, FeedbackTargetType, FeedbackType,
    NewUserFeedback, UserFeedback,
};

impl Database {
    /// Create a new feedback record
    pub fn create_feedback(&self, feedback: &NewUserFeedback) -> Result<i64> {
        let conn = self.conn()?;

        let context_json = feedback
            .context
            .as_ref()
            .map(|c| serde_json::to_string(c).ok())
            .flatten();

        conn.execute(
            r#"
            INSERT INTO user_feedback (
                feedback_type, target_type, target_id, original_value,
                corrected_value, reason, context
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                feedback.feedback_type.as_str(),
                feedback.target_type.as_str(),
                feedback.target_id,
                feedback.original_value,
                feedback.corrected_value,
                feedback.reason,
                context_json,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get a feedback record by ID
    pub fn get_feedback(&self, id: i64) -> Result<UserFeedback> {
        let conn = self.conn()?;

        conn.query_row(
            r#"
            SELECT id, feedback_type, target_type, target_id, original_value,
                   corrected_value, reason, context, created_at, reverted_at
            FROM user_feedback
            WHERE id = ?
            "#,
            params![id],
            |row| {
                let feedback_type_str: String = row.get(1)?;
                let target_type_str: String = row.get(2)?;
                let context_json: Option<String> = row.get(7)?;
                let created_at_str: String = row.get(8)?;
                let reverted_at_str: Option<String> = row.get(9)?;

                Ok(UserFeedback {
                    id: row.get(0)?,
                    feedback_type: feedback_type_str.parse().unwrap_or(FeedbackType::Helpful),
                    target_type: target_type_str.parse().unwrap_or(FeedbackTargetType::Alert),
                    target_id: row.get(3)?,
                    original_value: row.get(4)?,
                    corrected_value: row.get(5)?,
                    reason: row.get(6)?,
                    context: context_json.and_then(|j| serde_json::from_str(&j).ok()),
                    created_at: parse_datetime(&created_at_str),
                    reverted_at: reverted_at_str.map(|s| parse_datetime(&s)),
                })
            },
        )
        .map_err(|e| e.into())
    }

    /// List feedback records with optional filters
    pub fn list_feedback(
        &self,
        target_type: Option<FeedbackTargetType>,
        feedback_type: Option<FeedbackType>,
        include_reverted: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<UserFeedback>> {
        let conn = self.conn()?;

        let mut sql = String::from(
            r#"
            SELECT id, feedback_type, target_type, target_id, original_value,
                   corrected_value, reason, context, created_at, reverted_at
            FROM user_feedback
            WHERE 1=1
            "#,
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(tt) = target_type {
            sql.push_str(" AND target_type = ?");
            params_vec.push(Box::new(tt.as_str().to_string()));
        }

        if let Some(ft) = feedback_type {
            sql.push_str(" AND feedback_type = ?");
            params_vec.push(Box::new(ft.as_str().to_string()));
        }

        if !include_reverted {
            sql.push_str(" AND reverted_at IS NULL");
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        params_vec.push(Box::new(limit));
        params_vec.push(Box::new(offset));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let feedback = stmt
            .query_map(params_refs.as_slice(), |row| {
                let feedback_type_str: String = row.get(1)?;
                let target_type_str: String = row.get(2)?;
                let context_json: Option<String> = row.get(7)?;
                let created_at_str: String = row.get(8)?;
                let reverted_at_str: Option<String> = row.get(9)?;

                Ok(UserFeedback {
                    id: row.get(0)?,
                    feedback_type: feedback_type_str.parse().unwrap_or(FeedbackType::Helpful),
                    target_type: target_type_str.parse().unwrap_or(FeedbackTargetType::Alert),
                    target_id: row.get(3)?,
                    original_value: row.get(4)?,
                    corrected_value: row.get(5)?,
                    reason: row.get(6)?,
                    context: context_json.and_then(|j| serde_json::from_str(&j).ok()),
                    created_at: parse_datetime(&created_at_str),
                    reverted_at: reverted_at_str.map(|s| parse_datetime(&s)),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(feedback)
    }

    /// List feedback for a specific target
    pub fn list_feedback_for_target(
        &self,
        target_type: FeedbackTargetType,
        target_id: i64,
    ) -> Result<Vec<UserFeedback>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, feedback_type, target_type, target_id, original_value,
                   corrected_value, reason, context, created_at, reverted_at
            FROM user_feedback
            WHERE target_type = ? AND target_id = ?
            ORDER BY created_at DESC
            "#,
        )?;

        let feedback = stmt
            .query_map(params![target_type.as_str(), target_id], |row| {
                let feedback_type_str: String = row.get(1)?;
                let target_type_str: String = row.get(2)?;
                let context_json: Option<String> = row.get(7)?;
                let created_at_str: String = row.get(8)?;
                let reverted_at_str: Option<String> = row.get(9)?;

                Ok(UserFeedback {
                    id: row.get(0)?,
                    feedback_type: feedback_type_str.parse().unwrap_or(FeedbackType::Helpful),
                    target_type: target_type_str.parse().unwrap_or(FeedbackTargetType::Alert),
                    target_id: row.get(3)?,
                    original_value: row.get(4)?,
                    corrected_value: row.get(5)?,
                    reason: row.get(6)?,
                    context: context_json.and_then(|j| serde_json::from_str(&j).ok()),
                    created_at: parse_datetime(&created_at_str),
                    reverted_at: reverted_at_str.map(|s| parse_datetime(&s)),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(feedback)
    }

    /// Revert (undo) a feedback record
    pub fn revert_feedback(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE user_feedback SET reverted_at = CURRENT_TIMESTAMP WHERE id = ? AND reverted_at IS NULL",
            params![id],
        )?;

        Ok(())
    }

    /// Unrevert a feedback record (restore it)
    pub fn unrevert_feedback(&self, id: i64) -> Result<()> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE user_feedback SET reverted_at = NULL WHERE id = ?",
            params![id],
        )?;

        Ok(())
    }

    /// Get feedback statistics
    pub fn get_feedback_stats(&self) -> Result<FeedbackStats> {
        let conn = self.conn()?;

        // Total counts by feedback type
        let total_feedback: i64 =
            conn.query_row("SELECT COUNT(*) FROM user_feedback", [], |row| row.get(0))?;

        let helpful_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE feedback_type = 'helpful' AND reverted_at IS NULL",
            [],
            |row| row.get(0),
        )?;

        let not_helpful_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE feedback_type = 'not_helpful' AND reverted_at IS NULL",
            [],
            |row| row.get(0),
        )?;

        let correction_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE feedback_type = 'correction' AND reverted_at IS NULL",
            [],
            |row| row.get(0),
        )?;

        let dismissal_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE feedback_type = 'dismissal' AND reverted_at IS NULL",
            [],
            |row| row.get(0),
        )?;

        let reverted_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE reverted_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;

        // Breakdown by target type
        let mut by_target_type = Vec::new();
        let target_types = [
            FeedbackTargetType::Alert,
            FeedbackTargetType::Insight,
            FeedbackTargetType::Classification,
            FeedbackTargetType::Explanation,
            FeedbackTargetType::ReceiptMatch,
        ];

        for tt in target_types {
            let total: i64 = conn.query_row(
                "SELECT COUNT(*) FROM user_feedback WHERE target_type = ? AND reverted_at IS NULL",
                params![tt.as_str()],
                |row| row.get(0),
            )?;

            if total > 0 {
                let helpful: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM user_feedback WHERE target_type = ? AND feedback_type = 'helpful' AND reverted_at IS NULL",
                    params![tt.as_str()],
                    |row| row.get(0),
                )?;

                let not_helpful: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM user_feedback WHERE target_type = ? AND feedback_type = 'not_helpful' AND reverted_at IS NULL",
                    params![tt.as_str()],
                    |row| row.get(0),
                )?;

                let helpfulness_ratio = if helpful + not_helpful > 0 {
                    helpful as f64 / (helpful + not_helpful) as f64
                } else {
                    0.0
                };

                by_target_type.push(FeedbackTargetStats {
                    target_type: tt,
                    total,
                    helpful,
                    not_helpful,
                    helpfulness_ratio,
                });
            }
        }

        Ok(FeedbackStats {
            total_feedback,
            helpful_count,
            not_helpful_count,
            correction_count,
            dismissal_count,
            reverted_count,
            by_target_type,
        })
    }

    /// Record implicit feedback when user dismisses an alert
    /// This is a convenience method that creates a dismissal feedback record
    pub fn record_alert_dismissal(
        &self,
        alert_id: i64,
        context: Option<FeedbackContext>,
    ) -> Result<i64> {
        self.create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::Dismissal,
            target_type: FeedbackTargetType::Alert,
            target_id: Some(alert_id),
            original_value: None,
            corrected_value: None,
            reason: None,
            context,
        })
    }

    /// Record explicit helpful/not helpful feedback on an explanation
    pub fn record_explanation_feedback(
        &self,
        alert_id: i64,
        helpful: bool,
        reason: Option<String>,
        context: Option<FeedbackContext>,
    ) -> Result<i64> {
        self.create_feedback(&NewUserFeedback {
            feedback_type: if helpful {
                FeedbackType::Helpful
            } else {
                FeedbackType::NotHelpful
            },
            target_type: FeedbackTargetType::Explanation,
            target_id: Some(alert_id),
            original_value: None,
            corrected_value: None,
            reason,
            context,
        })
    }

    /// Record a classification correction (user changed a tag/category)
    pub fn record_classification_correction(
        &self,
        transaction_id: i64,
        original_tag_id: i64,
        corrected_tag_id: i64,
        context: Option<FeedbackContext>,
    ) -> Result<i64> {
        self.create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::Correction,
            target_type: FeedbackTargetType::Classification,
            target_id: Some(transaction_id),
            original_value: Some(original_tag_id.to_string()),
            corrected_value: Some(corrected_tag_id.to_string()),
            reason: None,
            context,
        })
    }

    /// Get a summary of recent feedback for prompt context
    /// Returns formatted text describing user preferences based on feedback
    pub fn get_feedback_summary_for_prompt(
        &self,
        target_type: FeedbackTargetType,
    ) -> Result<String> {
        let conn = self.conn()?;

        // Get recent not_helpful feedback with reasons (most valuable for improving prompts)
        let mut stmt = conn.prepare(
            r#"
            SELECT reason, original_value
            FROM user_feedback
            WHERE target_type = ?
              AND feedback_type = 'not_helpful'
              AND reverted_at IS NULL
              AND reason IS NOT NULL
              AND reason != ''
            ORDER BY created_at DESC
            LIMIT 5
            "#,
        )?;

        let not_helpful_reasons: Vec<String> = stmt
            .query_map(params![target_type.as_str()], |row| {
                let reason: String = row.get(0)?;
                Ok(reason)
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Get helpfulness ratio
        let helpful: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE target_type = ? AND feedback_type = 'helpful' AND reverted_at IS NULL",
            params![target_type.as_str()],
            |row| row.get(0),
        )?;

        let not_helpful: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_feedback WHERE target_type = ? AND feedback_type = 'not_helpful' AND reverted_at IS NULL",
            params![target_type.as_str()],
            |row| row.get(0),
        )?;

        // Build summary text
        let mut summary_parts = Vec::new();

        // Add helpfulness context if we have enough data
        let total = helpful + not_helpful;
        if total >= 5 {
            let ratio = helpful as f64 / total as f64 * 100.0;
            summary_parts.push(format!(
                "Previous responses were rated helpful {:.0}% of the time ({} ratings).",
                ratio, total
            ));
        }

        // Add reasons for not helpful feedback
        if !not_helpful_reasons.is_empty() {
            summary_parts.push("User feedback on previous responses:".to_string());
            for reason in not_helpful_reasons.iter().take(3) {
                summary_parts.push(format!("- \"{}\"", reason));
            }
        }

        Ok(summary_parts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_feedback() {
        let db = Database::in_memory().unwrap();

        let feedback = NewUserFeedback {
            feedback_type: FeedbackType::Helpful,
            target_type: FeedbackTargetType::Explanation,
            target_id: Some(42),
            original_value: None,
            corrected_value: None,
            reason: Some("Great explanation!".to_string()),
            context: Some(FeedbackContext {
                model: Some("gemma3".to_string()),
                prompt_version: Some("1".to_string()),
                transaction_id: None,
                extra: None,
            }),
        };

        let id = db.create_feedback(&feedback).unwrap();
        assert!(id > 0);

        let fetched = db.get_feedback(id).unwrap();
        assert_eq!(fetched.feedback_type, FeedbackType::Helpful);
        assert_eq!(fetched.target_type, FeedbackTargetType::Explanation);
        assert_eq!(fetched.target_id, Some(42));
        assert_eq!(fetched.reason, Some("Great explanation!".to_string()));
        assert!(fetched.context.is_some());
        assert_eq!(
            fetched.context.as_ref().unwrap().model,
            Some("gemma3".to_string())
        );
        assert!(fetched.reverted_at.is_none());
    }

    #[test]
    fn test_list_feedback() {
        let db = Database::in_memory().unwrap();

        // Create some feedback
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

        // List all
        let all = db.list_feedback(None, None, false, 100, 0).unwrap();
        assert_eq!(all.len(), 2);

        // Filter by target type
        let alerts = db
            .list_feedback(Some(FeedbackTargetType::Alert), None, false, 100, 0)
            .unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].target_type, FeedbackTargetType::Alert);

        // Filter by feedback type
        let helpful = db
            .list_feedback(None, Some(FeedbackType::Helpful), false, 100, 0)
            .unwrap();
        assert_eq!(helpful.len(), 1);
        assert_eq!(helpful[0].feedback_type, FeedbackType::Helpful);
    }

    #[test]
    fn test_revert_feedback() {
        let db = Database::in_memory().unwrap();

        let id = db
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

        // Initially not reverted
        let feedback = db.get_feedback(id).unwrap();
        assert!(feedback.reverted_at.is_none());

        // Revert
        db.revert_feedback(id).unwrap();
        let reverted = db.get_feedback(id).unwrap();
        assert!(reverted.reverted_at.is_some());

        // Should be excluded from non-reverted list
        let active = db.list_feedback(None, None, false, 100, 0).unwrap();
        assert_eq!(active.len(), 0);

        // Should be included when including reverted
        let all = db.list_feedback(None, None, true, 100, 0).unwrap();
        assert_eq!(all.len(), 1);

        // Unrevert
        db.unrevert_feedback(id).unwrap();
        let unreverted = db.get_feedback(id).unwrap();
        assert!(unreverted.reverted_at.is_none());
    }

    #[test]
    fn test_feedback_stats() {
        let db = Database::in_memory().unwrap();

        // Create various feedback
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
            target_type: FeedbackTargetType::Alert,
            target_id: Some(2),
            original_value: None,
            corrected_value: None,
            reason: None,
            context: None,
        })
        .unwrap();

        db.create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::Correction,
            target_type: FeedbackTargetType::Classification,
            target_id: Some(100),
            original_value: Some("5".to_string()),
            corrected_value: Some("10".to_string()),
            reason: None,
            context: None,
        })
        .unwrap();

        let stats = db.get_feedback_stats().unwrap();
        assert_eq!(stats.total_feedback, 3);
        assert_eq!(stats.helpful_count, 1);
        assert_eq!(stats.not_helpful_count, 1);
        assert_eq!(stats.correction_count, 1);
        assert_eq!(stats.reverted_count, 0);

        // Check target type breakdown
        let alert_stats = stats
            .by_target_type
            .iter()
            .find(|s| s.target_type == FeedbackTargetType::Alert)
            .unwrap();
        assert_eq!(alert_stats.total, 2);
        assert_eq!(alert_stats.helpful, 1);
        assert_eq!(alert_stats.not_helpful, 1);
        assert!((alert_stats.helpfulness_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_convenience_methods() {
        let db = Database::in_memory().unwrap();

        // Record alert dismissal
        let id = db.record_alert_dismissal(42, None).unwrap();
        let feedback = db.get_feedback(id).unwrap();
        assert_eq!(feedback.feedback_type, FeedbackType::Dismissal);
        assert_eq!(feedback.target_type, FeedbackTargetType::Alert);
        assert_eq!(feedback.target_id, Some(42));

        // Record explanation feedback
        let id = db
            .record_explanation_feedback(10, true, Some("Very helpful!".to_string()), None)
            .unwrap();
        let feedback = db.get_feedback(id).unwrap();
        assert_eq!(feedback.feedback_type, FeedbackType::Helpful);
        assert_eq!(feedback.target_type, FeedbackTargetType::Explanation);
        assert_eq!(feedback.reason, Some("Very helpful!".to_string()));

        // Record classification correction
        let id = db
            .record_classification_correction(100, 5, 10, None)
            .unwrap();
        let feedback = db.get_feedback(id).unwrap();
        assert_eq!(feedback.feedback_type, FeedbackType::Correction);
        assert_eq!(feedback.target_type, FeedbackTargetType::Classification);
        assert_eq!(feedback.original_value, Some("5".to_string()));
        assert_eq!(feedback.corrected_value, Some("10".to_string()));
    }

    #[test]
    fn test_list_feedback_for_target() {
        let db = Database::in_memory().unwrap();

        // Create feedback for specific target
        db.create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::Helpful,
            target_type: FeedbackTargetType::Alert,
            target_id: Some(42),
            original_value: None,
            corrected_value: None,
            reason: None,
            context: None,
        })
        .unwrap();

        db.create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::NotHelpful,
            target_type: FeedbackTargetType::Alert,
            target_id: Some(42),
            original_value: None,
            corrected_value: None,
            reason: Some("Changed my mind".to_string()),
            context: None,
        })
        .unwrap();

        // Different target
        db.create_feedback(&NewUserFeedback {
            feedback_type: FeedbackType::Helpful,
            target_type: FeedbackTargetType::Alert,
            target_id: Some(99),
            original_value: None,
            corrected_value: None,
            reason: None,
            context: None,
        })
        .unwrap();

        let feedback = db
            .list_feedback_for_target(FeedbackTargetType::Alert, 42)
            .unwrap();
        assert_eq!(feedback.len(), 2);
        assert!(feedback.iter().all(|f| f.target_id == Some(42)));
    }
}

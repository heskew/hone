//! Ollama metrics database operations

use chrono::NaiveDate;
use rusqlite::params;

use super::{parse_datetime, Database};
use crate::error::Result;
use crate::models::{
    AccuracyStats, ModelComparisonStats, ModelStats, NewOllamaMetric, OllamaHealthStatus,
    OllamaMetric, OllamaOperation, OllamaStats, OperationStats,
};

impl Database {
    /// Record an Ollama call metric
    pub fn record_ollama_metric(&self, metric: &NewOllamaMetric) -> Result<i64> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT INTO ollama_metrics (
                operation, model, latency_ms, success, error_message,
                confidence, transaction_id, input_text, result_text, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                metric.operation.as_str(),
                metric.model,
                metric.latency_ms,
                metric.success,
                metric.error_message,
                metric.confidence,
                metric.transaction_id,
                metric.input_text,
                metric.result_text,
                metric.metadata,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Record a user correction of an Ollama tag assignment
    pub fn record_ollama_correction(
        &self,
        transaction_id: i64,
        original_tag_id: i64,
        original_confidence: Option<f64>,
        corrected_tag_id: i64,
    ) -> Result<i64> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT INTO ollama_corrections (
                transaction_id, original_tag_id, original_confidence, corrected_tag_id
            ) VALUES (?, ?, ?, ?)
            ON CONFLICT(transaction_id, original_tag_id)
            DO UPDATE SET corrected_tag_id = excluded.corrected_tag_id,
                          corrected_at = CURRENT_TIMESTAMP
            "#,
            params![
                transaction_id,
                original_tag_id,
                original_confidence,
                corrected_tag_id,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get aggregated Ollama statistics for a time period
    pub fn get_ollama_stats(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<OllamaStats> {
        let conn = self.conn()?;

        // Overall stats
        let (total, successful, failed, avg_latency): (i64, i64, i64, f64) = conn
            .query_row(
                r#"
                SELECT
                    COUNT(*) as total,
                    SUM(CASE WHEN success THEN 1 ELSE 0 END) as successful,
                    SUM(CASE WHEN NOT success THEN 1 ELSE 0 END) as failed,
                    COALESCE(AVG(latency_ms), 0) as avg_latency
                FROM ollama_metrics
                WHERE DATE(started_at) BETWEEN ? AND ?
                "#,
                params![from_date.to_string(), to_date.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap_or((0, 0, 0, 0.0));

        // Latency percentiles
        let latencies: Vec<i64> = {
            let mut stmt = conn.prepare(
                r#"
                SELECT latency_ms FROM ollama_metrics
                WHERE DATE(started_at) BETWEEN ? AND ? AND success = 1
                ORDER BY latency_ms
                "#,
            )?;
            let result = stmt
                .query_map(params![from_date.to_string(), to_date.to_string()], |row| {
                    row.get(0)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        };

        let p50 = percentile(&latencies, 50);
        let p95 = percentile(&latencies, 95);
        let max = latencies.last().copied().unwrap_or(0);

        // By operation breakdown
        let by_operation: Vec<OperationStats> = {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    operation,
                    COUNT(*) as call_count,
                    AVG(CASE WHEN success THEN 1.0 ELSE 0.0 END) as success_rate,
                    AVG(latency_ms) as avg_latency,
                    AVG(confidence) as avg_confidence
                FROM ollama_metrics
                WHERE DATE(started_at) BETWEEN ? AND ?
                GROUP BY operation
                "#,
            )?;
            let result = stmt
                .query_map(params![from_date.to_string(), to_date.to_string()], |row| {
                    Ok(OperationStats {
                        operation: row.get(0)?,
                        call_count: row.get(1)?,
                        success_rate: row.get(2)?,
                        avg_latency_ms: row.get(3)?,
                        avg_confidence: row.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        };

        // Accuracy from corrections
        let accuracy = self.get_accuracy_stats(from_date, to_date)?;

        Ok(OllamaStats {
            period_start: from_date.to_string(),
            period_end: to_date.to_string(),
            total_calls: total,
            successful_calls: successful,
            failed_calls: failed,
            success_rate: if total > 0 {
                successful as f64 / total as f64
            } else {
                0.0
            },
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            max_latency_ms: max,
            by_operation,
            accuracy,
        })
    }

    /// Get accuracy statistics from corrections
    fn get_accuracy_stats(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<AccuracyStats> {
        let conn = self.conn()?;

        // Count corrections (where user changed the tag)
        let total_corrections: i64 = conn
            .query_row(
                r#"
                SELECT COUNT(*) FROM ollama_corrections
                WHERE DATE(corrected_at) BETWEEN ? AND ?
                  AND original_tag_id != corrected_tag_id
                "#,
                params![from_date.to_string(), to_date.to_string()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Count total Ollama tags in the period
        let total_ollama_tags: i64 = conn
            .query_row(
                r#"
                SELECT COUNT(*) FROM transaction_tags
                WHERE source = 'ollama'
                  AND DATE(created_at) BETWEEN ? AND ?
                "#,
                params![from_date.to_string(), to_date.to_string()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let correction_rate = if total_ollama_tags > 0 {
            total_corrections as f64 / total_ollama_tags as f64
        } else {
            0.0
        };

        Ok(AccuracyStats {
            total_corrections,
            total_ollama_tags,
            correction_rate,
            estimated_accuracy: 1.0 - correction_rate,
        })
    }

    /// Get recent Ollama calls for debugging
    pub fn get_recent_ollama_calls(&self, limit: i64) -> Result<Vec<OllamaMetric>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, operation, model, started_at, latency_ms,
                   success, error_message, confidence, transaction_id,
                   input_text, result_text, metadata
            FROM ollama_metrics
            ORDER BY started_at DESC
            LIMIT ?
            "#,
        )?;

        let metrics = stmt
            .query_map(params![limit], |row| {
                let op_str: String = row.get(1)?;
                let started_at_str: String = row.get(3)?;
                Ok(OllamaMetric {
                    id: row.get(0)?,
                    operation: op_str.parse().unwrap_or(OllamaOperation::ClassifyMerchant),
                    model: row.get(2)?,
                    started_at: parse_datetime(&started_at_str),
                    latency_ms: row.get(4)?,
                    success: row.get(5)?,
                    error_message: row.get(6)?,
                    confidence: row.get(7)?,
                    transaction_id: row.get(8)?,
                    input_text: row.get(9)?,
                    result_text: row.get(10)?,
                    metadata: row.get(11)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(metrics)
    }

    /// Get Ollama health status from metrics
    pub fn get_ollama_health(&self) -> Result<OllamaHealthStatus> {
        let conn = self.conn()?;

        // Last successful call
        let last_success: Option<String> = conn
            .query_row(
                "SELECT started_at FROM ollama_metrics WHERE success = 1 ORDER BY started_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        // Last failed call
        let last_failure: Option<String> = conn
            .query_row(
                "SELECT started_at FROM ollama_metrics WHERE success = 0 ORDER BY started_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        // Recent error rate (last 100 calls)
        let (total, failures): (i64, i64) = conn
            .query_row(
                r#"
                SELECT COUNT(*), COALESCE(SUM(CASE WHEN NOT success THEN 1 ELSE 0 END), 0)
                FROM (SELECT success FROM ollama_metrics ORDER BY started_at DESC LIMIT 100)
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));

        let error_rate = if total > 0 {
            failures as f64 / total as f64
        } else {
            0.0
        };

        let host = std::env::var("OLLAMA_HOST").ok();
        let model = std::env::var("OLLAMA_MODEL").ok();

        // Orchestrator (agentic mode) configuration
        let orchestrator_host = std::env::var("ANTHROPIC_COMPATIBLE_HOST").ok();
        let orchestrator_model = std::env::var("ANTHROPIC_COMPATIBLE_MODEL").ok();

        Ok(OllamaHealthStatus {
            available: false, // Will be updated by API layer with live check
            host,
            model,
            last_successful_call: last_success.map(|s| parse_datetime(&s)),
            last_failed_call: last_failure.map(|s| parse_datetime(&s)),
            recent_error_rate: error_rate,
            orchestrator_available: orchestrator_host.is_some(),
            orchestrator_host,
            orchestrator_model,
        })
    }

    /// Get latency trend for model analysis
    pub fn get_latency_trend(&self, days: i64) -> Result<Vec<(String, f64)>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT DATE(started_at) as day, AVG(latency_ms) as avg_latency
            FROM ollama_metrics
            WHERE started_at >= datetime('now', ? || ' days')
              AND success = 1
            GROUP BY DATE(started_at)
            ORDER BY day
            "#,
        )?;

        let trend = stmt
            .query_map(params![format!("-{}", days)], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(trend)
    }

    /// Get list of all models that have been used
    pub fn get_ollama_models(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT model FROM ollama_metrics
            WHERE model IS NOT NULL AND model != ''
            ORDER BY model
            "#,
        )?;

        let models = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(models)
    }

    /// Get aggregated Ollama statistics grouped by model for comparison
    pub fn get_ollama_stats_by_model(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<ModelComparisonStats> {
        let conn = self.conn()?;

        // Get all models used in the period
        let models: Vec<String> = {
            let mut stmt = conn.prepare(
                r#"
                SELECT DISTINCT model FROM ollama_metrics
                WHERE DATE(started_at) BETWEEN ? AND ?
                  AND model IS NOT NULL AND model != ''
                ORDER BY model
                "#,
            )?;
            let result = stmt
                .query_map(params![from_date.to_string(), to_date.to_string()], |row| {
                    row.get(0)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        };

        let mut model_stats = Vec::new();

        for model in models {
            // Overall stats for this model
            let (total, successful, failed, avg_latency, avg_conf): (
                i64,
                i64,
                i64,
                f64,
                Option<f64>,
            ) = conn
                .query_row(
                    r#"
                    SELECT
                        COUNT(*) as total,
                        SUM(CASE WHEN success THEN 1 ELSE 0 END) as successful,
                        SUM(CASE WHEN NOT success THEN 1 ELSE 0 END) as failed,
                        COALESCE(AVG(latency_ms), 0) as avg_latency,
                        AVG(confidence) as avg_confidence
                    FROM ollama_metrics
                    WHERE DATE(started_at) BETWEEN ? AND ?
                      AND model = ?
                    "#,
                    params![from_date.to_string(), to_date.to_string(), model],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .unwrap_or((0, 0, 0, 0.0, None));

            // Latency percentiles for this model
            let latencies: Vec<i64> = {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT latency_ms FROM ollama_metrics
                    WHERE DATE(started_at) BETWEEN ? AND ?
                      AND model = ?
                      AND success = 1
                    ORDER BY latency_ms
                    "#,
                )?;
                let result = stmt
                    .query_map(
                        params![from_date.to_string(), to_date.to_string(), model],
                        |row| row.get(0),
                    )?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            };

            let p50 = percentile(&latencies, 50);
            let p95 = percentile(&latencies, 95);
            let max = latencies.last().copied().unwrap_or(0);

            // By operation breakdown for this model
            let by_operation: Vec<OperationStats> = {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT
                        operation,
                        COUNT(*) as call_count,
                        AVG(CASE WHEN success THEN 1.0 ELSE 0.0 END) as success_rate,
                        AVG(latency_ms) as avg_latency,
                        AVG(confidence) as avg_confidence
                    FROM ollama_metrics
                    WHERE DATE(started_at) BETWEEN ? AND ?
                      AND model = ?
                    GROUP BY operation
                    "#,
                )?;
                let result = stmt
                    .query_map(
                        params![from_date.to_string(), to_date.to_string(), model],
                        |row| {
                            Ok(OperationStats {
                                operation: row.get(0)?,
                                call_count: row.get(1)?,
                                success_rate: row.get(2)?,
                                avg_latency_ms: row.get(3)?,
                                avg_confidence: row.get(4)?,
                            })
                        },
                    )?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            };

            // First and last usage
            let (first_used, last_used): (Option<String>, Option<String>) = conn
                .query_row(
                    r#"
                    SELECT MIN(started_at), MAX(started_at)
                    FROM ollama_metrics
                    WHERE model = ?
                    "#,
                    params![model],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((None, None));

            model_stats.push(ModelStats {
                model,
                total_calls: total,
                successful_calls: successful,
                failed_calls: failed,
                success_rate: if total > 0 {
                    successful as f64 / total as f64
                } else {
                    0.0
                },
                avg_latency_ms: avg_latency,
                p50_latency_ms: p50,
                p95_latency_ms: p95,
                max_latency_ms: max,
                avg_confidence: avg_conf,
                by_operation,
                first_used,
                last_used,
            });
        }

        Ok(ModelComparisonStats {
            period_start: from_date.to_string(),
            period_end: to_date.to_string(),
            models: model_stats,
        })
    }
}

/// Helper: compute percentile from sorted array
fn percentile(sorted: &[i64], p: usize) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (sorted.len() * p / 100).min(sorted.len() - 1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_retrieve_metrics() {
        let db = Database::in_memory().unwrap();

        let metric = NewOllamaMetric {
            operation: OllamaOperation::ClassifyMerchant,
            model: "llama3.2".to_string(),
            latency_ms: 1500,
            success: true,
            error_message: None,
            confidence: Some(0.85),
            transaction_id: None,
            input_text: Some("NETFLIX.COM".to_string()),
            result_text: Some("Netflix → streaming".to_string()),
            metadata: None,
        };

        let id = db.record_ollama_metric(&metric).unwrap();
        assert!(id > 0);

        let recent = db.get_recent_ollama_calls(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].model, "llama3.2");
        assert_eq!(recent[0].latency_ms, 1500);
        assert!(recent[0].success);
    }

    #[test]
    fn test_ollama_stats() {
        let db = Database::in_memory().unwrap();

        // Add some metrics
        for i in 0..5 {
            let metric = NewOllamaMetric {
                operation: OllamaOperation::ClassifyMerchant,
                model: "llama3.2".to_string(),
                latency_ms: 1000 + i * 100,
                success: i != 2, // One failure
                error_message: if i == 2 {
                    Some("timeout".to_string())
                } else {
                    None
                },
                confidence: Some(0.8),
                transaction_id: None,
                input_text: None,
                result_text: None,
                metadata: None,
            };
            db.record_ollama_metric(&metric).unwrap();
        }

        let today = chrono::Utc::now().date_naive();
        let stats = db.get_ollama_stats(today, today).unwrap();

        assert_eq!(stats.total_calls, 5);
        assert_eq!(stats.successful_calls, 4);
        assert_eq!(stats.failed_calls, 1);
        assert_eq!(stats.success_rate, 0.8);
    }

    #[test]
    fn test_percentile() {
        assert_eq!(percentile(&[], 50), 0);
        assert_eq!(percentile(&[100], 50), 100);
        assert_eq!(percentile(&[100, 200, 300], 50), 200);
        assert_eq!(percentile(&[100, 200, 300, 400, 500], 95), 500);
    }
}

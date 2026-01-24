//! Insight Engine - orchestrates insight generation and persistence

use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};

use crate::ai::AIClient;
use crate::db::Database;
use crate::Result;

use super::types::{Finding, InsightType};
use super::{ExpenseForecasterInsight, SavingsOpportunityInsight, SpendingExplainerInsight};

/// Context provided to insight analyzers
pub struct AnalysisContext<'a> {
    /// Database for querying transaction data
    pub db: &'a Database,
    /// Optional AI client for LLM-powered analysis
    pub ai: Option<&'a AIClient>,
    /// Date range for analysis (start, end)
    pub date_range: (NaiveDate, NaiveDate),
}

impl<'a> AnalysisContext<'a> {
    /// Create a new analysis context
    pub fn new(
        db: &'a Database,
        ai: Option<&'a AIClient>,
        date_range: (NaiveDate, NaiveDate),
    ) -> Self {
        Self { db, ai, date_range }
    }

    /// Create context for "current month" analysis
    pub fn current_month(db: &'a Database, ai: Option<&'a AIClient>) -> Self {
        let today = chrono::Local::now().date_naive();
        let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
        let end = today;
        Self::new(db, ai, (start, end))
    }

    /// Deprecated: use with ai parameter instead
    #[deprecated(
        since = "0.1.0",
        note = "Use new() or current_month() with ai parameter"
    )]
    pub fn with_ollama(
        db: &'a Database,
        ai: Option<&'a AIClient>,
        date_range: (NaiveDate, NaiveDate),
    ) -> Self {
        Self::new(db, ai, date_range)
    }
}

/// Trait for insight analyzers (async for AI-powered insights)
#[async_trait]
pub trait Insight: Send + Sync {
    /// Unique identifier for this insight type
    fn id(&self) -> InsightType;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Analyze data and produce findings
    async fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>>;
}

/// The main insight engine that orchestrates analysis
pub struct InsightEngine {
    insights: Vec<Box<dyn Insight>>,
}

impl Default for InsightEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InsightEngine {
    /// Create a new insight engine with built-in insight types
    pub fn new() -> Self {
        let mut engine = Self { insights: vec![] };

        // Register built-in insights
        engine.register(Box::new(SavingsOpportunityInsight::new()));
        engine.register(Box::new(ExpenseForecasterInsight::new()));
        engine.register(Box::new(SpendingExplainerInsight::new()));

        engine
    }

    /// Register an insight analyzer
    pub fn register(&mut self, insight: Box<dyn Insight>) {
        self.insights.push(insight);
    }

    /// Run all insight analyzers and collect findings
    pub async fn analyze_all(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let mut all_findings = vec![];

        for insight in &self.insights {
            match insight.analyze(ctx).await {
                Ok(findings) => {
                    tracing::debug!(
                        insight = insight.id().as_str(),
                        count = findings.len(),
                        "Insight analysis complete"
                    );
                    all_findings.extend(findings);
                }
                Err(e) => {
                    tracing::warn!(
                        insight = insight.id().as_str(),
                        error = %e,
                        "Insight analysis failed"
                    );
                }
            }
        }

        // Sort by severity (highest first), then by detection time (most recent first)
        all_findings.sort_by(|a, b| {
            b.severity
                .priority()
                .cmp(&a.severity.priority())
                .then_with(|| b.detected_at.cmp(&a.detected_at))
        });

        Ok(all_findings)
    }

    /// Run all analyzers and persist findings to the database
    ///
    /// Returns the number of findings that were new or updated
    pub async fn run_and_persist(&self, ctx: &AnalysisContext<'_>) -> Result<usize> {
        let findings = self.analyze_all(ctx).await?;
        let mut count = 0;

        for finding in findings {
            match ctx.db.upsert_insight_finding(&finding) {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::warn!(
                        key = finding.key,
                        error = %e,
                        "Failed to persist insight finding"
                    );
                }
            }
        }

        tracing::info!(persisted = count, "Insight analysis complete");
        Ok(count)
    }

    /// Get list of registered insight types
    pub fn insight_types(&self) -> Vec<InsightType> {
        self.insights.iter().map(|i| i.id()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::Bank;

    #[test]
    fn test_engine_creation() {
        let engine = InsightEngine::new();
        let types = engine.insight_types();

        assert!(types.contains(&InsightType::SavingsOpportunity));
        assert!(types.contains(&InsightType::ExpenseForecaster));
        assert!(types.contains(&InsightType::SpendingExplainer));
    }

    #[tokio::test]
    async fn test_analyze_empty_db() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let engine = InsightEngine::new();
        let ctx = AnalysisContext::current_month(&db, None);

        let findings = engine.analyze_all(&ctx).await.unwrap();
        // Empty database should produce no findings (or minimal)
        // The exact count depends on insight implementation
        assert!(findings.len() < 10);
    }

    #[tokio::test]
    async fn test_run_and_persist() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Add some test data
        let account_id = db.upsert_account("Test", Bank::Chase, None).unwrap();
        let tx = crate::models::NewTransaction {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "NETFLIX".to_string(),
            amount: -15.99,
            category: None,
            import_hash: "insight_test_1".to_string(),
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
        };
        db.insert_transaction(account_id, &tx).unwrap();

        let engine = InsightEngine::new();
        let ctx = AnalysisContext::current_month(&db, None);

        // Should not panic
        let count = engine.run_and_persist(&ctx).await.unwrap();
        // Test that the engine runs successfully (count is usize, always >= 0)
        let _ = count;
    }
}

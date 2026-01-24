//! Context Assembler
//!
//! Given a question or task, retrieves everything the LLM needs to generate
//! high-quality responses. This module assembles relevant data including:
//! - Transactions for the time period
//! - User's rules, patterns, preferences
//! - Historical baselines and comparisons
//! - Previous feedback on similar items

use chrono::{Duration, NaiveDate, Utc};
use std::collections::HashMap;

use crate::db::Database;
use crate::error::Result;
use crate::models::{FeedbackTargetType, Subscription, Tag, TagRuleWithTag, Transaction};

/// Assembled context for LLM prompts
#[derive(Debug)]
pub struct Context {
    /// Transactions relevant to the query
    pub transactions: Vec<Transaction>,
    /// Active subscriptions
    pub subscriptions: Vec<Subscription>,
    /// User-defined tag rules
    pub tag_rules: Vec<TagRuleWithTag>,
    /// Relevant tags
    pub tags: Vec<Tag>,
    /// Baseline statistics for comparison
    pub baseline: Option<BaselineStats>,
    /// User feedback summary for this context type
    pub feedback_summary: Option<String>,
    /// Additional metadata for prompt rendering
    pub metadata: HashMap<String, String>,
}

impl Context {
    /// Create an empty context
    pub fn new() -> Self {
        Self {
            transactions: Vec::new(),
            subscriptions: Vec::new(),
            tag_rules: Vec::new(),
            tags: Vec::new(),
            baseline: None,
            feedback_summary: None,
            metadata: HashMap::new(),
        }
    }

    /// Convert context to template variables for prompt rendering
    ///
    /// Returns a map of variable names to their string values,
    /// ready to be passed to PromptTemplate::render()
    pub fn to_template_vars(&self) -> HashMap<&'static str, String> {
        let mut vars = HashMap::new();

        // Add feedback if present
        if let Some(ref feedback) = self.feedback_summary {
            vars.insert("feedback", feedback.clone());
        }

        // Add transaction count
        vars.insert("transaction_count", self.transactions.len().to_string());

        // Add subscription count
        vars.insert("subscription_count", self.subscriptions.len().to_string());

        // Add tag rules summary
        if !self.tag_rules.is_empty() {
            let rules_summary = self
                .tag_rules
                .iter()
                .take(10) // Limit to avoid token overflow
                .map(|r| format!("{}: {}", r.rule.pattern, r.tag_name))
                .collect::<Vec<_>>()
                .join("\n");
            vars.insert("user_rules", rules_summary);
        }

        // Add baseline if present
        if let Some(ref baseline) = self.baseline {
            vars.insert("baseline_total", format!("{:.2}", baseline.total_spending));
            vars.insert("baseline_months", baseline.months.to_string());
            vars.insert(
                "baseline_monthly_avg",
                format!("{:.2}", baseline.total_spending / baseline.months as f64),
            );

            // Add category breakdown
            if !baseline.by_category.is_empty() {
                let categories = baseline
                    .by_category
                    .iter()
                    .take(10)
                    .map(|(cat, amount)| format!("{}: ${:.2}", cat, amount))
                    .collect::<Vec<_>>()
                    .join(", ");
                vars.insert("baseline_categories", categories);
            }
        }

        // Add metadata
        for (key, value) in &self.metadata {
            // Convert String keys to &'static str by leaking (acceptable for small numbers)
            // In practice, use predefined keys
            match key.as_str() {
                "period_start" => vars.insert("period_start", value.clone()),
                "period_end" => vars.insert("period_end", value.clone()),
                "category" => vars.insert("category", value.clone()),
                "merchant" => vars.insert("merchant", value.clone()),
                "query" => vars.insert("query", value.clone()),
                _ => None,
            };
        }

        vars
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Baseline statistics for comparison
#[derive(Debug)]
pub struct BaselineStats {
    /// Total spending in baseline period
    pub total_spending: f64,
    /// Number of months in baseline
    pub months: i32,
    /// Spending broken down by category
    pub by_category: HashMap<String, f64>,
    /// Transaction count in baseline
    pub transaction_count: i64,
}

/// Context type determines what data to assemble
#[derive(Debug, Clone, Copy)]
pub enum ContextType {
    /// Explaining spending changes
    SpendingExplanation,
    /// Analyzing duplicate subscriptions
    DuplicateAnalysis,
    /// Classifying a merchant
    MerchantClassification,
    /// General query about finances
    GeneralQuery,
    /// Receipt matching
    ReceiptMatch,
}

impl ContextType {
    /// Get the corresponding feedback target type
    pub fn feedback_target(&self) -> FeedbackTargetType {
        match self {
            ContextType::SpendingExplanation => FeedbackTargetType::Explanation,
            ContextType::DuplicateAnalysis => FeedbackTargetType::Insight,
            ContextType::MerchantClassification => FeedbackTargetType::Classification,
            ContextType::GeneralQuery => FeedbackTargetType::Insight,
            ContextType::ReceiptMatch => FeedbackTargetType::ReceiptMatch,
        }
    }
}

/// Assembles context for LLM prompts
pub struct ContextAssembler<'a> {
    db: &'a Database,
}

impl<'a> ContextAssembler<'a> {
    /// Create a new context assembler
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Assemble context for spending explanation
    ///
    /// Retrieves:
    /// - Transactions for the specified period
    /// - 3-month baseline for comparison
    /// - Active subscriptions
    /// - User tag rules
    /// - Previous feedback on spending explanations
    pub fn for_spending_explanation(
        &self,
        period_start: NaiveDate,
        period_end: NaiveDate,
        category: Option<&str>,
    ) -> Result<Context> {
        let mut ctx = Context::new();

        // Get transactions for the period
        ctx.transactions = self.get_transactions_in_range(period_start, period_end, category)?;

        // Calculate 3-month baseline
        let baseline_end = period_start - Duration::days(1);
        let baseline_start = baseline_end - Duration::days(90);
        ctx.baseline = Some(self.calculate_baseline(baseline_start, baseline_end, category)?);

        // Get active subscriptions
        ctx.subscriptions = self.db.list_subscriptions(None)?;

        // Get user tag rules
        ctx.tag_rules = self.db.list_tag_rules()?;

        // Get feedback for spending explanations
        ctx.feedback_summary = self
            .db
            .get_feedback_summary_for_prompt(FeedbackTargetType::Explanation)
            .ok()
            .filter(|f| !f.is_empty());

        // Add metadata
        ctx.metadata
            .insert("period_start".to_string(), period_start.to_string());
        ctx.metadata
            .insert("period_end".to_string(), period_end.to_string());
        if let Some(cat) = category {
            ctx.metadata.insert("category".to_string(), cat.to_string());
        }

        Ok(ctx)
    }

    /// Assemble context for duplicate subscription analysis
    ///
    /// Retrieves:
    /// - Active subscriptions in the specified category
    /// - Recent transactions for those subscriptions
    /// - Previous feedback on insights
    pub fn for_duplicate_analysis(&self, category: &str) -> Result<Context> {
        let mut ctx = Context::new();

        // Get subscriptions (all, then filter would happen at caller)
        ctx.subscriptions = self.db.list_subscriptions(None)?;

        // Get feedback for insights
        ctx.feedback_summary = self
            .db
            .get_feedback_summary_for_prompt(FeedbackTargetType::Insight)
            .ok()
            .filter(|f| !f.is_empty());

        // Add metadata
        ctx.metadata
            .insert("category".to_string(), category.to_string());

        Ok(ctx)
    }

    /// Assemble context for merchant classification
    ///
    /// Retrieves:
    /// - User tag rules that might match
    /// - Tags hierarchy
    /// - Previous feedback on classifications
    pub fn for_merchant_classification(&self, merchant: &str) -> Result<Context> {
        let mut ctx = Context::new();

        // Get user tag rules
        ctx.tag_rules = self.db.list_tag_rules()?;

        // Get all tags for context
        ctx.tags = self.db.list_tags()?;

        // Get feedback for classifications
        ctx.feedback_summary = self
            .db
            .get_feedback_summary_for_prompt(FeedbackTargetType::Classification)
            .ok()
            .filter(|f| !f.is_empty());

        // Add metadata
        ctx.metadata
            .insert("merchant".to_string(), merchant.to_string());

        Ok(ctx)
    }

    /// Assemble context for a general query
    ///
    /// Retrieves:
    /// - Recent transactions (last 30 days)
    /// - Active subscriptions
    /// - User tag rules
    /// - Previous feedback
    pub fn for_general_query(&self, query: &str) -> Result<Context> {
        let mut ctx = Context::new();

        // Get recent transactions (last 30 days)
        let today = Utc::now().date_naive();
        let month_ago = today - Duration::days(30);
        ctx.transactions = self.get_transactions_in_range(month_ago, today, None)?;

        // Get active subscriptions
        ctx.subscriptions = self.db.list_subscriptions(None)?;

        // Get user tag rules
        ctx.tag_rules = self.db.list_tag_rules()?;

        // Get feedback for insights
        ctx.feedback_summary = self
            .db
            .get_feedback_summary_for_prompt(FeedbackTargetType::Insight)
            .ok()
            .filter(|f| !f.is_empty());

        // Add query to metadata
        ctx.metadata.insert("query".to_string(), query.to_string());

        Ok(ctx)
    }

    /// Assemble context for receipt matching
    ///
    /// Retrieves:
    /// - Recent transactions that might match
    /// - Previous feedback on receipt matches
    pub fn for_receipt_match(
        &self,
        receipt_date: NaiveDate,
        receipt_amount: f64,
    ) -> Result<Context> {
        let mut ctx = Context::new();

        // Get transactions around the receipt date (+/- 7 days)
        let date_start = receipt_date - Duration::days(7);
        let date_end = receipt_date + Duration::days(7);
        ctx.transactions = self.get_transactions_in_range(date_start, date_end, None)?;

        // Filter to transactions close to the receipt amount (+/- 20%)
        let amount_threshold = receipt_amount.abs() * 0.2;
        ctx.transactions
            .retain(|tx| (tx.amount.abs() - receipt_amount.abs()).abs() <= amount_threshold);

        // Get feedback for receipt matches
        ctx.feedback_summary = self
            .db
            .get_feedback_summary_for_prompt(FeedbackTargetType::ReceiptMatch)
            .ok()
            .filter(|f| !f.is_empty());

        // Add metadata
        ctx.metadata
            .insert("receipt_date".to_string(), receipt_date.to_string());
        ctx.metadata.insert(
            "receipt_amount".to_string(),
            format!("{:.2}", receipt_amount),
        );

        Ok(ctx)
    }

    /// Get transactions in a date range, optionally filtered by category
    fn get_transactions_in_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        category: Option<&str>,
    ) -> Result<Vec<Transaction>> {
        // Get tag IDs if category specified
        let tag_ids: Option<Vec<i64>> = if let Some(cat) = category {
            self.db.get_tag_by_path(cat)?.map(|tag| vec![tag.id])
        } else {
            None
        };

        // Use existing search method with date range and optional tags
        let transactions = self.db.search_transactions_with_tags_and_dates(
            None,               // account_id
            None,               // search
            tag_ids.as_deref(), // tag_ids
            Some((start, end)), // date_range
            10000,              // limit
            0,                  // offset
        )?;

        Ok(transactions)
    }

    /// Calculate baseline statistics for a period
    fn calculate_baseline(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        category: Option<&str>,
    ) -> Result<BaselineStats> {
        let transactions = self.get_transactions_in_range(start, end, category)?;

        let total_spending: f64 = transactions
            .iter()
            .filter(|tx| tx.amount < 0.0)
            .map(|tx| tx.amount.abs())
            .sum();

        let transaction_count = transactions.len() as i64;

        // Calculate months in period
        let days = (end - start).num_days();
        let months = (days as f64 / 30.0).ceil() as i32;

        // Build category breakdown
        // Note: This is a simplified version - for full category breakdown,
        // use the reports API which handles tag hierarchy properly
        let mut by_category: HashMap<String, f64> = HashMap::new();
        for tx in &transactions {
            if tx.amount >= 0.0 {
                continue; // Skip income
            }
            if let Ok(tags) = self.db.get_transaction_tags(tx.id) {
                for tag in tags {
                    // Look up tag name
                    if let Ok(Some(tag_info)) = self.db.get_tag(tag.tag_id) {
                        *by_category.entry(tag_info.name.clone()).or_insert(0.0) += tx.amount.abs();
                    }
                }
            }
        }

        Ok(BaselineStats {
            total_spending,
            months: months.max(1),
            by_category,
            transaction_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use chrono::Datelike;

    #[test]
    fn test_context_new() {
        let ctx = Context::new();
        assert!(ctx.transactions.is_empty());
        assert!(ctx.subscriptions.is_empty());
        assert!(ctx.tag_rules.is_empty());
        assert!(ctx.baseline.is_none());
        assert!(ctx.feedback_summary.is_none());
    }

    #[test]
    fn test_context_to_template_vars() {
        let mut ctx = Context::new();
        ctx.feedback_summary = Some("User prefers concise explanations".to_string());
        ctx.baseline = Some(BaselineStats {
            total_spending: 1500.0,
            months: 3,
            by_category: HashMap::new(),
            transaction_count: 50,
        });

        let vars = ctx.to_template_vars();

        assert_eq!(
            vars.get("feedback"),
            Some(&"User prefers concise explanations".to_string())
        );
        assert_eq!(vars.get("baseline_total"), Some(&"1500.00".to_string()));
        assert_eq!(vars.get("baseline_months"), Some(&"3".to_string()));
        assert_eq!(
            vars.get("baseline_monthly_avg"),
            Some(&"500.00".to_string())
        );
    }

    #[test]
    fn test_context_type_feedback_target() {
        assert!(matches!(
            ContextType::SpendingExplanation.feedback_target(),
            FeedbackTargetType::Explanation
        ));
        assert!(matches!(
            ContextType::DuplicateAnalysis.feedback_target(),
            FeedbackTargetType::Insight
        ));
        assert!(matches!(
            ContextType::MerchantClassification.feedback_target(),
            FeedbackTargetType::Classification
        ));
    }

    #[test]
    fn test_assembler_for_merchant_classification() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let assembler = ContextAssembler::new(&db);
        let ctx = assembler.for_merchant_classification("NETFLIX").unwrap();

        assert!(ctx.tags.len() > 0); // Should have seeded tags
        assert!(ctx.tag_rules.is_empty()); // No rules created yet
        assert_eq!(ctx.metadata.get("merchant"), Some(&"NETFLIX".to_string()));
    }

    #[test]
    fn test_assembler_for_duplicate_analysis() {
        let db = Database::in_memory().unwrap();

        let assembler = ContextAssembler::new(&db);
        let ctx = assembler.for_duplicate_analysis("Streaming").unwrap();

        assert!(ctx.subscriptions.is_empty()); // No subscriptions created
        assert_eq!(ctx.metadata.get("category"), Some(&"Streaming".to_string()));
    }

    #[test]
    fn test_assembler_for_general_query() {
        let db = Database::in_memory().unwrap();

        let assembler = ContextAssembler::new(&db);
        let ctx = assembler
            .for_general_query("Why did I spend so much on dining?")
            .unwrap();

        assert!(ctx.transactions.is_empty()); // No transactions
        assert_eq!(
            ctx.metadata.get("query"),
            Some(&"Why did I spend so much on dining?".to_string())
        );
    }

    #[test]
    fn test_assembler_for_spending_explanation() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        let assembler = ContextAssembler::new(&db);
        let today = Utc::now().date_naive();
        let month_start = today.with_day(1).unwrap();

        let ctx = assembler
            .for_spending_explanation(month_start, today, None)
            .unwrap();

        assert!(ctx.baseline.is_some());
        let baseline = ctx.baseline.unwrap();
        assert_eq!(baseline.months, 3); // 90 days = 3 months
    }
}

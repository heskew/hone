//! Spending Explainer Insight
//!
//! Explains spending changes compared to historical baseline.
//! Unlike alerts (which trigger at 30%+ change), this insight proactively
//! surfaces ANY notable spending changes, even smaller ones.
//!
//! When an AI backend is available, generates AI-powered narrative explanations
//! of why spending changed (e.g., "shift from groceries to delivery apps").

use async_trait::async_trait;
use chrono::{Datelike, Duration, Utc};

use crate::ai::AIBackend;
use crate::error::Result;

use super::engine::{AnalysisContext, Insight};
use super::types::{Finding, InsightType, MerchantContribution, Severity, SpendingExplainerData};

/// Insight that explains spending changes vs baseline
pub struct SpendingExplainerInsight {
    /// Minimum percent change to report (default 15%)
    min_percent_change: f64,
    /// Minimum absolute dollar change to report (default $25)
    min_absolute_change: f64,
    /// Minimum baseline to consider (default $50/month)
    min_baseline: f64,
    /// Maximum number of categories to report (default 5)
    max_categories: usize,
}

impl SpendingExplainerInsight {
    pub fn new() -> Self {
        Self {
            min_percent_change: 15.0,
            min_absolute_change: 25.0,
            min_baseline: 50.0,
            max_categories: 5,
        }
    }

    pub fn with_thresholds(
        min_percent: f64,
        min_absolute: f64,
        min_baseline: f64,
        max_categories: usize,
    ) -> Self {
        Self {
            min_percent_change: min_percent,
            min_absolute_change: min_absolute,
            min_baseline,
            max_categories,
        }
    }
}

impl Default for SpendingExplainerInsight {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Insight for SpendingExplainerInsight {
    fn id(&self) -> InsightType {
        InsightType::SpendingExplainer
    }

    fn name(&self) -> &'static str {
        "Spending Explainer"
    }

    async fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let mut findings = Vec::new();

        let today = Utc::now().date_naive();

        // Current month period
        let current_month_start = today.with_day(1).expect("Day 1 always valid");

        // 3-month baseline period (the 3 months before current month)
        let baseline_end = current_month_start - Duration::days(1);
        let baseline_start = baseline_end - Duration::days(90);

        // Get spending by category for current month
        let current =
            ctx.db
                .get_spending_summary(current_month_start, today, None, false, None, None)?;

        // Get spending by category for baseline period
        let baseline =
            ctx.db
                .get_spending_summary(baseline_start, baseline_end, None, false, None, None)?;

        // Collect categories with notable changes
        let mut changes: Vec<(String, i64, f64, f64, f64)> = Vec::new(); // (tag, tag_id, current, baseline, percent_change)

        for current_cat in &current.categories {
            // Find matching baseline category
            let baseline_cat = baseline
                .categories
                .iter()
                .find(|c| c.tag_id == current_cat.tag_id);

            let Some(baseline_cat) = baseline_cat else {
                // New category this month - flag if significant
                let current_amount = current_cat.amount.abs();
                if current_amount >= self.min_absolute_change {
                    changes.push((
                        current_cat.tag.clone(),
                        current_cat.tag_id,
                        current_amount,
                        0.0,
                        100.0, // 100% increase from nothing
                    ));
                }
                continue;
            };

            // Calculate monthly baseline average
            let baseline_monthly_avg = baseline_cat.amount.abs() / 3.0;

            // Skip if baseline too small
            if baseline_monthly_avg < self.min_baseline {
                continue;
            }

            let current_amount = current_cat.amount.abs();
            let absolute_change = (current_amount - baseline_monthly_avg).abs();
            let percent_change = if baseline_monthly_avg > 0.0 {
                ((current_amount - baseline_monthly_avg) / baseline_monthly_avg) * 100.0
            } else {
                continue;
            };

            // Check if change is significant enough
            if percent_change.abs() >= self.min_percent_change
                && absolute_change >= self.min_absolute_change
            {
                changes.push((
                    current_cat.tag.clone(),
                    current_cat.tag_id,
                    current_amount,
                    baseline_monthly_avg,
                    percent_change,
                ));
            }
        }

        // Sort by absolute percent change (largest first)
        changes.sort_by(|a, b| {
            b.4.abs()
                .partial_cmp(&a.4.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top N changes
        changes.truncate(self.max_categories);

        // Create findings for each significant change
        for (tag_name, tag_id, current_amount, baseline_amount, percent_change) in changes {
            let is_increase = percent_change > 0.0;

            // Get top merchants for this category to explain the change
            let merchants_report = ctx.db.get_top_merchants(
                current_month_start,
                today,
                5,
                Some(&tag_name),
                None,
                None,
            )?;

            // Get baseline merchants for comparison
            let baseline_merchants = ctx.db.get_top_merchants(
                baseline_start,
                baseline_end,
                10,
                Some(&tag_name),
                None,
                None,
            )?;

            // Calculate merchant contributions
            let top_merchants: Vec<MerchantContribution> = merchants_report
                .merchants
                .iter()
                .map(|m| {
                    let baseline_merchant_amount = baseline_merchants
                        .merchants
                        .iter()
                        .find(|bm| bm.merchant.to_lowercase() == m.merchant.to_lowercase())
                        .map(|bm| bm.amount.abs() / 3.0) // Monthly average
                        .unwrap_or(0.0);

                    MerchantContribution {
                        merchant: m.merchant.clone(),
                        current: m.amount.abs(),
                        baseline: baseline_merchant_amount,
                        change: m.amount.abs() - baseline_merchant_amount,
                    }
                })
                .collect();

            // Identify new merchants (in current but not in baseline)
            let new_merchants: Vec<String> = merchants_report
                .merchants
                .iter()
                .filter(|m| {
                    !baseline_merchants
                        .merchants
                        .iter()
                        .any(|bm| bm.merchant.to_lowercase() == m.merchant.to_lowercase())
                })
                .map(|m| m.merchant.clone())
                .collect();

            // Try to get AI-generated explanation if AI backend is available
            let explanation = if let Some(ai) = ctx.ai {
                // Prepare merchant data for AI
                let ai_merchants: Vec<(String, f64, i32)> = merchants_report
                    .merchants
                    .iter()
                    .map(|m| {
                        (
                            m.merchant.clone(),
                            m.amount.abs(),
                            m.transaction_count as i32,
                        )
                    })
                    .collect();

                // Get baseline transaction count (approximate)
                let baseline_tx_count = baseline_merchants
                    .merchants
                    .iter()
                    .map(|m| m.transaction_count)
                    .sum::<i64>() as i32
                    / 3; // Monthly average

                let current_tx_count = merchants_report
                    .merchants
                    .iter()
                    .map(|m| m.transaction_count)
                    .sum::<i64>() as i32;

                match ai
                    .explain_spending_change(
                        &tag_name,
                        baseline_amount,
                        current_amount,
                        baseline_tx_count,
                        current_tx_count,
                        &ai_merchants,
                        &new_merchants,
                        None, // TODO: Could inject feedback context here
                    )
                    .await
                {
                    Ok(explanation) => {
                        // Combine summary and reasons into a narrative
                        let mut narrative = explanation.summary;
                        if !explanation.reasons.is_empty() {
                            narrative.push_str(" ");
                            narrative.push_str(&explanation.reasons.join(" "));
                        }
                        Some(narrative)
                    }
                    Err(e) => {
                        tracing::warn!(
                            category = tag_name,
                            error = %e,
                            "Failed to get AI explanation for spending change"
                        );
                        None
                    }
                }
            } else {
                None
            };

            // Use AI explanation as detail if available, otherwise generate from merchant data
            // (compute before moving top_merchants)
            let detail = explanation.clone().or_else(|| {
                if !top_merchants.is_empty() {
                    let top_contributor = &top_merchants[0];
                    Some(format!(
                        "Top spending: {} (${:.0})",
                        top_contributor.merchant, top_contributor.current
                    ))
                } else {
                    None
                }
            });

            let data = SpendingExplainerData {
                tag_id,
                tag_name: tag_name.clone(),
                current_amount,
                baseline_amount,
                percent_change,
                explanation,
                top_merchants,
            };

            let severity = if percent_change.abs() > 50.0 {
                Severity::Warning
            } else if percent_change.abs() > 30.0 {
                Severity::Attention
            } else {
                Severity::Info
            };

            let direction = if is_increase { "up" } else { "down" };
            let key = format!(
                "spending:{}:{}",
                tag_name.to_lowercase().replace(' ', "_"),
                today.format("%Y-%m")
            );

            let mut finding = Finding::new(
                InsightType::SpendingExplainer,
                key,
                severity,
                format!(
                    "{} Spending {} {:.0}%",
                    tag_name,
                    direction,
                    percent_change.abs()
                ),
                format!(
                    "${:.0} this month vs ${:.0}/mo average",
                    current_amount, baseline_amount
                ),
            )
            .with_data(serde_json::to_value(&data).unwrap_or_default());

            if let Some(detail_text) = detail {
                finding = finding.with_detail(detail_text);
            }

            findings.push(finding);
        }

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{Bank, NewTransaction, TagSource};
    use chrono::Duration;

    #[tokio::test]
    async fn test_spending_explainer_detects_changes() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Create account
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();

        let today = Utc::now().date_naive();

        // Add transactions in current month (high spending)
        for i in 0..5 {
            db.insert_transaction(
                account_id,
                &NewTransaction {
                    date: today - Duration::days(i),
                    description: format!("RESTAURANT {}", i),
                    amount: -50.0,
                    category: None,
                    import_hash: format!("current_{}", i),
                    original_data: None,
                    import_format: None,
                    card_member: None,
                    payment_method: None,
                },
            )
            .unwrap();
        }

        // Tag them as Dining
        let dining_tag = db.get_tag_by_path("Dining").unwrap().unwrap();
        let transactions = db.list_transactions(None, 100, 0).unwrap();
        for tx in &transactions {
            db.add_transaction_tag(tx.id, dining_tag.id, TagSource::Manual, None)
                .unwrap();
        }

        // Add baseline transactions (lower spending) - need 3 months of data
        for month in 1..=3 {
            for i in 0..2 {
                let baseline_date = today - Duration::days(30 * month + i);
                db.insert_transaction(
                    account_id,
                    &NewTransaction {
                        date: baseline_date,
                        description: format!("RESTAURANT BASELINE {}_{}", month, i),
                        amount: -30.0,
                        category: None,
                        import_hash: format!("baseline_{}_{}", month, i),
                        original_data: None,
                        import_format: None,
                        card_member: None,
                        payment_method: None,
                    },
                )
                .unwrap();

                let txs = db.list_transactions(None, 1, 0).unwrap();
                db.add_transaction_tag(txs[0].id, dining_tag.id, TagSource::Manual, None)
                    .unwrap();
            }
        }

        let insight = SpendingExplainerInsight::new();
        let ctx = AnalysisContext::current_month(&db, None);
        let findings = insight.analyze(&ctx).await.unwrap();

        // Should detect the spending increase in Dining
        // Current: $250 (5 * $50), Baseline avg: $60/mo (2 * $30)
        // Change: 316% increase
        assert!(!findings.is_empty());

        let dining_finding = findings.iter().find(|f| f.title.contains("Dining"));
        assert!(dining_finding.is_some());

        let finding = dining_finding.unwrap();
        let data: SpendingExplainerData = serde_json::from_value(finding.data.clone()).unwrap();
        assert!(data.percent_change > 0.0); // Should be an increase
                                            // Without Ollama, explanation should be None
        assert!(data.explanation.is_none());
    }
}

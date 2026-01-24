//! Expense Forecaster Insight
//!
//! Predicts upcoming expenses based on:
//! - Active subscriptions with expected charge dates
//! - Rolling averages for variable categories

use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};

use crate::error::Result;
use crate::models::{Frequency, SubscriptionStatus};

use super::engine::{AnalysisContext, Insight};
use super::types::{
    ExpenseForecasterData, Finding, ForecastItem, ForecastItemType, InsightType, Severity,
};

/// Insight that forecasts upcoming expenses
pub struct ExpenseForecasterInsight {
    /// Number of days to forecast (default 30)
    forecast_days: u32,
}

impl ExpenseForecasterInsight {
    pub fn new() -> Self {
        Self { forecast_days: 30 }
    }

    pub fn with_forecast_days(days: u32) -> Self {
        Self {
            forecast_days: days,
        }
    }

    /// Calculate expected next charge date for a subscription
    fn next_charge_date(last_seen: NaiveDate, frequency: Frequency, today: NaiveDate) -> NaiveDate {
        let interval_days = match frequency {
            Frequency::Weekly => 7,
            Frequency::Monthly => 30,
            Frequency::Yearly => 365,
        };

        let mut next = last_seen + Duration::days(interval_days);

        // Advance until we're in the future
        while next <= today {
            next = next + Duration::days(interval_days);
        }

        next
    }
}

impl Default for ExpenseForecasterInsight {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Insight for ExpenseForecasterInsight {
    fn id(&self) -> InsightType {
        InsightType::ExpenseForecaster
    }

    fn name(&self) -> &'static str {
        "Expense Forecaster"
    }

    async fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let today = Utc::now().date_naive();
        let forecast_end = today + Duration::days(self.forecast_days as i64);

        let mut items: Vec<ForecastItem> = Vec::new();
        let mut total_expected = 0.0;

        // 1. Get active subscriptions and calculate expected charges
        let subscriptions = ctx.db.list_subscriptions(None)?;

        for sub in subscriptions {
            // Only forecast active subscriptions
            if sub.status != SubscriptionStatus::Active {
                continue;
            }

            let (amount, frequency, last_seen) = match (sub.amount, sub.frequency, sub.last_seen) {
                (Some(a), Some(f), Some(ls)) => (a.abs(), f, ls),
                _ => continue,
            };

            if amount < 1.0 {
                continue; // Skip tiny amounts
            }

            let next_charge = Self::next_charge_date(last_seen, frequency, today);

            // Check if within forecast window
            if next_charge <= forecast_end {
                let item = ForecastItem {
                    item_type: ForecastItemType::Subscription,
                    name: sub.merchant.clone(),
                    amount,
                    due_date: Some(next_charge.format("%Y-%m-%d").to_string()),
                    basis: Some(format!(
                        "Recurring {:?} since {}",
                        frequency,
                        sub.first_seen
                            .map(|d| d.format("%b %Y").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    )),
                };

                total_expected += amount;
                items.push(item);
            }
        }

        // 2. Add estimates for variable categories based on historical averages
        // Get 4-week spending averages for key categories
        let baseline_start = today - Duration::days(28 * 3); // 3 months of data
        let baseline_end = today;

        // Get spending summary for baseline period
        let spending =
            ctx.db
                .get_spending_summary(baseline_start, baseline_end, None, false, None, None)?;

        // Variable expense categories to estimate
        let variable_categories = ["Groceries", "Dining", "Gas", "Transport"];

        for cat_name in &variable_categories {
            if let Some(cat) = spending.categories.iter().find(|c| c.tag == *cat_name) {
                let monthly_avg = cat.amount.abs() / 3.0; // 3 months of data

                if monthly_avg > 50.0 {
                    // Only include if significant
                    let item = ForecastItem {
                        item_type: ForecastItemType::Estimate,
                        name: cat_name.to_string(),
                        amount: monthly_avg,
                        due_date: None,
                        basis: Some("3-month average".to_string()),
                    };

                    total_expected += monthly_avg;
                    items.push(item);
                }
            }
        }

        // 3. Check for large one-time expenses based on patterns
        // Look for any large upcoming expenses (like quarterly insurance)
        for sub in ctx.db.list_subscriptions(None)? {
            if sub.status != SubscriptionStatus::Active {
                continue;
            }

            let (amount, frequency, last_seen) = match (sub.amount, sub.frequency, sub.last_seen) {
                (Some(a), Some(f), Some(ls)) => (a.abs(), f, ls),
                _ => continue,
            };

            // Flag large yearly subscriptions as notable
            if frequency == Frequency::Yearly && amount > 100.0 {
                let next_charge = Self::next_charge_date(last_seen, frequency, today);

                // Check if within 60 days (give more notice for large expenses)
                let extended_window = today + Duration::days(60);
                if next_charge <= extended_window && next_charge > forecast_end {
                    let item = ForecastItem {
                        item_type: ForecastItemType::LargeExpense,
                        name: format!("{} (annual)", sub.merchant),
                        amount,
                        due_date: Some(next_charge.format("%Y-%m-%d").to_string()),
                        basis: Some("Annual charge".to_string()),
                    };

                    // Don't add to total (outside forecast window), but include as heads-up
                    items.push(item);
                }
            }
        }

        // Sort items: large expenses first, then by due date, then by amount
        items.sort_by(|a, b| {
            // Large expenses first
            let a_is_large = matches!(a.item_type, ForecastItemType::LargeExpense);
            let b_is_large = matches!(b.item_type, ForecastItemType::LargeExpense);
            if a_is_large != b_is_large {
                return b_is_large.cmp(&a_is_large);
            }

            // Then by due date (earliest first, None last)
            match (&a.due_date, &b.due_date) {
                (Some(a_date), Some(b_date)) => a_date.cmp(b_date),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b
                    .amount
                    .partial_cmp(&a.amount)
                    .unwrap_or(std::cmp::Ordering::Equal),
            }
        });

        // Only create a finding if we have items
        if items.is_empty() {
            return Ok(vec![]);
        }

        // Determine severity based on total expected and large items
        let has_large_expense = items
            .iter()
            .any(|i| matches!(i.item_type, ForecastItemType::LargeExpense));

        let severity = if has_large_expense || total_expected > 2000.0 {
            Severity::Attention
        } else {
            Severity::Info
        };

        let data = ExpenseForecasterData {
            period_days: self.forecast_days,
            total_expected,
            items: items.clone(),
        };

        // Create a single forecast finding
        let key = format!("forecast:{}:{}", today.format("%Y-%m"), self.forecast_days);

        let finding = Finding::new(
            InsightType::ExpenseForecaster,
            key,
            severity,
            format!("{}-Day Expense Forecast", self.forecast_days),
            format!(
                "Expected spending: ${:.0} from {} known expenses",
                total_expected,
                items
                    .iter()
                    .filter(|i| !matches!(i.item_type, ForecastItemType::LargeExpense))
                    .count()
            ),
        )
        .with_data(serde_json::to_value(&data).unwrap_or_default())
        .with_expiration(
            (today + Duration::days(self.forecast_days as i64))
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc(),
        );

        Ok(vec![finding])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::Bank;

    #[test]
    fn test_next_charge_date_monthly() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let last_seen = NaiveDate::from_ymd_opt(2025, 12, 10).unwrap();

        let next = ExpenseForecasterInsight::next_charge_date(last_seen, Frequency::Monthly, today);

        // Should be January 9, 2026 (30 days after Dec 10)
        assert!(next > today);
        assert!(next < today + Duration::days(30));
    }

    #[test]
    fn test_next_charge_date_weekly() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let last_seen = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();

        let next = ExpenseForecasterInsight::next_charge_date(last_seen, Frequency::Weekly, today);

        // Should be January 17, 2026 (7 days after Jan 10)
        assert_eq!(next, NaiveDate::from_ymd_opt(2026, 1, 17).unwrap());
    }

    #[tokio::test]
    async fn test_expense_forecaster_with_subscriptions() {
        let db = Database::in_memory().unwrap();

        // Create account
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();

        // Create an active subscription with recent last_seen
        let recent_date = Utc::now().date_naive() - Duration::days(10);
        db.upsert_subscription(
            "NETFLIX",
            Some(account_id),
            Some(22.99),
            Some(Frequency::Monthly),
            Some(recent_date - Duration::days(60)),
            Some(recent_date),
        )
        .unwrap();

        let insight = ExpenseForecasterInsight::new();
        let ctx = AnalysisContext::current_month(&db, None);
        let findings = insight.analyze(&ctx).await.unwrap();

        // Should have a forecast finding
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].insight_type, InsightType::ExpenseForecaster);

        let data: ExpenseForecasterData = serde_json::from_value(findings[0].data.clone()).unwrap();
        assert!(data.total_expected > 0.0);
        assert!(!data.items.is_empty());

        // Should include NETFLIX
        let netflix = data.items.iter().find(|i| i.name == "NETFLIX");
        assert!(netflix.is_some());
        assert!((netflix.unwrap().amount - 22.99).abs() < 0.01);
    }
}

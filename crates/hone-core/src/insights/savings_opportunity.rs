//! Savings Opportunity Insight
//!
//! Identifies potential savings opportunities from:
//! - Zombie subscriptions (unused for 3+ months)
//! - Duplicate services (multiple in same category)

use async_trait::async_trait;

use crate::error::Result;
use crate::models::{AlertType, SubscriptionStatus};

use super::engine::{AnalysisContext, Insight};
use super::types::{
    Finding, InsightType, SavingsOpportunityData, SavingsOpportunityType, Severity,
};

/// Insight that identifies savings opportunities
pub struct SavingsOpportunityInsight;

impl SavingsOpportunityInsight {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SavingsOpportunityInsight {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Insight for SavingsOpportunityInsight {
    fn id(&self) -> InsightType {
        InsightType::SavingsOpportunity
    }

    fn name(&self) -> &'static str {
        "Savings Opportunity"
    }

    async fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let mut findings = Vec::new();

        // Get subscriptions and alerts
        let subscriptions = ctx.db.list_subscriptions(None)?;
        let alerts = ctx.db.list_alerts(false)?; // Active alerts only

        // 1. Find zombie subscriptions (unused for 3+ months)
        for sub in &subscriptions {
            if sub.status != SubscriptionStatus::Zombie {
                continue;
            }

            let monthly_amount = sub.amount.unwrap_or(0.0).abs();
            if monthly_amount < 1.0 {
                continue; // Skip tiny amounts
            }

            // Check if there's already a zombie alert for this
            let has_alert = alerts
                .iter()
                .any(|a| a.alert_type == AlertType::Zombie && a.subscription_id == Some(sub.id));

            let annual_savings = monthly_amount * 12.0;
            let key = format!("savings:zombie:{}", sub.id);

            let data = SavingsOpportunityData {
                opportunity_type: SavingsOpportunityType::Zombie,
                subscription_id: Some(sub.id),
                subscription_name: Some(sub.merchant.clone()),
                monthly_amount,
                annual_savings,
                reason: format!(
                    "No activity detected for {} since {}",
                    sub.merchant,
                    sub.last_seen
                        .map(|d| d.format("%B %Y").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                alert_id: if has_alert {
                    alerts
                        .iter()
                        .find(|a| {
                            a.alert_type == AlertType::Zombie && a.subscription_id == Some(sub.id)
                        })
                        .map(|a| a.id)
                } else {
                    None
                },
            };

            let severity = if annual_savings > 200.0 {
                Severity::Warning
            } else if annual_savings > 50.0 {
                Severity::Attention
            } else {
                Severity::Info
            };

            let finding = Finding::new(
                InsightType::SavingsOpportunity,
                key,
                severity,
                "Unused Subscription",
                format!(
                    "{} hasn't been used recently. Cancel to save ${:.0}/year",
                    sub.merchant, annual_savings
                ),
            )
            .with_detail(data.reason.clone())
            .with_data(serde_json::to_value(&data).unwrap_or_default());

            findings.push(finding);
        }

        // 2. Find duplicate services
        for alert in &alerts {
            if alert.alert_type != AlertType::Duplicate {
                continue;
            }

            // Parse the duplicate info from the alert message
            // Format: "You have N category services: A, B, C. Total: $X.XX/mo"
            let message = alert.message.as_deref().unwrap_or("");

            // Extract total monthly cost from message
            let monthly_amount = message
                .split("Total: $")
                .nth(1)
                .and_then(|s| s.split("/mo").next())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            if monthly_amount < 5.0 {
                continue; // Skip if we couldn't parse or too small
            }

            // Assume saving ~half by consolidating
            let potential_savings = monthly_amount / 2.0;
            let annual_savings = potential_savings * 12.0;

            let key = format!("savings:duplicate:{}", alert.id);

            let data = SavingsOpportunityData {
                opportunity_type: SavingsOpportunityType::Duplicate,
                subscription_id: alert.subscription_id,
                subscription_name: alert.subscription.as_ref().map(|s| s.merchant.clone()),
                monthly_amount,
                annual_savings,
                reason: message.to_string(),
                alert_id: Some(alert.id),
            };

            let severity = if annual_savings > 150.0 {
                Severity::Warning
            } else if annual_savings > 50.0 {
                Severity::Attention
            } else {
                Severity::Info
            };

            let finding = Finding::new(
                InsightType::SavingsOpportunity,
                key,
                severity,
                "Overlapping Services",
                format!(
                    "Consolidating duplicate services could save ~${:.0}/year",
                    annual_savings
                ),
            )
            .with_detail(message.to_string())
            .with_data(serde_json::to_value(&data).unwrap_or_default());

            findings.push(finding);
        }

        // Sort by annual savings (highest first)
        findings.sort_by(|a, b| {
            let a_savings = serde_json::from_value::<SavingsOpportunityData>(a.data.clone())
                .map(|d| d.annual_savings)
                .unwrap_or(0.0);
            let b_savings = serde_json::from_value::<SavingsOpportunityData>(b.data.clone())
                .map(|d| d.annual_savings)
                .unwrap_or(0.0);
            b_savings
                .partial_cmp(&a_savings)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{Bank, Frequency};
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn test_savings_opportunity_detects_zombies() {
        let db = Database::in_memory().unwrap();

        // Create account
        let account_id = db
            .upsert_account("Test Account", Bank::Chase, None)
            .unwrap();

        // Create a zombie subscription
        let old_date = Utc::now().date_naive() - Duration::days(120);
        let sub_id = db
            .upsert_subscription(
                "UNUSED STREAMING",
                Some(account_id),
                Some(15.99),
                Some(Frequency::Monthly),
                Some(old_date - Duration::days(90)),
                Some(old_date),
            )
            .unwrap();

        // Mark as zombie
        db.update_subscription_status(sub_id, SubscriptionStatus::Zombie)
            .unwrap();

        let insight = SavingsOpportunityInsight::new();
        let ctx = AnalysisContext::current_month(&db, None);
        let findings = insight.analyze(&ctx).await.unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].insight_type, InsightType::SavingsOpportunity);
        assert!(findings[0].title.contains("Unused"));

        let data: SavingsOpportunityData =
            serde_json::from_value(findings[0].data.clone()).unwrap();
        assert_eq!(data.opportunity_type, SavingsOpportunityType::Zombie);
        assert_eq!(data.subscription_id, Some(sub_id));
        assert!((data.annual_savings - 191.88).abs() < 0.01); // 15.99 * 12
    }
}

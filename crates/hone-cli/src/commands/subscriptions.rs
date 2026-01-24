//! Subscription command implementations

use anyhow::{Context, Result};
use chrono::NaiveDate;
use hone_core::db::Database;

use super::truncate;

pub fn cmd_subscriptions_list(db: &Database) -> Result<()> {
    let subscriptions = db.list_subscriptions(None)?;

    if subscriptions.is_empty() {
        println!("No subscriptions detected yet. Run:");
        println!("  hone detect --kind all");
        return Ok(());
    }

    println!();
    println!("📋 Detected Subscriptions");
    println!("   ─────────────────────────────────────────────────────────────");

    for sub in subscriptions {
        let status_icon = match sub.status {
            hone_core::models::SubscriptionStatus::Active => "✅",
            hone_core::models::SubscriptionStatus::Zombie => "🧟",
            hone_core::models::SubscriptionStatus::Cancelled => "❌",
            hone_core::models::SubscriptionStatus::Excluded => "🚫",
        };

        let amount_str = sub
            .amount
            .map(|a| format!("${:.2}", a))
            .unwrap_or_else(|| "?".to_string());
        let freq_str = sub.frequency.map(|f| f.as_str()).unwrap_or("?");

        println!(
            "   {} {:20} │ {:>8}/{:<7} │ since {}",
            status_icon,
            truncate(&sub.merchant, 20),
            amount_str,
            freq_str,
            sub.first_seen
                .map(|d| d.to_string())
                .unwrap_or_else(|| "?".to_string())
        );
    }

    Ok(())
}

pub fn cmd_subscriptions_cancel(db: &Database, name_or_id: &str, date: Option<&str>) -> Result<()> {
    // Find subscription by name or ID
    let sub_id = db
        .find_subscription_by_merchant_or_id(name_or_id)?
        .ok_or_else(|| anyhow::anyhow!("Subscription not found: {}", name_or_id))?;

    // Parse optional date
    let cancel_date = date
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .context("Invalid --date format (use YYYY-MM-DD)")?;

    // Cancel the subscription
    db.cancel_subscription(sub_id, cancel_date)?;

    let date_str = cancel_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| "today".to_string());

    println!(
        "✅ Subscription cancelled (ID: {}) as of {}",
        sub_id, date_str
    );
    println!("   Savings will be tracked in: hone report savings");

    Ok(())
}

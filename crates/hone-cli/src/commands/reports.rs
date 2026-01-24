//! Report command implementations

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, Utc};
use hone_core::db::Database;
use hone_core::models::Granularity;

use super::truncate;

/// Resolve a period string to (from_date, to_date)
pub fn resolve_period(
    period: &str,
    custom_from: Option<&str>,
    custom_to: Option<&str>,
) -> Result<(NaiveDate, NaiveDate)> {
    // If custom dates provided, use those
    if let (Some(from), Some(to)) = (custom_from, custom_to) {
        let from_date = NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .context("Invalid --from date format (use YYYY-MM-DD)")?;
        let to_date = NaiveDate::parse_from_str(to, "%Y-%m-%d")
            .context("Invalid --to date format (use YYYY-MM-DD)")?;
        return Ok((from_date, to_date));
    }

    let today = Utc::now().date_naive();

    match period.to_lowercase().as_str() {
        "this-month" => {
            let from = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            Ok((from, today))
        }
        "last-month" => {
            let last_month = if today.month() == 1 {
                NaiveDate::from_ymd_opt(today.year() - 1, 12, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() - 1, 1).unwrap()
            };
            let last_day = if today.month() == 1 {
                NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap().pred_opt().unwrap()
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap().pred_opt().unwrap()
            };
            Ok((last_month, last_day))
        }
        "this-year" => {
            let from = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            Ok((from, today))
        }
        "last-30-days" => {
            let from = today - chrono::Duration::days(30);
            Ok((from, today))
        }
        "last-90-days" => {
            let from = today - chrono::Duration::days(90);
            Ok((from, today))
        }
        "last-12-months" => {
            let from = if today.month() == 1 {
                NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(today.year() - 1, today.month(), 1).unwrap()
            };
            Ok((from, today))
        }
        "all" => {
            let from = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
            Ok((from, today))
        }
        _ => anyhow::bail!("Unknown period: {}. Available: this-month, last-month, this-year, last-30-days, last-90-days, last-12-months, all", period),
    }
}

pub fn cmd_report_spending(
    db: &Database,
    from: NaiveDate,
    to: NaiveDate,
    tag_filter: Option<&str>,
    expand: bool,
) -> Result<()> {
    let summary = db.get_spending_summary(from, to, tag_filter, expand, None, None)?;

    println!();
    println!("📊 Spending Summary");
    println!(
        "   Period: {} to {}",
        summary.period.from, summary.period.to
    );
    println!("   ─────────────────────────────────────────────────────────────");

    if summary.categories.is_empty() && summary.untagged.transaction_count == 0 {
        println!("   No spending found in this period.");
        return Ok(());
    }

    println!("   Total: ${:.2}", summary.total);
    println!();
    println!(
        "   {:25} │ {:>10} │ {:>6} │ {:>5}",
        "Category", "Amount", "%", "Count"
    );
    println!("   ──────────────────────────┼────────────┼────────┼───────");

    fn print_category(cat: &hone_core::models::CategorySpending, indent: usize) {
        let prefix = "  ".repeat(indent);
        println!(
            "   {:25} │ {:>10.2} │ {:>5.1}% │ {:>5}",
            format!("{}{}", prefix, truncate(&cat.tag, 25 - prefix.len())),
            cat.amount,
            cat.percentage,
            cat.transaction_count
        );
        for child in &cat.children {
            print_category(child, indent + 1);
        }
    }

    for cat in &summary.categories {
        print_category(cat, 0);
    }

    // Show untagged if any
    if summary.untagged.transaction_count > 0 {
        println!(
            "   {:25} │ {:>10.2} │ {:>5.1}% │ {:>5}",
            "\x1b[2mUntagged\x1b[0m",
            summary.untagged.amount,
            summary.untagged.percentage,
            summary.untagged.transaction_count
        );
    }

    Ok(())
}

pub fn cmd_report_trends(
    db: &Database,
    from: NaiveDate,
    to: NaiveDate,
    granularity: Granularity,
    tag_filter: Option<&str>,
) -> Result<()> {
    let report = db.get_spending_trends(from, to, granularity, tag_filter, None, None)?;

    println!();
    println!("📈 Spending Trends ({})", granularity.as_str());
    if let Some(ref tag) = report.tag {
        println!("   Tag: {}", tag);
    }
    println!("   Period: {} to {}", report.period.from, report.period.to);
    println!("   ─────────────────────────────────────────────────────────────");

    if report.data.is_empty() {
        println!("   No spending data found.");
        return Ok(());
    }

    println!("   {:12} │ {:>10} │ {:>5}", "Period", "Amount", "Count");
    println!("   ─────────────┼────────────┼───────");

    for point in &report.data {
        println!(
            "   {:12} │ {:>10.2} │ {:>5}",
            point.period, point.amount, point.transaction_count
        );
    }

    // Show totals
    let total_amount: f64 = report.data.iter().map(|p| p.amount).sum();
    let total_count: i64 = report.data.iter().map(|p| p.transaction_count).sum();
    let avg_amount = if !report.data.is_empty() {
        total_amount / report.data.len() as f64
    } else {
        0.0
    };

    println!("   ─────────────┼────────────┼───────");
    println!(
        "   {:12} │ {:>10.2} │ {:>5}",
        "Total", total_amount, total_count
    );
    println!("   {:12} │ {:>10.2} │", "Average", avg_amount);

    Ok(())
}

pub fn cmd_report_merchants(
    db: &Database,
    from: NaiveDate,
    to: NaiveDate,
    limit: i64,
    tag_filter: Option<&str>,
) -> Result<()> {
    let report = db.get_top_merchants(from, to, limit, tag_filter, None, None)?;

    println!();
    println!("🏪 Top Merchants");
    println!("   Period: {} to {}", report.period.from, report.period.to);
    println!("   ─────────────────────────────────────────────────────────────");

    if report.merchants.is_empty() {
        println!("   No spending found.");
        return Ok(());
    }

    println!(
        "   {:3} │ {:30} │ {:>10} │ {:>5}",
        "#", "Merchant", "Amount", "Count"
    );
    println!("   ────┼────────────────────────────────┼────────────┼───────");

    for (i, merchant) in report.merchants.iter().enumerate() {
        println!(
            "   {:>3} │ {:30} │ {:>10.2} │ {:>5}",
            i + 1,
            truncate(&merchant.merchant, 30),
            merchant.amount,
            merchant.transaction_count
        );
    }

    Ok(())
}

pub fn cmd_report_subscriptions(db: &Database) -> Result<()> {
    let report = db.get_subscription_summary()?;

    println!();
    println!("📋 Subscription Summary");
    println!("   ─────────────────────────────────────────────────────────────");
    println!(
        "   Active: {}    Cancelled: {}",
        report.active_count, report.cancelled_count
    );
    println!("   Monthly Cost: ${:.2}", report.total_monthly);
    println!();

    if report.waste.total_waste_monthly > 0.0 {
        println!("   ⚠️  Potential Waste:");
        if report.waste.zombie_count > 0 {
            println!(
                "      🧟 {} zombie subscription(s): ${:.2}/mo",
                report.waste.zombie_count, report.waste.zombie_monthly
            );
        }
        if report.waste.duplicate_count > 0 {
            println!(
                "      👯 {} duplicate(s): ${:.2}/mo",
                report.waste.duplicate_count, report.waste.duplicate_monthly
            );
        }
        if report.waste.price_increase_count > 0 {
            println!(
                "      📈 {} price increase(s)",
                report.waste.price_increase_count
            );
        }
        println!("      Total: ${:.2}/mo", report.waste.total_waste_monthly);
        println!();
    }

    if !report.subscriptions.is_empty() {
        println!(
            "   {:20} │ {:>10} │ {:>8} │ {:>10}",
            "Merchant", "Amount", "Freq", "Status"
        );
        println!("   ─────────────────────┼────────────┼──────────┼────────────");

        for sub in &report.subscriptions {
            let status_icon = match sub.status.as_str() {
                "active" => "✅",
                "cancelled" => "❌",
                "zombie" => "🧟",
                _ => "  ",
            };
            println!(
                "   {:20} │ {:>10.2} │ {:>8} │ {} {}",
                truncate(&sub.merchant, 20),
                sub.amount,
                sub.frequency,
                status_icon,
                sub.status
            );
        }
    }

    Ok(())
}

pub fn cmd_report_savings(db: &Database) -> Result<()> {
    let report = db.get_savings_report()?;

    println!();
    println!("💰 Savings Report");
    println!("   ─────────────────────────────────────────────────────────────");

    if report.cancelled_count == 0 {
        println!("   No cancelled subscriptions to track savings from.");
        println!("   Cancel unwanted subscriptions with: hone subscriptions cancel <name>");
        return Ok(());
    }

    println!("   Cancelled Subscriptions: {}", report.cancelled_count);
    println!("   Monthly Savings: ${:.2}/mo", report.total_monthly_saved);
    println!(
        "   Total Saved: ${:.2} (capped at 12 months per subscription)",
        report.total_savings
    );
    println!();

    println!(
        "   {:20} │ {:>8} │ {:>12} │ {:>8} │ {:>10}",
        "Merchant", "$/mo", "Cancelled", "Months", "Saved"
    );
    println!("   ─────────────────────┼──────────┼──────────────┼──────────┼────────────");

    for sub in &report.cancelled {
        let months_str = if sub.months_remaining > 0 {
            format!(
                "{}/{}",
                sub.months_counted,
                sub.months_counted + sub.months_remaining
            )
        } else {
            format!("{} ✓", sub.months_counted)
        };
        println!(
            "   {:20} │ {:>8.2} │ {:>12} │ {:>8} │ {:>10.2}",
            truncate(&sub.merchant, 20),
            sub.monthly_amount,
            sub.cancelled_at,
            months_str,
            sub.savings
        );
    }

    Ok(())
}

pub fn cmd_report_by_tag(
    db: &Database,
    max_depth: Option<i32>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<()> {
    let spending = db.get_spending_by_tag(from, to)?;

    if spending.is_empty() {
        println!("No tagged transactions found.");
        return Ok(());
    }

    println!();
    println!("📊 Spending by Tag");
    if from.is_some() || to.is_some() {
        let from_str = from
            .map(|d| d.to_string())
            .unwrap_or_else(|| "start".to_string());
        let to_str = to
            .map(|d| d.to_string())
            .unwrap_or_else(|| "now".to_string());
        println!("   Period: {} to {}", from_str, to_str);
    }
    println!("   ─────────────────────────────────────────────────────────────");
    println!(
        "   {:30} │ {:>10} │ {:>10} │ {:>5}",
        "Tag", "Direct", "Total", "Count"
    );
    println!("   ───────────────────────────────┼────────────┼────────────┼───────");

    for ts in spending {
        // Calculate depth from path
        let depth = ts.tag_path.matches('.').count() as i32;

        // Skip if beyond max depth
        if let Some(max) = max_depth {
            if depth > max {
                continue;
            }
        }

        let indent = "  ".repeat(depth as usize);
        let name = if depth > 0 {
            ts.tag_path.rsplit('.').next().unwrap_or(&ts.tag_name)
        } else {
            &ts.tag_name
        };

        println!(
            "   {:30} │ {:>10.2} │ {:>10.2} │ {:>5}",
            format!("{}{}", indent, truncate(name, 30 - indent.len())),
            ts.direct_amount.abs(),
            ts.total_amount.abs(),
            ts.transaction_count
        );
    }

    Ok(())
}

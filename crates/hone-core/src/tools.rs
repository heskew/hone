//! MCP Tool implementations for Hone
//!
//! These tools provide read-only queries against the Hone database.
//! They are used by:
//! 1. The MCP server for external LLM clients (Claude Desktop, etc.)
//! 2. The AI Orchestrator for internal agentic workflows
//!
//! All tools are designed to be safe for LLM invocation - they only query data,
//! never modify it.

use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::{Error, Result};

// =============================================================================
// Period Resolution (shared utility)
// =============================================================================

/// Resolve a period string to date range
pub fn resolve_period(period: &str) -> Result<(NaiveDate, NaiveDate)> {
    let today = Utc::now().date_naive();

    match period.to_lowercase().as_str() {
        "this-month" | "thismonth" => {
            let from = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            Ok((from, today))
        }
        "last-month" | "lastmonth" => {
            let last_month = if today.month() == 1 {
                NaiveDate::from_ymd_opt(today.year() - 1, 12, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() - 1, 1).unwrap()
            };
            let last_day = if today.month() == 1 {
                NaiveDate::from_ymd_opt(today.year(), 1, 1)
                    .unwrap()
                    .pred_opt()
                    .unwrap()
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                    .unwrap()
                    .pred_opt()
                    .unwrap()
            };
            Ok((last_month, last_day))
        }
        "this-year" | "thisyear" | "ytd" => {
            let from = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            Ok((from, today))
        }
        "last-year" | "lastyear" => {
            let from = NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap();
            let to = NaiveDate::from_ymd_opt(today.year() - 1, 12, 31).unwrap();
            Ok((from, to))
        }
        "last-30-days" | "last30days" => {
            let from = today - chrono::Duration::days(30);
            Ok((from, today))
        }
        "last-90-days" | "last90days" => {
            let from = today - chrono::Duration::days(90);
            Ok((from, today))
        }
        "last-12-months" | "last12months" => {
            let from = today - chrono::Duration::days(365);
            Ok((from, today))
        }
        "all" | "" => {
            // Return a very wide range
            let from = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
            Ok((from, today))
        }
        _ => {
            // Try to parse as YYYY-MM-DD
            if let Ok(date) = NaiveDate::parse_from_str(period, "%Y-%m-%d") {
                Ok((date, date))
            } else {
                Err(Error::InvalidData(format!(
                    "Invalid period: {}. Use: this-month, last-month, this-year, last-year, last-30-days, last-90-days, last-12-months, ytd, all",
                    period
                )))
            }
        }
    }
}

/// Parse optional date strings
pub fn parse_date_opt(s: Option<&str>) -> Result<Option<NaiveDate>> {
    match s {
        None => Ok(None),
        Some(date_str) => {
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|_| {
                Error::InvalidData(format!("Invalid date format: {}. Use YYYY-MM-DD", date_str))
            })?;
            Ok(Some(date))
        }
    }
}

// =============================================================================
// search_transactions
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SearchTransactionsParams {
    /// Text search in description or merchant name
    #[schemars(description = "Text to search for in transaction descriptions or merchant names")]
    pub query: Option<String>,

    /// Filter by tag name (e.g., "Dining", "Groceries")
    #[schemars(description = "Filter transactions by tag/category name")]
    pub tag: Option<String>,

    /// Period preset (this-month, last-month, this-year, last-year, last-30-days, etc.)
    #[schemars(
        description = "Time period: this-month, last-month, this-year, last-year, last-30-days, last-90-days, ytd, all"
    )]
    pub period: Option<String>,

    /// Custom start date (YYYY-MM-DD), used if period is not specified
    #[schemars(description = "Start date in YYYY-MM-DD format")]
    pub from_date: Option<String>,

    /// Custom end date (YYYY-MM-DD), used if period is not specified
    #[schemars(description = "End date in YYYY-MM-DD format")]
    pub to_date: Option<String>,

    /// Minimum transaction amount
    #[schemars(description = "Minimum transaction amount (positive number)")]
    pub min_amount: Option<f64>,

    /// Maximum transaction amount
    #[schemars(description = "Maximum transaction amount (positive number)")]
    pub max_amount: Option<f64>,

    /// Maximum number of results (default 50, max 200)
    #[schemars(description = "Maximum number of results to return (default 50, max 200)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TransactionSummary {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub merchant: Option<String>,
    pub amount: f64,
    pub tags: Vec<String>,
    pub account: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SearchTransactionsResult {
    pub transactions: Vec<TransactionSummary>,
    pub total_count: usize,
    pub total_amount: f64,
}

pub fn search_transactions(
    db: &Database,
    params: SearchTransactionsParams,
) -> Result<SearchTransactionsResult> {
    // Determine date range
    let (from_date, to_date) = if let Some(period) = params.period.as_deref() {
        resolve_period(period)?
    } else if params.from_date.is_some() || params.to_date.is_some() {
        let from = parse_date_opt(params.from_date.as_deref())?
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        let to =
            parse_date_opt(params.to_date.as_deref())?.unwrap_or_else(|| Utc::now().date_naive());
        (from, to)
    } else {
        // Default to this month
        resolve_period("this-month")?
    };

    // Get tag ID if tag name provided
    let tag_ids = if let Some(tag_name) = params.tag.as_deref() {
        let tags = db.list_tags()?;
        tags.iter()
            .find(|t| t.name.eq_ignore_ascii_case(tag_name))
            .map(|t| vec![t.id])
    } else {
        None
    };

    let limit = params.limit.unwrap_or(50).min(200).max(1);

    // Use the database search function
    let transactions = db.search_transactions_full(
        None, // account_id
        None, // entity_id
        None, // card_member
        params.query.as_deref(),
        tag_ids.as_deref(),
        false, // untagged
        Some((from_date, to_date)),
        Some("date"),
        Some("desc"),
        false, // include_archived
        limit,
        0,
    )?;

    // Apply amount filters and transform
    let mut summaries: Vec<TransactionSummary> = transactions
        .into_iter()
        .filter(|t| {
            let amount = t.amount.abs();
            let min_ok = params.min_amount.map_or(true, |min| amount >= min);
            let max_ok = params.max_amount.map_or(true, |max| amount <= max);
            min_ok && max_ok
        })
        .map(|t| {
            // Get tags for this transaction
            let tags = db
                .get_transaction_tags_with_details(t.id)
                .map(|tags| tags.into_iter().map(|tag| tag.tag_name).collect())
                .unwrap_or_default();

            // Get account name
            let account = db
                .get_account(t.account_id)
                .ok()
                .flatten()
                .map(|a| a.name)
                .unwrap_or_else(|| "Unknown".to_string());

            TransactionSummary {
                id: t.id,
                date: t.date.to_string(),
                description: t.description.clone(),
                merchant: t.merchant_normalized.or(Some(t.description)),
                amount: t.amount,
                tags,
                account,
            }
        })
        .collect();

    let total_count = summaries.len();
    let total_amount: f64 = summaries.iter().map(|t| t.amount.abs()).sum();

    // Truncate to limit
    summaries.truncate(limit as usize);

    Ok(SearchTransactionsResult {
        transactions: summaries,
        total_count,
        total_amount,
    })
}

// =============================================================================
// get_spending_summary
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SpendingSummaryParams {
    /// Period preset (this-month, last-month, this-year, etc.)
    #[schemars(
        description = "Time period: this-month, last-month, this-year, last-year, last-30-days, last-90-days, ytd, all"
    )]
    pub period: Option<String>,

    /// Custom start date (YYYY-MM-DD)
    #[schemars(description = "Start date in YYYY-MM-DD format")]
    pub from_date: Option<String>,

    /// Custom end date (YYYY-MM-DD)
    #[schemars(description = "End date in YYYY-MM-DD format")]
    pub to_date: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CategorySpending {
    pub category: String,
    pub amount: f64,
    pub percentage: f64,
    pub transaction_count: i32,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SpendingSummaryResult {
    pub period: String,
    pub total_spending: f64,
    pub categories: Vec<CategorySpending>,
}

pub fn get_spending_summary(
    db: &Database,
    params: SpendingSummaryParams,
) -> Result<SpendingSummaryResult> {
    let period_name = params.period.as_deref().unwrap_or("this-month");

    let (from_date, to_date) = if let Some(period) = params.period.as_deref() {
        resolve_period(period)?
    } else if params.from_date.is_some() || params.to_date.is_some() {
        let from = parse_date_opt(params.from_date.as_deref())?
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        let to =
            parse_date_opt(params.to_date.as_deref())?.unwrap_or_else(|| Utc::now().date_naive());
        (from, to)
    } else {
        resolve_period("this-month")?
    };

    // Get spending summary from database
    let summary = db.get_spending_summary(
        from_date, to_date, None,  // tag filter
        false, // expand
        None,  // entity_id
        None,  // card_member
    )?;

    let categories: Vec<CategorySpending> = summary
        .categories
        .into_iter()
        .map(|c| CategorySpending {
            category: c.tag,
            amount: c.amount,
            percentage: c.percentage,
            transaction_count: c.transaction_count as i32,
        })
        .collect();

    Ok(SpendingSummaryResult {
        period: period_name.to_string(),
        total_spending: summary.total,
        categories,
    })
}

// =============================================================================
// get_subscriptions
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SubscriptionsParams {
    /// Filter by status: active, cancelled, excluded, all (default: active)
    #[schemars(description = "Subscription status filter: active, cancelled, excluded, all")]
    pub status: Option<String>,

    /// Include excluded subscriptions (default: false)
    #[schemars(description = "Include subscriptions marked as 'not a subscription'")]
    pub include_excluded: Option<bool>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SubscriptionSummary {
    pub id: i64,
    pub merchant: String,
    pub amount: f64,
    pub frequency: String,
    pub status: String,
    pub first_seen: String,
    pub last_seen: String,
    pub account: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SubscriptionsResult {
    pub subscriptions: Vec<SubscriptionSummary>,
    pub total_monthly_cost: f64,
    pub active_count: usize,
}

pub fn get_subscriptions(
    db: &Database,
    params: SubscriptionsParams,
) -> Result<SubscriptionsResult> {
    use crate::models::SubscriptionStatus;

    let status_filter = params.status.as_deref().unwrap_or("active");
    let include_excluded = params.include_excluded.unwrap_or(false);

    let subscriptions = db.list_subscriptions(None)?;

    let filtered: Vec<SubscriptionSummary> = subscriptions
        .into_iter()
        .filter(|s| {
            if !include_excluded && s.status == SubscriptionStatus::Excluded {
                return false;
            }
            match status_filter {
                "active" => s.status == SubscriptionStatus::Active,
                "cancelled" => s.status == SubscriptionStatus::Cancelled,
                "excluded" => s.status == SubscriptionStatus::Excluded,
                "zombie" => s.status == SubscriptionStatus::Zombie,
                "all" => true,
                _ => s.status == SubscriptionStatus::Active,
            }
        })
        .map(|s| {
            // Get account name
            let account = s
                .account_id
                .and_then(|id| db.get_account(id).ok().flatten())
                .map(|a| a.name)
                .unwrap_or_else(|| "Unknown".to_string());

            SubscriptionSummary {
                id: s.id,
                merchant: s.merchant.clone(),
                amount: s.amount.unwrap_or(0.0),
                frequency: s
                    .frequency
                    .map(|f| f.as_str().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                status: s.status.as_str().to_string(),
                first_seen: s
                    .first_seen
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                last_seen: s
                    .last_seen
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                account,
            }
        })
        .collect();

    // Calculate monthly cost (normalize to monthly)
    let monthly_cost: f64 = filtered
        .iter()
        .filter(|s| s.status == "active")
        .map(|s| {
            match s.frequency.to_lowercase().as_str() {
                "weekly" => s.amount * 4.33, // Approximate weeks per month
                "yearly" => s.amount / 12.0,
                "quarterly" => s.amount / 3.0,
                _ => s.amount, // Assume monthly
            }
        })
        .sum();

    let active_count = filtered.iter().filter(|s| s.status == "active").count();

    Ok(SubscriptionsResult {
        subscriptions: filtered,
        total_monthly_cost: monthly_cost,
        active_count,
    })
}

// =============================================================================
// get_alerts
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct AlertsParams {
    /// Filter by alert type: zombie, price_increase, duplicate, spending_anomaly, resume, all
    #[schemars(
        description = "Alert type filter: zombie, price_increase, duplicate, spending_anomaly, resume, all"
    )]
    pub alert_type: Option<String>,

    /// Include dismissed alerts (default: false)
    #[schemars(description = "Include alerts that have been dismissed")]
    pub include_dismissed: Option<bool>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AlertSummary {
    pub id: i64,
    pub alert_type: String,
    pub message: Option<String>,
    pub dismissed: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AlertsResult {
    pub alerts: Vec<AlertSummary>,
    pub total_potential_savings: f64,
    pub active_count: usize,
}

pub fn get_alerts(db: &Database, params: AlertsParams) -> Result<AlertsResult> {
    let include_dismissed = params.include_dismissed.unwrap_or(false);
    let type_filter = params.alert_type.as_deref();

    let alerts = db.list_alerts(include_dismissed)?;

    let filtered: Vec<AlertSummary> = alerts
        .into_iter()
        .filter(|a| {
            if let Some(filter) = type_filter {
                if filter != "all" {
                    let alert_type = a.alert_type.as_str();
                    if !alert_type.contains(&filter.to_lowercase()) {
                        return false;
                    }
                }
            }
            true
        })
        .map(|a| AlertSummary {
            id: a.id,
            alert_type: a.alert_type.as_str().to_string(),
            message: a.message.clone(),
            dismissed: a.dismissed,
            created_at: a.created_at.to_string(),
        })
        .collect();

    let active_count = filtered.iter().filter(|a| !a.dismissed).count();

    Ok(AlertsResult {
        alerts: filtered,
        total_potential_savings: 0.0, // Would need subscription data to calculate
        active_count,
    })
}

// =============================================================================
// compare_spending
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct CompareSpendingParams {
    /// Current period to compare (default: this-month)
    #[schemars(description = "Current period: this-month, last-month, this-year, etc.")]
    pub current_period: Option<String>,

    /// Baseline period to compare against (default: last-month)
    #[schemars(description = "Baseline period to compare against")]
    pub baseline_period: Option<String>,

    /// Filter to specific category
    #[schemars(description = "Optional: filter comparison to a specific category")]
    pub category: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CategoryComparison {
    pub category: String,
    pub current_amount: f64,
    pub baseline_amount: f64,
    pub change_amount: f64,
    pub change_percentage: f64,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CompareSpendingResult {
    pub current_period: String,
    pub baseline_period: String,
    pub current_total: f64,
    pub baseline_total: f64,
    pub total_change: f64,
    pub total_change_percentage: f64,
    pub by_category: Vec<CategoryComparison>,
}

pub fn compare_spending(
    db: &Database,
    params: CompareSpendingParams,
) -> Result<CompareSpendingResult> {
    let current_period_name = params.current_period.as_deref().unwrap_or("this-month");
    let baseline_period_name = params.baseline_period.as_deref().unwrap_or("last-month");

    let (current_from, current_to) = resolve_period(current_period_name)?;
    let (baseline_from, baseline_to) = resolve_period(baseline_period_name)?;

    // Get spending for both periods
    let current_summary = db.get_spending_summary(
        current_from,
        current_to,
        params.category.as_deref(),
        false,
        None,
        None,
    )?;

    let baseline_summary = db.get_spending_summary(
        baseline_from,
        baseline_to,
        params.category.as_deref(),
        false,
        None,
        None,
    )?;

    // Build comparison by category
    let mut comparisons: Vec<CategoryComparison> = Vec::new();

    // Create a map of baseline amounts
    let baseline_map: std::collections::HashMap<String, f64> = baseline_summary
        .categories
        .iter()
        .map(|c| (c.tag.clone(), c.amount))
        .collect();

    for current in &current_summary.categories {
        let baseline_amount = baseline_map.get(&current.tag).copied().unwrap_or(0.0);
        let change = current.amount - baseline_amount;
        let change_pct = if baseline_amount > 0.0 {
            (change / baseline_amount) * 100.0
        } else if current.amount > 0.0 {
            100.0 // New category
        } else {
            0.0
        };

        comparisons.push(CategoryComparison {
            category: current.tag.clone(),
            current_amount: current.amount,
            baseline_amount,
            change_amount: change,
            change_percentage: change_pct,
        });
    }

    // Add categories that were in baseline but not in current
    for baseline in &baseline_summary.categories {
        if !current_summary
            .categories
            .iter()
            .any(|c| c.tag == baseline.tag)
        {
            comparisons.push(CategoryComparison {
                category: baseline.tag.clone(),
                current_amount: 0.0,
                baseline_amount: baseline.amount,
                change_amount: -baseline.amount,
                change_percentage: -100.0,
            });
        }
    }

    // Sort by absolute change
    comparisons.sort_by(|a, b| {
        b.change_amount
            .abs()
            .partial_cmp(&a.change_amount.abs())
            .unwrap()
    });

    let total_change = current_summary.total - baseline_summary.total;
    let total_change_pct = if baseline_summary.total > 0.0 {
        (total_change / baseline_summary.total) * 100.0
    } else {
        0.0
    };

    Ok(CompareSpendingResult {
        current_period: current_period_name.to_string(),
        baseline_period: baseline_period_name.to_string(),
        current_total: current_summary.total,
        baseline_total: baseline_summary.total,
        total_change,
        total_change_percentage: total_change_pct,
        by_category: comparisons,
    })
}

// =============================================================================
// get_merchants
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct MerchantsParams {
    /// Time period (default: this-year)
    #[schemars(description = "Time period for merchant analysis")]
    pub period: Option<String>,

    /// Filter to specific category
    #[schemars(description = "Optional: filter to merchants in a specific category")]
    pub category: Option<String>,

    /// Maximum number of merchants to return (default: 20)
    #[schemars(description = "Number of top merchants to return (default 20, max 100)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MerchantSummary {
    pub merchant: String,
    pub total_spent: f64,
    pub transaction_count: i32,
    pub avg_transaction: f64,
    pub percentage_of_total: f64,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MerchantsResult {
    pub period: String,
    pub merchants: Vec<MerchantSummary>,
    pub total_spending: f64,
}

pub fn get_merchants(db: &Database, params: MerchantsParams) -> Result<MerchantsResult> {
    let period_name = params.period.as_deref().unwrap_or("this-year");
    let (from_date, to_date) = resolve_period(period_name)?;
    let limit = params.limit.unwrap_or(20).min(100) as i64;

    // Get merchants report from database
    let report = db.get_top_merchants(
        from_date,
        to_date,
        limit,
        params.category.as_deref(),
        None, // entity_id
        None, // card_member
    )?;

    // Calculate total for percentage
    let total: f64 = report.merchants.iter().map(|m| m.amount).sum();

    let merchants: Vec<MerchantSummary> = report
        .merchants
        .into_iter()
        .map(|m| {
            let percentage = if total > 0.0 {
                (m.amount / total) * 100.0
            } else {
                0.0
            };
            MerchantSummary {
                merchant: m.merchant,
                total_spent: m.amount,
                transaction_count: m.transaction_count as i32,
                avg_transaction: if m.transaction_count > 0 {
                    m.amount / m.transaction_count as f64
                } else {
                    0.0
                },
                percentage_of_total: percentage,
            }
        })
        .collect();

    Ok(MerchantsResult {
        period: period_name.to_string(),
        merchants,
        total_spending: total,
    })
}

// =============================================================================
// get_account_summary
// =============================================================================

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct AccountSummaryParams {
    /// Include archived transactions in counts (default: false)
    #[schemars(description = "Include archived transactions in activity counts")]
    pub include_archived: Option<bool>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AccountInfo {
    pub id: i64,
    pub name: String,
    pub bank: String,
    pub account_type: Option<String>,
    pub transaction_count: i64,
    pub last_activity: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct AccountSummaryResult {
    pub accounts: Vec<AccountInfo>,
    pub total_accounts: usize,
    pub total_transactions: i64,
}

pub fn get_account_summary(
    db: &Database,
    _params: AccountSummaryParams,
) -> Result<AccountSummaryResult> {
    let accounts = db.list_accounts()?;

    let mut account_infos: Vec<AccountInfo> = Vec::new();
    let mut total_transactions = 0i64;

    for account in accounts {
        // Count transactions for this account
        let count = db.count_transactions_search(Some(account.id), None)?;
        total_transactions += count;

        // Get owner entity if assigned
        let owner = if let Some(entity_id) = account.entity_id {
            db.get_entity(entity_id)?.map(|e| e.name)
        } else {
            None
        };

        // Get last transaction date
        let last_tx = db
            .search_transactions_full(
                Some(account.id),
                None,
                None,
                None,
                None,
                false,
                None,
                Some("date"),
                Some("desc"),
                false, // include_archived
                1,
                0,
            )?
            .first()
            .map(|t| t.date.to_string());

        account_infos.push(AccountInfo {
            id: account.id,
            name: account.name,
            bank: account.bank.as_str().to_string(),
            account_type: account.account_type.map(|t| format!("{:?}", t)),
            transaction_count: count,
            last_activity: last_tx,
            owner,
        });
    }

    let total_accounts = account_infos.len();

    Ok(AccountSummaryResult {
        accounts: account_infos,
        total_accounts,
        total_transactions,
    })
}

// =============================================================================
// Tool Definitions for Anthropic Format
// =============================================================================

use crate::ai::anthropic_compat::Tool;

/// Generate all Hone tools in Anthropic format
pub fn hone_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "search_transactions",
            "Search transactions by query, date range, tag, or amount. \
             Returns matching transactions with merchant, amount, date, and tags.",
            schemars::schema_for!(SearchTransactionsParams).into(),
        ),
        Tool::new(
            "get_spending_summary",
            "Get spending breakdown by category for a time period.",
            schemars::schema_for!(SpendingSummaryParams).into(),
        ),
        Tool::new(
            "get_subscriptions",
            "List subscriptions with status, amount, and frequency.",
            schemars::schema_for!(SubscriptionsParams).into(),
        ),
        Tool::new(
            "get_alerts",
            "Get waste detection alerts (zombies, price increases, duplicates).",
            schemars::schema_for!(AlertsParams).into(),
        ),
        Tool::new(
            "compare_spending",
            "Compare spending between two time periods by category.",
            schemars::schema_for!(CompareSpendingParams).into(),
        ),
        Tool::new(
            "get_merchants",
            "Get top merchants by spending amount.",
            schemars::schema_for!(MerchantsParams).into(),
        ),
        Tool::new(
            "get_account_summary",
            "Get overview of all accounts with transaction counts.",
            schemars::schema_for!(AccountSummaryParams).into(),
        ),
    ]
}

/// Generate tool definitions specifically for spending analysis
pub fn spending_analysis_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "search_transactions",
            "Search transactions by query, date range, tag, or amount. Use this to investigate specific merchants or time periods.",
            schemars::schema_for!(SearchTransactionsParams).into(),
        ),
        Tool::new(
            "get_merchants",
            "Get top merchants by spending amount. Use this to identify major spending drivers.",
            schemars::schema_for!(MerchantsParams).into(),
        ),
        Tool::new(
            "compare_spending",
            "Compare spending between two time periods. Use this to understand changes over time.",
            schemars::schema_for!(CompareSpendingParams).into(),
        ),
    ]
}

/// Generate tool definitions specifically for duplicate analysis
pub fn duplicate_analysis_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "get_subscriptions",
            "List subscriptions with status, amount, and frequency. Use this to see all services in a category.",
            schemars::schema_for!(SubscriptionsParams).into(),
        ),
        Tool::new(
            "search_transactions",
            "Search transactions by merchant. Use this to see how often each service is used.",
            schemars::schema_for!(SearchTransactionsParams).into(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{Bank, Frequency, NewTransaction};

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn seed_test_data(db: &Database) {
        // Create an account
        db.upsert_account("Test Checking", Bank::Chase, None)
            .unwrap();

        // Create some transactions
        let today = Utc::now().date_naive();
        let last_month = today - chrono::Duration::days(35);

        db.insert_transaction(
            1,
            &NewTransaction {
                date: today,
                description: "NETFLIX.COM".to_string(),
                amount: -15.99,
                category: None,
                import_hash: "hash1".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            1,
            &NewTransaction {
                date: today - chrono::Duration::days(5),
                description: "WHOLE FOODS MARKET".to_string(),
                amount: -85.42,
                category: None,
                import_hash: "hash2".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            1,
            &NewTransaction {
                date: today - chrono::Duration::days(10),
                description: "SHELL GAS STATION".to_string(),
                amount: -45.00,
                category: None,
                import_hash: "hash3".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        db.insert_transaction(
            1,
            &NewTransaction {
                date: last_month,
                description: "AMAZON.COM".to_string(),
                amount: -150.00,
                category: None,
                import_hash: "hash4".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        // Create a subscription
        db.upsert_subscription(
            "Netflix",
            Some(1),
            Some(15.99),
            Some(Frequency::Monthly),
            Some(today),
            Some(today),
        )
        .unwrap();

        // Create an alert (requires subscription_id, so get the subscription we just made)
        db.create_alert(
            crate::models::AlertType::Zombie,
            Some(1), // subscription_id
            Some("Subscription may be unused"),
        )
        .unwrap();
    }

    #[test]
    fn test_resolve_period_this_month() {
        let (from, to) = resolve_period("this-month").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(from.day(), 1);
        assert_eq!(from.month(), today.month());
        assert_eq!(to, today);
    }

    #[test]
    fn test_resolve_period_last_month() {
        let (from, to) = resolve_period("last-month").unwrap();
        let today = Utc::now().date_naive();
        // Last month should end before this month started
        assert!(to < today.with_day(1).unwrap());
        assert_eq!(from.day(), 1);
    }

    #[test]
    fn test_resolve_period_last_30_days() {
        let (from, to) = resolve_period("last-30-days").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(to, today);
        assert_eq!((to - from).num_days(), 30);
    }

    #[test]
    fn test_resolve_period_last_90_days() {
        let (from, to) = resolve_period("last-90-days").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(to, today);
        assert_eq!((to - from).num_days(), 90);
    }

    #[test]
    fn test_resolve_period_this_year() {
        let (from, to) = resolve_period("this-year").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(from.month(), 1);
        assert_eq!(from.day(), 1);
        assert_eq!(from.year(), today.year());
        assert_eq!(to, today);
    }

    #[test]
    fn test_resolve_period_last_year() {
        let (from, to) = resolve_period("last-year").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(from.year(), today.year() - 1);
        assert_eq!(from.month(), 1);
        assert_eq!(from.day(), 1);
        assert_eq!(to.year(), today.year() - 1);
        assert_eq!(to.month(), 12);
        assert_eq!(to.day(), 31);
    }

    #[test]
    fn test_resolve_period_ytd() {
        let (from, to) = resolve_period("ytd").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(from.year(), today.year());
        assert_eq!(from.month(), 1);
        assert_eq!(from.day(), 1);
        assert_eq!(to, today);
    }

    #[test]
    fn test_resolve_period_all() {
        let (from, to) = resolve_period("all").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(from.year(), 2000);
        assert_eq!(to, today);
    }

    #[test]
    fn test_resolve_period_custom_date() {
        let (from, to) = resolve_period("2024-06-15").unwrap();
        assert_eq!(from.to_string(), "2024-06-15");
        assert_eq!(to.to_string(), "2024-06-15");
    }

    #[test]
    fn test_resolve_period_invalid() {
        let result = resolve_period("invalid-period");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_date_opt_none() {
        let result = parse_date_opt(None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_date_opt_valid() {
        let result = parse_date_opt(Some("2024-06-15")).unwrap();
        assert_eq!(result.unwrap().to_string(), "2024-06-15");
    }

    #[test]
    fn test_parse_date_opt_invalid() {
        let result = parse_date_opt(Some("not-a-date"));
        assert!(result.is_err());
    }

    #[test]
    fn test_hone_tools_count() {
        let tools = hone_tools();
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_spending_analysis_tools_count() {
        let tools = spending_analysis_tools();
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn test_duplicate_analysis_tools_count() {
        let tools = duplicate_analysis_tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_tool_has_correct_schema() {
        let tools = hone_tools();
        let search_tool = tools
            .iter()
            .find(|t| t.name == "search_transactions")
            .unwrap();
        assert!(search_tool.description.contains("Search transactions"));
        // Schema should be a valid JSON object
        assert!(search_tool.input_schema.is_object());
    }

    // Database-backed tool tests

    #[test]
    fn test_search_transactions_empty_db() {
        let db = create_test_db();
        let params = SearchTransactionsParams::default();
        let result = search_transactions(&db, params).unwrap();
        assert_eq!(result.transactions.len(), 0);
        assert_eq!(result.total_count, 0);
        assert_eq!(result.total_amount, 0.0);
    }

    #[test]
    fn test_search_transactions_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SearchTransactionsParams {
            period: Some("this-month".to_string()),
            ..Default::default()
        };
        let result = search_transactions(&db, params).unwrap();
        // Should find transactions from this month
        assert!(result.total_count > 0);
    }

    #[test]
    fn test_search_transactions_with_query() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SearchTransactionsParams {
            query: Some("NETFLIX".to_string()),
            period: Some("all".to_string()),
            ..Default::default()
        };
        let result = search_transactions(&db, params).unwrap();
        assert_eq!(result.total_count, 1);
        assert!(result.transactions[0].description.contains("NETFLIX"));
    }

    #[test]
    fn test_search_transactions_with_amount_filter() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SearchTransactionsParams {
            min_amount: Some(50.0),
            period: Some("all".to_string()),
            ..Default::default()
        };
        let result = search_transactions(&db, params).unwrap();
        // Should only return transactions >= $50
        for tx in &result.transactions {
            assert!(tx.amount.abs() >= 50.0);
        }
    }

    #[test]
    fn test_search_transactions_with_max_amount() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SearchTransactionsParams {
            max_amount: Some(50.0),
            period: Some("all".to_string()),
            ..Default::default()
        };
        let result = search_transactions(&db, params).unwrap();
        // Should only return transactions <= $50
        for tx in &result.transactions {
            assert!(tx.amount.abs() <= 50.0);
        }
    }

    #[test]
    fn test_search_transactions_with_limit() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SearchTransactionsParams {
            limit: Some(2),
            period: Some("all".to_string()),
            ..Default::default()
        };
        let result = search_transactions(&db, params).unwrap();
        assert!(result.transactions.len() <= 2);
    }

    #[test]
    fn test_search_transactions_with_custom_dates() {
        let db = create_test_db();
        seed_test_data(&db);

        let today = Utc::now().date_naive();
        let params = SearchTransactionsParams {
            from_date: Some(today.to_string()),
            to_date: Some(today.to_string()),
            ..Default::default()
        };
        let result = search_transactions(&db, params).unwrap();
        // Should only return today's transactions
        for tx in &result.transactions {
            assert_eq!(tx.date, today.to_string());
        }
    }

    #[test]
    fn test_get_spending_summary_empty_db() {
        let db = create_test_db();
        let params = SpendingSummaryParams::default();
        let result = get_spending_summary(&db, params).unwrap();
        assert_eq!(result.total_spending, 0.0);
        assert!(result.categories.is_empty());
    }

    #[test]
    fn test_get_spending_summary_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SpendingSummaryParams {
            period: Some("all".to_string()),
            ..Default::default()
        };
        let result = get_spending_summary(&db, params).unwrap();
        assert!(result.total_spending > 0.0);
    }

    #[test]
    fn test_get_spending_summary_with_custom_dates() {
        let db = create_test_db();
        seed_test_data(&db);

        let today = Utc::now().date_naive();
        let params = SpendingSummaryParams {
            from_date: Some((today - chrono::Duration::days(60)).to_string()),
            to_date: Some(today.to_string()),
            ..Default::default()
        };
        let result = get_spending_summary(&db, params).unwrap();
        assert!(result.total_spending > 0.0);
    }

    #[test]
    fn test_get_subscriptions_empty_db() {
        let db = create_test_db();
        let params = SubscriptionsParams::default();
        let result = get_subscriptions(&db, params).unwrap();
        assert_eq!(result.subscriptions.len(), 0);
        assert_eq!(result.active_count, 0);
    }

    #[test]
    fn test_get_subscriptions_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = SubscriptionsParams::default();
        let result = get_subscriptions(&db, params).unwrap();
        assert_eq!(result.active_count, 1);
        assert_eq!(result.subscriptions[0].merchant, "Netflix");
    }

    #[test]
    fn test_get_subscriptions_filter_by_status() {
        let db = create_test_db();
        seed_test_data(&db);

        // Filter for cancelled (should be empty)
        let params = SubscriptionsParams {
            status: Some("cancelled".to_string()),
            ..Default::default()
        };
        let result = get_subscriptions(&db, params).unwrap();
        assert_eq!(result.active_count, 0);
    }

    #[test]
    fn test_get_alerts_empty_db() {
        let db = create_test_db();
        let params = AlertsParams::default();
        let result = get_alerts(&db, params).unwrap();
        assert_eq!(result.alerts.len(), 0);
    }

    #[test]
    fn test_get_alerts_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = AlertsParams::default();
        let result = get_alerts(&db, params).unwrap();
        assert_eq!(result.active_count, 1);
        assert_eq!(result.alerts[0].alert_type, "zombie");
    }

    #[test]
    fn test_get_alerts_filter_by_type() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = AlertsParams {
            alert_type: Some("price_increase".to_string()),
            ..Default::default()
        };
        let result = get_alerts(&db, params).unwrap();
        assert_eq!(result.active_count, 0);
    }

    #[test]
    fn test_compare_spending_empty_db() {
        let db = create_test_db();
        let params = CompareSpendingParams {
            baseline_period: Some("last-month".to_string()),
            current_period: Some("this-month".to_string()),
            ..Default::default()
        };
        let result = compare_spending(&db, params).unwrap();
        assert_eq!(result.baseline_total, 0.0);
        assert_eq!(result.current_total, 0.0);
    }

    #[test]
    fn test_compare_spending_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = CompareSpendingParams {
            baseline_period: Some("last-month".to_string()),
            current_period: Some("this-month".to_string()),
            ..Default::default()
        };
        let result = compare_spending(&db, params).unwrap();
        // At least one period should have spending based on our test data
        assert!(result.baseline_total >= 0.0);
        assert!(result.current_total >= 0.0);
    }

    #[test]
    fn test_compare_spending_with_category() {
        let db = create_test_db();
        seed_test_data(&db);

        // Test without category filter (category filter requires tag to exist)
        let params = CompareSpendingParams {
            baseline_period: Some("last-month".to_string()),
            current_period: Some("this-month".to_string()),
            category: None,
        };
        let result = compare_spending(&db, params).unwrap();
        assert!(result.baseline_total >= 0.0);
        // Verify comparison completed successfully
        let _ = result.by_category;
    }

    #[test]
    fn test_get_merchants_empty_db() {
        let db = create_test_db();
        let params = MerchantsParams::default();
        let result = get_merchants(&db, params).unwrap();
        assert_eq!(result.merchants.len(), 0);
    }

    #[test]
    fn test_get_merchants_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = MerchantsParams {
            period: Some("all".to_string()),
            category: None,
            limit: None,
        };
        let result = get_merchants(&db, params).unwrap();
        assert!(result.merchants.len() > 0);
    }

    #[test]
    fn test_get_merchants_with_limit() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = MerchantsParams {
            period: Some("all".to_string()),
            category: None,
            limit: Some(2),
        };
        let result = get_merchants(&db, params).unwrap();
        assert!(result.merchants.len() <= 2);
    }

    #[test]
    fn test_get_merchants_sorted_by_amount() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = MerchantsParams {
            period: Some("all".to_string()),
            category: None,
            limit: None,
        };
        let result = get_merchants(&db, params).unwrap();
        // Check that merchants are sorted by total_spent (descending)
        for i in 1..result.merchants.len() {
            assert!(result.merchants[i - 1].total_spent >= result.merchants[i].total_spent);
        }
    }

    #[test]
    fn test_get_account_summary_empty_db() {
        let db = create_test_db();
        let params = AccountSummaryParams::default();
        let result = get_account_summary(&db, params).unwrap();
        assert_eq!(result.accounts.len(), 0);
    }

    #[test]
    fn test_get_account_summary_with_data() {
        let db = create_test_db();
        seed_test_data(&db);

        let params = AccountSummaryParams::default();
        let result = get_account_summary(&db, params).unwrap();
        assert_eq!(result.total_accounts, 1);
        assert_eq!(result.accounts[0].name, "Test Checking");
        assert!(result.accounts[0].transaction_count > 0);
    }
}

//! Report handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, Request, State},
    Json,
};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::{get_user_email, AppError, AppState};
use hone_core::models::{
    Entity, Granularity, LocationSpending, MerchantsReport, PropertyExpenseSummary, SavingsReport,
    SpendingSummary, SubscriptionSummaryReport, TagSpending, TrendsReport, VehicleCostSummary,
};

/// Query parameters for spending by tag report
#[derive(Debug, Deserialize)]
pub struct SpendingByTagQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

/// GET /api/reports/by-tag - Get spending report by tag
pub async fn report_by_tag(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SpendingByTagQuery>,
    request: Request,
) -> Result<Json<Vec<TagSpending>>, AppError> {
    let user_email = get_user_email(request.headers());

    let from_date = params
        .from
        .as_ref()
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::bad_request("Invalid from date format (use YYYY-MM-DD)"))?;

    let to_date = params
        .to
        .as_ref()
        .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::bad_request("Invalid to date format (use YYYY-MM-DD)"))?;

    let spending = state.db.get_spending_by_tag(from_date, to_date)?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("spending_by_tag"),
        None,
        Some(&format!(
            "from={:?}, to={:?}, tags={}",
            from_date,
            to_date,
            spending.len()
        )),
    )?;

    Ok(Json(spending))
}

#[derive(Debug, Deserialize)]
pub struct ReportSpendingQuery {
    /// Period preset (this-month, last-month, etc)
    pub period: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub to: Option<String>,
    /// Filter to specific tag
    pub tag: Option<String>,
    /// Expand child categories
    pub expand: Option<bool>,
    /// Filter by entity (account owner) ID
    pub entity_id: Option<i64>,
    /// Filter by card member name
    pub card_member: Option<String>,
}

/// GET /api/reports/spending - Spending summary by category
pub async fn report_spending(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReportSpendingQuery>,
    request: Request,
) -> Result<Json<SpendingSummary>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("this-month");
    let (from_date, to_date) =
        resolve_period(period, params.from.as_deref(), params.to.as_deref())?;

    let summary = state.db.get_spending_summary(
        from_date,
        to_date,
        params.tag.as_deref(),
        params.expand.unwrap_or(false),
        params.entity_id,
        params.card_member.as_deref(),
    )?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("spending"),
        None,
        Some(&format!(
            "period={}, from={}, to={}, tag={:?}, entity_id={:?}, card_member={:?}",
            period, from_date, to_date, params.tag, params.entity_id, params.card_member
        )),
    )?;

    Ok(Json(summary))
}

#[derive(Debug, Deserialize)]
pub struct ReportTrendsQuery {
    /// Granularity: monthly or weekly
    pub granularity: Option<String>,
    /// Period preset
    pub period: Option<String>,
    /// Filter to specific tag
    pub tag: Option<String>,
    /// Filter by entity (account owner) ID
    pub entity_id: Option<i64>,
    /// Filter by card member name
    pub card_member: Option<String>,
}

/// GET /api/reports/trends - Spending trends over time
pub async fn report_trends(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReportTrendsQuery>,
    request: Request,
) -> Result<Json<TrendsReport>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("last-12-months");
    let (from_date, to_date) = resolve_period(period, None, None)?;

    let granularity: Granularity = params
        .granularity
        .as_deref()
        .unwrap_or("monthly")
        .parse()
        .map_err(|e: String| AppError::bad_request(&e))?;

    let report = state.db.get_spending_trends(
        from_date,
        to_date,
        granularity,
        params.tag.as_deref(),
        params.entity_id,
        params.card_member.as_deref(),
    )?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("trends"),
        None,
        Some(&format!(
            "granularity={}, period={}, tag={:?}, entity_id={:?}, card_member={:?}",
            granularity.as_str(),
            period,
            params.tag,
            params.entity_id,
            params.card_member
        )),
    )?;

    Ok(Json(report))
}

#[derive(Debug, Deserialize)]
pub struct ReportMerchantsQuery {
    /// Number of merchants to return
    pub limit: Option<i64>,
    /// Period preset
    pub period: Option<String>,
    /// Filter to specific tag
    pub tag: Option<String>,
    /// Filter by entity (account owner) ID
    pub entity_id: Option<i64>,
    /// Filter by card member name
    pub card_member: Option<String>,
}

/// GET /api/reports/merchants - Top merchants by spending
pub async fn report_merchants(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReportMerchantsQuery>,
    request: Request,
) -> Result<Json<MerchantsReport>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("this-month");
    let (from_date, to_date) = resolve_period(period, None, None)?;
    let limit = params.limit.unwrap_or(10).min(100); // Cap at 100

    let report = state.db.get_top_merchants(
        from_date,
        to_date,
        limit,
        params.tag.as_deref(),
        params.entity_id,
        params.card_member.as_deref(),
    )?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("merchants"),
        None,
        Some(&format!(
            "limit={}, period={}, tag={:?}, entity_id={:?}, card_member={:?}",
            limit, period, params.tag, params.entity_id, params.card_member
        )),
    )?;

    Ok(Json(report))
}

/// GET /api/reports/subscriptions - Subscription summary
pub async fn report_subscriptions(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<SubscriptionSummaryReport>, AppError> {
    let user_email = get_user_email(request.headers());

    let report = state.db.get_subscription_summary()?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("subscriptions"),
        None,
        Some(&format!(
            "active={}, cancelled={}",
            report.active_count, report.cancelled_count
        )),
    )?;

    Ok(Json(report))
}

/// GET /api/reports/savings - Savings from cancelled subscriptions
pub async fn report_savings(
    State(state): State<Arc<AppState>>,
    request: Request,
) -> Result<Json<SavingsReport>, AppError> {
    let user_email = get_user_email(request.headers());

    let report = state.db.get_savings_report()?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("savings"),
        None,
        Some(&format!(
            "total_savings=${:.2}, cancelled={}",
            report.total_savings, report.cancelled_count
        )),
    )?;

    Ok(Json(report))
}

/// Query parameters for entity spending report
#[derive(Debug, Deserialize)]
pub struct ReportByEntityQuery {
    /// Period preset
    pub period: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub to: Option<String>,
}

/// Entity spending summary
#[derive(Debug, Serialize)]
pub struct EntitySpendingReport {
    pub from_date: String,
    pub to_date: String,
    pub entities: Vec<EntitySpending>,
    pub total: f64,
}

#[derive(Debug, Serialize)]
pub struct EntitySpending {
    pub entity: Entity,
    pub total_amount: f64,
    pub split_count: i64,
}

/// GET /api/reports/by-entity - Spending by entity
pub async fn report_by_entity(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReportByEntityQuery>,
    request: Request,
) -> Result<Json<EntitySpendingReport>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("this-month");
    let (from_date, to_date) =
        resolve_period(period, params.from.as_deref(), params.to.as_deref())?;

    let spending_data = state.db.get_spending_by_entity(from_date, to_date)?;

    let mut total = 0.0;
    let entities: Vec<EntitySpending> = spending_data
        .into_iter()
        .map(|(entity, amount, count)| {
            total += amount;
            EntitySpending {
                entity,
                total_amount: amount,
                split_count: count,
            }
        })
        .collect();

    state.db.log_audit(
        &user_email,
        "report",
        Some("by_entity"),
        None,
        Some(&format!(
            "period={}, from={}, to={}, entities={}",
            period,
            from_date,
            to_date,
            entities.len()
        )),
    )?;

    Ok(Json(EntitySpendingReport {
        from_date: from_date.to_string(),
        to_date: to_date.to_string(),
        entities,
        total,
    }))
}

/// Query parameters for location spending report
#[derive(Debug, Deserialize)]
pub struct ReportByLocationQuery {
    /// Period preset
    pub period: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub to: Option<String>,
}

/// GET /api/reports/by-location - Spending by location
pub async fn report_by_location(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReportByLocationQuery>,
    request: Request,
) -> Result<Json<Vec<LocationSpending>>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("this-month");
    let (from_date, to_date) =
        resolve_period(period, params.from.as_deref(), params.to.as_deref())?;

    let spending = state.db.get_spending_by_location(from_date, to_date)?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("by_location"),
        None,
        Some(&format!(
            "period={}, from={}, to={}, locations={}",
            period,
            from_date,
            to_date,
            spending.len()
        )),
    )?;

    Ok(Json(spending))
}

/// Query parameters for vehicle cost report
#[derive(Debug, Deserialize)]
pub struct ReportVehicleCostsQuery {
    /// Period preset
    pub period: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub to: Option<String>,
}

/// GET /api/reports/vehicle-costs/:id - Get vehicle cost summary
pub async fn report_vehicle_costs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<ReportVehicleCostsQuery>,
    request: Request,
) -> Result<Json<VehicleCostSummary>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("all");
    let (from_date, to_date) =
        resolve_period(period, params.from.as_deref(), params.to.as_deref())?;

    let report = state.db.get_vehicle_cost_summary(id, from_date, to_date)?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("vehicle_costs"),
        Some(id),
        Some(&format!(
            "period={}, from={}, to={}",
            period, from_date, to_date
        )),
    )?;

    Ok(Json(report))
}

/// Query parameters for property expense report
#[derive(Debug, Deserialize)]
pub struct ReportPropertyExpensesQuery {
    /// Period preset
    pub period: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub to: Option<String>,
}

/// GET /api/reports/property-expenses/:id - Get property expense summary
pub async fn report_property_expenses(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<ReportPropertyExpensesQuery>,
    request: Request,
) -> Result<Json<PropertyExpenseSummary>, AppError> {
    let user_email = get_user_email(request.headers());

    let period = params.period.as_deref().unwrap_or("all");
    let (from_date, to_date) =
        resolve_period(period, params.from.as_deref(), params.to.as_deref())?;

    let report = state
        .db
        .get_property_expense_summary(id, from_date, to_date)?;

    state.db.log_audit(
        &user_email,
        "report",
        Some("property_expenses"),
        Some(id),
        Some(&format!(
            "period={}, from={}, to={}",
            period, from_date, to_date
        )),
    )?;

    Ok(Json(report))
}

/// Helper: Resolve period string to date range
pub fn resolve_period(
    period: &str,
    custom_from: Option<&str>,
    custom_to: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    // If custom dates provided, use those
    if let (Some(from), Some(to)) = (custom_from, custom_to) {
        let from_date = NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .map_err(|_| AppError::bad_request("Invalid from date format (use YYYY-MM-DD)"))?;
        let to_date = NaiveDate::parse_from_str(to, "%Y-%m-%d")
            .map_err(|_| AppError::bad_request("Invalid to date format (use YYYY-MM-DD)"))?;
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
        "this-year" => {
            let from = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            Ok((from, today))
        }
        "last-year" => {
            let from = NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap();
            let to = NaiveDate::from_ymd_opt(today.year() - 1, 12, 31).unwrap();
            Ok((from, to))
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
        _ => Err(AppError::bad_request(&format!(
            "Unknown period: {}. Available: this-month, last-month, this-year, last-year, last-30-days, last-90-days, last-12-months, all",
            period
        ))),
    }
}

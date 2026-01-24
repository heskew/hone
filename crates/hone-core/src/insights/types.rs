//! Core types for the Insight Engine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Types of insights that can be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightType {
    /// Explains spending changes compared to baseline
    SpendingExplainer,
    /// Forecasts upcoming expenses
    ExpenseForecaster,
    /// Identifies potential savings opportunities
    SavingsOpportunity,
}

impl InsightType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InsightType::SpendingExplainer => "spending_explainer",
            InsightType::ExpenseForecaster => "expense_forecaster",
            InsightType::SavingsOpportunity => "savings_opportunity",
        }
    }
}

impl fmt::Display for InsightType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for InsightType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "spending_explainer" => Ok(InsightType::SpendingExplainer),
            "expense_forecaster" => Ok(InsightType::ExpenseForecaster),
            "savings_opportunity" => Ok(InsightType::SavingsOpportunity),
            _ => Err(format!("Unknown insight type: {}", s)),
        }
    }
}

/// Severity level of an insight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational - no action needed
    Info,
    /// Worth attention but not urgent
    Attention,
    /// Should be addressed soon
    Warning,
    /// Requires immediate attention
    Alert,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Attention => "attention",
            Severity::Warning => "warning",
            Severity::Alert => "alert",
        }
    }

    /// Numeric priority for sorting (higher = more urgent)
    pub fn priority(&self) -> u8 {
        match self {
            Severity::Info => 1,
            Severity::Attention => 2,
            Severity::Warning => 3,
            Severity::Alert => 4,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "info" => Ok(Severity::Info),
            "attention" => Ok(Severity::Attention),
            "warning" => Ok(Severity::Warning),
            "alert" => Ok(Severity::Alert),
            _ => Err(format!("Unknown severity: {}", s)),
        }
    }
}

/// Status of an insight finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightStatus {
    /// Currently active and should be displayed
    Active,
    /// User dismissed this insight
    Dismissed,
    /// User snoozed this insight temporarily
    Snoozed,
}

impl InsightStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            InsightStatus::Active => "active",
            InsightStatus::Dismissed => "dismissed",
            InsightStatus::Snoozed => "snoozed",
        }
    }
}

impl fmt::Display for InsightStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for InsightStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(InsightStatus::Active),
            "dismissed" => Ok(InsightStatus::Dismissed),
            "snoozed" => Ok(InsightStatus::Snoozed),
            _ => Err(format!("Unknown insight status: {}", s)),
        }
    }
}

/// A finding produced by an insight analyzer (before persistence)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Type of insight that generated this finding
    pub insight_type: InsightType,
    /// Unique key for deduplication (e.g., "savings:subscription:123")
    pub key: String,
    /// How urgent/important this finding is
    pub severity: Severity,
    /// Short title for the finding (e.g., "Unused Subscription")
    pub title: String,
    /// One-line summary (e.g., "Netflix hasn't been used in 45 days")
    pub summary: String,
    /// Optional longer explanation with details
    pub detail: Option<String>,
    /// Insight-specific structured data
    pub data: serde_json::Value,
    /// When this finding was detected
    pub detected_at: DateTime<Utc>,
    /// Optional expiration (e.g., forecast only valid until end of month)
    pub expires_at: Option<DateTime<Utc>>,
}

impl Finding {
    /// Create a new finding with the current timestamp
    pub fn new(
        insight_type: InsightType,
        key: impl Into<String>,
        severity: Severity,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            insight_type,
            key: key.into(),
            severity,
            title: title.into(),
            summary: summary.into(),
            detail: None,
            data: serde_json::Value::Null,
            detected_at: Utc::now(),
            expires_at: None,
        }
    }

    /// Add optional detail text
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Add structured data payload
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Set expiration time
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}

/// A persisted insight finding from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightFinding {
    pub id: i64,
    pub insight_type: InsightType,
    pub finding_key: String,
    pub severity: Severity,
    pub title: String,
    pub summary: String,
    pub detail: Option<String>,
    pub data: serde_json::Value,
    pub first_detected_at: DateTime<Utc>,
    pub last_detected_at: DateTime<Utc>,
    pub status: InsightStatus,
    pub snoozed_until: Option<DateTime<Utc>>,
    pub user_feedback: Option<String>,
}

/// Data for spending explainer insight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingExplainerData {
    pub tag_id: i64,
    pub tag_name: String,
    pub current_amount: f64,
    pub baseline_amount: f64,
    pub percent_change: f64,
    pub explanation: Option<String>,
    pub top_merchants: Vec<MerchantContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantContribution {
    pub merchant: String,
    pub current: f64,
    pub baseline: f64,
    pub change: f64,
}

/// Data for expense forecaster insight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseForecasterData {
    pub period_days: u32,
    pub total_expected: f64,
    pub items: Vec<ForecastItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastItem {
    pub item_type: ForecastItemType,
    pub name: String,
    pub amount: f64,
    pub due_date: Option<String>,
    pub basis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastItemType {
    Subscription,
    Estimate,
    LargeExpense,
}

/// Data for savings opportunity insight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsOpportunityData {
    pub opportunity_type: SavingsOpportunityType,
    pub subscription_id: Option<i64>,
    pub subscription_name: Option<String>,
    pub monthly_amount: f64,
    pub annual_savings: f64,
    pub reason: String,
    pub alert_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavingsOpportunityType {
    /// Subscription with no recent activity
    Zombie,
    /// Multiple subscriptions in same category
    Duplicate,
    /// Could save by switching to annual billing
    AnnualSwitch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insight_type_serialization() {
        assert_eq!(
            InsightType::SpendingExplainer.as_str(),
            "spending_explainer"
        );
        assert_eq!(
            InsightType::from_str("expense_forecaster").unwrap(),
            InsightType::ExpenseForecaster
        );
    }

    #[test]
    fn test_severity_priority() {
        assert!(Severity::Alert.priority() > Severity::Warning.priority());
        assert!(Severity::Warning.priority() > Severity::Attention.priority());
        assert!(Severity::Attention.priority() > Severity::Info.priority());
    }

    #[test]
    fn test_finding_builder() {
        let finding = Finding::new(
            InsightType::SavingsOpportunity,
            "test:key",
            Severity::Warning,
            "Test Title",
            "Test summary",
        )
        .with_detail("More details here")
        .with_data(serde_json::json!({"amount": 10.0}));

        assert_eq!(finding.key, "test:key");
        assert_eq!(finding.detail.unwrap(), "More details here");
        assert_eq!(finding.data["amount"], 10.0);
    }
}

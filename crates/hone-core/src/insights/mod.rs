//! Insight Engine - Proactive Financial Insights
//!
//! The Insight Engine is a pluggable system that proactively surfaces financial
//! insights. Instead of waiting for users to ask the right questions, it
//! continuously analyzes spending data and surfaces what's interesting,
//! actionable, or concerning.
//!
//! ## Core Insight Types
//!
//! - **Spending Explainer** - Explains spending changes vs baseline
//! - **Expense Forecaster** - Predicts upcoming expenses
//! - **Savings Opportunity** - Identifies ways to reduce spending
//!
//! ## Usage
//!
//! ```rust,ignore
//! use hone_core::insights::{InsightEngine, AnalysisContext};
//!
//! let engine = InsightEngine::new();
//! let ctx = AnalysisContext::current_month(&db, ollama.as_ref());
//! let findings = engine.analyze_all(&ctx)?;
//! ```

pub mod engine;
pub mod expense_forecaster;
pub mod savings_opportunity;
pub mod spending_explainer;
pub mod types;

pub use engine::{AnalysisContext, Insight, InsightEngine};
pub use expense_forecaster::ExpenseForecasterInsight;
pub use savings_opportunity::SavingsOpportunityInsight;
pub use spending_explainer::SpendingExplainerInsight;
pub use types::{
    ExpenseForecasterData, Finding, ForecastItem, ForecastItemType, InsightFinding, InsightStatus,
    InsightType, MerchantContribution, SavingsOpportunityData, SavingsOpportunityType, Severity,
    SpendingExplainerData,
};

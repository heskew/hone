//! MCP Tool implementations for Hone
//!
//! Re-exports from hone_core::tools for MCP server use.
//! The actual implementations live in hone-core so they can be shared
//! with the AI Orchestrator.

// Re-export all tool types and functions from hone-core
pub use hone_core::tools::{
    // Functions
    compare_spending,
    get_account_summary,
    get_alerts,
    get_merchants,
    get_spending_summary,
    get_subscriptions,
    resolve_period,
    search_transactions,
    // Result types
    AccountInfo,
    // Params types
    AccountSummaryParams,
    AccountSummaryResult,
    AlertSummary,
    AlertsParams,
    AlertsResult,
    CategoryComparison,
    CategorySpending,
    CompareSpendingParams,
    CompareSpendingResult,
    MerchantSummary,
    MerchantsParams,
    MerchantsResult,
    SearchTransactionsParams,
    SearchTransactionsResult,
    SpendingSummaryParams,
    SpendingSummaryResult,
    SubscriptionSummary,
    SubscriptionsParams,
    SubscriptionsResult,
    TransactionSummary,
};

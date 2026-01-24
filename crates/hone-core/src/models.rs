//! Domain models for Hone

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::ollama::{DuplicateAnalysis, ReceiptMatchEvaluation};

/// A bank account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub bank: Bank,
    pub account_type: Option<AccountType>,
    /// The entity (person) who owns this account
    pub entity_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Supported banks for CSV import
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bank {
    Chase,
    Bofa,
    Amex,
    CapitalOne,
}

impl Bank {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chase => "chase",
            Self::Bofa => "bofa",
            Self::Amex => "amex",
            Self::CapitalOne => "capitalone",
        }
    }
}

impl std::str::FromStr for Bank {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "chase" => Ok(Self::Chase),
            "bofa" | "bankofamerica" => Ok(Self::Bofa),
            "amex" | "americanexpress" => Ok(Self::Amex),
            "capitalone" | "capital_one" => Ok(Self::CapitalOne),
            _ => Err(format!("Unknown bank: {}", s)),
        }
    }
}

impl std::fmt::Display for Bank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Account types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Checking,
    Savings,
    Credit,
}

/// Transaction source - how it was created
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransactionSource {
    /// Imported from bank CSV
    #[default]
    Import,
    /// Created as placeholder from receipt
    Receipt,
    /// Manually entered
    Manual,
}

impl TransactionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Receipt => "receipt",
            Self::Manual => "manual",
        }
    }
}

impl std::str::FromStr for TransactionSource {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "import" => Ok(Self::Import),
            "receipt" => Ok(Self::Receipt),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("Unknown transaction source: {}", s)),
        }
    }
}

impl std::fmt::Display for TransactionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Payment method used for a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    /// Apple Pay (mobile wallet)
    ApplePay,
    /// Google Pay (mobile wallet)
    GooglePay,
    /// Physical card swipe/tap
    PhysicalCard,
    /// Online/card-not-present
    Online,
    /// Recurring/automatic charge
    Recurring,
}

impl PaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApplePay => "apple_pay",
            Self::GooglePay => "google_pay",
            Self::PhysicalCard => "physical_card",
            Self::Online => "online",
            Self::Recurring => "recurring",
        }
    }
}

impl std::str::FromStr for PaymentMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "apple_pay" => Ok(Self::ApplePay),
            "google_pay" => Ok(Self::GooglePay),
            "physical_card" => Ok(Self::PhysicalCard),
            "online" => Ok(Self::Online),
            "recurring" => Ok(Self::Recurring),
            _ => Err(format!("Unknown payment method: {}", s)),
        }
    }
}

impl std::fmt::Display for PaymentMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub account_id: i64,
    pub date: NaiveDate,
    pub description: String,
    /// Negative = expense, positive = income
    pub amount: f64,
    pub category: Option<String>,
    /// AI-normalized merchant name
    pub merchant_normalized: Option<String>,
    /// Hash for deduplication
    pub import_hash: String,
    /// Where the purchase was made (delivery address for online)
    pub purchase_location_id: Option<i64>,
    /// Where the vendor is based
    pub vendor_location_id: Option<i64>,
    /// Trip/event this transaction belongs to
    pub trip_id: Option<i64>,
    /// How this transaction was created
    pub source: TransactionSource,
    /// Expected amount from receipt (for tip discrepancy tracking)
    pub expected_amount: Option<f64>,
    /// Whether this transaction is archived (hidden from reports/lists)
    pub archived: bool,
    /// Original import data as JSON (for reprocessing)
    pub original_data: Option<String>,
    /// Import format identifier (e.g., chase_csv, amex_csv, receipt, manual)
    pub import_format: Option<String>,
    /// Card member name (from Amex extended format)
    pub card_member: Option<String>,
    /// Payment method (apple_pay, google_pay, physical_card, online, etc.)
    pub payment_method: Option<PaymentMethod>,
    pub created_at: DateTime<Utc>,
}

/// A new transaction to be imported (before DB insertion)
#[derive(Debug, Clone)]
pub struct NewTransaction {
    pub date: NaiveDate,
    pub description: String,
    pub amount: f64,
    pub category: Option<String>,
    pub import_hash: String,
    /// Original import data as JSON (for reprocessing)
    pub original_data: Option<String>,
    /// Import format identifier (e.g., chase_csv, amex_csv, receipt, manual)
    pub import_format: Option<String>,
    /// Card member name from CSV (Amex extended format)
    pub card_member: Option<String>,
    /// Payment method (apple_pay, google_pay, etc.)
    pub payment_method: Option<PaymentMethod>,
}

/// A detected subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: i64,
    pub merchant: String,
    /// The account where this subscription was detected
    pub account_id: Option<i64>,
    pub amount: Option<f64>,
    pub frequency: Option<Frequency>,
    pub first_seen: Option<NaiveDate>,
    pub last_seen: Option<NaiveDate>,
    pub status: SubscriptionStatus,
    pub user_acknowledged: bool,
    /// When the subscription was last acknowledged (for stale acknowledgment detection)
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Subscription billing frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Weekly,
    Monthly,
    Yearly,
}

impl Frequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }
}

/// Subscription status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionStatus {
    Active,
    Cancelled,
    Zombie,
    /// User marked as "not a subscription" - excluded from detection
    Excluded,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Cancelled => "cancelled",
            Self::Zombie => "zombie",
            Self::Excluded => "excluded",
        }
    }
}

/// A price history entry for tracking subscription cost changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHistory {
    pub id: i64,
    pub subscription_id: i64,
    pub amount: f64,
    pub detected_at: NaiveDate,
}

/// A waste detection alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    pub alert_type: AlertType,
    pub subscription_id: Option<i64>,
    pub message: Option<String>,
    pub dismissed: bool,
    pub created_at: DateTime<Utc>,
    /// Ollama analysis for duplicate alerts (overlap/unique features)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_analysis: Option<DuplicateAnalysis>,
    /// Spending anomaly data (for spending_anomaly alerts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spending_anomaly: Option<SpendingAnomalyData>,
    // Joined data for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription: Option<Subscription>,
}

/// Types of waste detection alerts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    /// A recurring charge that might be forgotten
    Zombie,
    /// A subscription that increased in price
    PriceIncrease,
    /// Multiple services in the same category
    Duplicate,
    /// A cancelled subscription that started charging again
    Resume,
    /// Spending in a category changed dramatically vs baseline
    SpendingAnomaly,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Zombie => "zombie",
            Self::PriceIncrease => "price_increase",
            Self::Duplicate => "duplicate",
            Self::Resume => "resume",
            Self::SpendingAnomaly => "spending_anomaly",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Zombie => "Zombie Subscription",
            Self::PriceIncrease => "Price Increase",
            Self::Duplicate => "Duplicate Service",
            Self::Resume => "Subscription Resumed",
            Self::SpendingAnomaly => "Spending Change",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Zombie => "A recurring charge you might have forgotten about",
            Self::PriceIncrease => "This service quietly raised its price",
            Self::Duplicate => "You have multiple services in this category",
            Self::Resume => "A subscription you cancelled has started charging again",
            Self::SpendingAnomaly => "Your spending in this category changed significantly",
        }
    }
}

/// Ollama-generated explanation for a spending change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingChangeExplanation {
    /// One-sentence summary (e.g., "Dining spending increased significantly")
    pub summary: String,
    /// Specific reasons for the change (max 3)
    pub reasons: Vec<String>,
    /// Which Ollama model generated this analysis
    pub model: String,
    /// When this analysis was generated
    pub analyzed_at: DateTime<Utc>,
}

/// Data about a spending anomaly (for alerts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingAnomalyData {
    /// The tag/category with the spending change
    pub tag_id: i64,
    pub tag_name: String,
    /// 3-month average baseline
    pub baseline_amount: f64,
    /// Current month spending
    pub current_amount: f64,
    /// Percentage change (positive = increase, negative = decrease)
    pub percent_change: f64,
    /// Ollama-generated explanation (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<SpendingChangeExplanation>,
}

/// Dashboard summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_transactions: i64,
    pub total_accounts: i64,
    pub active_subscriptions: i64,
    pub monthly_subscription_cost: f64,
    pub active_alerts: i64,
    pub potential_monthly_savings: f64,
    pub recent_imports: Vec<RecentImport>,
    /// Transactions without any tags
    pub untagged_transactions: i64,
}

/// Info about a recent import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentImport {
    pub account_name: String,
    pub bank: Bank,
    pub transaction_count: i64,
    pub imported_at: DateTime<Utc>,
}

// ========== Tag System Models ==========

/// A hierarchical tag for categorizing transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    /// Parent tag ID for hierarchy (None = root tag)
    pub parent_id: Option<i64>,
    /// Optional color for UI display (e.g., "#10b981")
    pub color: Option<String>,
    /// Optional icon identifier
    pub icon: Option<String>,
    /// Pipe-separated patterns for auto-matching (e.g., "NETFLIX|HULU")
    pub auto_patterns: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A tag with its computed path and hierarchy info (for tree display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagWithPath {
    #[serde(flatten)]
    pub tag: Tag,
    /// Full path from root (e.g., "Entertainment.Streaming")
    pub path: String,
    /// Depth in hierarchy (0 = root)
    pub depth: i32,
    /// Child tags (for tree structure)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TagWithPath>,
}

/// How a tag was assigned to a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagSource {
    /// Manually assigned by user
    Manual,
    /// Matched by auto_patterns on tag
    Pattern,
    /// Classified by Ollama LLM
    Ollama,
    /// Matched by user-defined rule
    Rule,
    /// Mapped from bank-provided category (e.g., Amex "Transportation-Fuel")
    BankCategory,
    /// Learned from user's manual tag assignments (merchant→tag association)
    Learned,
}

impl TagSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Pattern => "pattern",
            Self::Ollama => "ollama",
            Self::Rule => "rule",
            Self::BankCategory => "bank_category",
            Self::Learned => "learned",
        }
    }
}

impl std::str::FromStr for TagSource {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "pattern" => Ok(Self::Pattern),
            "ollama" => Ok(Self::Ollama),
            "rule" => Ok(Self::Rule),
            "bank_category" => Ok(Self::BankCategory),
            "learned" => Ok(Self::Learned),
            _ => Err(format!("Unknown tag source: {}", s)),
        }
    }
}

/// Junction table entry linking a transaction to a tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionTag {
    pub transaction_id: i64,
    pub tag_id: i64,
    /// How the tag was assigned
    pub source: TagSource,
    /// Confidence level (0.0-1.0), mainly for Ollama classifications
    pub confidence: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// Transaction tag with full tag details for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionTagWithDetails {
    pub tag_id: i64,
    pub tag_name: String,
    pub tag_path: String,
    pub tag_color: Option<String>,
    pub source: TagSource,
    pub confidence: Option<f64>,
}

/// Pattern matching type for tag rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    /// Case-insensitive substring match (supports | for OR)
    Contains,
    /// Regular expression match
    Regex,
    /// Exact string match (case-insensitive)
    Exact,
}

impl PatternType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Regex => "regex",
            Self::Exact => "exact",
        }
    }
}

impl std::str::FromStr for PatternType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "contains" => Ok(Self::Contains),
            "regex" => Ok(Self::Regex),
            "exact" => Ok(Self::Exact),
            _ => Err(format!("Unknown pattern type: {}", s)),
        }
    }
}

/// A user-defined rule for auto-tagging transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRule {
    pub id: i64,
    pub tag_id: i64,
    /// The pattern to match against transaction descriptions
    pub pattern: String,
    pub pattern_type: PatternType,
    /// Higher priority rules are checked first
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

/// A tag rule with its associated tag info (for display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRuleWithTag {
    #[serde(flatten)]
    pub rule: TagRule,
    pub tag_name: String,
    pub tag_path: String,
}

/// Spending summary by tag (for reports)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSpending {
    pub tag_id: i64,
    pub tag_name: String,
    pub tag_path: String,
    /// Direct spending on this tag
    pub direct_amount: f64,
    /// Spending including all child tags (rollup)
    pub total_amount: f64,
    pub transaction_count: i64,
}

/// Result of deleting a tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteTagResult {
    pub deleted_tag_id: i64,
    /// Transactions moved to parent (or untagged if root)
    pub transactions_moved: i64,
    /// Child tags that were orphaned or reparented
    pub children_affected: i64,
}

// ========== Entity & Split Models ==========

/// Entity types (who/what spending is for)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    /// A person (household member)
    Person,
    /// A pet
    Pet,
    /// A vehicle
    Vehicle,
    /// A property (house, cabin, rental)
    Property,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Pet => "pet",
            Self::Vehicle => "vehicle",
            Self::Property => "property",
        }
    }
}

impl std::str::FromStr for EntityType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "person" => Ok(Self::Person),
            "pet" => Ok(Self::Pet),
            "vehicle" => Ok(Self::Vehicle),
            "property" => Ok(Self::Property),
            _ => Err(format!("Unknown entity type: {}", s)),
        }
    }
}

/// An entity that spending can be attributed to (person, pet, vehicle, property)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: i64,
    pub name: String,
    pub entity_type: EntityType,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
}

/// New entity for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEntity {
    pub name: String,
    pub entity_type: EntityType,
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// Location type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationType {
    Home,
    Work,
    Store,
    Online,
    Travel,
}

impl LocationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Work => "work",
            Self::Store => "store",
            Self::Online => "online",
            Self::Travel => "travel",
        }
    }
}

impl std::str::FromStr for LocationType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "home" => Ok(Self::Home),
            "work" => Ok(Self::Work),
            "store" => Ok(Self::Store),
            "online" => Ok(Self::Online),
            "travel" => Ok(Self::Travel),
            _ => Err(format!("Unknown location type: {}", s)),
        }
    }
}

/// A location (for tracking where purchases were made or where vendors are based)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: i64,
    pub name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_type: Option<LocationType>,
    pub created_at: DateTime<Utc>,
}

/// New location for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLocation {
    pub name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_type: Option<LocationType>,
}

/// Split types (what kind of line item)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitType {
    /// A product or service
    Item,
    /// Sales tax, VAT, etc.
    Tax,
    /// Gratuity
    Tip,
    /// Delivery, service, convenience fees
    Fee,
    /// Coupons, promo codes (negative amount)
    Discount,
    /// Points redeemed, cashback (negative amount)
    Rewards,
}

impl SplitType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Tax => "tax",
            Self::Tip => "tip",
            Self::Fee => "fee",
            Self::Discount => "discount",
            Self::Rewards => "rewards",
        }
    }
}

impl std::str::FromStr for SplitType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "item" => Ok(Self::Item),
            "tax" => Ok(Self::Tax),
            "tip" => Ok(Self::Tip),
            "fee" => Ok(Self::Fee),
            "discount" => Ok(Self::Discount),
            "rewards" => Ok(Self::Rewards),
            _ => Err(format!("Unknown split type: {}", s)),
        }
    }
}

/// A transaction split (line item within a transaction)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSplit {
    pub id: i64,
    pub transaction_id: i64,
    pub amount: f64,
    pub description: Option<String>,
    pub split_type: SplitType,
    /// Who/what this split is for (NULL = household/shared)
    pub entity_id: Option<i64>,
    /// Who made the purchase (NULL = account owner)
    pub purchaser_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// New split for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTransactionSplit {
    pub transaction_id: i64,
    pub amount: f64,
    pub description: Option<String>,
    pub split_type: SplitType,
    pub entity_id: Option<i64>,
    pub purchaser_id: Option<i64>,
}

/// Split with entity and tag details for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSplitWithDetails {
    #[serde(flatten)]
    pub split: TransactionSplit,
    /// Entity name if entity_id is set
    pub entity_name: Option<String>,
    /// Purchaser name if purchaser_id is set
    pub purchaser_name: Option<String>,
    /// Tags assigned to this split
    pub tags: Vec<TransactionTagWithDetails>,
}

/// Receipt status for workflow tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    /// Matched to an imported transaction
    #[default]
    Matched,
    /// Waiting for matching transaction to be imported
    Pending,
    /// Needs manual review (multiple matches, discrepancy, etc.)
    ManualReview,
    /// Never matched after 90 days
    Orphaned,
}

impl ReceiptStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::Pending => "pending",
            Self::ManualReview => "manual_review",
            Self::Orphaned => "orphaned",
        }
    }
}

impl std::str::FromStr for ReceiptStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "matched" => Ok(Self::Matched),
            "pending" => Ok(Self::Pending),
            "manual_review" => Ok(Self::ManualReview),
            "orphaned" => Ok(Self::Orphaned),
            _ => Err(format!("Unknown receipt status: {}", s)),
        }
    }
}

/// Receipt role - relationship to transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReceiptRole {
    /// Main itemized receipt (drives splits)
    #[default]
    Primary,
    /// Additional documentation (credit card slip, warranty, etc.)
    Supplementary,
}

impl ReceiptRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Supplementary => "supplementary",
        }
    }
}

impl std::str::FromStr for ReceiptRole {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "primary" => Ok(Self::Primary),
            "supplementary" => Ok(Self::Supplementary),
            _ => Err(format!("Unknown receipt role: {}", s)),
        }
    }
}

/// A receipt attached to a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: i64,
    /// Transaction ID (NULL for pending receipts awaiting match)
    pub transaction_id: Option<i64>,
    /// Path to stored image file
    pub image_path: Option<String>,
    /// Cached LLM parsing output
    pub parsed_json: Option<String>,
    pub parsed_at: Option<DateTime<Utc>>,
    /// Workflow status
    pub status: ReceiptStatus,
    /// Role in relation to transaction
    pub role: ReceiptRole,
    /// Parsed date from receipt (for matching)
    pub receipt_date: Option<NaiveDate>,
    /// Parsed total from receipt (for matching)
    pub receipt_total: Option<f64>,
    /// Parsed merchant name (for matching)
    pub receipt_merchant: Option<String>,
    /// SHA256 hash for deduplication
    pub content_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// New receipt for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewReceipt {
    /// Transaction ID (optional - NULL for receipt-first workflow)
    pub transaction_id: Option<i64>,
    /// Path to stored image file
    pub image_path: Option<String>,
    /// Image data (for in-DB storage of small receipts)
    pub image_data: Option<Vec<u8>>,
    /// Workflow status
    pub status: ReceiptStatus,
    /// Role in relation to transaction
    pub role: ReceiptRole,
    /// Parsed date from receipt
    pub receipt_date: Option<NaiveDate>,
    /// Parsed total from receipt
    pub receipt_total: Option<f64>,
    /// Parsed merchant name
    pub receipt_merchant: Option<String>,
    /// SHA256 hash for deduplication
    pub content_hash: Option<String>,
}

/// A merchant alias for learning name variations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantAlias {
    pub id: i64,
    /// Name as it appears on receipt (e.g., "TARGET T-1234")
    pub receipt_name: String,
    /// Normalized/canonical name (e.g., "TARGET")
    pub canonical_name: String,
    /// Which bank uses this format (optional)
    pub bank: Option<String>,
    /// Confidence: 1.0 = user confirmed, <1.0 = auto-learned
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
}

/// A potential transaction match for a receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptMatchCandidate {
    pub transaction: Transaction,
    /// Overall match score (0.0-1.0, higher is better)
    pub score: f64,
    /// Individual match factors
    pub match_factors: MatchFactors,
}

/// Individual factors contributing to a match score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchFactors {
    /// How close the amounts are (1.0 = exact, 0.0 = way off)
    pub amount_score: f64,
    /// How close the dates are (1.0 = same day, lower for more days apart)
    pub date_score: f64,
    /// How similar the merchant names are (1.0 = exact match)
    pub merchant_score: f64,
    /// Absolute amount difference
    pub amount_diff: f64,
    /// Days between receipt and transaction date
    pub days_diff: i64,
    /// Optional Ollama evaluation for ambiguous matches
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ollama_evaluation: Option<ReceiptMatchEvaluation>,
}

// ========== Trip/Event Models ==========

/// A trip or event that groups related transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trip {
    pub id: i64,
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Start date of trip
    pub start_date: Option<NaiveDate>,
    /// End date of trip
    pub end_date: Option<NaiveDate>,
    /// Location associated with trip (e.g., "Paris, France")
    pub location_id: Option<i64>,
    /// Budget for this trip
    pub budget: Option<f64>,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
}

/// New trip for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTrip {
    pub name: String,
    pub description: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub location_id: Option<i64>,
    pub budget: Option<f64>,
}

/// Trip with spending summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripWithSpending {
    #[serde(flatten)]
    pub trip: Trip,
    pub total_spent: f64,
    pub transaction_count: i64,
    /// Location name if location_id is set
    pub location_name: Option<String>,
}

// ========== Vehicle Mileage Models ==========

/// A mileage log entry for a vehicle entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MileageLog {
    pub id: i64,
    pub entity_id: i64,
    pub date: NaiveDate,
    pub odometer: f64,
    /// Optional note (e.g., "oil change", "road trip start")
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// New mileage entry for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMileageLog {
    pub entity_id: i64,
    pub date: NaiveDate,
    pub odometer: f64,
    pub note: Option<String>,
}

/// Vehicle cost summary (for reports)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleCostSummary {
    pub entity_id: i64,
    pub entity_name: String,
    pub total_cost: f64,
    pub fuel_cost: f64,
    pub maintenance_cost: f64,
    pub insurance_cost: f64,
    pub other_cost: f64,
    pub total_miles: Option<f64>,
    pub cost_per_mile: Option<f64>,
}

// ========== Property Expense Models ==========

/// Property expense summary (for reports)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyExpenseSummary {
    pub entity_id: i64,
    pub entity_name: String,
    pub total_expenses: f64,
    pub mortgage_rent: f64,
    pub utilities: f64,
    pub maintenance: f64,
    pub taxes: f64,
    pub insurance: f64,
    pub improvements: f64,
    pub other: f64,
}

// ========== Location Spending Models ==========

/// Spending by location for reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationSpending {
    pub location_id: i64,
    pub location_name: Option<String>,
    pub city: Option<String>,
    pub country: String,
    pub total_spent: f64,
    pub transaction_count: i64,
}

// ========== Report Models ==========

/// Report time granularity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Granularity {
    Monthly,
    Weekly,
}

impl Granularity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Weekly => "weekly",
        }
    }
}

impl std::str::FromStr for Granularity {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "monthly" => Ok(Self::Monthly),
            "weekly" => Ok(Self::Weekly),
            _ => Err(format!(
                "Unknown granularity: {} (valid: monthly, weekly)",
                s
            )),
        }
    }
}

/// A spending category in a report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpending {
    pub tag: String,
    pub tag_id: i64,
    pub amount: f64,
    pub percentage: f64,
    pub transaction_count: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CategorySpending>,
}

/// Untagged transaction summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UntaggedSummary {
    pub amount: f64,
    pub percentage: f64,
    pub transaction_count: i64,
}

/// Report period info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPeriod {
    pub from: String,
    pub to: String,
}

/// Spending summary report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingSummary {
    pub period: ReportPeriod,
    pub total: f64,
    pub categories: Vec<CategorySpending>,
    pub untagged: UntaggedSummary,
}

/// A single data point in a trends report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDataPoint {
    pub period: String,
    pub amount: f64,
    pub transaction_count: i64,
}

/// Trends report (spending over time)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendsReport {
    pub granularity: Granularity,
    pub period: ReportPeriod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub data: Vec<TrendDataPoint>,
}

/// Merchant spending summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantSummary {
    pub merchant: String,
    pub amount: f64,
    pub transaction_count: i64,
}

/// Top merchants report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantsReport {
    pub period: ReportPeriod,
    pub limit: i64,
    pub merchants: Vec<MerchantSummary>,
}

/// Waste breakdown in subscription summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasteBreakdown {
    pub zombie_count: i64,
    pub zombie_monthly: f64,
    pub duplicate_count: i64,
    pub duplicate_monthly: f64,
    pub price_increase_count: i64,
    pub price_increase_delta: f64,
    pub total_waste_monthly: f64,
}

/// Subscription info for reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub id: i64,
    pub merchant: String,
    pub amount: f64,
    pub frequency: String,
    pub status: String,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
}

/// Subscription summary report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionSummaryReport {
    pub total_monthly: f64,
    pub active_count: i64,
    pub cancelled_count: i64,
    pub subscriptions: Vec<SubscriptionInfo>,
    pub waste: WasteBreakdown,
}

/// A cancelled subscription for savings tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelledSubscriptionInfo {
    pub id: i64,
    pub merchant: String,
    pub monthly_amount: f64,
    pub cancelled_at: String,
    pub months_counted: i64,
    pub months_remaining: i64,
    pub savings: f64,
}

/// Savings report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsReport {
    pub total_savings: f64,
    pub total_monthly_saved: f64,
    pub cancelled_count: i64,
    pub cancelled: Vec<CancelledSubscriptionInfo>,
}

// ========== Ollama Metrics Models ==========

/// Types of Ollama operations for metrics tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaOperation {
    ClassifyMerchant,
    NormalizeMerchant,
    ParseReceipt,
    SuggestEntity,
    SuggestSplit,
    /// Classify if a merchant is a subscription service vs retail
    ClassifySubscription,
    /// Evaluate if a receipt and transaction are the same purchase
    EvaluateReceiptMatch,
    /// Analyze duplicate services for overlap and unique features
    AnalyzeDuplicates,
    /// Explain why spending changed in a category
    ExplainSpendingChange,
    /// Agentic explore query (conversational finance assistant)
    ExploreQuery,
}

impl OllamaOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClassifyMerchant => "classify_merchant",
            Self::NormalizeMerchant => "normalize_merchant",
            Self::ParseReceipt => "parse_receipt",
            Self::SuggestEntity => "suggest_entity",
            Self::SuggestSplit => "suggest_split",
            Self::ClassifySubscription => "classify_subscription",
            Self::EvaluateReceiptMatch => "evaluate_receipt_match",
            Self::AnalyzeDuplicates => "analyze_duplicates",
            Self::ExplainSpendingChange => "explain_spending_change",
            Self::ExploreQuery => "explore_query",
        }
    }
}

impl std::str::FromStr for OllamaOperation {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "classify_merchant" => Ok(Self::ClassifyMerchant),
            "normalize_merchant" => Ok(Self::NormalizeMerchant),
            "parse_receipt" => Ok(Self::ParseReceipt),
            "suggest_entity" => Ok(Self::SuggestEntity),
            "suggest_split" => Ok(Self::SuggestSplit),
            "classify_subscription" => Ok(Self::ClassifySubscription),
            "evaluate_receipt_match" => Ok(Self::EvaluateReceiptMatch),
            "analyze_duplicates" => Ok(Self::AnalyzeDuplicates),
            "explain_spending_change" => Ok(Self::ExplainSpendingChange),
            "explore_query" => Ok(Self::ExploreQuery),
            _ => Err(format!("Unknown Ollama operation: {}", s)),
        }
    }
}

impl std::fmt::Display for OllamaOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single Ollama call metric record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaMetric {
    pub id: i64,
    pub operation: OllamaOperation,
    pub model: String,
    pub started_at: DateTime<Utc>,
    pub latency_ms: i64,
    pub success: bool,
    pub error_message: Option<String>,
    pub confidence: Option<f64>,
    pub transaction_id: Option<i64>,
    pub input_text: Option<String>,
    pub result_text: Option<String>,
    /// Additional metadata as JSON (e.g., tool calls for explore queries)
    pub metadata: Option<String>,
}

/// New metric for creation (before DB insertion)
#[derive(Debug, Clone)]
pub struct NewOllamaMetric {
    pub operation: OllamaOperation,
    pub model: String,
    pub latency_ms: i64,
    pub success: bool,
    pub error_message: Option<String>,
    pub confidence: Option<f64>,
    pub transaction_id: Option<i64>,
    pub input_text: Option<String>,
    pub result_text: Option<String>,
    /// Additional metadata as JSON (e.g., tool calls for explore queries)
    pub metadata: Option<String>,
}

/// A user correction of an Ollama tag assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaCorrection {
    pub id: i64,
    pub transaction_id: i64,
    pub original_tag_id: i64,
    pub original_confidence: Option<f64>,
    pub corrected_tag_id: i64,
    pub corrected_at: DateTime<Utc>,
}

/// Statistics for a single operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub operation: String,
    pub call_count: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub avg_confidence: Option<f64>,
}

/// Accuracy statistics from user corrections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyStats {
    pub total_corrections: i64,
    pub total_ollama_tags: i64,
    pub correction_rate: f64,
    pub estimated_accuracy: f64,
}

/// Aggregated Ollama statistics for a time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStats {
    pub period_start: String,
    pub period_end: String,
    pub total_calls: i64,
    pub successful_calls: i64,
    pub failed_calls: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: i64,
    pub p95_latency_ms: i64,
    pub max_latency_ms: i64,
    pub by_operation: Vec<OperationStats>,
    pub accuracy: AccuracyStats,
}

/// Ollama health and availability status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaHealthStatus {
    pub available: bool,
    pub host: Option<String>,
    pub model: Option<String>,
    pub last_successful_call: Option<DateTime<Utc>>,
    pub last_failed_call: Option<DateTime<Utc>>,
    pub recent_error_rate: f64,
    /// Whether the AI orchestrator (agentic mode) is configured
    #[serde(default)]
    pub orchestrator_available: bool,
    /// Host for orchestrator (Anthropic-compatible endpoint)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestrator_host: Option<String>,
    /// Model used for orchestrator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestrator_model: Option<String>,
}

/// Summary of current stats for recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSummary {
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub estimated_accuracy: f64,
    pub latency_trend: String,
}

/// Model switch recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    pub current_model: Option<String>,
    pub stats_summary: StatsSummary,
    pub recommendations: Vec<String>,
    pub should_switch: bool,
}

/// Statistics for a single model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub model: String,
    pub total_calls: i64,
    pub successful_calls: i64,
    pub failed_calls: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: i64,
    pub p95_latency_ms: i64,
    pub max_latency_ms: i64,
    pub avg_confidence: Option<f64>,
    pub by_operation: Vec<OperationStats>,
    pub first_used: Option<String>,
    pub last_used: Option<String>,
}

/// Model comparison response with stats for multiple models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelComparisonStats {
    pub period_start: String,
    pub period_end: String,
    pub models: Vec<ModelStats>,
}

// ========== Import History Models ==========

/// Import session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl ImportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for ImportStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown import status: {}", s)),
        }
    }
}

impl std::fmt::Display for ImportStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An import session record tracking a single import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSession {
    pub id: i64,
    pub account_id: i64,
    pub filename: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub bank: Bank,
    pub imported_count: i64,
    pub skipped_count: i64,
    // Tagging breakdown
    pub tagged_by_learned: i64,
    pub tagged_by_rule: i64,
    pub tagged_by_pattern: i64,
    pub tagged_by_ollama: i64,
    pub tagged_by_bank_category: i64,
    pub tagged_fallback: i64,
    // Detection results
    pub subscriptions_found: i64,
    pub zombies_detected: i64,
    pub price_increases_detected: i64,
    pub duplicates_detected: i64,
    pub receipts_matched: i64,
    // Metadata
    pub user_email: Option<String>,
    pub ollama_model: Option<String>,
    // Processing status for async imports
    pub status: ImportStatus,
    pub processing_phase: Option<String>,
    pub processing_current: i64,
    pub processing_total: i64,
    pub processing_error: Option<String>,
    // Phase timing (milliseconds)
    pub tagging_duration_ms: Option<i64>,
    pub normalizing_duration_ms: Option<i64>,
    pub matching_duration_ms: Option<i64>,
    pub detecting_duration_ms: Option<i64>,
    pub total_duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Import session with account name for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSessionWithAccount {
    pub session: ImportSession,
    pub account_name: String,
}

/// New import session for creation
#[derive(Debug, Clone)]
pub struct NewImportSession {
    pub account_id: i64,
    pub filename: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub bank: Bank,
    pub user_email: Option<String>,
    pub ollama_model: Option<String>,
}

/// Tagging breakdown for an import session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportTaggingBreakdown {
    pub by_learned: i64,
    pub by_rule: i64,
    pub by_pattern: i64,
    pub by_ollama: i64,
    pub by_bank_category: i64,
    pub fallback: i64,
}

/// A skipped (duplicate) transaction from an import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedTransaction {
    pub id: i64,
    pub import_session_id: i64,
    pub date: NaiveDate,
    pub description: String,
    pub amount: f64,
    pub import_hash: String,
    pub existing_transaction_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

// ========== User Feedback Models ==========

/// Type of feedback provided by the user
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    /// User explicitly marked something as helpful (thumbs up)
    Helpful,
    /// User explicitly marked something as not helpful (thumbs down)
    NotHelpful,
    /// User corrected a classification or value
    Correction,
    /// User dismissed/ignored something (implicit signal)
    Dismissal,
}

impl FeedbackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Helpful => "helpful",
            Self::NotHelpful => "not_helpful",
            Self::Correction => "correction",
            Self::Dismissal => "dismissal",
        }
    }
}

impl std::str::FromStr for FeedbackType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "helpful" => Ok(Self::Helpful),
            "not_helpful" => Ok(Self::NotHelpful),
            "correction" => Ok(Self::Correction),
            "dismissal" => Ok(Self::Dismissal),
            _ => Err(format!("Unknown feedback type: {}", s)),
        }
    }
}

impl std::fmt::Display for FeedbackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// What type of content the feedback is about
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackTargetType {
    /// Feedback on an alert (zombie, duplicate, spending anomaly, etc.)
    Alert,
    /// Feedback on an AI-generated insight or explanation
    Insight,
    /// Feedback on a merchant/tag classification
    Classification,
    /// Feedback on an AI-generated explanation (spending change, etc.)
    Explanation,
    /// Feedback on a receipt match suggestion
    ReceiptMatch,
}

impl FeedbackTargetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Insight => "insight",
            Self::Classification => "classification",
            Self::Explanation => "explanation",
            Self::ReceiptMatch => "receipt_match",
        }
    }
}

impl std::str::FromStr for FeedbackTargetType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "alert" => Ok(Self::Alert),
            "insight" => Ok(Self::Insight),
            "classification" => Ok(Self::Classification),
            "explanation" => Ok(Self::Explanation),
            "receipt_match" => Ok(Self::ReceiptMatch),
            _ => Err(format!("Unknown feedback target type: {}", s)),
        }
    }
}

impl std::fmt::Display for FeedbackTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Additional context stored with feedback (model, prompt version, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeedbackContext {
    /// Which Ollama model generated the content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Which prompt version was used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<String>,
    /// Related transaction ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i64>,
    /// Any additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// A user feedback record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedback {
    pub id: i64,
    pub feedback_type: FeedbackType,
    pub target_type: FeedbackTargetType,
    /// ID of the target item (alert_id, transaction_id, etc.)
    pub target_id: Option<i64>,
    /// What was originally shown (JSON for complex values)
    pub original_value: Option<String>,
    /// What user changed it to (if correction)
    pub corrected_value: Option<String>,
    /// Optional user-provided reason
    pub reason: Option<String>,
    /// Additional context (model, prompt version, etc.)
    pub context: Option<FeedbackContext>,
    pub created_at: DateTime<Utc>,
    /// When this feedback was reverted (NULL = active)
    pub reverted_at: Option<DateTime<Utc>>,
}

/// New feedback for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserFeedback {
    pub feedback_type: FeedbackType,
    pub target_type: FeedbackTargetType,
    pub target_id: Option<i64>,
    pub original_value: Option<String>,
    pub corrected_value: Option<String>,
    pub reason: Option<String>,
    pub context: Option<FeedbackContext>,
}

/// Feedback summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackStats {
    pub total_feedback: i64,
    pub helpful_count: i64,
    pub not_helpful_count: i64,
    pub correction_count: i64,
    pub dismissal_count: i64,
    pub reverted_count: i64,
    /// Breakdown by target type
    pub by_target_type: Vec<FeedbackTargetStats>,
}

/// Stats for a specific target type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackTargetStats {
    pub target_type: FeedbackTargetType,
    pub total: i64,
    pub helpful: i64,
    pub not_helpful: i64,
    pub helpfulness_ratio: f64,
}

// ============================================================================
// Reprocess Comparison Models
// ============================================================================

/// Snapshot of import session state for before/after comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessSnapshot {
    pub tagging_breakdown: ImportTaggingBreakdown,
    pub subscriptions_found: i64,
    pub zombies_detected: i64,
    pub price_increases_detected: i64,
    pub duplicates_detected: i64,
    pub receipts_matched: i64,
    /// Sample of transactions with their current tags for change detection
    pub sample_transactions: Vec<TransactionTagSnapshot>,
}

/// Snapshot of a transaction's tags at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionTagSnapshot {
    pub id: i64,
    pub description: String,
    pub merchant_normalized: Option<String>,
    pub tags: Vec<String>,
}

/// Comparison between before and after reprocessing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessComparison {
    pub before: ReprocessSnapshot,
    pub after: ReprocessSnapshot,
    /// Transactions whose tags changed
    pub tag_changes: Vec<TagChange>,
    /// Transactions whose merchant name changed
    pub merchant_changes: Vec<MerchantChange>,
}

/// A transaction whose tags changed during reprocessing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagChange {
    pub transaction_id: i64,
    pub description: String,
    pub before_tags: Vec<String>,
    pub after_tags: Vec<String>,
}

/// A transaction whose merchant name changed during reprocessing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantChange {
    pub transaction_id: i64,
    pub description: String,
    pub before_merchant: Option<String>,
    pub after_merchant: Option<String>,
}

// ============================================================================
// Reprocess Run Models (for historical comparison)
// ============================================================================

/// Status of a reprocess run
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReprocessRunStatus {
    #[default]
    Running,
    Completed,
    Failed,
}

impl ReprocessRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for ReprocessRunStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("Unknown reprocess run status: {}", s)),
        }
    }
}

impl std::fmt::Display for ReprocessRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A reprocess run record tracking a single reprocess operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessRun {
    pub id: i64,
    pub import_session_id: i64,
    /// Run number within this session (1, 2, 3...)
    pub run_number: i64,
    /// Ollama model used for this reprocess (if any)
    pub ollama_model: Option<String>,
    pub status: ReprocessRunStatus,
    /// User who initiated this reprocess
    pub initiated_by: Option<String>,
    /// Optional reason/notes for this run
    pub reason: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// New reprocess run for creation
#[derive(Debug, Clone)]
pub struct NewReprocessRun {
    pub import_session_id: i64,
    pub ollama_model: Option<String>,
    pub initiated_by: Option<String>,
    pub reason: Option<String>,
}

/// A reprocess run with its comparison data (for API responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessRunWithComparison {
    #[serde(flatten)]
    pub run: ReprocessRun,
    /// Before snapshot
    pub before: Option<ReprocessSnapshot>,
    /// After snapshot (None if run still in progress or failed)
    pub after: Option<ReprocessSnapshot>,
    /// Computed changes (None if missing snapshots)
    pub tag_changes: Option<Vec<TagChange>>,
    pub merchant_changes: Option<Vec<MerchantChange>>,
}

/// Summary of a reprocess run (for list views)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessRunSummary {
    pub id: i64,
    pub run_number: i64,
    pub ollama_model: Option<String>,
    pub status: ReprocessRunStatus,
    pub initiated_by: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Quick stats about what changed
    pub tags_changed: i64,
    pub merchants_changed: i64,
}

/// Comparison between two specific runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunComparison {
    pub run_a: ReprocessRunSummary,
    pub run_b: ReprocessRunSummary,
    /// Differences in tagging breakdown
    pub tagging_diff: TaggingBreakdownDiff,
    /// Differences in detection results
    pub detection_diff: DetectionResultsDiff,
    /// Transactions with different tags between the two runs
    pub tag_differences: Vec<TagDifference>,
    /// Transactions with different merchant names between the two runs
    pub merchant_differences: Vec<MerchantDifference>,
}

/// Difference in tagging breakdown between two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggingBreakdownDiff {
    pub learned_diff: i64,
    pub rule_diff: i64,
    pub pattern_diff: i64,
    pub ollama_diff: i64,
    pub bank_category_diff: i64,
    pub fallback_diff: i64,
}

/// Difference in detection results between two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResultsDiff {
    pub subscriptions_diff: i64,
    pub zombies_diff: i64,
    pub price_increases_diff: i64,
    pub duplicates_diff: i64,
    pub receipts_matched_diff: i64,
}

/// A transaction's tag difference between two runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagDifference {
    pub transaction_id: i64,
    pub description: String,
    pub run_a_tags: Vec<String>,
    pub run_b_tags: Vec<String>,
}

/// A transaction's merchant difference between two runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantDifference {
    pub transaction_id: i64,
    pub description: String,
    pub run_a_merchant: Option<String>,
    pub run_b_merchant: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_source_as_str() {
        assert_eq!(TagSource::Manual.as_str(), "manual");
        assert_eq!(TagSource::Pattern.as_str(), "pattern");
        assert_eq!(TagSource::Ollama.as_str(), "ollama");
        assert_eq!(TagSource::Rule.as_str(), "rule");
    }

    #[test]
    fn test_tag_source_from_str() {
        assert_eq!("manual".parse::<TagSource>().unwrap(), TagSource::Manual);
        assert_eq!("PATTERN".parse::<TagSource>().unwrap(), TagSource::Pattern);
        assert_eq!("Ollama".parse::<TagSource>().unwrap(), TagSource::Ollama);
        assert_eq!("rule".parse::<TagSource>().unwrap(), TagSource::Rule);
        assert!("invalid".parse::<TagSource>().is_err());
    }

    #[test]
    fn test_tag_source_serde() {
        let source = TagSource::Ollama;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, r#""ollama""#);

        let parsed: TagSource = serde_json::from_str(r#""pattern""#).unwrap();
        assert_eq!(parsed, TagSource::Pattern);
    }

    #[test]
    fn test_pattern_type_as_str() {
        assert_eq!(PatternType::Contains.as_str(), "contains");
        assert_eq!(PatternType::Regex.as_str(), "regex");
        assert_eq!(PatternType::Exact.as_str(), "exact");
    }

    #[test]
    fn test_pattern_type_from_str() {
        assert_eq!(
            "contains".parse::<PatternType>().unwrap(),
            PatternType::Contains
        );
        assert_eq!("REGEX".parse::<PatternType>().unwrap(), PatternType::Regex);
        assert_eq!("Exact".parse::<PatternType>().unwrap(), PatternType::Exact);
        assert!("invalid".parse::<PatternType>().is_err());
    }

    #[test]
    fn test_pattern_type_serde() {
        let pt = PatternType::Regex;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, r#""regex""#);

        let parsed: PatternType = serde_json::from_str(r#""contains""#).unwrap();
        assert_eq!(parsed, PatternType::Contains);
    }

    #[test]
    fn test_tag_serde() {
        let tag = Tag {
            id: 1,
            name: "Groceries".to_string(),
            parent_id: None,
            color: Some("#10b981".to_string()),
            icon: Some("shopping-cart".to_string()),
            auto_patterns: Some("SAFEWAY|TRADER JOE".to_string()),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&tag).unwrap();
        assert!(json.contains("Groceries"));
        assert!(json.contains("#10b981"));
        assert!(json.contains("SAFEWAY|TRADER JOE"));

        let parsed: Tag = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Groceries");
        assert_eq!(parsed.color, Some("#10b981".to_string()));
    }

    #[test]
    fn test_tag_with_path_serde() {
        let tag = Tag {
            id: 2,
            name: "Streaming".to_string(),
            parent_id: Some(1),
            color: None,
            icon: None,
            auto_patterns: Some("NETFLIX|HULU|DISNEY".to_string()),
            created_at: Utc::now(),
        };

        let tag_with_path = TagWithPath {
            tag,
            path: "Entertainment.Streaming".to_string(),
            depth: 1,
            children: vec![],
        };

        let json = serde_json::to_string(&tag_with_path).unwrap();
        assert!(json.contains("Entertainment.Streaming"));
        assert!(json.contains("\"depth\":1"));
        // children should be omitted when empty
        assert!(!json.contains("children"));
    }

    #[test]
    fn test_transaction_tag_serde() {
        let tt = TransactionTag {
            transaction_id: 100,
            tag_id: 5,
            source: TagSource::Ollama,
            confidence: Some(0.85),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&tt).unwrap();
        assert!(json.contains("\"source\":\"ollama\""));
        assert!(json.contains("0.85"));

        let parsed: TransactionTag = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.transaction_id, 100);
        assert_eq!(parsed.source, TagSource::Ollama);
    }

    #[test]
    fn test_tag_rule_serde() {
        let rule = TagRule {
            id: 1,
            tag_id: 3,
            pattern: "SHELL|CHEVRON|EXXON".to_string(),
            pattern_type: PatternType::Contains,
            priority: 10,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("SHELL|CHEVRON|EXXON"));
        assert!(json.contains("\"pattern_type\":\"contains\""));
        assert!(json.contains("\"priority\":10"));
    }

    #[test]
    fn test_tag_spending_serde() {
        let spending = TagSpending {
            tag_id: 1,
            tag_name: "Transport".to_string(),
            tag_path: "Transport".to_string(),
            direct_amount: 150.0,
            total_amount: 450.0,
            transaction_count: 15,
        };

        let json = serde_json::to_string(&spending).unwrap();
        assert!(json.contains("Transport"));
        assert!(json.contains("150"));
        assert!(json.contains("450"));
    }

    #[test]
    fn test_delete_tag_result_serde() {
        let result = DeleteTagResult {
            deleted_tag_id: 5,
            transactions_moved: 42,
            children_affected: 3,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: DeleteTagResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.deleted_tag_id, 5);
        assert_eq!(parsed.transactions_moved, 42);
        assert_eq!(parsed.children_affected, 3);
    }
}

// API Response types that mirror the Rust models

export type Bank = "chase" | "bofa" | "amex" | "capitalone";

export type AccountType = "checking" | "savings" | "credit";

export interface Account {
  id: number;
  name: string;
  bank: Bank;
  account_type: AccountType | null;
  entity_id: number | null;
  created_at: string;
}

export type PaymentMethod = "apple_pay" | "google_pay" | "physical_card" | "online" | "recurring";

export interface Transaction {
  id: number;
  account_id: number;
  date: string;
  description: string;
  amount: number;
  category: string | null;
  merchant_normalized: string | null;
  import_hash: string;
  purchase_location_id: number | null;
  vendor_location_id: number | null;
  trip_id: number | null;
  source: TransactionSource;
  expected_amount: number | null;
  archived: boolean;
  original_data: string | null;
  import_format: string | null;
  card_member: string | null;
  payment_method: PaymentMethod | null;
  created_at: string;
  tags?: TransactionTag[];
}

export type Frequency = "weekly" | "monthly" | "yearly";

export type SubscriptionStatus = "active" | "cancelled" | "zombie" | "excluded";

export interface Subscription {
  id: number;
  merchant: string;
  account_id: number | null;
  amount: number | null;
  frequency: Frequency | null;
  first_seen: string | null;
  last_seen: string | null;
  status: SubscriptionStatus;
  user_acknowledged: boolean;
  acknowledged_at: string | null;
  created_at: string;
}

export type AlertType = "zombie" | "price_increase" | "duplicate" | "resume" | "spending_anomaly" | "tip_discrepancy";

export interface ServiceFeature {
  service: string;
  unique: string;
}

export interface DuplicateAnalysis {
  overlap: string;
  unique_features: ServiceFeature[];
}

export interface SpendingChangeExplanation {
  summary: string;
  reasons: string[];
  model: string;
  analyzed_at: string;
}

export interface SpendingAnomalyData {
  tag_id: number;
  tag_name: string;
  baseline_amount: number;
  current_amount: number;
  percent_change: number;
  explanation?: SpendingChangeExplanation;
}

export interface Alert {
  id: number;
  alert_type: AlertType;
  subscription_id: number | null;
  message: string | null;
  dismissed: boolean;
  created_at: string;
  ollama_analysis?: DuplicateAnalysis;
  spending_anomaly?: SpendingAnomalyData;
  subscription?: Subscription;
}

export interface DashboardStats {
  total_transactions: number;
  total_accounts: number;
  active_subscriptions: number;
  monthly_subscription_cost: number;
  active_alerts: number;
  potential_monthly_savings: number;
  recent_imports: RecentImport[];
  untagged_transactions: number;
}

export type TagSource = "manual" | "pattern" | "ollama" | "rule" | "bank_category" | "learned";

export type TransactionSource = "import" | "receipt" | "manual";

export interface Tag {
  id: number;
  name: string;
  parent_id: number | null;
  color: string | null;
  icon: string | null;
  auto_patterns: string | null;
  created_at: string;
}

export interface TagWithPath {
  id: number;
  name: string;
  parent_id: number | null;
  color: string | null;
  icon: string | null;
  auto_patterns: string | null;
  path: string;
  depth: number;
  children: TagWithPath[];
}

export type PatternType = "contains" | "regex" | "exact";

export interface TagRule {
  id: number;
  tag_id: number;
  pattern: string;
  pattern_type: PatternType;
  priority: number;
  created_at: string;
  tag_name: string;
  tag_path: string;
}

export interface TransactionTag {
  tag_id: number;
  tag_name: string;
  tag_path: string;
  tag_color: string | null;
  source: TagSource;
  confidence: number | null;
}

export interface RecentImport {
  account_name: string;
  bank: Bank;
  transaction_count: number;
  imported_at: string;
}

export interface TransactionResponse {
  transactions: Transaction[];
  total: number;
  limit: number;
  offset: number;
}

export interface DetectionResults {
  subscriptions_found: number;
  zombies_detected: number;
  price_increases_detected: number;
  duplicates_detected: number;
  auto_cancelled: number;
  resumes_detected: number;
  tip_discrepancies_detected: number;
}

export interface ImportTaggingBreakdown {
  by_learned: number;
  by_rule: number;
  by_pattern: number;
  by_ollama: number;
  by_bank_category: number;
  fallback: number;
}

export interface ImportResponse {
  imported: number;
  skipped: number;
  account_name: string;
  bank: string;
  // Import session ID for retrieving history
  import_session_id: number;
  // Tagging results (auto-run after import)
  transactions_tagged: number;
  // Tagging breakdown by source
  tagging_breakdown: ImportTaggingBreakdown;
  // Receipt matching results
  receipts_matched: number;
  // Detection results (auto-run after import)
  subscriptions_found: number;
  zombies_detected: number;
  price_increases_detected: number;
  duplicates_detected: number;
  tip_discrepancies_detected: number;
}

// ========== Import History Types ==========

export type ImportStatus = "pending" | "processing" | "completed" | "failed" | "cancelled";

export interface ImportSession {
  id: number;
  account_id: number;
  filename: string | null;
  file_size_bytes: number | null;
  bank: Bank;
  imported_count: number;
  skipped_count: number;
  tagged_by_learned: number;
  tagged_by_rule: number;
  tagged_by_pattern: number;
  tagged_by_ollama: number;
  tagged_by_bank_category: number;
  tagged_fallback: number;
  subscriptions_found: number;
  zombies_detected: number;
  price_increases_detected: number;
  duplicates_detected: number;
  receipts_matched: number;
  user_email: string | null;
  ollama_model: string | null;
  // Processing status for async imports
  status: ImportStatus;
  processing_phase: string | null;
  processing_current: number;
  processing_total: number;
  processing_error: string | null;
  // Phase timing (milliseconds)
  tagging_duration_ms: number | null;
  normalizing_duration_ms: number | null;
  matching_duration_ms: number | null;
  detecting_duration_ms: number | null;
  total_duration_ms: number | null;
  created_at: string;
}

export interface ImportSessionWithAccount {
  session: ImportSession;
  account_name: string;
}

export interface ImportSessionsResponse {
  sessions: ImportSessionWithAccount[];
  total: number;
}

export interface ImportTransactionsResponse {
  transactions: Transaction[];
  total: number;
}

export interface SkippedTransaction {
  id: number;
  import_session_id: number;
  date: string;
  description: string;
  amount: number;
  import_hash: string;
  existing_transaction_id: number | null;
  created_at: string;
}

export interface ReprocessStartResponse {
  session_id: number;
  run_id: number;
  run_number: number;
  message: string;
}

export interface CancelImportResponse {
  cancelled: boolean;
  message: string;
}

// Snapshot of a transaction's tags and merchant name for comparison
export interface TransactionTagSnapshot {
  id: number;
  description: string;
  merchant_normalized: string | null;
  tags: string[];
}

// Snapshot of import session state for before/after comparison
export interface ReprocessSnapshot {
  tagging_breakdown: ImportTaggingBreakdown;
  subscriptions_found: number;
  zombies_detected: number;
  price_increases_detected: number;
  duplicates_detected: number;
  receipts_matched: number;
  sample_transactions: TransactionTagSnapshot[];
}

// A transaction whose tags changed during reprocessing
export interface TagChange {
  transaction_id: number;
  description: string;
  before_tags: string[];
  after_tags: string[];
}

// A transaction whose merchant name changed during reprocessing
export interface MerchantChange {
  transaction_id: number;
  description: string;
  before_merchant: string | null;
  after_merchant: string | null;
}

// Complete before/after comparison from reprocessing
export interface ReprocessComparison {
  before: ReprocessSnapshot;
  after: ReprocessSnapshot;
  tag_changes: TagChange[];
  merchant_changes: MerchantChange[];
}

// ========== Reprocess Run Types (Historical Comparison) ==========

export type ReprocessRunStatus = "running" | "completed" | "failed";

// Summary of a reprocess run (for list views)
export interface ReprocessRunSummary {
  id: number;
  run_number: number;
  ollama_model: string | null;
  status: ReprocessRunStatus;
  initiated_by: string | null;
  started_at: string;
  completed_at: string | null;
  tags_changed: number;
  merchants_changed: number;
}

// Full reprocess run with comparison data
export interface ReprocessRunWithComparison {
  id: number;
  import_session_id: number;
  run_number: number;
  ollama_model: string | null;
  status: ReprocessRunStatus;
  initiated_by: string | null;
  reason: string | null;
  started_at: string;
  completed_at: string | null;
  created_at: string;
  before: ReprocessSnapshot | null;
  after: ReprocessSnapshot | null;
  tag_changes: TagChange[] | null;
  merchant_changes: MerchantChange[] | null;
}

// Difference in tagging breakdown between two snapshots
export interface TaggingBreakdownDiff {
  learned_diff: number;
  rule_diff: number;
  pattern_diff: number;
  ollama_diff: number;
  bank_category_diff: number;
  fallback_diff: number;
}

// Difference in detection results between two snapshots
export interface DetectionResultsDiff {
  subscriptions_diff: number;
  zombies_diff: number;
  price_increases_diff: number;
  duplicates_diff: number;
  receipts_matched_diff: number;
}

// A transaction's tag difference between two runs
export interface TagDifference {
  transaction_id: number;
  description: string;
  run_a_tags: string[];
  run_b_tags: string[];
}

// A transaction's merchant difference between two runs
export interface MerchantDifference {
  transaction_id: number;
  description: string;
  run_a_merchant: string | null;
  run_b_merchant: string | null;
}

// Comparison between two specific runs
export interface RunComparison {
  run_a: ReprocessRunSummary;
  run_b: ReprocessRunSummary;
  tagging_diff: TaggingBreakdownDiff;
  detection_diff: DetectionResultsDiff;
  tag_differences: TagDifference[];
  merchant_differences: MerchantDifference[];
}

// ========== Report Types ==========

export type ReportTab = "spending" | "trends" | "merchants" | "subscriptions";

export type Period = "this-month" | "last-month" | "last-30-days" | "last-90-days" | "this-year" | "last-year" | "all" | "custom";

export interface DateRange {
  from?: string;
  to?: string;
}

export interface ReportPeriod {
  from: string;
  to: string;
}

export interface CategorySpending {
  tag: string;
  tag_id: number;
  amount: number;
  percentage: number;
  transaction_count: number;
  children: CategorySpending[];
}

export interface UntaggedSummary {
  amount: number;
  percentage: number;
  transaction_count: number;
}

export interface SpendingSummary {
  period: ReportPeriod;
  total: number;
  categories: CategorySpending[];
  untagged: UntaggedSummary;
}

export type Granularity = "weekly" | "monthly";

export interface TrendDataPoint {
  period: string;
  amount: number;
  transaction_count: number;
}

export interface TrendsReport {
  granularity: Granularity;
  period: ReportPeriod;
  tag?: string;
  data: TrendDataPoint[];
}

export interface MerchantSummary {
  merchant: string;
  amount: number;
  transaction_count: number;
}

export interface MerchantsReport {
  period: ReportPeriod;
  limit: number;
  merchants: MerchantSummary[];
}

export interface WasteBreakdown {
  zombie_count: number;
  zombie_monthly: number;
  duplicate_count: number;
  duplicate_monthly: number;
  price_increase_count: number;
  price_increase_delta: number;
  total_waste_monthly: number;
}

export interface SubscriptionInfo {
  id: number;
  merchant: string;
  amount: number;
  frequency: Frequency;
  status: SubscriptionStatus;
  first_seen: string;
  last_seen: string;
}

export interface SubscriptionSummaryReport {
  total_monthly: number;
  active_count: number;
  cancelled_count: number;
  subscriptions: SubscriptionInfo[];
  waste: WasteBreakdown;
}

export interface CancelledSubscriptionInfo {
  id: number;
  merchant: string;
  monthly_amount: number;
  cancelled_at: string;
  months_counted: number;
  savings: number;
}

export interface SavingsReport {
  total_savings: number;
  total_monthly_saved: number;
  cancelled_count: number;
  cancelled: CancelledSubscriptionInfo[];
}

// ========== Entity Types ==========

export type EntityType = "person" | "pet" | "vehicle" | "property";

export interface Entity {
  id: number;
  name: string;
  entity_type: EntityType;
  icon: string | null;
  color: string | null;
  archived: boolean;
  created_at: string;
}

// ========== Split Types ==========

export type SplitType = "item" | "tax" | "tip" | "fee" | "discount" | "rewards";

export interface TransactionSplit {
  id: number;
  transaction_id: number;
  amount: number;
  description: string | null;
  split_type: SplitType;
  entity_id: number | null;
  purchaser_id: number | null;
  created_at: string;
}

export interface TransactionSplitWithDetails extends TransactionSplit {
  entity_name: string | null;
  purchaser_name: string | null;
  tags: TransactionTag[];
}

// ========== Location Types ==========

export type LocationType = "home" | "work" | "store" | "online" | "travel";

export interface Location {
  id: number;
  name: string | null;
  address: string | null;
  city: string | null;
  state: string | null;
  country: string;
  latitude: number | null;
  longitude: number | null;
  location_type: LocationType | null;
  created_at: string;
}

// ========== Receipt Types ==========

export type ReceiptStatus = "pending" | "matched" | "manual_review" | "orphaned";
export type ReceiptRole = "primary" | "supplementary";

export interface Receipt {
  id: number;
  transaction_id: number | null;
  image_path: string | null;
  parsed_json: string | null;
  parsed_at: string | null;
  status: ReceiptStatus;
  role: ReceiptRole;
  receipt_date: string | null;
  receipt_total: number | null;
  receipt_merchant: string | null;
  content_hash: string | null;
  created_at: string;
}

export interface ParsedReceiptItem {
  description: string;
  quantity: number | null;
  unit_price: number | null;
  total: number | null;
  category: string | null;
}

export interface ParsedReceipt {
  merchant: string | null;
  date: string | null;
  total: number | null;
  subtotal: number | null;
  tax: number | null;
  tip: number | null;
  items: ParsedReceiptItem[];
  payment_method: string | null;
  confidence: number | null;
}

export interface ReceiptUploadResponse {
  receipt: Receipt;
  image_path: string;
}

export interface PendingReceiptResponse {
  receipt: Receipt;
  image_path: string;
  parsed: ParsedReceipt | null;
}

export interface ReceiptParseResponse {
  receipt_id: number;
  parsed: ParsedReceipt;
  raw_json: string;
}

// ========== Ollama Metrics Types ==========

export type OllamaOperation = "classify_merchant" | "parse_receipt" | "suggest_entity" | "suggest_split";

export interface ToolCallRecord {
  name: string;
  input: Record<string, unknown>;
  success: boolean;
  output: string | null;
}

export interface OllamaMetric {
  id: number;
  operation: OllamaOperation;
  model: string;
  started_at: string;
  latency_ms: number;
  success: boolean;
  error_message: string | null;
  confidence: number | null;
  transaction_id: number | null;
  input_text: string | null;
  result_text: string | null;
  metadata: string | null;
}

export interface OperationStats {
  operation: string;
  call_count: number;
  success_rate: number;
  avg_latency_ms: number;
  avg_confidence: number | null;
}

export interface AccuracyStats {
  total_corrections: number;
  total_ollama_tags: number;
  correction_rate: number;
  estimated_accuracy: number;
}

export interface OllamaStats {
  period_start: string;
  period_end: string;
  total_calls: number;
  successful_calls: number;
  failed_calls: number;
  success_rate: number;
  avg_latency_ms: number;
  p50_latency_ms: number;
  p95_latency_ms: number;
  max_latency_ms: number;
  by_operation: OperationStats[];
  accuracy: AccuracyStats;
}

export interface OllamaHealthStatus {
  available: boolean;
  host: string | null;
  model: string | null;
  last_successful_call: string | null;
  last_failed_call: string | null;
  recent_error_rate: number;
  /** Whether the AI orchestrator (agentic mode) is configured and available */
  orchestrator_available: boolean;
  /** Host for orchestrator (Anthropic-compatible endpoint) */
  orchestrator_host?: string | null;
  /** Model used for orchestrator */
  orchestrator_model?: string | null;
}

export interface StatsSummary {
  success_rate: number;
  avg_latency_ms: number;
  estimated_accuracy: number;
  latency_trend: string;
}

export interface ModelRecommendation {
  current_model: string | null;
  stats_summary: StatsSummary;
  recommendations: string[];
  should_switch: boolean;
}

export interface ModelStats {
  model: string;
  total_calls: number;
  successful_calls: number;
  failed_calls: number;
  success_rate: number;
  avg_latency_ms: number;
  p50_latency_ms: number;
  p95_latency_ms: number;
  max_latency_ms: number;
  avg_confidence: number | null;
  by_operation: OperationStats[];
  first_used: string | null;
  last_used: string | null;
}

export interface ModelComparisonStats {
  period_start: string;
  period_end: string;
  models: ModelStats[];
}

export interface ReprocessResponse {
  transaction_id: number;
  success: boolean;
  new_tag: string | null;
  normalized_merchant: string | null;
  source: string | null;
  confidence: number | null;
  error: string | null;
}

export interface BulkReprocessResponse {
  processed: number;
  success_count: number;
  failed_count: number;
  results: ReprocessResponse[];
}

export interface BulkTagsResponse {
  processed: number;
  success_count: number;
  failed_count: number;
}

// ========== Feedback Types ==========

export type FeedbackType = "helpful" | "not_helpful" | "correction" | "dismissal";

export type FeedbackTargetType = "alert" | "insight" | "classification" | "explanation" | "receipt_match";

export interface FeedbackContext {
  model?: string;
  prompt_version?: string;
  transaction_id?: number;
  extra?: Record<string, unknown>;
}

export interface UserFeedback {
  id: number;
  feedback_type: FeedbackType;
  target_type: FeedbackTargetType;
  target_id: number | null;
  original_value: string | null;
  corrected_value: string | null;
  reason: string | null;
  context: FeedbackContext | null;
  created_at: string;
  reverted_at: string | null;
}

export interface FeedbackTargetStats {
  target_type: FeedbackTargetType;
  total: number;
  helpful: number;
  not_helpful: number;
  helpfulness_ratio: number;
}

export interface FeedbackStats {
  total_feedback: number;
  helpful_count: number;
  not_helpful_count: number;
  correction_count: number;
  dismissal_count: number;
  reverted_count: number;
  by_target_type: FeedbackTargetStats[];
}

export interface FeedbackResponse {
  id: number;
  feedback: UserFeedback;
}

// Insight Engine types

export type InsightType = "spending_explainer" | "expense_forecaster" | "savings_opportunity";

export type InsightSeverity = "info" | "attention" | "warning" | "alert";

export type InsightStatus = "active" | "dismissed" | "snoozed";

export interface MerchantContribution {
  merchant: string;
  current: number;
  baseline: number;
  change: number;
}

export interface SpendingExplainerData {
  tag_id: number;
  tag_name: string;
  current_amount: number;
  baseline_amount: number;
  percent_change: number;
  explanation?: string;
  top_merchants: MerchantContribution[];
}

export type ForecastItemType = "subscription" | "estimate" | "large_expense";

export interface ForecastItem {
  item_type: ForecastItemType;
  name: string;
  amount: number;
  due_date?: string;
  basis?: string;
}

export interface ExpenseForecasterData {
  period_days: number;
  total_expected: number;
  items: ForecastItem[];
}

export type SavingsOpportunityType = "zombie" | "duplicate" | "annual_switch";

export interface SavingsOpportunityData {
  opportunity_type: SavingsOpportunityType;
  subscription_id?: number;
  subscription_name?: string;
  monthly_amount: number;
  annual_savings: number;
  reason: string;
  alert_id?: number;
}

export interface InsightFinding {
  id: number;
  insight_type: InsightType;
  finding_key: string;
  severity: InsightSeverity;
  title: string;
  summary: string;
  detail?: string;
  data: SpendingExplainerData | ExpenseForecasterData | SavingsOpportunityData | Record<string, unknown>;
  first_detected_at: string;
  last_detected_at: string;
  status: InsightStatus;
  snoozed_until?: string;
  user_feedback?: string;
}

export interface InsightRefreshResponse {
  count: number;
}

// Explore mode types
export interface ExploreResponse {
  response: string;
  processing_time_ms: number;
  session_id: string;
  model: string;
  tool_calls: ToolCallRecord[];
  iterations: number;
}

export interface ExploreModelsResponse {
  models: string[];
  default_model: string;
}

export interface ExploreSessionInfo {
  session_id: string;
  message_count: number;
  created_at_secs_ago: number;
  last_activity_secs_ago: number;
}

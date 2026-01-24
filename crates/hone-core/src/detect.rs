//! Waste detection algorithms
//!
//! Detects:
//! - Zombie subscriptions: recurring charges you might have forgotten
//! - Price increases: services that quietly raised prices
//! - Duplicate services: multiple subscriptions in the same category

use chrono::{Datelike, Duration, NaiveDate, Utc};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::ai::orchestrator::AIOrchestrator;
use crate::ai::{AIBackend, AIClient, DuplicateAnalysis, ServiceFeature};
use crate::db::Database;
use crate::error::Result;
use crate::models::{
    AlertType, FeedbackTargetType, Frequency, SpendingAnomalyData, SpendingChangeExplanation,
    SubscriptionStatus, Transaction,
};
use crate::prompts::{PromptId, PromptLibrary};
use crate::tools;

/// Progress callback for detection phases
/// Parameters: (phase_name, current, total)
pub type ProgressCallback = Box<dyn Fn(&str, i64, i64) + Send + Sync>;

/// Detection configuration
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Minimum months of recurring charges to flag as potential zombie
    pub zombie_min_months: i64,
    /// Price increase threshold (percentage)
    pub price_increase_percent: f64,
    /// Price increase threshold (absolute dollars)
    pub price_increase_absolute: f64,
    /// Grace period (in days) after expected charge date before marking as cancelled
    /// For monthly: 7 days grace, for weekly: 3 days, for yearly: 30 days
    pub cancellation_grace_days_monthly: i64,

    // Smart detection thresholds (used when Ollama confirms subscription)
    /// Amount variance allowed for Ollama-confirmed subscriptions (e.g., 0.50 = 50%)
    pub smart_amount_variance: f64,
    /// Interval consistency required for Ollama-confirmed subscriptions (e.g., 0.50 = 50%)
    pub smart_interval_consistency: f64,
    /// Minimum transactions for Ollama-confirmed subscriptions
    pub smart_min_transactions: usize,
    /// Minimum Ollama confidence to use relaxed thresholds
    pub ollama_confidence_threshold: f64,

    // Spending anomaly detection thresholds
    /// Threshold for spending increase (percentage over 3-month baseline)
    pub spending_increase_threshold: f64,
    /// Threshold for spending decrease (percentage below 3-month baseline)
    pub spending_decrease_threshold: f64,
    /// Minimum baseline amount to consider for anomaly detection
    pub spending_anomaly_min_baseline: f64,

    // Subscription re-acknowledgment
    /// Days after which an acknowledgment is considered stale and the subscription
    /// may be flagged as a zombie again (0 = never stale)
    pub acknowledgment_stale_days: i64,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            zombie_min_months: 3,
            price_increase_percent: 5.0,
            price_increase_absolute: 1.0,
            cancellation_grace_days_monthly: 7,
            // Smart detection defaults
            smart_amount_variance: 0.50,      // 50% variance allowed
            smart_interval_consistency: 0.50, // 50% interval consistency required
            smart_min_transactions: 2,        // Only need 2 transactions
            ollama_confidence_threshold: 0.7, // 70% Ollama confidence required
            // Spending anomaly defaults
            spending_increase_threshold: 30.0, // 30% increase from baseline
            spending_decrease_threshold: 40.0, // 40% decrease from baseline
            spending_anomaly_min_baseline: 50.0, // Only flag if baseline >= $50/month
            // Re-acknowledgment defaults
            acknowledgment_stale_days: 90, // Re-check after 90 days (~quarterly)
        }
    }
}

/// Results of running detection
#[derive(Debug, Default)]
pub struct DetectionResults {
    pub subscriptions_found: usize,
    pub zombies_detected: usize,
    pub price_increases_detected: usize,
    pub duplicates_detected: usize,
    pub auto_cancelled: usize,
    pub resumes_detected: usize,
    pub spending_anomalies_detected: usize,
}

/// Main detector that runs all algorithms
pub struct WasteDetector<'a> {
    db: &'a Database,
    config: DetectionConfig,
    ai: Option<&'a AIClient>,
    orchestrator: Option<&'a AIOrchestrator>,
}

impl<'a> WasteDetector<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            config: DetectionConfig::default(),
            ai: None,
            orchestrator: None,
        }
    }

    pub fn with_config(db: &'a Database, config: DetectionConfig) -> Self {
        Self {
            db,
            config,
            ai: None,
            orchestrator: None,
        }
    }

    pub fn with_ai(db: &'a Database, ai: &'a AIClient) -> Self {
        Self {
            db,
            config: DetectionConfig::default(),
            ai: Some(ai),
            orchestrator: None,
        }
    }

    pub fn with_config_and_ai(db: &'a Database, config: DetectionConfig, ai: &'a AIClient) -> Self {
        Self {
            db,
            config,
            ai: Some(ai),
            orchestrator: None,
        }
    }

    /// Create detector with AI orchestrator for agentic analysis
    ///
    /// When the orchestrator is set, the detector will use tool-calling
    /// for richer spending analysis (the AI can query transactions, merchants, etc.)
    pub fn with_orchestrator(db: &'a Database, orchestrator: &'a AIOrchestrator) -> Self {
        Self {
            db,
            config: DetectionConfig::default(),
            ai: None,
            orchestrator: Some(orchestrator),
        }
    }

    /// Create detector with both standard AI client and orchestrator
    pub fn with_ai_and_orchestrator(
        db: &'a Database,
        ai: &'a AIClient,
        orchestrator: &'a AIOrchestrator,
    ) -> Self {
        Self {
            db,
            config: DetectionConfig::default(),
            ai: Some(ai),
            orchestrator: Some(orchestrator),
        }
    }

    /// Create detector with config, AI, and orchestrator
    pub fn with_all(
        db: &'a Database,
        config: DetectionConfig,
        ai: &'a AIClient,
        orchestrator: &'a AIOrchestrator,
    ) -> Self {
        Self {
            db,
            config,
            ai: Some(ai),
            orchestrator: Some(orchestrator),
        }
    }

    /// Run all detection algorithms
    pub async fn detect_all(&self) -> Result<DetectionResults> {
        self.detect_all_with_progress(None).await
    }

    /// Run all detection algorithms with progress callback
    ///
    /// The callback receives (phase, current, total) updates during long-running operations.
    /// Phases: "classifying_merchants", "analyzing_duplicates", "analyzing_spending"
    pub async fn detect_all_with_progress(
        &self,
        progress: Option<&ProgressCallback>,
    ) -> Result<DetectionResults> {
        let subscriptions_found = self.identify_subscriptions_with_progress(progress).await?;
        let auto_cancelled = self.detect_cancelled()?;
        let resumes_detected = self.detect_resumed()?;
        let zombies_detected = self.detect_zombies()?;
        let price_increases_detected = self.detect_price_increases()?;
        let duplicates_detected = self.detect_duplicates_with_progress(progress).await?;
        let spending_anomalies_detected = self
            .detect_spending_anomalies_with_progress(progress)
            .await?;

        info!(
            "Detection complete: {} subscriptions, {} auto-cancelled, {} resumed, {} zombies, {} price increases, {} duplicates, {} spending anomalies",
            subscriptions_found, auto_cancelled, resumes_detected, zombies_detected, price_increases_detected, duplicates_detected, spending_anomalies_detected
        );

        Ok(DetectionResults {
            subscriptions_found,
            zombies_detected,
            price_increases_detected,
            duplicates_detected,
            auto_cancelled,
            resumes_detected,
            spending_anomalies_detected,
        })
    }

    /// Run only zombie detection
    pub async fn detect_zombies_only(&self) -> Result<DetectionResults> {
        Ok(DetectionResults {
            subscriptions_found: self.identify_subscriptions().await?,
            zombies_detected: self.detect_zombies()?,
            ..Default::default()
        })
    }

    /// Run only price increase detection
    pub async fn detect_increases_only(&self) -> Result<DetectionResults> {
        Ok(DetectionResults {
            subscriptions_found: self.identify_subscriptions().await?,
            price_increases_detected: self.detect_price_increases()?,
            ..Default::default()
        })
    }

    /// Run only duplicate detection
    pub async fn detect_duplicates_only(&self) -> Result<DetectionResults> {
        Ok(DetectionResults {
            subscriptions_found: self.identify_subscriptions().await?,
            duplicates_detected: self.detect_duplicates().await?,
            ..Default::default()
        })
    }

    /// Identify recurring charges and create/update subscriptions
    async fn identify_subscriptions(&self) -> Result<usize> {
        self.identify_subscriptions_with_progress(None).await
    }

    /// Identify recurring charges and create/update subscriptions with progress reporting
    ///
    /// Uses a multi-layer approach:
    /// 1. Checks merchant_subscription_cache for known retail vs subscription merchants
    /// 2. If Ollama available and not cached, classifies merchant as subscription/retail
    /// 3. For Ollama-confirmed subscriptions, uses relaxed pattern detection thresholds
    /// 4. Falls back to strict pattern-based detection otherwise
    ///
    /// User exclusions (source='user_override') take precedence over Ollama classifications.
    async fn identify_subscriptions_with_progress(
        &self,
        progress: Option<&ProgressCallback>,
    ) -> Result<usize> {
        // Get all transactions (excluding archived)
        let transactions = self.db.list_transactions(None, 10000, 0)?;
        if transactions.is_empty() {
            return Ok(0);
        }

        // Get transaction IDs tagged with "Fees" (or Financial > Fees) to exclude from subscription detection
        // These are bank-generated charges (interest, late fees, annual fees) not merchant subscriptions
        let fees_transaction_ids =
            if let Ok(Some(fees_tag)) = self.db.get_tag_by_path("Financial.Fees") {
                self.db
                    .get_transaction_ids_with_tag(fees_tag.id)
                    .unwrap_or_default()
            } else {
                std::collections::HashSet::new()
            };

        // Group by (account_id, normalized merchant) to make subscriptions account-specific
        let mut by_account_merchant: HashMap<(i64, String), Vec<&Transaction>> = HashMap::new();
        for tx in &transactions {
            if tx.amount >= 0.0 {
                continue; // Skip income/credits
            }

            // Skip transactions tagged as Fees
            if fees_transaction_ids.contains(&tx.id) {
                debug!("Skipping transaction {} - tagged as Fees", tx.id);
                continue;
            }

            let merchant = tx
                .merchant_normalized
                .clone()
                .unwrap_or_else(|| normalize_merchant(&tx.description));

            by_account_merchant
                .entry((tx.account_id, merchant))
                .or_default()
                .push(tx);
        }

        let mut count = 0;

        // Count merchants that need Ollama classification (not cached)
        let merchants_to_check: Vec<_> = by_account_merchant
            .iter()
            .filter(|(_, txs)| txs.len() >= 2)
            .filter(|((_, merchant), _)| {
                // Check if NOT cached - these will need Ollama calls
                self.db
                    .get_merchant_subscription_cache(merchant)
                    .ok()
                    .flatten()
                    .is_none()
            })
            .collect();

        let total_merchants = merchants_to_check.len() as i64;
        let mut processed_merchants = 0i64;

        // Report initial progress if we have merchants to classify
        if let Some(cb) = progress {
            if total_merchants > 0 && self.ai.is_some() {
                cb("classifying_merchants", 0, total_merchants);
            }
        }

        for ((account_id, merchant), txs) in by_account_merchant {
            if txs.len() < 2 {
                continue; // Need at least 2 transactions to detect pattern
            }

            // Layer 1: Check cache for known retail merchants (from Ollama or user exclusion)
            let cached_result = self.db.get_merchant_subscription_cache(&merchant);
            if let Ok(Some(is_subscription)) = cached_result {
                if !is_subscription {
                    debug!(
                        "Skipping {} - cached as retail/not a subscription",
                        merchant
                    );
                    continue;
                }
                // Cached as subscription - use strict pattern detection
                // (user confirmed or previously detected)
                if let Some(sub_info) = detect_subscription_pattern(&txs) {
                    self.db.upsert_subscription(
                        &merchant,
                        Some(account_id),
                        Some(sub_info.amount),
                        Some(sub_info.frequency),
                        Some(sub_info.first_seen),
                        Some(sub_info.last_seen),
                    )?;
                    count += 1;
                    debug!(
                        "Found subscription (cached): {} (account {}) @ ${:.2}/{:?}",
                        merchant, account_id, sub_info.amount, sub_info.frequency
                    );
                }
                continue;
            }

            // Layer 2: If Ollama available and not cached, classify merchant
            let mut use_relaxed_detection = false;
            if let Some(ollama) = self.ai {
                // Update progress before Ollama call
                processed_merchants += 1;
                if let Some(cb) = progress {
                    cb(
                        "classifying_merchants",
                        processed_merchants,
                        total_merchants,
                    );
                }

                match ollama.is_subscription_service(&merchant).await {
                    Ok(classification) => {
                        // Cache the result
                        let _ = self.db.cache_subscription_classification(
                            &merchant,
                            classification.is_subscription,
                            Some(classification.confidence),
                        );

                        if !classification.is_subscription {
                            debug!(
                                "Skipping {} - Ollama classified as retail (confidence: {:.2}): {}",
                                merchant, classification.confidence, classification.reason
                            );
                            continue;
                        }

                        // Use relaxed detection if Ollama confidence is high enough
                        if classification.confidence >= self.config.ollama_confidence_threshold {
                            use_relaxed_detection = true;
                            debug!(
                                "Using smart detection for {} - Ollama confidence: {:.2} ({})",
                                merchant, classification.confidence, classification.reason
                            );
                        }
                    }
                    Err(e) => {
                        debug!("Ollama classification failed for {}: {}", merchant, e);
                        // Fall through to strict pattern detection
                    }
                }
            }

            // Layer 3: Pattern-based detection (relaxed or strict)
            let sub_info = if use_relaxed_detection {
                detect_subscription_pattern_relaxed(&txs, &self.config)
            } else {
                detect_subscription_pattern(&txs)
            };

            if let Some(sub_info) = sub_info {
                self.db.upsert_subscription(
                    &merchant,
                    Some(account_id),
                    Some(sub_info.amount),
                    Some(sub_info.frequency),
                    Some(sub_info.first_seen),
                    Some(sub_info.last_seen),
                )?;
                count += 1;
                let detection_type = if use_relaxed_detection {
                    "smart"
                } else {
                    "strict"
                };
                debug!(
                    "Found subscription ({}): {} (account {}) @ ${:.2}/{:?}",
                    detection_type, merchant, account_id, sub_info.amount, sub_info.frequency
                );
            }
        }

        Ok(count)
    }

    /// Detect zombie subscriptions
    ///
    /// Flags subscriptions as zombies if:
    /// 1. Never acknowledged and running for >= zombie_min_months, OR
    /// 2. Previously acknowledged but acknowledgment is stale (> acknowledgment_stale_days old)
    fn detect_zombies(&self) -> Result<usize> {
        let subscriptions = self.db.list_subscriptions(None)?;
        let mut count = 0;
        let now = Utc::now();
        let today = now.date_naive();
        let threshold = today - Duration::days(self.config.zombie_min_months * 30);

        for sub in subscriptions {
            // Skip if not active (excluded, cancelled, or already zombie)
            if sub.status != SubscriptionStatus::Active {
                continue;
            }

            // Check if acknowledgment is stale (if configured and acknowledged)
            let is_stale_acknowledgment =
                if sub.user_acknowledged && self.config.acknowledgment_stale_days > 0 {
                    match sub.acknowledged_at {
                        Some(ack_time) => {
                            let stale_threshold =
                                now - chrono::Duration::days(self.config.acknowledgment_stale_days);
                            ack_time < stale_threshold
                        }
                        // If acknowledged but no timestamp, treat as fresh (legacy data)
                        None => false,
                    }
                } else {
                    false
                };

            // Skip if acknowledged and not stale
            if sub.user_acknowledged && !is_stale_acknowledgment {
                continue;
            }

            // Check if first seen is old enough
            let first_seen = match sub.first_seen {
                Some(d) => d,
                None => continue,
            };

            if first_seen > threshold {
                continue; // Too new to be a zombie
            }

            // This subscription is either:
            // - Never acknowledged and running for a while, OR
            // - Has a stale acknowledgment (time to re-check)
            self.db
                .update_subscription_status(sub.id, SubscriptionStatus::Zombie)?;

            let message = if is_stale_acknowledgment {
                format!(
                    "It's been a while since you confirmed {} (${:.2}/mo). Still using it?",
                    sub.merchant,
                    sub.amount.unwrap_or(0.0)
                )
            } else {
                format!(
                    "You've been paying ${:.2} for {} since {}. Still using it?",
                    sub.amount.unwrap_or(0.0),
                    sub.merchant,
                    first_seen.format("%B %Y")
                )
            };

            self.db
                .create_alert(AlertType::Zombie, Some(sub.id), Some(&message))?;
            count += 1;
        }

        Ok(count)
    }

    /// Detect price increases
    fn detect_price_increases(&self) -> Result<usize> {
        let subscriptions = self.db.list_subscriptions(None)?;
        let transactions = self.db.list_transactions(None, 10000, 0)?;

        let mut count = 0;
        let three_months_ago = Utc::now().date_naive() - Duration::days(90);

        for sub in subscriptions {
            // Skip excluded subscriptions (user said "not a subscription")
            if sub.status == SubscriptionStatus::Excluded {
                continue;
            }

            if sub.amount.is_none() {
                continue;
            }

            let current_amount = sub.amount.unwrap().abs();

            // Find transactions for this merchant
            let merchant_txs: Vec<&Transaction> = transactions
                .iter()
                .filter(|tx| {
                    let tx_merchant = tx
                        .merchant_normalized
                        .clone()
                        .unwrap_or_else(|| normalize_merchant(&tx.description));
                    tx_merchant == sub.merchant
                })
                .collect();

            // Find the amount from ~3 months ago
            let old_txs: Vec<&&Transaction> = merchant_txs
                .iter()
                .filter(|tx| tx.date < three_months_ago)
                .collect();

            if old_txs.is_empty() {
                continue;
            }

            // Get the most recent old amount
            let old_amount = old_txs
                .iter()
                .max_by_key(|tx| tx.date)
                .map(|tx| tx.amount.abs())
                .unwrap_or(current_amount);

            // Check if price increased
            let increase = current_amount - old_amount;
            let increase_percent = (increase / old_amount) * 100.0;

            if increase > self.config.price_increase_absolute
                || increase_percent > self.config.price_increase_percent
            {
                let message = format!(
                    "{} increased from ${:.2} to ${:.2} (+{:.1}%)",
                    sub.merchant, old_amount, current_amount, increase_percent
                );

                self.db
                    .create_alert(AlertType::PriceIncrease, Some(sub.id), Some(&message))?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Detect duplicate services in the same category
    /// Uses tag-based categorization from Subscriptions children
    /// If Ollama is available, analyzes overlap and unique features of each service
    async fn detect_duplicates(&self) -> Result<usize> {
        self.detect_duplicates_with_progress(None).await
    }

    /// Detect duplicate subscriptions with progress reporting
    async fn detect_duplicates_with_progress(
        &self,
        progress: Option<&ProgressCallback>,
    ) -> Result<usize> {
        let subscriptions = self.db.list_subscriptions(None)?;

        // Categorize subscriptions using tag-based patterns
        let mut by_category: HashMap<String, Vec<_>> = HashMap::new();

        for sub in &subscriptions {
            if sub.status != SubscriptionStatus::Active && sub.status != SubscriptionStatus::Zombie
            {
                continue;
            }

            // Use tag-based categorization (falls back to legacy if no tags seeded)
            if let Ok(Some(category)) = self.db.categorize_merchant_by_tags(&sub.merchant) {
                by_category.entry(category).or_default().push(sub);
            } else if let Some(category) = categorize_subscription_fallback(&sub.merchant) {
                // Fallback to hardcoded patterns if tags not available
                by_category
                    .entry(category.to_string())
                    .or_default()
                    .push(sub);
            }
        }

        // Count categories with duplicates (these need Ollama analysis)
        let duplicate_categories: Vec<_> = by_category
            .iter()
            .filter(|(_, subs)| subs.len() >= 2)
            .collect();
        let total_duplicates = duplicate_categories.len() as i64;
        let mut processed_duplicates = 0i64;

        let mut count = 0;

        for (category, subs) in by_category {
            if subs.len() < 2 {
                continue; // Need 2+ to be duplicates
            }

            // Update progress before Ollama call
            processed_duplicates += 1;
            if let Some(cb) = progress {
                if self.ai.is_some() {
                    cb(
                        "analyzing_duplicates",
                        processed_duplicates,
                        total_duplicates,
                    );
                }
            }

            let total_cost: f64 = subs.iter().filter_map(|s| s.amount).sum();
            let names: Vec<_> = subs.iter().map(|s| s.merchant.as_str()).collect();

            let message = format!(
                "You have {} {} services: {}. Total: ${:.2}/mo",
                subs.len(),
                category,
                names.join(", "),
                total_cost
            );

            // Try to get analysis using orchestrator (agentic) or standard AI
            let analysis = if let Some(orchestrator) = self.orchestrator {
                // Use agentic analysis - AI can query transaction history
                self.analyze_duplicates_with_orchestrator(orchestrator, &category, &names)
                    .await
            } else if let Some(ollama) = self.ai {
                // Get user feedback to improve response quality
                let feedback = self
                    .db
                    .get_feedback_summary_for_prompt(FeedbackTargetType::Insight)
                    .ok()
                    .filter(|f| !f.is_empty());

                match ollama
                    .analyze_duplicate_services(&category, &names, feedback.as_deref())
                    .await
                {
                    Ok(analysis) => {
                        debug!(
                            "Ollama duplicate analysis for {}: overlap='{}', {} unique features",
                            category,
                            analysis.overlap,
                            analysis.unique_features.len()
                        );
                        Some(analysis)
                    }
                    Err(e) => {
                        debug!("Ollama duplicate analysis failed for {}: {}", category, e);
                        None
                    }
                }
            } else {
                None
            };

            // Create alert for the first subscription in the group (with analysis if available)
            self.db.create_alert_with_analysis(
                AlertType::Duplicate,
                Some(subs[0].id),
                Some(&message),
                analysis.as_ref(),
            )?;
            count += 1;
        }

        Ok(count)
    }

    /// Detect subscriptions that have likely been cancelled
    ///
    /// A subscription is considered cancelled when:
    /// 1. It's been acknowledged by the user (they know about it)
    /// 2. Its last_seen date plus the expected interval (+ grace period) is in the past
    /// 3. No new matching transaction has been imported
    ///
    /// Note: We only auto-cancel acknowledged subscriptions. Unacknowledged ones
    /// should be reviewed via zombie detection first.
    fn detect_cancelled(&self) -> Result<usize> {
        let subscriptions = self.db.list_subscriptions(None)?;
        let today = Utc::now().date_naive();
        let mut count = 0;

        for sub in subscriptions {
            // Only auto-cancel acknowledged, non-cancelled subscriptions
            // Unacknowledged subscriptions should go through zombie detection first
            if sub.status == SubscriptionStatus::Cancelled || !sub.user_acknowledged {
                continue;
            }

            let (last_seen, frequency) = match (sub.last_seen, sub.frequency) {
                (Some(ls), Some(freq)) => (ls, freq),
                _ => continue,
            };

            // Calculate expected next charge date with grace period
            let (interval_days, grace_days) = match frequency {
                Frequency::Weekly => (7, 3),
                Frequency::Monthly => (30, self.config.cancellation_grace_days_monthly),
                Frequency::Yearly => (365, 30),
            };

            let expected_by = last_seen + Duration::days(interval_days + grace_days);

            // If we're past the expected date, mark as cancelled
            if today > expected_by {
                self.db.cancel_subscription(sub.id, None)?;

                debug!(
                    "Auto-cancelled subscription: {} (last seen: {}, expected by: {})",
                    sub.merchant, last_seen, expected_by
                );
                count += 1;
            }
        }

        Ok(count)
    }

    /// Detect resumed subscriptions
    ///
    /// A subscription is considered resumed when:
    /// 1. It was previously cancelled
    /// 2. A new transaction matching the merchant has been imported after cancellation
    fn detect_resumed(&self) -> Result<usize> {
        let subscriptions = self.db.list_subscriptions(None)?;
        let transactions = self.db.list_transactions(None, 10000, 0)?;
        let mut count = 0;

        for sub in subscriptions {
            // Only check cancelled subscriptions
            if sub.status != SubscriptionStatus::Cancelled {
                continue;
            }

            // Get the cancellation date (when we last saw a charge)
            let last_seen = match sub.last_seen {
                Some(ls) => ls,
                None => continue,
            };

            // Find transactions for this merchant after last_seen
            let new_txs: Vec<&Transaction> = transactions
                .iter()
                .filter(|tx| {
                    if tx.amount >= 0.0 {
                        return false; // Skip income/credits
                    }
                    let tx_merchant = tx
                        .merchant_normalized
                        .clone()
                        .unwrap_or_else(|| normalize_merchant(&tx.description));
                    tx_merchant == sub.merchant && tx.date > last_seen
                })
                .collect();

            if !new_txs.is_empty() {
                // Found new charges - subscription resumed!
                let latest = new_txs.iter().max_by_key(|tx| tx.date).unwrap();
                let amount = latest.amount.abs();

                // Reactivate the subscription
                self.db
                    .reactivate_subscription(sub.id, latest.date, amount)?;

                let message = format!(
                    "{} started charging again: ${:.2} on {}",
                    sub.merchant,
                    amount,
                    latest.date.format("%B %d, %Y")
                );

                self.db
                    .create_alert(AlertType::Resume, Some(sub.id), Some(&message))?;

                debug!("Detected resumed subscription: {}", sub.merchant);
                count += 1;
            }
        }

        Ok(count)
    }

    /// Detect spending anomalies
    ///
    /// Compares current month spending by category against a 3-month rolling baseline.
    /// Creates alerts when spending increases by >30% or decreases by >40%.
    async fn detect_spending_anomalies_with_progress(
        &self,
        progress: Option<&ProgressCallback>,
    ) -> Result<usize> {
        let today = Utc::now().date_naive();

        // Calculate current month period
        let current_month_start = today.with_day(1).expect("Day 1 always valid");

        // Calculate 3-month baseline period (the 3 months before current month)
        let baseline_end = current_month_start - Duration::days(1);
        let baseline_start = baseline_end - Duration::days(90);

        // Get spending by category for current month
        // (tag_filter=None, expand=false, entity_id=None, card_member=None)
        let current =
            self.db
                .get_spending_summary(current_month_start, today, None, false, None, None)?;

        // Get spending by category for baseline period
        let baseline =
            self.db
                .get_spending_summary(baseline_start, baseline_end, None, false, None, None)?;

        let mut count = 0;

        // First pass: identify categories with anomalies (for progress tracking)
        let anomaly_categories: Vec<_> = current
            .categories
            .iter()
            .filter(|current_cat| {
                let Some(baseline_cat) = baseline
                    .categories
                    .iter()
                    .find(|c| c.tag_id == current_cat.tag_id)
                else {
                    return false;
                };
                let baseline_monthly_avg = baseline_cat.amount.abs() / 3.0;
                if baseline_monthly_avg < self.config.spending_anomaly_min_baseline {
                    return false;
                }
                let current_amount = current_cat.amount.abs();
                let percent_change = if baseline_monthly_avg > 0.0 {
                    ((current_amount - baseline_monthly_avg) / baseline_monthly_avg) * 100.0
                } else {
                    return false;
                };
                let is_increase = percent_change > self.config.spending_increase_threshold;
                let is_decrease = percent_change < -self.config.spending_decrease_threshold;
                is_increase || is_decrease
            })
            .collect();

        let total_anomalies = anomaly_categories.len() as i64;
        let mut processed_anomalies = 0i64;

        // Compare each category
        for current_cat in &current.categories {
            // Find matching baseline category
            let baseline_cat = baseline
                .categories
                .iter()
                .find(|c| c.tag_id == current_cat.tag_id);

            let Some(baseline_cat) = baseline_cat else {
                // No baseline for this category (new this month) - skip
                continue;
            };

            // Calculate average monthly baseline (divide by 3 months)
            let baseline_monthly_avg = baseline_cat.amount.abs() / 3.0;

            // Skip if baseline is too small
            if baseline_monthly_avg < self.config.spending_anomaly_min_baseline {
                continue;
            }

            let current_amount = current_cat.amount.abs();

            // Calculate percent change from baseline
            let percent_change = if baseline_monthly_avg > 0.0 {
                ((current_amount - baseline_monthly_avg) / baseline_monthly_avg) * 100.0
            } else {
                continue; // Can't calculate change from zero
            };

            // Check if change exceeds thresholds
            let is_increase = percent_change > self.config.spending_increase_threshold;
            let is_decrease = percent_change < -self.config.spending_decrease_threshold;

            if !is_increase && !is_decrease {
                continue; // Change not significant enough
            }

            // Update progress before Ollama call
            processed_anomalies += 1;
            if let Some(cb) = progress {
                if self.ai.is_some() {
                    cb("analyzing_spending", processed_anomalies, total_anomalies);
                }
            }

            debug!(
                "Spending anomaly detected: {} {} by {:.1}% (${:.2} -> ${:.2})",
                current_cat.tag,
                if is_increase {
                    "increased"
                } else {
                    "decreased"
                },
                percent_change.abs(),
                baseline_monthly_avg,
                current_amount
            );

            // Try to get explanation using orchestrator (agentic) or standard AI
            let explanation = if let Some(orchestrator) = self.orchestrator {
                // Use agentic analysis - AI can query data dynamically via tools
                self.explain_spending_with_orchestrator(
                    orchestrator,
                    &current_cat.tag,
                    baseline_monthly_avg,
                    current_amount,
                    percent_change,
                )
                .await
            } else if let Some(ollama) = self.ai {
                // Fall back to standard pre-assembled context approach
                // Get top merchants for this category in current period
                let merchants_report = self.db.get_top_merchants(
                    current_month_start,
                    today,
                    5, // top 5 merchants
                    Some(&current_cat.tag),
                    None,
                    None,
                )?;

                // Get top merchants for this category in baseline period
                let baseline_merchants = self.db.get_top_merchants(
                    baseline_start,
                    baseline_end,
                    10, // more for baseline comparison
                    Some(&current_cat.tag),
                    None,
                    None,
                )?;

                // Format merchant data for Ollama
                let top_merchants: Vec<(String, f64, i32)> = merchants_report
                    .merchants
                    .iter()
                    .map(|m| {
                        (
                            m.merchant.clone(),
                            m.amount.abs(),
                            m.transaction_count as i32,
                        )
                    })
                    .collect();

                // Find new merchants (in current but not in baseline)
                let baseline_names: std::collections::HashSet<_> = baseline_merchants
                    .merchants
                    .iter()
                    .map(|m| m.merchant.to_lowercase())
                    .collect();

                let new_merchants: Vec<String> = merchants_report
                    .merchants
                    .iter()
                    .filter(|m| !baseline_names.contains(&m.merchant.to_lowercase()))
                    .map(|m| m.merchant.clone())
                    .collect();

                // Get user feedback to improve response quality
                let feedback = self
                    .db
                    .get_feedback_summary_for_prompt(FeedbackTargetType::Explanation)
                    .ok()
                    .filter(|f| !f.is_empty());

                match ollama
                    .explain_spending_change(
                        &current_cat.tag,
                        baseline_monthly_avg,
                        current_amount,
                        baseline_cat.transaction_count as i32,
                        current_cat.transaction_count as i32,
                        &top_merchants,
                        &new_merchants,
                        feedback.as_deref(),
                    )
                    .await
                {
                    Ok(explanation) => {
                        debug!("Ollama spending explanation: {}", explanation.summary);
                        Some(explanation)
                    }
                    Err(e) => {
                        debug!("Ollama spending explanation failed: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Create spending anomaly data
            let data = SpendingAnomalyData {
                tag_id: current_cat.tag_id,
                tag_name: current_cat.tag.clone(),
                baseline_amount: baseline_monthly_avg,
                current_amount,
                percent_change,
                explanation,
            };

            // Create alert
            self.db.create_spending_anomaly_alert(&data)?;
            count += 1;
        }

        Ok(count)
    }

    /// Use the AI orchestrator for agentic spending analysis
    ///
    /// The AI can query data dynamically via tools, enabling more thorough
    /// investigation of spending changes.
    async fn explain_spending_with_orchestrator(
        &self,
        orchestrator: &AIOrchestrator,
        category: &str,
        baseline_amount: f64,
        current_amount: f64,
        percent_change: f64,
    ) -> Option<SpendingChangeExplanation> {
        let change_direction = if percent_change > 0.0 {
            "increased"
        } else {
            "decreased"
        };

        // Load prompt from library
        let mut prompt_lib = PromptLibrary::new();
        let prompt = match prompt_lib.get(PromptId::SpendingAnalysisAgent) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "Failed to load spending_analysis_agent prompt");
                return None;
            }
        };

        let system_prompt = match prompt.system_section() {
            Some(s) => s.to_string(),
            None => {
                warn!("spending_analysis_agent prompt missing system section");
                return None;
            }
        };

        // Build template variables for rendering user prompt
        let mut vars = std::collections::HashMap::new();
        vars.insert("category", category);
        let change_direction_str: &str = change_direction;
        vars.insert("change_direction", change_direction_str);
        let percent_str = format!("{:.0}", percent_change.abs());
        let baseline_str = format!("{:.2}", baseline_amount);
        let current_str = format!("{:.2}", current_amount);
        vars.insert("percent_change", percent_str.as_str());
        vars.insert("baseline_amount", baseline_str.as_str());
        vars.insert("current_amount", current_str.as_str());

        let user_prompt = prompt.render_user(&vars);

        let available_tools = tools::spending_analysis_tools();

        match orchestrator
            .execute(&system_prompt, &user_prompt, &available_tools)
            .await
        {
            Ok(response) => {
                // Parse the structured response
                let (summary, reasons) = parse_orchestrator_explanation(&response);

                if summary.is_empty() {
                    warn!("Orchestrator returned empty explanation");
                    None
                } else {
                    info!(
                        category = category,
                        summary = %summary,
                        "Agentic spending explanation generated"
                    );

                    Some(SpendingChangeExplanation {
                        summary,
                        reasons,
                        model: orchestrator.model().to_string(),
                        analyzed_at: Utc::now(),
                    })
                }
            }
            Err(e) => {
                warn!(
                    category = category,
                    error = %e,
                    "Orchestrator spending explanation failed"
                );
                None
            }
        }
    }

    /// Use the AI orchestrator for agentic duplicate subscription analysis
    ///
    /// The AI can query transaction history to understand usage patterns,
    /// providing better insights into which services might be redundant.
    async fn analyze_duplicates_with_orchestrator(
        &self,
        orchestrator: &AIOrchestrator,
        category: &str,
        services: &[&str],
    ) -> Option<DuplicateAnalysis> {
        let services_list = services.join(", ");

        // Load prompt from library
        let mut prompt_lib = PromptLibrary::new();
        let prompt = match prompt_lib.get(PromptId::DuplicateAnalysisAgent) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "Failed to load duplicate_analysis_agent prompt");
                return None;
            }
        };

        let system_prompt = match prompt.system_section() {
            Some(s) => s.to_string(),
            None => {
                warn!("duplicate_analysis_agent prompt missing system section");
                return None;
            }
        };

        // Build template variables for rendering user prompt
        let mut vars = std::collections::HashMap::new();
        vars.insert("category", category);
        vars.insert("services", services_list.as_str());

        let user_prompt = prompt.render_user(&vars);

        let available_tools = tools::duplicate_analysis_tools();

        match orchestrator
            .execute(&system_prompt, &user_prompt, &available_tools)
            .await
        {
            Ok(response) => {
                // Parse the structured response
                let analysis = parse_orchestrator_duplicate_analysis(&response, services);

                if analysis.overlap.is_empty() {
                    warn!("Orchestrator returned empty duplicate analysis");
                    None
                } else {
                    info!(
                        category = category,
                        services_count = services.len(),
                        overlap = %analysis.overlap,
                        "Agentic duplicate analysis generated"
                    );
                    Some(analysis)
                }
            }
            Err(e) => {
                warn!(
                    category = category,
                    error = %e,
                    "Orchestrator duplicate analysis failed"
                );
                None
            }
        }
    }
}

/// Parse the orchestrator's structured duplicate analysis response
fn parse_orchestrator_duplicate_analysis(response: &str, services: &[&str]) -> DuplicateAnalysis {
    let mut overlap = String::new();
    let mut unique_features = Vec::new();
    let mut current_service: Option<String> = None;

    for line in response.lines() {
        let line = line.trim();
        if let Some(o) = line.strip_prefix("OVERLAP:") {
            overlap = o.trim().to_string();
        } else if let Some(s) = line.strip_prefix("SERVICE:") {
            current_service = Some(s.trim().to_string());
        } else if let Some(u) = line.strip_prefix("UNIQUE:") {
            if let Some(service) = current_service.take() {
                unique_features.push(ServiceFeature {
                    service,
                    unique: u.trim().to_string(),
                });
            }
        }
    }

    // If parsing failed, try to extract something useful
    if overlap.is_empty() && !response.trim().is_empty() {
        // Use first sentence as overlap description
        let first_sentence = response
            .split(&['.', '!', '?'][..])
            .next()
            .unwrap_or(response)
            .trim();
        overlap = if first_sentence.len() > 200 {
            format!("{}...", &first_sentence[..200])
        } else {
            first_sentence.to_string()
        };

        // Create placeholder unique features for each service
        for service in services {
            unique_features.push(ServiceFeature {
                service: service.to_string(),
                unique: "See full analysis for details".to_string(),
            });
        }
    }

    DuplicateAnalysis {
        overlap,
        unique_features,
    }
}

/// Parse the orchestrator's structured explanation response
fn parse_orchestrator_explanation(response: &str) -> (String, Vec<String>) {
    let mut summary = String::new();
    let mut reasons = Vec::new();

    for line in response.lines() {
        let line = line.trim();
        if let Some(s) = line.strip_prefix("SUMMARY:") {
            summary = s.trim().to_string();
        } else if let Some(r) = line.strip_prefix("REASON 1:") {
            reasons.push(r.trim().to_string());
        } else if let Some(r) = line.strip_prefix("REASON 2:") {
            reasons.push(r.trim().to_string());
        } else if let Some(r) = line.strip_prefix("REASON 3:") {
            reasons.push(r.trim().to_string());
        }
    }

    // If parsing failed, use the whole response as the summary
    if summary.is_empty() && !response.trim().is_empty() {
        // Take first sentence as summary
        let first_sentence = response
            .split(&['.', '!', '?'][..])
            .next()
            .unwrap_or(response)
            .trim();
        summary = if first_sentence.len() > 200 {
            format!("{}...", &first_sentence[..200])
        } else {
            first_sentence.to_string()
        };
    }

    (summary, reasons)
}

/// Info about a detected subscription pattern
struct SubscriptionInfo {
    amount: f64,
    frequency: Frequency,
    first_seen: NaiveDate,
    last_seen: NaiveDate,
}

/// Check if transactions have similar raw descriptions (likely same merchant).
///
/// This prevents false positives where different merchants happen to share part
/// of a name (e.g., different stores in "Monroe" city being grouped together).
///
/// Returns true if at least 70% of transactions have a common description prefix
/// (first 2 significant words, excluding payment prefixes and trailing IDs).
fn descriptions_are_similar(transactions: &[&Transaction]) -> bool {
    if transactions.len() < 2 {
        return true;
    }

    // Clean and normalize descriptions for comparison
    let clean_description = |desc: &str| -> String {
        let upper = desc.to_uppercase();
        // Remove common payment method prefixes that vary per transaction
        let cleaned = upper
            .trim_start_matches("APLPAY ")
            .trim_start_matches("APPLEPAY ")
            .trim_start_matches("SP * ")
            .trim_start_matches("SP *")
            .trim_start_matches("SQ * ")
            .trim_start_matches("SQ *")
            .trim_start_matches("TST* ")
            .trim_start_matches("TST*")
            .replace("*", " ")
            .replace("#", " ");

        // Take first 2 significant words (merchant name, not IDs/store numbers)
        // Filter out purely numeric tokens which are usually transaction/store IDs
        cleaned
            .split_whitespace()
            .filter(|word| !word.chars().all(|c| c.is_ascii_digit()))
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
    };

    // Count occurrences of each cleaned description
    let mut desc_counts: HashMap<String, usize> = HashMap::new();
    for tx in transactions {
        let cleaned = clean_description(&tx.description);
        *desc_counts.entry(cleaned).or_insert(0) += 1;
    }

    // Find the most common description
    let max_count = desc_counts.values().max().copied().unwrap_or(0);

    // At least 70% of transactions should have the same cleaned description
    // for us to consider them from the same merchant
    let similarity_threshold = 0.7;
    (max_count as f64 / transactions.len() as f64) >= similarity_threshold
}

/// Detect if a set of transactions represents a subscription
///
/// A subscription is characterized by:
/// 1. Similar raw descriptions (same merchant, not just same normalized name)
/// 2. Consistent amounts (within 5% of median, allowing for small price changes)
/// 3. Regular intervals that match a known cadence (weekly, monthly, yearly)
/// 4. At least 3 transactions to establish a pattern
fn detect_subscription_pattern(transactions: &[&Transaction]) -> Option<SubscriptionInfo> {
    // Need at least 3 transactions to establish a reliable pattern
    // (2 could be coincidence, 3 suggests a real recurring charge)
    if transactions.len() < 3 {
        return None;
    }

    // Check that transactions have similar raw descriptions
    // This catches false positives like different stores in the same city
    if !descriptions_are_similar(transactions) {
        return None;
    }

    // Sort by date
    let mut sorted: Vec<_> = transactions.to_vec();
    sorted.sort_by_key(|t| t.date);

    let first_seen = sorted.first()?.date;
    let last_seen = sorted.last()?.date;

    // Get amounts (absolute values since we're dealing with expenses)
    let amounts: Vec<f64> = sorted.iter().map(|t| t.amount.abs()).collect();

    // Check if amounts are consistent (within 5% of median)
    // Real subscriptions have very consistent pricing; variable amounts suggest
    // regular shopping (like coffee shops, groceries) rather than subscriptions
    let median_amount = median(&amounts);
    if median_amount < 0.01 {
        return None; // Avoid division by zero on tiny amounts
    }

    let amount_variance_threshold = 0.05; // 5% variance allowed
    let amounts_consistent = amounts
        .iter()
        .all(|a| (a - median_amount).abs() / median_amount < amount_variance_threshold);

    if !amounts_consistent {
        return None;
    }

    // Detect frequency by looking at intervals between transactions
    let intervals: Vec<i64> = sorted
        .windows(2)
        .map(|w| (w[1].date - w[0].date).num_days())
        .collect();

    if intervals.is_empty() {
        return None;
    }

    // Determine the expected frequency based on average interval
    let avg_interval = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;

    let (frequency, expected_interval, tolerance) = if avg_interval < 10.0 {
        (Frequency::Weekly, 7.0, 3.0) // Weekly: expect ~7 days, allow ±3 days
    } else if avg_interval < 45.0 {
        (Frequency::Monthly, 30.0, 7.0) // Monthly: expect ~30 days, allow ±7 days
    } else if avg_interval < 400.0 {
        (Frequency::Yearly, 365.0, 30.0) // Yearly: expect ~365 days, allow ±30 days
    } else {
        return None; // Interval too long to be a subscription
    };

    // Verify that most intervals are consistent with the expected frequency
    // At least 70% of intervals should fall within tolerance of the expected interval
    // This filters out merchants you visit frequently but irregularly
    let consistent_interval_count = intervals
        .iter()
        .filter(|&&interval| {
            let diff = (interval as f64 - expected_interval).abs();
            diff <= tolerance
        })
        .count();

    let interval_consistency_threshold = 0.7; // 70% of intervals must be consistent
    let intervals_consistent = (consistent_interval_count as f64 / intervals.len() as f64)
        >= interval_consistency_threshold;

    if !intervals_consistent {
        return None;
    }

    Some(SubscriptionInfo {
        amount: median_amount,
        frequency,
        first_seen,
        last_seen,
    })
}

/// Detect if a set of transactions represents a subscription using relaxed thresholds
///
/// Used when Ollama confirms the merchant is a subscription service.
/// This allows detection of variable-amount recurring charges like:
/// - Utility bills (varying monthly amounts)
/// - Cloud services (usage-based billing)
/// - Metered subscriptions
///
/// The key insight: if Ollama says it's a subscription service, we trust the
/// merchant classification and use looser amount/interval requirements.
fn detect_subscription_pattern_relaxed(
    transactions: &[&Transaction],
    config: &DetectionConfig,
) -> Option<SubscriptionInfo> {
    // Use configurable minimum (default 2 vs strict's 3)
    if transactions.len() < config.smart_min_transactions {
        return None;
    }

    // Check that transactions have similar raw descriptions
    // Even with relaxed thresholds, we still require same merchant
    if !descriptions_are_similar(transactions) {
        return None;
    }

    // Sort by date
    let mut sorted: Vec<_> = transactions.to_vec();
    sorted.sort_by_key(|t| t.date);

    let first_seen = sorted.first()?.date;
    let last_seen = sorted.last()?.date;

    // Get amounts (absolute values since we're dealing with expenses)
    let amounts: Vec<f64> = sorted.iter().map(|t| t.amount.abs()).collect();

    let median_amount = median(&amounts);
    if median_amount < 0.01 {
        return None; // Avoid division by zero on tiny amounts
    }

    // Use relaxed amount variance threshold (e.g., 50% vs strict's 5%)
    // This allows for variable-amount subscriptions
    let amounts_consistent = amounts
        .iter()
        .all(|a| (a - median_amount).abs() / median_amount < config.smart_amount_variance);

    if !amounts_consistent {
        return None;
    }

    // Detect frequency by looking at intervals between transactions
    let intervals: Vec<i64> = sorted
        .windows(2)
        .map(|w| (w[1].date - w[0].date).num_days())
        .collect();

    if intervals.is_empty() {
        return None;
    }

    // Determine the expected frequency based on average interval
    let avg_interval = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;

    let (frequency, expected_interval, tolerance) = if avg_interval < 10.0 {
        (Frequency::Weekly, 7.0, 3.0)
    } else if avg_interval < 45.0 {
        (Frequency::Monthly, 30.0, 10.0) // Slightly more tolerance for monthly
    } else if avg_interval < 400.0 {
        (Frequency::Yearly, 365.0, 45.0) // More tolerance for yearly
    } else {
        return None;
    };

    // Use relaxed interval consistency threshold (e.g., 50% vs strict's 70%)
    let consistent_interval_count = intervals
        .iter()
        .filter(|&&interval| {
            let diff = (interval as f64 - expected_interval).abs();
            diff <= tolerance
        })
        .count();

    let intervals_consistent = (consistent_interval_count as f64 / intervals.len() as f64)
        >= config.smart_interval_consistency;

    if !intervals_consistent {
        return None;
    }

    Some(SubscriptionInfo {
        amount: median_amount,
        frequency,
        first_seen,
        last_seen,
    })
}

/// Simple merchant name normalization
fn normalize_merchant(description: &str) -> String {
    let desc = description.to_uppercase();

    // Common patterns to clean up
    let cleaned = desc
        .replace("*", " ")
        .replace("#", " ")
        .split_whitespace()
        .take(3) // Take first 3 words
        .collect::<Vec<_>>()
        .join(" ");

    cleaned
}

/// Categorize a subscription by service type using hardcoded patterns.
/// Used as a fallback when the tags system isn't available.
fn categorize_subscription_fallback(merchant: &str) -> Option<&'static str> {
    let m = merchant.to_uppercase();

    // Streaming video
    if m.contains("NETFLIX")
        || m.contains("HULU")
        || m.contains("DISNEY")
        || m.contains("HBO")
        || m.contains("PARAMOUNT")
        || m.contains("PEACOCK")
        || m.contains("PRIME VIDEO")
        || m.contains("APPLE TV")
    {
        return Some("Streaming");
    }

    // Music
    if m.contains("SPOTIFY")
        || m.contains("APPLE MUSIC")
        || m.contains("TIDAL")
        || m.contains("PANDORA")
        || m.contains("YOUTUBE MUSIC")
    {
        return Some("Music");
    }

    // Cloud storage
    if m.contains("ICLOUD")
        || m.contains("GOOGLE ONE")
        || m.contains("DROPBOX")
        || m.contains("ONEDRIVE")
        || m.contains("BOX.COM")
    {
        return Some("CloudStorage");
    }

    // News/Media
    if m.contains("NYT")
        || m.contains("NEW YORK TIMES")
        || m.contains("WSJ")
        || m.contains("WASHINGTON POST")
        || m.contains("MEDIUM")
        || m.contains("SUBSTACK")
    {
        return Some("News");
    }

    // Fitness
    if m.contains("PELOTON")
        || m.contains("STRAVA")
        || m.contains("FITBIT")
        || m.contains("MYFITNESSPAL")
        || m.contains("HEADSPACE")
        || m.contains("CALM")
    {
        return Some("Fitness");
    }

    None
}

/// Calculate median of a slice
fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn test_normalize_merchant() {
        assert_eq!(normalize_merchant("NETFLIX.COM*12345"), "NETFLIX.COM 12345");
        assert_eq!(normalize_merchant("SPOTIFY USA"), "SPOTIFY USA");
    }

    #[test]
    fn test_categorize_fallback() {
        // Fallback function returns category names matching tag names
        assert_eq!(
            categorize_subscription_fallback("NETFLIX.COM"),
            Some("Streaming")
        );
        assert_eq!(
            categorize_subscription_fallback("Spotify Premium"),
            Some("Music")
        );
        assert_eq!(categorize_subscription_fallback("RANDOM STORE"), None);
    }

    #[test]
    fn test_categorize_by_tags() {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();

        // Add auto_patterns to subscription children for this test
        // (seeded tags no longer have auto_patterns - they rely on bank categories)
        let streaming_tag = db
            .get_tag_by_path("Subscriptions.Streaming")
            .unwrap()
            .unwrap();
        db.update_tag(
            streaming_tag.id,
            None,
            None,
            None,
            None,
            Some(Some("NETFLIX|HULU")),
        )
        .unwrap();

        let music_tag = db.get_tag_by_path("Subscriptions.Music").unwrap().unwrap();
        db.update_tag(
            music_tag.id,
            None,
            None,
            None,
            None,
            Some(Some("SPOTIFY|APPLE MUSIC")),
        )
        .unwrap();

        let cloud_tag = db
            .get_tag_by_path("Subscriptions.CloudStorage")
            .unwrap()
            .unwrap();
        db.update_tag(
            cloud_tag.id,
            None,
            None,
            None,
            None,
            Some(Some("ICLOUD|DROPBOX")),
        )
        .unwrap();

        // Test each categorization separately to avoid pool contention
        let netflix = db.categorize_merchant_by_tags("NETFLIX.COM").unwrap();
        assert_eq!(netflix, Some("Streaming".to_string()));

        let spotify = db.categorize_merchant_by_tags("Spotify Premium").unwrap();
        assert_eq!(spotify, Some("Music".to_string()));

        let icloud = db.categorize_merchant_by_tags("ICLOUD STORAGE").unwrap();
        assert_eq!(icloud, Some("CloudStorage".to_string()));

        let random = db.categorize_merchant_by_tags("RANDOM STORE").unwrap();
        assert_eq!(random, None);
    }

    #[test]
    fn test_median() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[15.99, 15.99, 15.99]), 15.99);
    }

    #[test]
    fn test_descriptions_are_similar() {
        use crate::models::TransactionSource;
        use chrono::NaiveDate;

        // Helper to create a minimal transaction
        let make_tx = |desc: &str| Transaction {
            id: 1,
            account_id: 1,
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            description: desc.to_string(),
            amount: -10.0,
            category: None,
            merchant_normalized: None,
            import_hash: "test".to_string(),
            purchase_location_id: None,
            vendor_location_id: None,
            trip_id: None,
            source: TransactionSource::Import,
            expected_amount: None,
            archived: false,
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
            created_at: Utc::now(),
        };

        // Same merchant, different store numbers - should be similar
        let netflix1 = make_tx("NETFLIX.COM*12345");
        let netflix2 = make_tx("NETFLIX.COM*67890");
        let netflix3 = make_tx("NETFLIX.COM*11111");
        let netflix_refs: Vec<&Transaction> = vec![&netflix1, &netflix2, &netflix3];
        assert!(
            descriptions_are_similar(&netflix_refs),
            "Same merchant with different IDs should be similar"
        );

        // Different merchants in same location - should NOT be similar (the Monroe problem)
        let monroe1 = make_tx("FRED MEYER FUEL MONROE");
        let monroe2 = make_tx("LOWES #1234 MONROE");
        let monroe3 = make_tx("SAFEWAY MONROE WA");
        let monroe4 = make_tx("IXTAPA MONROE");
        let monroe_refs: Vec<&Transaction> = vec![&monroe1, &monroe2, &monroe3, &monroe4];
        assert!(
            !descriptions_are_similar(&monroe_refs),
            "Different merchants in same city should NOT be similar"
        );

        // Apple Pay prefix variations - should be similar
        let applepay1 = make_tx("APLPAY STARBUCKS SEATTLE");
        let applepay2 = make_tx("STARBUCKS SEATTLE WA");
        let applepay3 = make_tx("APPLEPAY STARBUCKS SEATTLE");
        let applepay_refs: Vec<&Transaction> = vec![&applepay1, &applepay2, &applepay3];
        assert!(
            descriptions_are_similar(&applepay_refs),
            "Same merchant with different payment prefixes should be similar"
        );

        // Single transaction - should return true (nothing to compare)
        let single_refs: Vec<&Transaction> = vec![&netflix1];
        assert!(
            descriptions_are_similar(&single_refs),
            "Single transaction should be similar to itself"
        );
    }

    #[tokio::test]
    async fn test_detect_cancelled_subscription() {
        use chrono::Duration;

        let db = Database::in_memory().unwrap();

        // Create a subscription that hasn't been seen in over a month + grace period
        let old_date = Utc::now().date_naive() - Duration::days(45);
        let old_sub_id = db
            .upsert_subscription(
                "OLD STREAMING SERVICE",
                None, // account_id
                Some(9.99),
                Some(Frequency::Monthly),
                Some(old_date - Duration::days(90)),
                Some(old_date),
            )
            .unwrap();

        // Mark as acknowledged (auto-cancellation only works on acknowledged subscriptions)
        db.acknowledge_subscription(old_sub_id).unwrap();

        // Create a subscription that was seen recently (shouldn't be cancelled)
        let recent_date = Utc::now().date_naive() - Duration::days(15);
        let recent_sub_id = db
            .upsert_subscription(
                "RECENT STREAMING",
                None, // account_id
                Some(14.99),
                Some(Frequency::Monthly),
                Some(recent_date - Duration::days(60)),
                Some(recent_date),
            )
            .unwrap();

        // Mark as acknowledged
        db.acknowledge_subscription(recent_sub_id).unwrap();

        let detector = WasteDetector::new(&db);
        let results = detector.detect_all().await.unwrap();

        // Should have auto-cancelled the old subscription
        assert_eq!(results.auto_cancelled, 1);

        // Verify the status change
        let subs = db.list_subscriptions(None).unwrap();
        let old_sub = subs
            .iter()
            .find(|s| s.merchant == "OLD STREAMING SERVICE")
            .unwrap();
        assert_eq!(old_sub.status, SubscriptionStatus::Cancelled);

        let recent_sub = subs
            .iter()
            .find(|s| s.merchant == "RECENT STREAMING")
            .unwrap();
        assert_eq!(recent_sub.status, SubscriptionStatus::Active);
    }

    #[tokio::test]
    async fn test_detect_resumed_subscription() {
        use chrono::Duration;

        let db = Database::in_memory().unwrap();

        // Create account
        let account_id = db
            .upsert_account("Test Account", crate::models::Bank::Chase, None)
            .unwrap();

        // Create a cancelled subscription (with account_id for proper matching)
        let cancelled_date = Utc::now().date_naive() - Duration::days(60);
        let sub_id = db
            .upsert_subscription(
                "CANCELLED SERVICE",
                Some(account_id), // account_id for proper matching with transactions
                Some(9.99),
                Some(Frequency::Monthly),
                Some(cancelled_date - Duration::days(90)),
                Some(cancelled_date),
            )
            .unwrap();
        db.cancel_subscription(sub_id, Some(cancelled_date))
            .unwrap();

        // Verify it's cancelled
        let subs = db.list_subscriptions(None).unwrap();
        let sub = subs.iter().find(|s| s.id == sub_id).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Cancelled);

        // Add a new transaction from this merchant after cancellation
        let new_charge_date = Utc::now().date_naive() - Duration::days(5);
        db.insert_transaction(
            account_id,
            &crate::models::NewTransaction {
                date: new_charge_date,
                description: "CANCELLED SERVICE".to_string(),
                amount: -9.99,
                category: None,
                import_hash: "resume_test_hash".to_string(),
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
            },
        )
        .unwrap();

        let detector = WasteDetector::new(&db);
        let results = detector.detect_all().await.unwrap();

        // Should have detected the resume
        assert_eq!(results.resumes_detected, 1);

        // Verify the subscription is now active again
        let subs = db.list_subscriptions(None).unwrap();
        let sub = subs.iter().find(|s| s.id == sub_id).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Active);

        // Verify a resume alert was created
        let alerts = db.list_alerts(false).unwrap();
        let resume_alert = alerts
            .iter()
            .find(|a| a.alert_type == AlertType::Resume)
            .expect("Should have created a resume alert");
        assert_eq!(resume_alert.subscription_id, Some(sub_id));
    }

    #[test]
    fn test_detect_subscription_pattern_relaxed_with_variable_amounts() {
        use crate::models::TransactionSource;
        use chrono::Duration;

        // Create transactions with varying amounts (like utility bills)
        let base_date = Utc::now().date_naive() - Duration::days(120);
        let now = Utc::now();
        let transactions: Vec<Transaction> = vec![
            Transaction {
                id: 1,
                account_id: 1,
                date: base_date,
                description: "ELECTRIC COMPANY".to_string(),
                amount: -85.00,
                category: None,
                import_hash: "hash1".to_string(),
                merchant_normalized: None,
                archived: false,
                purchase_location_id: None,
                vendor_location_id: None,
                trip_id: None,
                source: TransactionSource::Import,
                expected_amount: None,
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
                created_at: now,
            },
            Transaction {
                id: 2,
                account_id: 1,
                date: base_date + Duration::days(30),
                description: "ELECTRIC COMPANY".to_string(),
                amount: -120.50, // 42% higher - would fail strict 5% check
                category: None,
                import_hash: "hash2".to_string(),
                merchant_normalized: None,
                archived: false,
                purchase_location_id: None,
                vendor_location_id: None,
                trip_id: None,
                source: TransactionSource::Import,
                expected_amount: None,
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
                created_at: now,
            },
            Transaction {
                id: 3,
                account_id: 1,
                date: base_date + Duration::days(60),
                description: "ELECTRIC COMPANY".to_string(),
                amount: -95.25, // Variable amount
                category: None,
                import_hash: "hash3".to_string(),
                merchant_normalized: None,
                archived: false,
                purchase_location_id: None,
                vendor_location_id: None,
                trip_id: None,
                source: TransactionSource::Import,
                expected_amount: None,
                original_data: None,
                import_format: None,
                card_member: None,
                payment_method: None,
                created_at: now,
            },
        ];

        let tx_refs: Vec<&Transaction> = transactions.iter().collect();

        // Strict detection should fail (amounts vary too much)
        let strict_result = detect_subscription_pattern(&tx_refs);
        assert!(
            strict_result.is_none(),
            "Strict detection should fail for variable amounts"
        );

        // Relaxed detection should succeed (50% variance allowed)
        let config = DetectionConfig::default();
        let relaxed_result = detect_subscription_pattern_relaxed(&tx_refs, &config);
        assert!(
            relaxed_result.is_some(),
            "Relaxed detection should succeed for variable amounts"
        );

        let sub_info = relaxed_result.unwrap();
        assert_eq!(sub_info.frequency, Frequency::Monthly);
    }

    #[test]
    fn test_detect_subscription_pattern_relaxed_requires_min_transactions() {
        use crate::models::TransactionSource;

        // Create only 1 transaction (below the 2 minimum for smart detection)
        let base_date = Utc::now().date_naive();
        let now = Utc::now();
        let transactions: Vec<Transaction> = vec![Transaction {
            id: 1,
            account_id: 1,
            date: base_date,
            description: "AWS".to_string(),
            amount: -150.00,
            category: None,
            import_hash: "hash1".to_string(),
            merchant_normalized: None,
            archived: false,
            purchase_location_id: None,
            vendor_location_id: None,
            trip_id: None,
            source: TransactionSource::Import,
            expected_amount: None,
            original_data: None,
            import_format: None,
            card_member: None,
            payment_method: None,
            created_at: now,
        }];

        let tx_refs: Vec<&Transaction> = transactions.iter().collect();
        let config = DetectionConfig::default();

        let result = detect_subscription_pattern_relaxed(&tx_refs, &config);
        assert!(
            result.is_none(),
            "Should require at least 2 transactions for smart detection"
        );
    }

    #[test]
    fn test_detection_config_smart_defaults() {
        let config = DetectionConfig::default();

        // Verify smart detection defaults
        assert_eq!(config.smart_amount_variance, 0.50); // 50%
        assert_eq!(config.smart_interval_consistency, 0.50); // 50%
        assert_eq!(config.smart_min_transactions, 2);
        assert_eq!(config.ollama_confidence_threshold, 0.7);
    }
}

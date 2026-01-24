//! Tag assignment engine for automatic transaction categorization
//!
//! Tag classification uses patterns stored in the database (auto_patterns on tags
//! and user-defined rules) with Ollama LLM integration for intelligent merchant
//! classification when pattern matching fails.
//!
//! Ollama results are cached per-session to avoid repeated API calls for the same
//! merchant description within a single import operation.

use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tracing::{debug, warn};

use crate::ai::{AIBackend, AIClient};
use crate::db::Database;
use crate::error::Result;
use crate::models::{
    NewOllamaMetric, OllamaOperation, PatternType, Tag, TagRule, TagSource, Transaction,
    TransactionSource,
};

/// Result of assigning a tag to a transaction
#[derive(Debug, Clone)]
pub struct TagAssignment {
    pub tag_id: i64,
    pub tag_name: String,
    pub source: TagSource,
    pub confidence: Option<f64>,
    /// Normalized merchant name from Ollama (only set when source is Ollama)
    pub normalized_merchant: Option<String>,
}

/// Result of a backfill operation
#[derive(Debug, Clone)]
pub struct BackfillResult {
    pub transactions_processed: i64,
    pub transactions_tagged: i64,
    pub by_learned: i64,
    pub by_rule: i64,
    pub by_pattern: i64,
    pub by_bank_category: i64,
    pub by_ollama: i64,
    pub by_ollama_cached: i64,
    pub fallback_to_other: i64,
}

/// Progress callback for tagging operations
/// Parameters: (current, total)
pub type TaggingProgressCallback = Box<dyn Fn(i64, i64) + Send + Sync>;

/// Map bank-provided category string to Hone tag path
///
/// Amex provides human-readable categories like "Transportation-Fuel", "Restaurant-Restaurant",
/// "Merchandise & Supplies-Groceries" which are based on Merchant Category Codes (MCCs).
/// This function maps those to our tag paths (e.g., "Transport.Gas" for child tags).
///
/// Returns None if the category doesn't map to a specific tag (will fall through to Ollama).
pub fn map_bank_category_to_tag(category: &str) -> Option<&'static str> {
    let cat_lower = category.to_lowercase();

    // Transportation - map to specific child tags where possible
    if cat_lower.starts_with("transportation") {
        if cat_lower.contains("fuel") || cat_lower.contains("gas") {
            return Some("Transport.Gas");
        }
        if cat_lower.contains("auto") || cat_lower.contains("service") {
            return Some("Transport.Auto");
        }
        if cat_lower.contains("parking") {
            return Some("Transport.Parking");
        }
        if cat_lower.contains("toll") {
            return Some("Transport.Tolls");
        }
        // Generic transportation
        return Some("Transport");
    }

    // Dining/Restaurants
    if cat_lower.starts_with("restaurant") || cat_lower.contains("-restaurant") {
        return Some("Dining");
    }

    // Groceries - be specific to avoid misclassifying general merchandise
    if cat_lower.contains("-groceries") || cat_lower.contains("supermarket") {
        return Some("Groceries");
    }

    // Entertainment - with special handling for associations (sports clubs, gyms)
    if cat_lower.starts_with("entertainment") {
        // "Entertainment-Associations" is typically sports clubs, gyms, fitness centers
        if cat_lower.contains("association") {
            return Some("Personal.Fitness");
        }
        return Some("Entertainment");
    }

    // Travel - airlines, lodging, car rental
    if cat_lower.starts_with("airlines")
        || cat_lower.starts_with("lodging")
        || cat_lower.starts_with("car rental")
        || cat_lower.contains("travel")
    {
        return Some("Travel");
    }

    // Healthcare/Medical
    if cat_lower.starts_with("healthcare")
        || cat_lower.starts_with("medical")
        || cat_lower.starts_with("pharmacy")
        || cat_lower.starts_with("drug")
        || cat_lower.contains("health care")
    {
        return Some("Healthcare");
    }

    // Utilities (including communications/internet/cable)
    if cat_lower.starts_with("utilities")
        || cat_lower.contains("-utilities")
        || cat_lower.starts_with("communications")
    {
        return Some("Utilities");
    }

    // Financial services
    if cat_lower.starts_with("financial")
        || cat_lower.starts_with("insurance")
        || cat_lower.contains("bank")
    {
        return Some("Financial");
    }

    // Fees and adjustments - map to Financial.Fees to exclude from subscription detection
    if cat_lower.starts_with("fees") || cat_lower.contains("fee") || cat_lower.contains("interest")
    {
        return Some("Financial.Fees");
    }

    // Shopping/Retail - route to specific subcategories where possible

    // Clothing
    if cat_lower.contains("clothing") || cat_lower.contains("apparel") {
        return Some("Shopping.Clothing");
    }

    // Electronics
    if cat_lower.contains("electronics store") || cat_lower.contains("computer") {
        return Some("Shopping.Electronics");
    }

    // Home & Garden
    if cat_lower.contains("hardware store")
        || cat_lower.contains("hardware supplies")
        || cat_lower.contains("home improvement")
        || cat_lower.contains("garden")
        || cat_lower.contains("nursery")
        || cat_lower.contains("furniture")
        || cat_lower.contains("florist")
    {
        return Some("Shopping.Home & Garden");
    }

    // Auto Parts (distinct from Transport.Auto which is services like oil changes)
    if cat_lower.contains("auto parts") || cat_lower.contains("automotive parts") {
        return Some("Shopping.Auto Parts");
    }

    // General shopping (department stores, discount stores, etc.)
    if cat_lower.contains("department store")
        || cat_lower.contains("discount store")
        || cat_lower.contains("sporting goods")
        || cat_lower.contains("office supplies")
        || cat_lower.contains("book store")
        || cat_lower.contains("jewelry")
        || cat_lower.contains("toy store")
    {
        return Some("Shopping.General");
    }

    // Education
    if cat_lower.starts_with("education") || cat_lower.contains("-education") {
        return Some("Education");
    }

    // Pet-related
    if cat_lower.contains("veterinary") || cat_lower.contains("pet ") {
        return Some("Pets");
    }

    // Government services
    if cat_lower.starts_with("government") {
        return Some("Financial"); // Fees, licenses, etc.
    }

    // Charitable donations
    if cat_lower.contains("charitable") || cat_lower.contains("donation") {
        return Some("Gifts");
    }

    // Personal services
    if cat_lower.contains("beauty")
        || cat_lower.contains("salon")
        || cat_lower.contains("barber")
        || cat_lower.contains("spa")
    {
        return Some("Personal");
    }

    // Internet purchases - typically subscriptions/software services
    if cat_lower.contains("internet purchase") {
        return Some("Subscriptions.Software");
    }

    // Payments received / credits - income
    // Note: This catches things like "ELECTRONIC PAYMENT RECEIVED" (bill payments)
    // and merchant refunds. These show as credits (positive amounts).
    if cat_lower.contains("payment received") || cat_lower.contains("refund") {
        return Some("Income");
    }

    // For generic categories like "Business Services", etc.,
    // return None to let Ollama classify based on merchant name
    None
}

/// Tag assignment engine with AI backend integration and per-session caching
pub struct TagAssigner<'a> {
    db: &'a Database,
    ai: Option<&'a AIClient>,
    /// Per-session cache for AI classifications (description -> TagAssignment)
    /// Uses Mutex for thread-safety in async contexts
    ai_cache: Mutex<HashMap<String, Option<TagAssignment>>>,
}

impl<'a> TagAssigner<'a> {
    /// Create a new tag assigner with optional AI client
    pub fn new(db: &'a Database, ai: Option<&'a AIClient>) -> Self {
        Self {
            db,
            ai,
            ai_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Auto-assign tags to a transaction
    /// Priority: learned (user corrections) → user rules → auto_patterns → bank category → Ollama → "Other"
    pub async fn assign_tags(&self, transaction: &Transaction) -> Result<Option<TagAssignment>> {
        let description = &transaction.description;

        // 0. HIGHEST PRIORITY: Check learned merchant→tag cache (from user's previous manual tags)
        // User corrections always win - if you manually tagged a COSTCO transaction as Groceries,
        // future COSTCO transactions should also be tagged Groceries.
        if let Some((tag_id, tag_name, confidence)) =
            self.db.get_cached_merchant_tag(description)?
        {
            debug!(
                "Learned tag matched for '{}': {} (confidence: {})",
                description, tag_name, confidence
            );
            return Ok(Some(TagAssignment {
                tag_id,
                tag_name,
                source: TagSource::Learned,
                confidence: Some(confidence),
                normalized_merchant: None,
            }));
        }

        // 1. Try user-defined rules (explicit patterns user created)
        if let Some(assignment) = self.apply_rules(description)? {
            debug!(
                "Rule matched for '{}': {}",
                description, assignment.tag_name
            );
            return Ok(Some(assignment));
        }

        // 2. Try auto_patterns on root tags (patterns stored in database)
        if let Some(assignment) = self.apply_auto_patterns(description)? {
            debug!(
                "Pattern matched for '{}': {}",
                description, assignment.tag_name
            );
            return Ok(Some(assignment));
        }

        // 3. Try bank-provided category (e.g., Amex "Transportation-Fuel")
        if let Some(assignment) = self.apply_bank_category(transaction)? {
            debug!(
                "Bank category matched for '{}': {} (from {:?})",
                description, assignment.tag_name, transaction.category
            );
            return Ok(Some(assignment));
        }

        // 4. Try Ollama LLM classification
        if let Some(assignment) = self.classify_with_ai(description).await? {
            debug!(
                "Ollama classified '{}': {} (confidence: {:?})",
                description, assignment.tag_name, assignment.confidence
            );
            return Ok(Some(assignment));
        }

        // 5. Fall back to "Other" tag
        if let Some(other_tag) = self.db.get_tag_by_path("Other")? {
            debug!("Falling back to 'Other' for '{}'", description);
            return Ok(Some(TagAssignment {
                tag_id: other_tag.id,
                tag_name: "Other".to_string(),
                source: TagSource::Pattern,
                confidence: Some(0.0),
                normalized_merchant: None,
            }));
        }

        Ok(None)
    }

    /// Try to map bank-provided category to a Hone tag
    fn apply_bank_category(&self, transaction: &Transaction) -> Result<Option<TagAssignment>> {
        let category = match &transaction.category {
            Some(cat) if !cat.is_empty() => cat,
            _ => return Ok(None),
        };

        let tag_name = match map_bank_category_to_tag(category) {
            Some(name) => name,
            None => return Ok(None),
        };

        // Look up the tag in the database
        if let Some(tag) = self.db.get_tag_by_path(tag_name)? {
            return Ok(Some(TagAssignment {
                tag_id: tag.id,
                tag_name: tag_name.to_string(),
                source: TagSource::BankCategory,
                confidence: Some(0.75), // High confidence for bank-provided categories
                normalized_merchant: None,
            }));
        }

        Ok(None)
    }

    /// Classify a transaction description using Ollama LLM (with per-session caching)
    async fn classify_with_ai(&self, description: &str) -> Result<Option<TagAssignment>> {
        self.classify_with_ai_inner(description, false).await
    }

    /// Check if a classification is cached (for counting cache hits in backfill)
    fn is_ai_cached(&self, description: &str) -> bool {
        self.ai_cache.lock().unwrap().contains_key(description)
    }

    /// Inner implementation that can optionally skip cache for fresh lookups
    async fn classify_with_ai_inner(
        &self,
        description: &str,
        _skip_cache: bool,
    ) -> Result<Option<TagAssignment>> {
        let ai = match self.ai {
            Some(o) => o,
            None => return Ok(None),
        };

        // Check cache first
        {
            let cache = self.ai_cache.lock().unwrap();
            if let Some(cached) = cache.get(description) {
                debug!("Ollama cache hit for '{}'", description);
                return Ok(cached.clone());
            }
        }

        // Time the AI call for metrics
        let start = Instant::now();
        let result = ai.classify_merchant(description).await;
        let latency_ms = start.elapsed().as_millis() as i64;

        // Record the metric with input/output for debugging
        let confidence = 0.7; // Default confidence for LLM
        let result_text = result
            .as_ref()
            .ok()
            .map(|c| format!("{} → {}", c.merchant, c.category));
        let metric = NewOllamaMetric {
            operation: OllamaOperation::ClassifyMerchant,
            model: ai.model().to_string(),
            latency_ms,
            success: result.is_ok(),
            error_message: result.as_ref().err().map(|e| e.to_string()),
            confidence: if result.is_ok() {
                Some(confidence)
            } else {
                None
            },
            transaction_id: None, // Will be set by caller if needed
            input_text: Some(description.to_string()),
            result_text,
            metadata: None,
        };
        if let Err(e) = self.db.record_ollama_metric(&metric) {
            warn!("Failed to record Ollama metric: {}", e);
        }

        let assignment = match result {
            Ok(classification) => {
                // Try to find the tag by category name
                let category = &classification.category;
                // Extract normalized merchant name from Ollama response
                let normalized_merchant = classification.merchant.clone();

                // Map Ollama categories to our tag names
                let tag_name = match category.as_str() {
                    "streaming" | "music" => "Subscriptions.Streaming",
                    "cloud_storage" => "Subscriptions.Cloud",
                    "software" => "Subscriptions.Software",
                    "home_security" => "Subscriptions.Software",
                    "fitness" => "Personal.Fitness",
                    "news" => "Subscriptions",
                    "food_delivery" => "Dining",
                    "shopping" => "Shopping",
                    "utilities" => "Utilities",
                    "groceries" => "Groceries",
                    "transport" | "gas" | "rideshare" => "Transport",
                    "entertainment" => "Entertainment",
                    "travel" | "hotel" | "airline" => "Travel",
                    "healthcare" | "pharmacy" => "Healthcare",
                    "dining" | "restaurant" => "Dining",
                    "income" | "salary" | "deposit" => "Income",
                    "housing" | "rent" | "mortgage" => "Housing",
                    "gifts" => "Gifts",
                    "financial" | "bank" | "investment" => "Financial",
                    _ => "Other",
                };

                if let Some(tag) = self.db.get_tag_by_path(tag_name)? {
                    Some(TagAssignment {
                        tag_id: tag.id,
                        tag_name: tag.name.clone(),
                        source: TagSource::Ollama,
                        confidence: Some(confidence),
                        normalized_merchant: Some(normalized_merchant),
                    })
                } else {
                    None
                }
            }
            Err(e) => {
                warn!("Ollama classification failed for '{}': {}", description, e);
                None
            }
        };

        // Cache the result (including None for failed lookups)
        self.ai_cache
            .lock()
            .unwrap()
            .insert(description.to_string(), assignment.clone());

        Ok(assignment)
    }

    /// Apply user-defined rules to find a matching tag
    fn apply_rules(&self, description: &str) -> Result<Option<TagAssignment>> {
        let rules = self.db.list_tag_rules()?;

        for rule_with_tag in rules {
            let rule = &rule_with_tag.rule;
            if self.pattern_matches(description, &rule.pattern, rule.pattern_type)? {
                return Ok(Some(TagAssignment {
                    tag_id: rule.tag_id,
                    tag_name: rule_with_tag.tag_name.clone(),
                    source: TagSource::Rule,
                    confidence: Some(1.0),
                    normalized_merchant: None,
                }));
            }
        }

        Ok(None)
    }

    /// Check if a description matches a pattern
    fn pattern_matches(
        &self,
        description: &str,
        pattern: &str,
        pattern_type: PatternType,
    ) -> Result<bool> {
        let desc_upper = description.to_uppercase();

        match pattern_type {
            PatternType::Contains => {
                // Support pipe-separated OR patterns
                let patterns: Vec<&str> = pattern.split('|').collect();
                for p in patterns {
                    if desc_upper.contains(&p.to_uppercase()) {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            PatternType::Regex => {
                let re = Regex::new(pattern)?;
                Ok(re.is_match(description) || re.is_match(&desc_upper))
            }
            PatternType::Exact => Ok(desc_upper == pattern.to_uppercase()),
        }
    }

    /// Apply auto_patterns from root tags
    fn apply_auto_patterns(&self, description: &str) -> Result<Option<TagAssignment>> {
        let root_tags = self.db.list_root_tags()?;

        for tag in root_tags {
            if let Some(ref patterns) = tag.auto_patterns {
                if self.pattern_matches(description, patterns, PatternType::Contains)? {
                    return Ok(Some(TagAssignment {
                        tag_id: tag.id,
                        tag_name: tag.name.clone(),
                        source: TagSource::Pattern,
                        confidence: Some(0.8),
                        normalized_merchant: None,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Backfill tags for untagged transactions
    pub async fn backfill_tags(&self, limit: i64) -> Result<BackfillResult> {
        let untagged = self.db.get_untagged_transactions(limit)?;
        self.backfill_transactions(&untagged, None).await
    }

    /// Backfill tags for untagged transactions from a specific import session
    pub async fn backfill_tags_for_session(&self, session_id: i64) -> Result<BackfillResult> {
        let untagged = self.db.get_untagged_transactions_for_session(session_id)?;
        self.backfill_transactions(&untagged, None).await
    }

    /// Backfill tags for untagged transactions from a specific import session with progress callback
    pub async fn backfill_tags_for_session_with_progress(
        &self,
        session_id: i64,
        progress: Option<&TaggingProgressCallback>,
    ) -> Result<BackfillResult> {
        let untagged = self.db.get_untagged_transactions_for_session(session_id)?;
        self.backfill_transactions(&untagged, progress).await
    }

    /// Internal helper to process a list of transactions for tagging
    async fn backfill_transactions(
        &self,
        untagged: &[Transaction],
        progress: Option<&TaggingProgressCallback>,
    ) -> Result<BackfillResult> {
        let mut result = BackfillResult {
            transactions_processed: untagged.len() as i64,
            transactions_tagged: 0,
            by_learned: 0,
            by_rule: 0,
            by_pattern: 0,
            by_bank_category: 0,
            by_ollama: 0,
            by_ollama_cached: 0,
            fallback_to_other: 0,
        };

        let total = untagged.len() as i64;
        let mut current = 0i64;

        for tx in untagged {
            // Report progress
            current += 1;
            if let Some(cb) = progress {
                cb(current, total);
            }
            // Check if this description is already cached (for Ollama hit tracking)
            let was_cached = self.is_ai_cached(&tx.description);

            if let Some(assignment) = self.assign_tags(tx).await? {
                self.db.add_transaction_tag(
                    tx.id,
                    assignment.tag_id,
                    assignment.source.clone(),
                    assignment.confidence,
                )?;

                // Merchant normalization handled by normalize_merchants() with specialized prompt

                result.transactions_tagged += 1;
                match assignment.source {
                    TagSource::Learned => result.by_learned += 1,
                    TagSource::Rule => result.by_rule += 1,
                    TagSource::BankCategory => result.by_bank_category += 1,
                    TagSource::Ollama => {
                        if was_cached {
                            result.by_ollama_cached += 1;
                        } else {
                            result.by_ollama += 1;
                        }
                    }
                    TagSource::Pattern => {
                        if assignment.tag_name == "Other" {
                            result.fallback_to_other += 1;
                        } else {
                            result.by_pattern += 1;
                        }
                    }
                    TagSource::Manual => {}
                }
            } else {
                // This should only happen if the "Other" tag doesn't exist
                warn!(
                    "Failed to assign any tag to transaction {} ({})",
                    tx.id, tx.description
                );
            }
        }

        Ok(result)
    }

    /// Test what tag a description would match
    pub async fn test_assignment(&self, description: &str) -> Result<Option<TagAssignment>> {
        // Create a fake transaction for testing
        let fake_tx = Transaction {
            id: 0,
            account_id: 0,
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            description: description.to_string(),
            amount: 0.0,
            category: None,
            merchant_normalized: None,
            import_hash: String::new(),
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
            created_at: chrono::Utc::now(),
        };

        self.assign_tags(&fake_tx).await
    }
}

/// Test rules against a description and return all matching rules
pub fn test_rules_against(db: &Database, description: &str) -> Result<Vec<(TagRule, Tag)>> {
    let rules = db.list_tag_rules()?;
    let mut matches = Vec::new();

    let desc_upper = description.to_uppercase();

    for rule_with_tag in rules {
        let rule = rule_with_tag.rule;
        let matched = match rule.pattern_type {
            PatternType::Contains => {
                let patterns: Vec<&str> = rule.pattern.split('|').collect();
                patterns
                    .iter()
                    .any(|p| desc_upper.contains(&p.to_uppercase()))
            }
            PatternType::Regex => {
                if let Ok(re) = Regex::new(&rule.pattern) {
                    re.is_match(description) || re.is_match(&desc_upper)
                } else {
                    false
                }
            }
            PatternType::Exact => desc_upper == rule.pattern.to_uppercase(),
        };

        if matched {
            if let Some(tag) = db.get_tag(rule.tag_id)? {
                matches.push((rule, tag));
            }
        }
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{AIClient, OllamaBackend};

    fn setup_test_db() -> Database {
        let db = Database::in_memory().unwrap();
        db.seed_root_tags().unwrap();
        db
    }

    #[test]
    fn test_pattern_matching_contains() {
        let db = setup_test_db();
        let assigner = TagAssigner::new(&db, None);

        // Basic contains match
        assert!(assigner
            .pattern_matches("SHELL OIL", "SHELL", PatternType::Contains)
            .unwrap());

        // Case insensitive
        assert!(assigner
            .pattern_matches("shell oil", "SHELL", PatternType::Contains)
            .unwrap());

        // Pipe-separated OR
        assert!(assigner
            .pattern_matches("CHEVRON", "SHELL|CHEVRON|EXXON", PatternType::Contains)
            .unwrap());

        // No match
        assert!(!assigner
            .pattern_matches("GROCERY STORE", "SHELL|CHEVRON", PatternType::Contains)
            .unwrap());
    }

    #[test]
    fn test_pattern_matching_regex() {
        let db = setup_test_db();
        let assigner = TagAssigner::new(&db, None);

        // Regex match
        assert!(assigner
            .pattern_matches("NETFLIX.COM/BILL", r"NETFLIX.*", PatternType::Regex)
            .unwrap());

        // No match
        assert!(!assigner
            .pattern_matches("HULU", r"^NETFLIX.*", PatternType::Regex)
            .unwrap());
    }

    #[test]
    fn test_pattern_matching_exact() {
        let db = setup_test_db();
        let assigner = TagAssigner::new(&db, None);

        // Exact match (case insensitive)
        assert!(assigner
            .pattern_matches("NETFLIX", "Netflix", PatternType::Exact)
            .unwrap());

        // Not exact
        assert!(!assigner
            .pattern_matches("NETFLIX.COM", "NETFLIX", PatternType::Exact)
            .unwrap());
    }

    #[test]
    fn test_auto_patterns() {
        let db = setup_test_db();

        // Add a pattern to test with (since seeded tags no longer have auto_patterns)
        let transport_tag = db.get_tag_by_path("Transport").unwrap().unwrap();
        db.update_tag(
            transport_tag.id,
            None,
            None,
            None,
            None,
            Some(Some("UBER|LYFT")),
        )
        .unwrap();

        let assigner = TagAssigner::new(&db, None);

        // Should match Transport tag's auto_patterns
        let result = assigner.apply_auto_patterns("UBER TRIP").unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        assert_eq!(assignment.tag_name, "Transport");
        assert_eq!(assignment.source, TagSource::Pattern);
    }

    #[test]
    fn test_rule_priority() {
        let db = setup_test_db();

        // Create a rule with high priority
        let transport_tag = db.get_tag_by_path("Transport").unwrap().unwrap();
        db.create_tag_rule(transport_tag.id, "UBER", PatternType::Contains, 100)
            .unwrap();

        // Create another tag and rule with lower priority
        let dining_tag = db.get_tag_by_path("Dining").unwrap().unwrap();
        db.create_tag_rule(dining_tag.id, "UBER EATS", PatternType::Contains, 50)
            .unwrap();

        let assigner = TagAssigner::new(&db, None);

        // "UBER EATS" matches both, but higher priority rule should win
        let result = assigner.apply_rules("UBER EATS DELIVERY").unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        // Higher priority (100) wins
        assert_eq!(assignment.tag_name, "Transport");
    }

    #[tokio::test]
    async fn test_assign_tags_priority() {
        let db = setup_test_db();

        // Create a rule that should take precedence
        let groceries_tag = db.get_tag_by_path("Groceries").unwrap().unwrap();
        db.create_tag_rule(groceries_tag.id, "COSTCO", PatternType::Contains, 10)
            .unwrap();

        let assigner = TagAssigner::new(&db, None);

        // Rule should match before auto_patterns
        let result = assigner.test_assignment("COSTCO WHOLESALE").await.unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        assert_eq!(assignment.tag_name, "Groceries");
        assert_eq!(assignment.source, TagSource::Rule);
    }

    #[tokio::test]
    async fn test_fallback_to_other() {
        let db = setup_test_db();
        let assigner = TagAssigner::new(&db, None);

        // Something that won't match anything (no Ollama, so falls back to Other)
        let result = assigner
            .test_assignment("RANDOM UNKNOWN MERCHANT XYZ123")
            .await
            .unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        assert_eq!(assignment.tag_name, "Other");
    }

    #[tokio::test]
    async fn test_backfill_without_ollama() {
        let db = setup_test_db();

        // Create account and untagged transactions
        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'UBER TRIP', -25.0, 'hash1')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-02', 'RANDOM MERCHANT', -15.0, 'hash2')",
                [],
            )
            .unwrap();
        }

        let assigner = TagAssigner::new(&db, None);
        let result = assigner.backfill_tags(100).await.unwrap();

        assert_eq!(result.transactions_processed, 2);
        assert_eq!(result.transactions_tagged, 2);
        // Without auto_patterns, both fall back to Other
        assert_eq!(result.by_pattern, 0);
        assert_eq!(result.by_ollama, 0); // No Ollama client
        assert_eq!(result.fallback_to_other, 2);
    }

    #[tokio::test]
    async fn test_ollama_classification() {
        use crate::test_utils::MockOllamaServer;

        let db = setup_test_db();

        // Start mock Ollama server
        let mut mock_server = MockOllamaServer::start().await;
        let ai = AIClient::Ollama(OllamaBackend::new(&mock_server.url(), "test-model"));

        let assigner = TagAssigner::new(&db, Some(&ai));

        // PELOTON is not in auto_patterns, so it will be classified by Ollama as "fitness" -> Personal.Fitness
        let result = assigner
            .test_assignment("PELOTON SUBSCRIPTION")
            .await
            .unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        assert_eq!(assignment.tag_name, "Fitness");
        assert_eq!(assignment.source, TagSource::Ollama);

        mock_server.stop();
    }

    #[tokio::test]
    async fn test_rule_takes_priority_over_ollama() {
        use crate::test_utils::MockOllamaServer;

        let db = setup_test_db();

        // Create a rule that should match before Ollama is tried
        let entertainment_tag = db.get_tag_by_path("Entertainment").unwrap().unwrap();
        db.create_tag_rule(entertainment_tag.id, "NETFLIX", PatternType::Contains, 10)
            .unwrap();

        // Start mock Ollama server
        let mut mock_server = MockOllamaServer::start().await;
        let ai = AIClient::Ollama(OllamaBackend::new(&mock_server.url(), "test-model"));

        let assigner = TagAssigner::new(&db, Some(&ai));

        // NETFLIX matches rule, so it matches before Ollama is tried
        let result = assigner.test_assignment("NETFLIX.COM BILL").await.unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        assert_eq!(assignment.tag_name, "Entertainment"); // Matched by rule
        assert_eq!(assignment.source, TagSource::Rule); // Not Ollama

        mock_server.stop();
    }

    #[tokio::test]
    async fn test_backfill_with_ollama() {
        use crate::test_utils::MockOllamaServer;

        let db = setup_test_db();

        // Create account and untagged transactions
        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, bank) VALUES ('Test', 'chase')",
                [],
            )
            .unwrap();
            // UBER will be classified by Ollama as "transport" -> Transport
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-01', 'UBER TRIP', -25.0, 'hash1')",
                [],
            )
            .unwrap();
            // STRAVA will be classified by Ollama as "fitness" -> Personal
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-02', 'STRAVA SUBSCRIPTION', -45.0, 'hash2')",
                [],
            )
            .unwrap();
            // Unknown will be classified by Ollama as "other" -> Other
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_hash) VALUES (1, '2024-01-03', 'XYZABC123 BLAH', -10.0, 'hash3')",
                [],
            )
            .unwrap();
        }

        // Start mock Ollama server
        let mut mock_server = MockOllamaServer::start().await;
        let ai = AIClient::Ollama(OllamaBackend::new(&mock_server.url(), "test-model"));

        let assigner = TagAssigner::new(&db, Some(&ai));
        let result = assigner.backfill_tags(100).await.unwrap();

        assert_eq!(result.transactions_processed, 3);
        assert_eq!(result.transactions_tagged, 3);
        assert_eq!(result.by_pattern, 0); // No auto_patterns on seeded tags
        assert_eq!(result.by_ollama, 3); // All three via Ollama
        assert_eq!(result.fallback_to_other, 0); // Nothing falls back when Ollama is available

        mock_server.stop();
    }

    #[test]
    fn test_rules_against_description() {
        let db = setup_test_db();

        // Create rules
        let transport_tag = db.get_tag_by_path("Transport").unwrap().unwrap();
        db.create_tag_rule(transport_tag.id, "SHELL|CHEVRON", PatternType::Contains, 10)
            .unwrap();
        db.create_tag_rule(transport_tag.id, "UBER", PatternType::Contains, 5)
            .unwrap();

        // Test what rules match
        let matches = test_rules_against(&db, "SHELL GAS STATION").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0.pattern, "SHELL|CHEVRON");

        let matches = test_rules_against(&db, "RANDOM").unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_bank_category_mapping() {
        // Transportation categories - now maps to child tags
        assert_eq!(
            map_bank_category_to_tag("Transportation-Fuel"),
            Some("Transport.Gas")
        );
        assert_eq!(
            map_bank_category_to_tag("Transportation-Auto Services"),
            Some("Transport.Auto")
        );
        assert_eq!(
            map_bank_category_to_tag("Transportation-Parking"),
            Some("Transport.Parking")
        );
        assert_eq!(
            map_bank_category_to_tag("Transportation-Tolls"),
            Some("Transport.Tolls")
        );
        // Generic transportation (no specific subcategory)
        assert_eq!(
            map_bank_category_to_tag("Transportation-Other"),
            Some("Transport")
        );

        // Restaurant/Dining categories
        assert_eq!(
            map_bank_category_to_tag("Restaurant-Restaurant"),
            Some("Dining")
        );
        assert_eq!(
            map_bank_category_to_tag("Restaurant-Fast Food"),
            Some("Dining")
        );

        // Grocery categories
        assert_eq!(
            map_bank_category_to_tag("Merchandise & Supplies-Groceries"),
            Some("Groceries")
        );

        // Entertainment - associations map to fitness
        assert_eq!(
            map_bank_category_to_tag("Entertainment-Associations"),
            Some("Personal.Fitness")
        );
        // Other entertainment stays as Entertainment
        assert_eq!(
            map_bank_category_to_tag("Entertainment-Movies"),
            Some("Entertainment")
        );

        // Travel
        assert_eq!(map_bank_category_to_tag("Airlines-Airline"), Some("Travel"));
        assert_eq!(
            map_bank_category_to_tag("Lodging-Hotels/Motels"),
            Some("Travel")
        );

        // Healthcare
        assert_eq!(
            map_bank_category_to_tag("Healthcare-Medical Services"),
            Some("Healthcare")
        );
        assert_eq!(
            map_bank_category_to_tag("Pharmacy-Drug Stores"),
            Some("Healthcare")
        );

        // Financial
        assert_eq!(
            map_bank_category_to_tag("Fees & Adjustments-Fees"),
            Some("Financial.Fees")
        );
        assert_eq!(
            map_bank_category_to_tag("Interest Charge"),
            Some("Financial.Fees")
        );
        assert_eq!(
            map_bank_category_to_tag("Financial Services-Insurance"),
            Some("Financial")
        );

        // Internet purchases map to Subscriptions.Software
        assert_eq!(
            map_bank_category_to_tag("Merchandise & Supplies-Internet Purchase"),
            Some("Subscriptions.Software")
        );

        // Generic categories should return None (let Ollama handle)
        assert_eq!(
            map_bank_category_to_tag("Business Services-Professional Services"),
            None
        );
    }

    #[tokio::test]
    async fn test_bank_category_priority() {
        let db = setup_test_db();
        let assigner = TagAssigner::new(&db, None);

        // Create a transaction with a bank category but description that won't match auto_patterns
        // Note: auto_patterns for Transport include GAS|UBER|LYFT|PARKING|TRANSIT
        let tx = Transaction {
            id: 1,
            account_id: 1,
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            description: "CHEVRON 12345 SEATTLE".to_string(), // Won't match "GAS" pattern
            amount: -50.0,
            category: Some("Transportation-Fuel".to_string()), // Bank category -> Transport.Gas
            merchant_normalized: None,
            import_hash: "test".to_string(),
            purchase_location_id: None,
            vendor_location_id: None,
            trip_id: None,
            source: TransactionSource::Import,
            expected_amount: None,
            archived: false,
            original_data: None,
            import_format: Some("amex_csv".to_string()),
            card_member: None,
            payment_method: None,
            created_at: chrono::Utc::now(),
        };

        let result = assigner.assign_tags(&tx).await.unwrap();
        assert!(result.is_some());
        let assignment = result.unwrap();
        // Now maps to child tag Transport.Gas
        assert_eq!(assignment.tag_name, "Transport.Gas");
        assert_eq!(assignment.source, TagSource::BankCategory);
        assert_eq!(assignment.confidence, Some(0.75));
    }
}

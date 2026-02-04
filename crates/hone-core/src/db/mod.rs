//! Database access layer with connection pooling and migrations
//!
//! This module is organized by domain:
//! - `accounts` - Bank account operations
//! - `transactions` - Transaction CRUD
//! - `subscriptions` - Subscription detection and management
//! - `alerts` - Alert and dashboard operations
//! - `tags` - Hierarchical tags, rules, and transaction-tag associations
//! - `entities` - Entities, splits, locations, trips, mileage
//! - `receipts` - Receipt workflow operations
//! - `reports` - Spending reports and analytics
//! - `ollama_metrics` - Ollama LLM call tracking and quality metrics

use chrono::{DateTime, Utc};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use tracing::info;

use crate::error::{Error, Result};

mod accounts;
mod alerts;
mod backup;
mod entities;
mod feedback;
mod import_history;
mod insights;
mod ollama_metrics;
mod receipts;
mod reports;
mod subscriptions;
mod tags;
mod transaction_filter;
mod transactions;

pub use transaction_filter::{FilterResult, TransactionFilter};
pub use transactions::TransactionInsertResult;

pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConn = PooledConnection<SqliteConnectionManager>;

/// Environment variable for database encryption key
pub const DB_KEY_ENV: &str = "HONE_DB_KEY";

/// Derive an encryption key from a passphrase using Argon2
///
/// Uses a fixed application salt so the same passphrase always produces the same key,
/// regardless of database path. This allows moving/renaming/restoring the database freely.
fn derive_key(passphrase: &str) -> Result<String> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};

    // Fixed application salt - changing this would invalidate all existing encrypted databases
    const APP_SALT: &[u8; 16] = b"hone-salt-v1-fix";

    let salt = SaltString::encode_b64(APP_SALT)
        .map_err(|e| Error::Encryption(format!("Failed to create salt: {}", e)))?;

    // Derive key using Argon2id
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(passphrase.as_bytes(), &salt)
        .map_err(|e| Error::Encryption(format!("Failed to derive key: {}", e)))?;

    // Extract the hash portion for use as SQLCipher key (hex encoded)
    let hash_str = hash
        .hash
        .ok_or_else(|| Error::Encryption("No hash output".to_string()))?;
    Ok(hex::encode(hash_str.as_bytes()))
}

/// Parse a SQLite datetime string into a DateTime<Utc>
pub(crate) fn parse_datetime(s: &str) -> DateTime<Utc> {
    // SQLite stores as "YYYY-MM-DD HH:MM:SS" format
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc())
        .unwrap_or_else(|_| Utc::now())
}

/// Database wrapper with connection pooling
#[derive(Clone)]
pub struct Database {
    pool: DbPool,
    /// Path to the database file
    db_path: String,
}

impl Database {
    /// Create a new database connection pool with encryption
    ///
    /// Requires `HONE_DB_KEY` environment variable to be set.
    /// The database will be encrypted using SQLCipher with a key derived
    /// from the passphrase via Argon2.
    ///
    /// Returns an error if `HONE_DB_KEY` is not set. Use `new_unencrypted()`
    /// for development/testing without encryption.
    pub fn new(path: &str) -> Result<Self> {
        let encryption_key = std::env::var(DB_KEY_ENV).ok();
        match encryption_key {
            Some(key) => Self::new_with_key(path, Some(&key)),
            None => Err(Error::Encryption(format!(
                "Database encryption required. Set {} environment variable with your passphrase, \
                or use --no-encrypt for unencrypted databases (not recommended for production).",
                DB_KEY_ENV
            ))),
        }
    }

    /// Create a new unencrypted database connection pool
    ///
    /// WARNING: This creates an unencrypted database. Only use for development
    /// or testing. For production, use `new()` with `HONE_DB_KEY` set.
    pub fn new_unencrypted(path: &str) -> Result<Self> {
        Self::new_with_key(path, None)
    }

    /// Create a new database with an explicit encryption key
    pub fn new_with_key(path: &str, passphrase: Option<&str>) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);

        let pool = if let Some(pass) = passphrase {
            let key = derive_key(pass)?;
            let key_pragma = format!("PRAGMA key = 'x\"{}\"';", key);

            // Use with_init to set the key on every new connection
            let manager = manager.with_init(move |conn| {
                conn.execute_batch(&key_pragma)?;
                Ok(())
            });

            Pool::builder().max_size(10).build(manager)?
        } else {
            Pool::builder().max_size(10).build(manager)?
        };

        let db = Self {
            pool,
            db_path: path.to_string(),
        };
        db.run_migrations()?;

        Ok(db)
    }

    /// Get the path to the database file
    pub fn path(&self) -> &str {
        &self.db_path
    }

    /// Create an in-memory database (for testing)
    ///
    /// Note: Uses a temporary file rather than `:memory:` because SQLCipher
    /// has issues with in-memory databases in the connection pool.
    pub fn in_memory() -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = format!("/tmp/hone_test_{}.db", id);

        // Remove any existing file
        let _ = std::fs::remove_file(&path);

        Self::new_unencrypted(&path)
    }

    /// Check if the database is encrypted
    pub fn is_encrypted(&self) -> Result<bool> {
        let conn = self.conn()?;
        // SQLCipher sets cipher_version if encryption is active
        let result: rusqlite::Result<String> =
            conn.query_row("PRAGMA cipher_version;", [], |row| row.get(0));
        Ok(result.is_ok() && std::env::var(DB_KEY_ENV).is_ok())
    }

    /// Get a connection from the pool
    pub fn conn(&self) -> Result<DbConn> {
        Ok(self.pool.get()?)
    }

    /// Soft reset: clear all transactional data but preserve configuration
    ///
    /// Clears: transactions, subscriptions, alerts, receipts, audit_log, ollama_metrics,
    ///         transaction_tags, transaction_splits, split_tags, price_history, mileage_logs,
    ///         import_sessions, import_skipped_transactions
    /// Preserves: accounts, tags, tag_rules, entities, locations, trips, merchant_aliases
    pub fn soft_reset(&self) -> Result<()> {
        let conn = self.conn()?;

        // Delete in order respecting foreign key constraints
        conn.execute_batch(
            r#"
            DELETE FROM split_tags;
            DELETE FROM transaction_splits;
            DELETE FROM transaction_tags;
            DELETE FROM mileage_logs;
            DELETE FROM price_history;
            DELETE FROM receipts;
            DELETE FROM alerts;
            DELETE FROM subscriptions;
            DELETE FROM import_skipped_transactions;
            DELETE FROM transactions;
            DELETE FROM import_sessions;
            DELETE FROM audit_log;
            DELETE FROM ollama_metrics;
            DELETE FROM ollama_corrections;
            DELETE FROM user_feedback;
            "#,
        )?;

        info!("Database soft reset complete");
        Ok(())
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn()?;

        conn.execute_batch(
            r#"
            -- Enable foreign keys
            PRAGMA foreign_keys = ON;

            -- Performance pragmas for local storage (SSD/M.2 recommended)
            -- WAL mode: better concurrency, readers don't block writers
            -- Note: creates -wal and -shm sidecar files alongside the database
            PRAGMA journal_mode = WAL;

            -- Cache size: ~8MB (2000 pages * 4KB default page size)
            PRAGMA cache_size = 2000;

            -- Synchronous NORMAL: good balance of safety and performance
            -- FULL is safer but slower; NORMAL is safe for most power-loss scenarios
            PRAGMA synchronous = NORMAL;

            -- Store temp tables in memory (faster for complex queries)
            PRAGMA temp_store = MEMORY;

            -- Accounts (bank accounts)
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                bank TEXT NOT NULL,
                account_type TEXT,
                entity_id INTEGER REFERENCES entities(id),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_accounts_entity ON accounts(entity_id);

            -- Locations (for tracking where purchases were made)
            -- Defined before transactions because transactions references locations
            CREATE TABLE IF NOT EXISTS locations (
                id INTEGER PRIMARY KEY,
                name TEXT,
                address TEXT,
                city TEXT,
                state TEXT,
                country TEXT DEFAULT 'US',
                latitude REAL,
                longitude REAL,
                location_type TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_locations_type ON locations(location_type);
            CREATE INDEX IF NOT EXISTS idx_locations_city ON locations(city);

            -- Transactions
            CREATE TABLE IF NOT EXISTS transactions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER REFERENCES accounts(id),
                date DATE NOT NULL,
                description TEXT NOT NULL,
                amount REAL NOT NULL,
                category TEXT,
                merchant_normalized TEXT,
                import_hash TEXT UNIQUE,
                purchase_location_id INTEGER REFERENCES locations(id),
                vendor_location_id INTEGER REFERENCES locations(id),
                trip_id INTEGER REFERENCES trips(id),
                source TEXT DEFAULT 'import',              -- import, receipt, manual
                expected_amount REAL,                      -- for tip discrepancy tracking
                archived BOOLEAN DEFAULT 0,                -- hidden from reports/lists
                original_data TEXT,                        -- JSON of original import data
                import_format TEXT,                        -- e.g., chase_csv, amex_csv, receipt, manual
                card_member TEXT,                          -- cardholder name (from Amex extended format)
                payment_method TEXT,                       -- apple_pay, google_pay, physical_card, online, etc.
                import_session_id INTEGER REFERENCES import_sessions(id),  -- which import this came from
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            -- Index for common queries
            CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
            CREATE INDEX IF NOT EXISTS idx_transactions_merchant ON transactions(merchant_normalized);
            CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id);
            CREATE INDEX IF NOT EXISTS idx_transactions_archived ON transactions(archived);
            CREATE INDEX IF NOT EXISTS idx_transactions_import_session ON transactions(import_session_id);
            CREATE INDEX IF NOT EXISTS idx_transactions_trip ON transactions(trip_id);
            CREATE INDEX IF NOT EXISTS idx_transactions_card_member ON transactions(card_member);
            CREATE INDEX IF NOT EXISTS idx_transactions_payment_method ON transactions(payment_method);

            -- Subscriptions (detected recurring charges)
            CREATE TABLE IF NOT EXISTS subscriptions (
                id INTEGER PRIMARY KEY,
                merchant TEXT NOT NULL,
                account_id INTEGER REFERENCES accounts(id),
                amount REAL,
                frequency TEXT,
                first_seen DATE,
                last_seen DATE,
                status TEXT DEFAULT 'active',
                user_acknowledged BOOLEAN DEFAULT FALSE,
                acknowledged_at DATETIME,
                cancelled_at DATE,
                cancelled_monthly_amount REAL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);
            CREATE INDEX IF NOT EXISTS idx_subscriptions_account ON subscriptions(account_id);

            -- Price History (track subscription price changes)
            CREATE TABLE IF NOT EXISTS price_history (
                id INTEGER PRIMARY KEY,
                subscription_id INTEGER REFERENCES subscriptions(id),
                amount REAL NOT NULL,
                detected_at DATE NOT NULL
            );

            -- Alerts (waste detection findings)
            CREATE TABLE IF NOT EXISTS alerts (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                subscription_id INTEGER REFERENCES subscriptions(id),
                message TEXT,
                dismissed BOOLEAN DEFAULT FALSE,
                ollama_analysis TEXT,  -- JSON for duplicate alert analysis
                spending_anomaly_data TEXT,  -- JSON for spending anomaly alerts (SpendingAnomalyData)
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_alerts_type ON alerts(type);
            CREATE INDEX IF NOT EXISTS idx_alerts_dismissed ON alerts(dismissed);

            -- Audit log (tracks all API access for security)
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                user_email TEXT NOT NULL,
                action TEXT NOT NULL,
                entity_type TEXT,
                entity_id INTEGER,
                details TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_audit_log_user ON audit_log(user_email);
            CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action);

            -- Tags (hierarchical categorization)
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                parent_id INTEGER REFERENCES tags(id),
                color TEXT,
                icon TEXT,
                auto_patterns TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(name, parent_id)
            );

            CREATE INDEX IF NOT EXISTS idx_tags_parent ON tags(parent_id);

            -- Transaction-Tag junction (many-to-many)
            CREATE TABLE IF NOT EXISTS transaction_tags (
                transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                source TEXT NOT NULL DEFAULT 'manual',
                confidence REAL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (transaction_id, tag_id)
            );

            CREATE INDEX IF NOT EXISTS idx_transaction_tags_tag ON transaction_tags(tag_id);

            -- Tag rules (user-defined auto-assignment patterns)
            CREATE TABLE IF NOT EXISTS tag_rules (
                id INTEGER PRIMARY KEY,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                pattern TEXT NOT NULL,
                pattern_type TEXT NOT NULL DEFAULT 'contains',
                priority INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_tag_rules_tag ON tag_rules(tag_id);
            CREATE INDEX IF NOT EXISTS idx_tag_rules_priority ON tag_rules(priority DESC);

            -- Entities (people, pets, vehicles, properties)
            CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                type TEXT NOT NULL,
                icon TEXT,
                color TEXT,
                archived BOOLEAN DEFAULT FALSE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(type);
            CREATE INDEX IF NOT EXISTS idx_entities_archived ON entities(archived);

            -- Transaction splits (line items within a transaction)
            CREATE TABLE IF NOT EXISTS transaction_splits (
                id INTEGER PRIMARY KEY,
                transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
                amount REAL NOT NULL,
                description TEXT,
                split_type TEXT NOT NULL DEFAULT 'item',
                entity_id INTEGER REFERENCES entities(id),
                purchaser_id INTEGER REFERENCES entities(id),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_splits_transaction ON transaction_splits(transaction_id);
            CREATE INDEX IF NOT EXISTS idx_splits_entity ON transaction_splits(entity_id);
            CREATE INDEX IF NOT EXISTS idx_splits_purchaser ON transaction_splits(purchaser_id);

            -- Split tags (tags assigned to individual splits)
            CREATE TABLE IF NOT EXISTS split_tags (
                split_id INTEGER NOT NULL REFERENCES transaction_splits(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                source TEXT NOT NULL DEFAULT 'manual',
                confidence REAL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (split_id, tag_id)
            );

            CREATE INDEX IF NOT EXISTS idx_split_tags_tag ON split_tags(tag_id);

            -- Receipts (for AI parsing and receipt-first workflow)
            CREATE TABLE IF NOT EXISTS receipts (
                id INTEGER PRIMARY KEY,
                transaction_id INTEGER REFERENCES transactions(id) ON DELETE CASCADE,
                image_path TEXT,
                image_data BLOB,                           -- for small receipts stored in DB
                parsed_json TEXT,
                parsed_at DATETIME,
                status TEXT DEFAULT 'matched',             -- matched, pending, manual_review, orphaned
                role TEXT DEFAULT 'primary',               -- primary, supplementary
                receipt_date DATE,                         -- parsed date for matching
                receipt_total REAL,                        -- parsed total for matching
                receipt_merchant TEXT,                     -- parsed merchant name
                content_hash TEXT,                         -- SHA256 for deduplication
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_receipts_transaction ON receipts(transaction_id);
            CREATE INDEX IF NOT EXISTS idx_receipts_status ON receipts(status);
            CREATE INDEX IF NOT EXISTS idx_receipts_hash ON receipts(content_hash);

            -- Merchant aliases (learned name variations)
            CREATE TABLE IF NOT EXISTS merchant_aliases (
                id INTEGER PRIMARY KEY,
                receipt_name TEXT NOT NULL,                -- "TARGET T-1234"
                canonical_name TEXT NOT NULL,              -- "TARGET"
                bank TEXT,                                 -- which bank uses this format
                confidence REAL DEFAULT 1.0,               -- 1.0 = user confirmed, <1.0 = auto-learned
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(receipt_name, bank)
            );

            CREATE INDEX IF NOT EXISTS idx_merchant_aliases_receipt ON merchant_aliases(receipt_name);
            CREATE INDEX IF NOT EXISTS idx_merchant_aliases_canonical ON merchant_aliases(canonical_name);

            -- Trips/Events (group related transactions)
            CREATE TABLE IF NOT EXISTS trips (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                start_date DATE,
                end_date DATE,
                location_id INTEGER REFERENCES locations(id),
                budget REAL,
                archived BOOLEAN DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_trips_dates ON trips(start_date, end_date);

            -- Mileage logs for vehicle entities
            CREATE TABLE IF NOT EXISTS mileage_logs (
                id INTEGER PRIMARY KEY,
                entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
                date DATE NOT NULL,
                odometer REAL NOT NULL,
                note TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_mileage_entity ON mileage_logs(entity_id);
            CREATE INDEX IF NOT EXISTS idx_mileage_date ON mileage_logs(date);

            -- Ollama metrics (tracks each LLM call for observability)
            CREATE TABLE IF NOT EXISTS ollama_metrics (
                id INTEGER PRIMARY KEY,
                operation TEXT NOT NULL,
                model TEXT NOT NULL,
                started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                latency_ms INTEGER NOT NULL,
                success BOOLEAN NOT NULL,
                error_message TEXT,
                confidence REAL,
                transaction_id INTEGER,
                input_hash TEXT,
                input_text TEXT,
                result_text TEXT,
                metadata TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_ollama_metrics_operation ON ollama_metrics(operation);
            CREATE INDEX IF NOT EXISTS idx_ollama_metrics_started_at ON ollama_metrics(started_at);
            CREATE INDEX IF NOT EXISTS idx_ollama_metrics_success ON ollama_metrics(success);

            -- Ollama corrections (tracks when user fixes Ollama's tag choices)
            CREATE TABLE IF NOT EXISTS ollama_corrections (
                id INTEGER PRIMARY KEY,
                transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
                original_tag_id INTEGER NOT NULL REFERENCES tags(id),
                original_confidence REAL,
                corrected_tag_id INTEGER NOT NULL REFERENCES tags(id),
                corrected_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(transaction_id, original_tag_id)
            );

            CREATE INDEX IF NOT EXISTS idx_ollama_corrections_transaction ON ollama_corrections(transaction_id);
            CREATE INDEX IF NOT EXISTS idx_ollama_corrections_corrected_at ON ollama_corrections(corrected_at);

            -- Merchant subscription cache (caches Ollama subscription classification)
            -- Used to avoid repeated API calls for the same merchant pattern
            CREATE TABLE IF NOT EXISTS merchant_subscription_cache (
                merchant_pattern TEXT PRIMARY KEY,           -- normalized merchant name
                is_subscription BOOLEAN NOT NULL,            -- true if subscription service
                confidence REAL,                             -- Ollama confidence or 1.0 for user override
                source TEXT DEFAULT 'ollama',                -- 'ollama' or 'user_override'
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_merchant_sub_cache_source ON merchant_subscription_cache(source);

            -- Merchant name cache (user-corrected merchant names for learning)
            -- When user edits a merchant name, cache it for future transactions with same description
            CREATE TABLE IF NOT EXISTS merchant_name_cache (
                id INTEGER PRIMARY KEY,
                description TEXT NOT NULL UNIQUE,            -- original bank description
                merchant_name TEXT NOT NULL,                 -- user-corrected or learned name
                source TEXT DEFAULT 'user',                  -- 'user', 'ollama', 'bank'
                confidence REAL DEFAULT 1.0,                 -- user corrections = 1.0
                hit_count INTEGER DEFAULT 0,                 -- how many times this mapping was used
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_merchant_name_cache_source ON merchant_name_cache(source);

            -- Merchant-to-tag learning cache (learned from manual tag assignments)
            CREATE TABLE IF NOT EXISTS merchant_tag_cache (
                id INTEGER PRIMARY KEY,
                merchant_pattern TEXT NOT NULL,              -- normalized merchant name or description pattern
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                source TEXT DEFAULT 'user',                  -- 'user' (manual) or 'frequent' (auto-learned)
                confidence REAL DEFAULT 1.0,                 -- user corrections = 1.0
                hit_count INTEGER DEFAULT 0,                 -- how many times this mapping was used
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(merchant_pattern, tag_id)
            );

            CREATE INDEX IF NOT EXISTS idx_merchant_tag_cache_pattern ON merchant_tag_cache(merchant_pattern);
            CREATE INDEX IF NOT EXISTS idx_merchant_tag_cache_tag ON merchant_tag_cache(tag_id);

            -- Import sessions (tracks each import operation for history/auditing)
            CREATE TABLE IF NOT EXISTS import_sessions (
                id INTEGER PRIMARY KEY,
                account_id INTEGER NOT NULL REFERENCES accounts(id),
                filename TEXT,                              -- original filename if available
                file_size_bytes INTEGER,                    -- size of CSV file
                bank TEXT NOT NULL,                         -- bank format used
                imported_count INTEGER NOT NULL DEFAULT 0,
                skipped_count INTEGER NOT NULL DEFAULT 0,
                -- Tagging breakdown
                tagged_by_learned INTEGER DEFAULT 0,
                tagged_by_rule INTEGER DEFAULT 0,
                tagged_by_pattern INTEGER DEFAULT 0,
                tagged_by_ollama INTEGER DEFAULT 0,
                tagged_by_bank_category INTEGER DEFAULT 0,
                tagged_fallback INTEGER DEFAULT 0,
                -- Detection results
                subscriptions_found INTEGER DEFAULT 0,
                zombies_detected INTEGER DEFAULT 0,
                price_increases_detected INTEGER DEFAULT 0,
                duplicates_detected INTEGER DEFAULT 0,
                receipts_matched INTEGER DEFAULT 0,
                spending_anomalies_detected INTEGER DEFAULT 0,
                tip_discrepancies_detected INTEGER DEFAULT 0,
                -- Metadata
                user_email TEXT,
                ollama_model TEXT,                          -- Ollama model used for tagging/normalization (if any)
                -- Processing status for async imports
                status TEXT DEFAULT 'pending',              -- pending, processing, completed, failed
                processing_phase TEXT,                      -- tagging, normalizing, detecting, matching_receipts
                processing_current INTEGER DEFAULT 0,       -- current item being processed
                processing_total INTEGER DEFAULT 0,         -- total items to process in current phase
                processing_error TEXT,                      -- error message if failed
                -- Phase timing (milliseconds)
                tagging_duration_ms INTEGER,
                normalizing_duration_ms INTEGER,
                matching_duration_ms INTEGER,
                detecting_duration_ms INTEGER,
                total_duration_ms INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_import_sessions_account ON import_sessions(account_id);
            CREATE INDEX IF NOT EXISTS idx_import_sessions_created ON import_sessions(created_at);

            -- Import skipped transactions (duplicates that were skipped during import)
            CREATE TABLE IF NOT EXISTS import_skipped_transactions (
                id INTEGER PRIMARY KEY,
                import_session_id INTEGER NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
                date DATE NOT NULL,
                description TEXT NOT NULL,
                amount REAL NOT NULL,
                import_hash TEXT NOT NULL,
                existing_transaction_id INTEGER REFERENCES transactions(id),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_import_skipped_session ON import_skipped_transactions(import_session_id);

            -- User feedback for tracking explicit and implicit signals
            CREATE TABLE IF NOT EXISTS user_feedback (
                id INTEGER PRIMARY KEY,
                feedback_type TEXT NOT NULL,        -- helpful, not_helpful, correction, dismissal
                target_type TEXT NOT NULL,          -- alert, insight, classification, explanation
                target_id INTEGER,                  -- ID of the target item (alert_id, transaction_id, etc.)
                original_value TEXT,                -- what was shown (JSON for complex values)
                corrected_value TEXT,               -- what user changed it to (if correction)
                reason TEXT,                        -- optional user-provided reason
                context TEXT,                       -- additional context as JSON (model used, prompt version, etc.)
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                reverted_at DATETIME                -- NULL = active, timestamp = undone
            );

            CREATE INDEX IF NOT EXISTS idx_feedback_target ON user_feedback(target_type, target_id);
            CREATE INDEX IF NOT EXISTS idx_feedback_type ON user_feedback(feedback_type);
            CREATE INDEX IF NOT EXISTS idx_feedback_active ON user_feedback(reverted_at);
            CREATE INDEX IF NOT EXISTS idx_feedback_created ON user_feedback(created_at);

            -- Training experiments for model fine-tuning versioning
            CREATE TABLE IF NOT EXISTS training_experiments (
                id INTEGER PRIMARY KEY,
                branch TEXT NOT NULL,                     -- branch name (e.g., "main", "experiment-v2")
                task TEXT NOT NULL,                       -- classify_merchant, normalize_merchant, etc.
                base_model TEXT NOT NULL,                 -- base model used (e.g., "gemma3")
                model_name TEXT NOT NULL,                 -- resulting model name for Ollama
                status TEXT NOT NULL DEFAULT 'pending',   -- pending, training, completed, failed, promoted, archived
                parent_id INTEGER REFERENCES training_experiments(id),  -- for branching
                training_examples INTEGER NOT NULL,       -- number of training examples
                training_data_path TEXT,                  -- path to JSONL training data
                adapter_path TEXT,                        -- path to LoRA adapter
                metrics TEXT,                             -- JSON evaluation metrics
                notes TEXT,                               -- user notes/description
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                started_at DATETIME,                      -- when training started
                completed_at DATETIME                     -- when training completed
            );

            CREATE INDEX IF NOT EXISTS idx_training_exp_task ON training_experiments(task);
            CREATE INDEX IF NOT EXISTS idx_training_exp_branch ON training_experiments(branch);
            CREATE INDEX IF NOT EXISTS idx_training_exp_status ON training_experiments(status);
            CREATE INDEX IF NOT EXISTS idx_training_exp_parent ON training_experiments(parent_id);

            -- Reprocess runs for tracking each reprocess operation
            CREATE TABLE IF NOT EXISTS reprocess_runs (
                id INTEGER PRIMARY KEY,
                import_session_id INTEGER NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
                run_number INTEGER NOT NULL,                  -- 1, 2, 3... within this session
                ollama_model TEXT,                            -- model used for this reprocess
                status TEXT NOT NULL DEFAULT 'running',       -- running, completed, failed
                initiated_by TEXT,                            -- user email who triggered it
                reason TEXT,                                  -- optional reason/notes for this run
                started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                completed_at DATETIME,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(import_session_id, run_number)
            );
            CREATE INDEX IF NOT EXISTS idx_reprocess_runs_session ON reprocess_runs(import_session_id);
            CREATE INDEX IF NOT EXISTS idx_reprocess_runs_status ON reprocess_runs(status);

            -- Reprocess snapshots for before/after comparison (now linked to runs)
            CREATE TABLE IF NOT EXISTS reprocess_snapshots (
                id INTEGER PRIMARY KEY,
                import_session_id INTEGER NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE,
                reprocess_run_id INTEGER REFERENCES reprocess_runs(id) ON DELETE CASCADE,
                snapshot_type TEXT NOT NULL,                  -- 'before' or 'after'
                tagging_breakdown TEXT NOT NULL,              -- JSON: tagged_by_* counts
                detection_results TEXT NOT NULL,              -- JSON: subscriptions, zombies, etc.
                sample_transactions TEXT,                     -- JSON: sample of transactions with tags
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_reprocess_snapshots_session ON reprocess_snapshots(import_session_id);
            CREATE INDEX IF NOT EXISTS idx_reprocess_snapshots_run ON reprocess_snapshots(reprocess_run_id);

            -- Insight findings (proactive financial insights)
            CREATE TABLE IF NOT EXISTS insight_findings (
                id INTEGER PRIMARY KEY,
                insight_type TEXT NOT NULL,              -- spending_explainer, expense_forecaster, savings_opportunity
                finding_key TEXT NOT NULL,               -- unique key for deduplication (e.g., "savings:zombie:123")
                severity TEXT NOT NULL,                  -- info, attention, warning, alert
                title TEXT NOT NULL,
                summary TEXT NOT NULL,
                detail TEXT,
                data TEXT NOT NULL,                      -- JSON: insight-specific structured data
                first_detected_at DATETIME NOT NULL,
                last_detected_at DATETIME NOT NULL,
                status TEXT DEFAULT 'active',            -- active, dismissed, snoozed
                snoozed_until DATETIME,
                user_feedback TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(insight_type, finding_key)
            );

            CREATE INDEX IF NOT EXISTS idx_insights_status ON insight_findings(status, last_detected_at);
            CREATE INDEX IF NOT EXISTS idx_insights_type ON insight_findings(insight_type);
            CREATE INDEX IF NOT EXISTS idx_insights_severity ON insight_findings(severity);
            "#,
        )?;

        info!("Database schema initialized");
        Ok(())
    }
}

/// Audit log entry
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub timestamp: String,
    pub user_email: String,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub details: Option<String>,
}

#[cfg(test)]
mod tests;

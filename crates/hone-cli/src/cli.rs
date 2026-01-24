//! CLI argument definitions using clap
//!
//! This module contains all the clap structs and enums for parsing CLI arguments.
//! The actual command implementations are in the `commands` module.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Hone - Find and eliminate wasteful spending
#[derive(Parser)]
#[command(name = "hone")]
#[command(about = "Self-hosted personal finance waste detector", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Database path
    #[arg(long, default_value = "hone.db", global = true)]
    pub db: PathBuf,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable database encryption (not recommended for production)
    ///
    /// By default, the database is encrypted using SQLCipher.
    /// Set HONE_DB_KEY environment variable with your passphrase.
    /// Use --no-encrypt only for development or testing.
    #[arg(long, global = true)]
    pub no_encrypt: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the database
    Init,

    /// Import transactions from CSV
    Import {
        /// CSV file to import
        #[arg(short, long)]
        file: PathBuf,

        /// Bank format (auto-detected if not specified)
        #[arg(short, long)]
        bank: Option<String>,

        /// Account name (defaults to bank name)
        #[arg(short, long)]
        account: Option<String>,

        /// Skip auto-tagging of imported transactions
        #[arg(long)]
        no_tag: bool,

        /// Skip waste detection after import
        #[arg(long)]
        no_detect: bool,
    },

    /// Run waste detection
    Detect {
        /// Detection type: zombies, increases, duplicates, all
        #[arg(short, long, default_value = "all")]
        kind: String,
    },

    /// Start the web server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Disable authentication (for local development only)
        ///
        /// WARNING: Do not use this flag when exposing the server to a network.
        /// By default, the server requires Cloudflare Access authentication headers.
        #[arg(long)]
        no_auth: bool,

        /// Directory containing static files to serve (e.g., ui/dist)
        #[arg(long)]
        static_dir: Option<PathBuf>,

        /// Port for MCP (Model Context Protocol) server
        ///
        /// When set, starts an MCP server for LLM tool access on the specified port.
        /// This enables conversational queries via Claude Desktop or other MCP clients.
        /// Example: --mcp-port 3001
        #[arg(long)]
        mcp_port: Option<u16>,
    },

    /// Show dashboard summary
    Dashboard,

    /// Show database status (encryption, size, etc.)
    Status,

    /// List accounts
    Accounts,

    /// Manage transactions (list, archive, unarchive)
    Transactions {
        #[command(subcommand)]
        action: Option<TransactionsAction>,
    },

    /// Manage subscriptions
    Subscriptions {
        #[command(subcommand)]
        action: Option<SubscriptionsAction>,
    },

    /// List active alerts
    Alerts {
        /// Include dismissed alerts
        #[arg(long)]
        all: bool,
    },

    /// Manage tags (list, add, rename, move, delete, merge)
    Tags {
        #[command(subcommand)]
        action: Option<TagsAction>,
    },

    /// Manage tag rules (list, add, delete, test)
    Rules {
        #[command(subcommand)]
        action: Option<RulesAction>,
    },

    /// Add a tag to a transaction
    Tag {
        /// Transaction ID
        transaction_id: i64,
        /// Tag name or path (e.g., "Groceries" or "Food.Groceries")
        tag: String,
    },

    /// Remove a tag from a transaction
    Untag {
        /// Transaction ID
        transaction_id: i64,
        /// Tag name or path
        tag: String,
    },

    /// Generate spending reports
    Report {
        #[command(subcommand)]
        report_type: ReportType,
    },

    /// Manage entities (people, pets, vehicles, properties)
    Entities {
        #[command(subcommand)]
        action: Option<EntitiesAction>,
    },

    /// Manage database backups (create, list, restore, prune)
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Test Ollama connection and AI features
    Ollama {
        #[command(subcommand)]
        action: OllamaAction,
    },

    /// Manage receipts (upload, list, match)
    Receipts {
        #[command(subcommand)]
        action: Option<ReceiptsAction>,
    },

    /// Reset the database (clear data)
    Reset {
        /// Soft reset: clear transactions/alerts but keep accounts, tags, rules, entities
        /// Without this flag, performs a hard reset (deletes DB file and re-initializes)
        #[arg(long)]
        soft: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Export data (transactions or full backup)
    Export {
        #[command(subcommand)]
        export_type: ExportType,
    },

    /// Import a full JSON backup
    ImportFull {
        /// JSON backup file to import
        #[arg(short, long)]
        file: PathBuf,

        /// Clear all existing data before import
        #[arg(long)]
        clear: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Manage AI prompts (list available prompts, view override status)
    Prompts {
        #[command(subcommand)]
        action: Option<PromptsAction>,
    },

    /// Re-process transactions (clear AI-generated data and re-run tagging/detection)
    Rebuild {
        /// Only rebuild a specific import session
        #[arg(long)]
        session: Option<i64>,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Model training data and fine-tuning utilities
    Training {
        #[command(subcommand)]
        action: TrainingAction,
    },
}

#[derive(Subcommand)]
pub enum ExportType {
    /// Export transactions to CSV
    Transactions {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// Filter by tag IDs (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Include child tags when filtering
        #[arg(long)]
        include_children: bool,
    },

    /// Export full database backup to JSON
    Full {
        /// Output file (required)
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum ReportType {
    /// Spending by category (hierarchical tag view)
    Spending {
        /// Time period: this-month, last-month, this-year, last-30-days, last-90-days, all
        #[arg(long, default_value = "this-month")]
        period: String,

        /// Custom start date (YYYY-MM-DD) - overrides period
        #[arg(long)]
        from: Option<String>,

        /// Custom end date (YYYY-MM-DD) - overrides period
        #[arg(long)]
        to: Option<String>,

        /// Filter to a specific tag
        #[arg(long)]
        tag: Option<String>,

        /// Show child categories
        #[arg(long)]
        expand: bool,
    },

    /// Spending trends over time
    Trends {
        /// Granularity: monthly or weekly
        #[arg(long, default_value = "monthly")]
        granularity: String,

        /// Time period
        #[arg(long, default_value = "last-12-months")]
        period: String,

        /// Filter to a specific tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Top merchants by spending
    Merchants {
        /// Number of merchants to show
        #[arg(long, default_value = "10")]
        limit: i64,

        /// Time period
        #[arg(long, default_value = "this-month")]
        period: String,

        /// Filter to a specific tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Subscription summary
    Subscriptions,

    /// Savings from cancelled subscriptions
    Savings,

    /// Spending by tag (legacy format)
    ByTag {
        /// Maximum depth for tag hierarchy (0 = root only)
        #[arg(long)]
        depth: Option<i32>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TransactionsAction {
    /// List recent transactions
    List {
        /// Number of transactions to show
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },

    /// List archived transactions
    Archived {
        /// Number of transactions to show
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },

    /// Archive a transaction (hide from reports and lists)
    Archive {
        /// Transaction ID to archive
        id: i64,
    },

    /// Unarchive a transaction (restore to reports and lists)
    Unarchive {
        /// Transaction ID to unarchive
        id: i64,
    },
}

#[derive(Subcommand)]
pub enum SubscriptionsAction {
    /// Cancel a subscription (marks it as cancelled for savings tracking)
    Cancel {
        /// Subscription name or ID
        name_or_id: String,
        /// Custom cancellation date (YYYY-MM-DD), defaults to today
        #[arg(long)]
        date: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TagsAction {
    /// Add a new tag (use dot notation for hierarchy: Parent.Child)
    Add {
        /// Tag path (e.g., "Transport.Gas" or just "NewTag" for root)
        path: String,
        /// Optional color (e.g., "#10b981")
        #[arg(long)]
        color: Option<String>,
        /// Optional auto-match patterns (pipe-separated, e.g., "SHELL|CHEVRON")
        #[arg(long)]
        patterns: Option<String>,
    },

    /// Rename a tag
    Rename {
        /// Current tag name or path
        old_name: String,
        /// New name (just the name, not full path)
        new_name: String,
    },

    /// Move a tag to a new parent
    Move {
        /// Tag to move
        tag: String,
        /// New parent tag (use "root" for no parent)
        #[arg(long)]
        to: String,
    },

    /// Delete a tag
    Delete {
        /// Tag to delete
        tag: String,
        /// Force deletion even if tag has transactions
        #[arg(long)]
        force: bool,
        /// Move transactions to parent tag instead of orphaning
        #[arg(long)]
        to_parent: bool,
    },

    /// Merge one tag into another
    Merge {
        /// Source tag to merge (will be deleted)
        source: String,
        /// Target tag to merge into
        #[arg(long)]
        into: String,
    },
}

#[derive(Subcommand)]
pub enum RulesAction {
    /// Add a new rule
    Add {
        /// Tag to assign when rule matches
        tag: String,
        /// Pattern to match against transaction descriptions
        pattern: String,
        /// Pattern type: contains, regex, exact (default: contains)
        #[arg(long, default_value = "contains")]
        pattern_type: String,
        /// Rule priority (higher = checked first)
        #[arg(long, default_value = "0")]
        priority: i32,
    },

    /// Delete a rule
    Delete {
        /// Rule ID to delete
        id: i64,
    },

    /// Test which rules match a description
    Test {
        /// Description to test
        description: String,
    },
}

#[derive(Subcommand)]
pub enum EntitiesAction {
    /// Add a new entity
    Add {
        /// Entity name
        name: String,
        /// Entity type: person, pet, vehicle, property
        #[arg(long, short = 't')]
        entity_type: String,
        /// Emoji or icon name
        #[arg(long)]
        icon: Option<String>,
        /// Hex color for UI
        #[arg(long)]
        color: Option<String>,
    },

    /// List entities of a specific type
    List {
        /// Filter by type: person, pet, vehicle, property
        #[arg(long, short = 't')]
        entity_type: Option<String>,
        /// Include archived entities
        #[arg(long)]
        all: bool,
    },

    /// Update an entity
    Update {
        /// Entity ID
        id: i64,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New icon
        #[arg(long)]
        icon: Option<String>,
        /// New color
        #[arg(long)]
        color: Option<String>,
    },

    /// Archive an entity (preserves history)
    Archive {
        /// Entity ID
        id: i64,
    },

    /// Unarchive an entity
    Unarchive {
        /// Entity ID
        id: i64,
    },

    /// Delete an entity
    Delete {
        /// Entity ID
        id: i64,
        /// Force deletion even if entity has associated splits
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum BackupAction {
    /// Create a new backup
    Create {
        /// Backup name (defaults to timestamped name)
        #[arg(short, long)]
        name: Option<String>,

        /// Backup directory (defaults to ~/.local/share/hone/backups)
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// List available backups
    List {
        /// Backup directory (defaults to ~/.local/share/hone/backups)
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// Restore from a backup
    Restore {
        /// Backup name to restore from
        name: String,

        /// Backup directory (defaults to ~/.local/share/hone/backups)
        #[arg(long)]
        dir: Option<PathBuf>,

        /// Overwrite existing database without prompting
        #[arg(long)]
        force: bool,
    },

    /// Delete old backups according to retention policy
    Prune {
        /// Number of backups to keep (default: 7)
        #[arg(long, default_value = "7")]
        keep: usize,

        /// Backup directory (defaults to ~/.local/share/hone/backups)
        #[arg(long)]
        dir: Option<PathBuf>,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum OllamaAction {
    /// Test Ollama connection and run sample classifications
    Test {
        /// Test classification with a specific merchant name
        #[arg(long)]
        merchant: Option<String>,

        /// Test receipt parsing with an image file
        #[arg(long)]
        receipt: Option<PathBuf>,

        /// Vision model to use for receipt parsing (default: llama3.2-vision:11b)
        #[arg(long)]
        vision_model: Option<String>,
    },

    /// Normalize merchant names for existing transactions
    Normalize {
        /// Maximum number of transactions to process
        #[arg(long, default_value = "1000")]
        limit: i64,
    },
}

#[derive(Subcommand)]
pub enum PromptsAction {
    /// List all available prompts and their override status
    List,

    /// Show the content of a specific prompt
    Show {
        /// Prompt ID (e.g., classify_merchant, normalize_merchant)
        prompt_id: String,
    },

    /// Show the path where prompt overrides should be placed
    Path,
}

#[derive(Subcommand)]
pub enum ReceiptsAction {
    /// Upload a receipt image (creates pending receipt for later matching)
    Add {
        /// Path to receipt image file
        #[arg(short, long)]
        file: PathBuf,

        /// Optional account hint (which account this receipt is for)
        #[arg(long)]
        account: Option<String>,
    },

    /// List receipts by status
    List {
        /// Filter by status: pending, matched, manual_review, orphaned (default: pending)
        #[arg(long, default_value = "pending")]
        status: String,
    },

    /// Link a receipt to a transaction
    Match {
        /// Receipt ID
        receipt_id: i64,
        /// Transaction ID to link to
        transaction_id: i64,
    },

    /// Update receipt status
    Status {
        /// Receipt ID
        receipt_id: i64,
        /// New status: pending, manual_review, orphaned
        status: String,
    },

    /// Dismiss (delete) a receipt
    Dismiss {
        /// Receipt ID
        receipt_id: i64,
        /// Optional reason for dismissal
        #[arg(long)]
        reason: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TrainingAction {
    /// List available training tasks and data counts
    List,

    /// Export training data in JSONL format for fine-tuning
    Export {
        /// Task to export: classify_merchant, normalize_merchant, classify_subscription
        #[arg(long)]
        task: String,

        /// Output file (JSONL format, defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Minimum confidence threshold (0.0-1.0, default: 0.5)
        #[arg(long, default_value = "0.5")]
        min_confidence: f64,
    },

    /// Show training data statistics
    Stats,

    /// Create a new training experiment
    Create {
        /// Task: classify_merchant, normalize_merchant, classify_subscription
        #[arg(long)]
        task: String,

        /// Branch name (e.g., "main", "experiment-v2")
        #[arg(long, default_value = "main")]
        branch: String,

        /// Base model for fine-tuning (default: gemma3)
        #[arg(long)]
        base_model: Option<String>,

        /// Notes or description
        #[arg(long)]
        notes: Option<String>,
    },

    /// Prepare training data for an experiment
    Prepare {
        /// Experiment ID
        #[arg(long)]
        id: i64,
    },

    /// Run fine-tuning for an experiment
    Train {
        /// Experiment ID
        #[arg(long)]
        id: i64,

        /// Skip MLX training (just show instructions)
        #[arg(long)]
        skip_mlx: bool,
    },

    /// Create Ollama model from trained adapter
    CreateModel {
        /// Experiment ID
        #[arg(long)]
        id: i64,
    },

    /// Promote an experiment to production
    Promote {
        /// Experiment ID
        #[arg(long)]
        id: i64,
    },

    /// List training experiments
    Experiments {
        /// Filter by task
        #[arg(long)]
        task: Option<String>,

        /// Filter by branch
        #[arg(long)]
        branch: Option<String>,
    },

    /// Branch from an existing experiment
    Branch {
        /// Source experiment ID
        #[arg(long)]
        id: i64,

        /// New branch name
        #[arg(long)]
        name: String,
    },

    /// Run training agent (automated recommendations)
    Agent {
        /// Only check status, don't suggest actions
        #[arg(long)]
        check_only: bool,
    },
}

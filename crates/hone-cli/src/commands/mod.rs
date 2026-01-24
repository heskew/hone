//! CLI command implementations
//!
//! Commands are organized by domain:
//! - `backup` - Backup management commands (create, list, restore, prune)
//! - `core` - Core commands (init, detect) and shared utilities (open_db)
//! - `entities` - Entity management commands (people, pets, vehicles, properties)
//! - `import` - Import/export commands (CSV import, transaction export, full backup)
//! - `ollama` - Ollama AI commands (test, normalize)
//! - `prompts` - Prompt library management commands
//! - `rebuild` - Re-process transactions with current models/rules
//! - `receipts` - Receipt workflow commands
//! - `reports` - Report generation commands
//! - `serve` - Web server command
//! - `status` - Status/dashboard/accounts/alerts/reset commands
//! - `subscriptions` - Subscription management commands
//! - `tags` - Tag management commands
//! - `transactions` - Transaction commands (list, archive, unarchive)

pub mod backup;
pub mod core;
pub mod entities;
pub mod import;
pub mod ollama;
pub mod prompts;
pub mod rebuild;
pub mod receipts;
pub mod reports;
pub mod serve;
pub mod status;
pub mod subscriptions;
pub mod tags;
pub mod training;
pub mod transactions;

// Re-export command functions for main.rs
pub use backup::*;
pub use core::*;
pub use entities::*;
pub use import::*;
pub use ollama::*;
pub use prompts::*;
pub use rebuild::*;
pub use receipts::*;
pub use reports::*;
pub use serve::*;
pub use status::*;
pub use subscriptions::*;
pub use tags::*;
pub use training::*;
pub use transactions::*;

/// Truncate a string to a maximum length, adding "..." if truncated
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

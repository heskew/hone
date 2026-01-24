//! Backup system with pluggable destinations
//!
//! Supports staged backups:
//! 1. Create encrypted, compressed local backup using SQLCipher export
//! 2. Optionally upload to remote destinations (R2, S3, etc.)
//!
//! # Architecture
//!
//! - `BackupDestination` trait defines the interface for storage backends
//! - `LocalDestination` stores backups in a local directory
//! - `R2Destination` uploads to Cloudflare R2 (future)
//!
//! # Backup Format
//!
//! Backups are created using SQLCipher's `sqlcipher_export()` function,
//! which creates a consistent, encrypted copy of the database while it's
//! in use. The backup is then gzip compressed.
//!
//! File naming: `hone-YYYY-MM-DD-HHMMSS.db.gz`

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;

mod local;
mod r2;

pub use local::LocalDestination;
pub use r2::{R2Config, R2Destination};

/// Information about a backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Backup filename
    pub name: String,
    /// Full path or remote key
    pub path: String,
    /// Size in bytes
    pub size: u64,
    /// When the backup was created
    pub created_at: DateTime<Utc>,
    /// Whether the backup is compressed
    pub compressed: bool,
    /// Whether the backup is encrypted
    pub encrypted: bool,
}

/// Result of a backup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    /// Information about the created backup
    pub info: BackupInfo,
    /// Number of accounts in backup
    pub accounts: i64,
    /// Number of transactions in backup
    pub transactions: i64,
    /// Number of subscriptions in backup
    pub subscriptions: i64,
}

/// Result of a prune operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneResult {
    /// Number of backups deleted
    pub deleted_count: usize,
    /// Names of deleted backups
    pub deleted_names: Vec<String>,
    /// Number of backups retained
    pub retained_count: usize,
    /// Total bytes freed
    pub bytes_freed: u64,
}

/// Backup retention policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Number of daily backups to keep
    pub daily: usize,
    /// Number of weekly backups to keep (optional)
    pub weekly: Option<usize>,
    /// Number of monthly backups to keep (optional)
    pub monthly: Option<usize>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            daily: 7,
            weekly: None,
            monthly: None,
        }
    }
}

impl RetentionPolicy {
    /// Create a simple policy keeping last N backups
    pub fn keep_last(n: usize) -> Self {
        Self {
            daily: n,
            weekly: None,
            monthly: None,
        }
    }
}

/// Trait for backup storage destinations
///
/// Implementations handle storing backups in different locations:
/// - Local filesystem
/// - Cloudflare R2
/// - AWS S3
/// - etc.
pub trait BackupDestination: Send + Sync {
    /// Human-readable name for this destination
    fn name(&self) -> &str;

    /// Store a backup file
    ///
    /// Takes a local file path and stores it in the destination.
    /// Returns the remote name/key for the backup.
    fn store(&self, local_path: &Path, backup_name: &str) -> Result<String>;

    /// Retrieve a backup file
    ///
    /// Downloads/copies a backup to the specified local path.
    fn retrieve(&self, backup_name: &str, local_path: &Path) -> Result<()>;

    /// List all backups in this destination
    fn list(&self) -> Result<Vec<BackupInfo>>;

    /// Delete a backup
    fn delete(&self, backup_name: &str) -> Result<()>;

    /// Apply retention policy and delete old backups
    fn prune(&self, policy: &RetentionPolicy) -> Result<PruneResult> {
        let mut backups = self.list()?;

        // Sort by creation time, newest first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let mut deleted_names = Vec::new();
        let mut bytes_freed = 0u64;

        // Simple retention: keep the most recent N backups
        // TODO: Implement weekly/monthly tiers
        let keep_count = policy.daily;

        for backup in backups.iter().skip(keep_count) {
            if let Err(e) = self.delete(&backup.name) {
                tracing::warn!("Failed to delete backup {}: {}", backup.name, e);
                continue;
            }
            bytes_freed += backup.size;
            deleted_names.push(backup.name.clone());
        }

        let retained_count = backups.len().saturating_sub(deleted_names.len());

        Ok(PruneResult {
            deleted_count: deleted_names.len(),
            deleted_names,
            retained_count,
            bytes_freed,
        })
    }
}

/// Generate a backup filename with timestamp
pub fn generate_backup_name() -> String {
    let now = Utc::now();
    format!("hone-{}.db.gz", now.format("%Y-%m-%d-%H%M%S"))
}

/// Parse backup creation time from filename
pub fn parse_backup_time(name: &str) -> Option<DateTime<Utc>> {
    // Expected format: hone-YYYY-MM-DD-HHMMSS.db.gz
    let name = name.strip_prefix("hone-")?;
    let name = name
        .strip_suffix(".db.gz")
        .or_else(|| name.strip_suffix(".db"))?;

    chrono::NaiveDateTime::parse_from_str(name, "%Y-%m-%d-%H%M%S")
        .ok()
        .map(|dt| dt.and_utc())
}

/// Default backup directory
pub fn default_backup_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hone")
        .join("backups")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_backup_name() {
        let name = generate_backup_name();
        assert!(name.starts_with("hone-"));
        assert!(name.ends_with(".db.gz"));
    }

    #[test]
    fn test_parse_backup_time() {
        let name = "hone-2024-01-15-143022.db.gz";
        let time = parse_backup_time(name).unwrap();
        assert_eq!(
            time.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-01-15 14:30:22"
        );
    }

    #[test]
    fn test_parse_backup_time_invalid() {
        assert!(parse_backup_time("invalid.db").is_none());
        assert!(parse_backup_time("hone-baddate.db.gz").is_none());
    }

    #[test]
    fn test_default_retention_policy() {
        let policy = RetentionPolicy::default();
        assert_eq!(policy.daily, 7);
        assert!(policy.weekly.is_none());
        assert!(policy.monthly.is_none());
    }
}

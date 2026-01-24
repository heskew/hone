//! Background task scheduler for automatic backups
//!
//! Provides optional scheduled backup functionality that can be enabled
//! via environment variables:
//!
//! - `HONE_BACKUP_SCHEDULE`: Interval in hours (e.g., "24" for daily, "168" for weekly)
//! - `HONE_BACKUP_RETENTION`: Number of backups to keep (default: 7)
//!
//! The scheduler runs in the background and automatically creates backups
//! and prunes old ones according to the retention policy.

use std::time::Duration;

use tokio::time::interval;
use tracing::{error, info, warn};

use hone_core::backup::{default_backup_dir, LocalDestination, RetentionPolicy};
use hone_core::Database;

/// Configuration for scheduled backups
#[derive(Debug, Clone)]
pub struct BackupScheduleConfig {
    /// Interval between backups in hours
    pub interval_hours: u64,
    /// Number of backups to retain
    pub retention_count: usize,
    /// Optional custom backup directory
    pub backup_dir: Option<std::path::PathBuf>,
}

impl BackupScheduleConfig {
    /// Parse configuration from environment variables
    ///
    /// Returns None if scheduling is not configured (HONE_BACKUP_SCHEDULE not set)
    pub fn from_env() -> Option<Self> {
        let interval_hours: u64 = std::env::var("HONE_BACKUP_SCHEDULE")
            .ok()
            .and_then(|s| s.parse().ok())?;

        if interval_hours == 0 {
            warn!("HONE_BACKUP_SCHEDULE is 0, automatic backups disabled");
            return None;
        }

        let retention_count = std::env::var("HONE_BACKUP_RETENTION")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7);

        let backup_dir = std::env::var("HONE_BACKUP_DIR")
            .ok()
            .map(std::path::PathBuf::from);

        Some(Self {
            interval_hours,
            retention_count,
            backup_dir,
        })
    }
}

/// Start the backup scheduler as a background task
///
/// This function spawns a tokio task that runs indefinitely, creating
/// backups at the configured interval.
pub fn start_backup_scheduler(db: Database, config: BackupScheduleConfig) {
    info!(
        "Starting backup scheduler: every {} hours, keeping {} backups",
        config.interval_hours, config.retention_count
    );

    tokio::spawn(async move {
        let backup_dir = config.backup_dir.unwrap_or_else(default_backup_dir);
        let mut ticker = interval(Duration::from_secs(config.interval_hours * 3600));

        // Skip the first immediate tick - we don't want to backup on startup
        ticker.tick().await;

        loop {
            ticker.tick().await;

            info!("Running scheduled backup...");

            match run_scheduled_backup(&db, &backup_dir, config.retention_count) {
                Ok(backup_name) => {
                    info!("Scheduled backup completed: {}", backup_name);
                }
                Err(e) => {
                    error!("Scheduled backup failed: {}", e);
                }
            }
        }
    });
}

/// Run a single scheduled backup
fn run_scheduled_backup(
    db: &Database,
    backup_dir: &std::path::Path,
    retention_count: usize,
) -> Result<String, String> {
    // Initialize destination
    let destination = LocalDestination::new(backup_dir)
        .map_err(|e| format!("Failed to initialize backup directory: {}", e))?;

    // Create backup
    let result = db
        .create_backup(&destination, None)
        .map_err(|e| format!("Failed to create backup: {}", e))?;

    let backup_name = result.info.name.clone();

    info!(
        "Backup created: {} ({} bytes, {} accounts, {} transactions)",
        result.info.name, result.info.size, result.accounts, result.transactions
    );

    // Log to audit (as "scheduler" user)
    if let Err(e) = db.log_audit(
        "scheduler",
        "backup_scheduled",
        Some("backup"),
        None,
        Some(&format!("name={}", result.info.name)),
    ) {
        warn!("Failed to log scheduled backup to audit: {}", e);
    }

    // Prune old backups
    let policy = RetentionPolicy::keep_last(retention_count);
    match Database::prune_backups(&destination, &policy) {
        Ok(prune_result) => {
            if prune_result.deleted_count > 0 {
                info!(
                    "Pruned {} old backup(s), freed {} bytes",
                    prune_result.deleted_count, prune_result.bytes_freed
                );
            }
        }
        Err(e) => {
            warn!("Failed to prune old backups: {}", e);
        }
    }

    Ok(backup_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_not_set() {
        // When HONE_BACKUP_SCHEDULE is not set, should return None
        std::env::remove_var("HONE_BACKUP_SCHEDULE");
        assert!(BackupScheduleConfig::from_env().is_none());
    }

    #[test]
    fn test_config_from_env_zero() {
        // When HONE_BACKUP_SCHEDULE is 0, should return None
        std::env::set_var("HONE_BACKUP_SCHEDULE", "0");
        assert!(BackupScheduleConfig::from_env().is_none());
        std::env::remove_var("HONE_BACKUP_SCHEDULE");
    }
}

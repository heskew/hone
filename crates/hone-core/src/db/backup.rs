//! Database backup operations using SQLCipher export
//!
//! Creates encrypted, consistent backups using SQLCipher's `sqlcipher_export()`
//! function, which works safely while the database is in use.

use std::path::Path;

use tempfile::NamedTempFile;
use tracing::info;

use super::Database;
use crate::backup::{generate_backup_name, BackupDestination, BackupResult, RetentionPolicy};
use crate::error::{Error, Result};

impl Database {
    /// Create a backup of the database
    ///
    /// Uses SQLCipher's `sqlcipher_export()` to create a consistent copy while
    /// the database is in use. The backup is encrypted with the same key and
    /// then compressed with gzip.
    ///
    /// # Arguments
    /// * `destination` - Where to store the backup (local, R2, etc.)
    /// * `backup_name` - Optional name override (defaults to timestamped name)
    ///
    /// # Returns
    /// Information about the created backup including stats
    pub fn create_backup(
        &self,
        destination: &dyn BackupDestination,
        backup_name: Option<&str>,
    ) -> Result<BackupResult> {
        let conn = self.conn()?;

        // Get stats before backup for reporting
        let stats = self.get_dashboard_stats()?;

        // Generate backup name if not provided
        let name = backup_name
            .map(String::from)
            .unwrap_or_else(generate_backup_name);

        // Create a temp file for the raw backup
        let temp_backup = NamedTempFile::new()
            .map_err(|e| Error::Backup(format!("Failed to create temp file: {}", e)))?;
        let temp_path = temp_backup.path();

        // Use sqlcipher_export to create a consistent backup
        // This exports to an attached database with the same encryption key
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS backup KEY '';",
            temp_path.display()
        );

        // Get the current encryption key from environment to use for backup
        let key_env = std::env::var(super::DB_KEY_ENV).ok();

        if let Some(ref passphrase) = key_env {
            // Encrypted database - export with same key
            let key = super::derive_key(passphrase)?;
            let attach_sql = format!(
                "ATTACH DATABASE '{}' AS backup KEY 'x\"{}\"';",
                temp_path.display(),
                key
            );

            conn.execute_batch(&attach_sql)
                .map_err(|e| Error::Backup(format!("Failed to attach backup database: {}", e)))?;
        } else {
            // Unencrypted database
            conn.execute_batch(&attach_sql)
                .map_err(|e| Error::Backup(format!("Failed to attach backup database: {}", e)))?;
        }

        // Export all data to the backup database
        // sqlcipher_export returns a result, so use query_row
        conn.query_row("SELECT sqlcipher_export('backup');", [], |_row| Ok(()))
            .map_err(|e| Error::Backup(format!("sqlcipher_export failed: {}", e)))?;

        // Detach the backup database
        conn.execute_batch("DETACH DATABASE backup;")
            .map_err(|e| Error::Backup(format!("Failed to detach backup database: {}", e)))?;

        info!("Created raw backup at: {}", temp_path.display());

        // Store the backup (compresses it)
        let stored_name = destination.store(temp_path, &name)?;

        // Get backup info
        let backups = destination.list()?;
        let info = backups
            .into_iter()
            .find(|b| b.name == stored_name)
            .ok_or_else(|| Error::Backup("Backup not found after storing".to_string()))?;

        info!("Backup complete: {} ({} bytes)", info.name, info.size);

        Ok(BackupResult {
            info,
            accounts: stats.total_accounts,
            transactions: stats.total_transactions,
            subscriptions: stats.active_subscriptions,
        })
    }

    /// Restore a database from backup
    ///
    /// # Arguments
    /// * `destination` - Where the backup is stored
    /// * `backup_name` - Name of the backup to restore
    /// * `target_path` - Where to restore the database
    /// * `force` - Overwrite existing database if present
    pub fn restore_backup(
        destination: &dyn BackupDestination,
        backup_name: &str,
        target_path: &Path,
        force: bool,
    ) -> Result<()> {
        use std::fs;

        // Check if target exists
        if target_path.exists() {
            if !force {
                return Err(Error::Backup(format!(
                    "Database already exists at {}. Use force=true to overwrite.",
                    target_path.display()
                )));
            }

            // Remove existing database and WAL files
            fs::remove_file(target_path)
                .map_err(|e| Error::Backup(format!("Failed to remove existing database: {}", e)))?;

            // Also remove WAL and SHM files if present
            let wal_path = target_path.with_extension("db-wal");
            let shm_path = target_path.with_extension("db-shm");
            let _ = fs::remove_file(wal_path);
            let _ = fs::remove_file(shm_path);
        }

        // Retrieve backup (decompresses if needed)
        destination.retrieve(backup_name, target_path)?;

        info!("Restored backup to: {}", target_path.display());
        Ok(())
    }

    /// List available backups
    pub fn list_backups(
        destination: &dyn BackupDestination,
    ) -> Result<Vec<crate::backup::BackupInfo>> {
        destination.list()
    }

    /// Prune old backups according to retention policy
    pub fn prune_backups(
        destination: &dyn BackupDestination,
        policy: &RetentionPolicy,
    ) -> Result<crate::backup::PruneResult> {
        destination.prune(policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::LocalDestination;
    use crate::models::{AccountType, Bank};
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::new_unencrypted(db_path.to_str().unwrap()).unwrap();
        db.seed_root_tags().unwrap();
        (dir, db)
    }

    #[test]
    fn test_create_backup() {
        let (dir, db) = setup_test_db();

        // Add some test data
        db.upsert_account("Test Bank", Bank::Chase, Some(AccountType::Checking))
            .unwrap();

        // Create backup destination
        let backup_dir = dir.path().join("backups");
        let destination = LocalDestination::new(&backup_dir).unwrap();

        // Create backup
        let result = db.create_backup(&destination, None).unwrap();

        assert!(result.info.name.starts_with("hone-"));
        assert!(result.info.name.ends_with(".db.gz"));
        assert!(result.info.size > 0);
        assert_eq!(result.accounts, 1);
    }

    #[test]
    fn test_restore_backup() {
        let (dir, db) = setup_test_db();

        // Add test data
        db.upsert_account("Test Bank", Bank::Chase, Some(AccountType::Checking))
            .unwrap();

        // Create backup
        let backup_dir = dir.path().join("backups");
        let destination = LocalDestination::new(&backup_dir).unwrap();
        let result = db.create_backup(&destination, None).unwrap();

        // Restore to new location
        let restore_path = dir.path().join("restored.db");
        Database::restore_backup(&destination, &result.info.name, &restore_path, false).unwrap();

        // Open restored database and verify
        let restored_db = Database::new_unencrypted(restore_path.to_str().unwrap()).unwrap();
        let stats = restored_db.get_dashboard_stats().unwrap();
        assert_eq!(stats.total_accounts, 1);
    }

    #[test]
    fn test_list_backups() {
        let (dir, db) = setup_test_db();

        let backup_dir = dir.path().join("backups");
        let destination = LocalDestination::new(&backup_dir).unwrap();

        // Initially empty
        let backups = Database::list_backups(&destination).unwrap();
        assert!(backups.is_empty());

        // Create a backup
        db.create_backup(&destination, Some("hone-2024-01-15-120000.db.gz"))
            .unwrap();

        // Now we have one
        let backups = Database::list_backups(&destination).unwrap();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn test_prune_backups() {
        let (dir, db) = setup_test_db();

        let backup_dir = dir.path().join("backups");
        let destination = LocalDestination::new(&backup_dir).unwrap();

        // Create multiple backups
        for i in 1..=5 {
            let name = format!("hone-2024-01-{:02}-120000.db.gz", i);
            db.create_backup(&destination, Some(&name)).unwrap();
        }

        assert_eq!(Database::list_backups(&destination).unwrap().len(), 5);

        // Prune to keep only 2
        let policy = RetentionPolicy::keep_last(2);
        let result = Database::prune_backups(&destination, &policy).unwrap();

        assert_eq!(result.deleted_count, 3);
        assert_eq!(result.retained_count, 2);
        assert_eq!(Database::list_backups(&destination).unwrap().len(), 2);
    }
}

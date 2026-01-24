//! Backup management commands

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use hone_core::backup::{default_backup_dir, LocalDestination, RetentionPolicy};
use hone_core::Database;

/// Create a new backup
pub fn cmd_backup_create(db: &Database, name: Option<&str>, dir: Option<PathBuf>) -> Result<()> {
    let backup_dir = dir.unwrap_or_else(default_backup_dir);
    let destination = LocalDestination::new(&backup_dir).with_context(|| {
        format!(
            "Failed to initialize backup directory: {}",
            backup_dir.display()
        )
    })?;

    println!("Creating backup...");

    let result = db
        .create_backup(&destination, name)
        .context("Failed to create backup")?;

    println!("✅ Backup created: {}", result.info.name);
    println!("   Location: {}", result.info.path);
    println!("   Size: {} bytes", format_size(result.info.size));
    println!("   Accounts: {}", result.accounts);
    println!("   Transactions: {}", result.transactions);
    println!("   Subscriptions: {}", result.subscriptions);
    if result.info.encrypted {
        println!("   🔒 Encrypted (same passphrase required to restore)");
    }
    if result.info.compressed {
        println!("   📦 Compressed");
    }

    Ok(())
}

/// List available backups
pub fn cmd_backup_list(dir: Option<PathBuf>) -> Result<()> {
    let backup_dir = dir.unwrap_or_else(default_backup_dir);

    if !backup_dir.exists() {
        println!("No backups found (backup directory does not exist)");
        println!("Directory: {}", backup_dir.display());
        return Ok(());
    }

    let destination = LocalDestination::new(&backup_dir).with_context(|| {
        format!(
            "Failed to access backup directory: {}",
            backup_dir.display()
        )
    })?;

    let backups = Database::list_backups(&destination).context("Failed to list backups")?;

    if backups.is_empty() {
        println!("No backups found");
        println!("Directory: {}", backup_dir.display());
        return Ok(());
    }

    println!("Available backups ({}):", backup_dir.display());
    println!();
    println!("{:<35} {:>12} {:>10}", "NAME", "SIZE", "CREATED");
    println!("{}", "-".repeat(60));

    for backup in backups {
        let created = backup.created_at.format("%Y-%m-%d %H:%M");
        let size = format_size(backup.size);
        let flags = format!(
            "{}{}",
            if backup.encrypted { "🔒" } else { "" },
            if backup.compressed { "📦" } else { "" }
        );

        println!("{:<35} {:>12} {:>10} {}", backup.name, size, created, flags);
    }

    Ok(())
}

/// Restore from a backup
pub fn cmd_backup_restore(
    db_path: &std::path::Path,
    name: &str,
    dir: Option<PathBuf>,
    force: bool,
    no_encrypt: bool,
) -> Result<()> {
    let backup_dir = dir.unwrap_or_else(default_backup_dir);
    let destination = LocalDestination::new(&backup_dir).with_context(|| {
        format!(
            "Failed to access backup directory: {}",
            backup_dir.display()
        )
    })?;

    // Check if backup exists
    let backups = Database::list_backups(&destination)?;
    let backup = backups
        .iter()
        .find(|b| b.name == name)
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

    // Check if target exists
    if db_path.exists() && !force {
        anyhow::bail!(
            "Database already exists at {}.\nUse --force to overwrite.",
            db_path.display()
        );
    }

    if db_path.exists() {
        println!(
            "⚠️  This will overwrite the existing database at {}",
            db_path.display()
        );
        print!("Continue? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    println!("Restoring from backup: {}", backup.name);

    Database::restore_backup(&destination, name, db_path, force)
        .context("Failed to restore backup")?;

    // Verify the restored database
    let restored_db = if no_encrypt {
        Database::new_unencrypted(db_path.to_str().unwrap())?
    } else {
        Database::new(db_path.to_str().unwrap())?
    };
    let stats = restored_db.get_dashboard_stats()?;

    println!("✅ Database restored from: {}", backup.name);
    println!("   Location: {}", db_path.display());
    println!("   Accounts: {}", stats.total_accounts);
    println!("   Transactions: {}", stats.total_transactions);
    println!("   Subscriptions: {}", stats.active_subscriptions);

    Ok(())
}

/// Prune old backups according to retention policy
pub fn cmd_backup_prune(keep: usize, dir: Option<PathBuf>, yes: bool) -> Result<()> {
    let backup_dir = dir.unwrap_or_else(default_backup_dir);
    let destination = LocalDestination::new(&backup_dir).with_context(|| {
        format!(
            "Failed to access backup directory: {}",
            backup_dir.display()
        )
    })?;

    let backups = Database::list_backups(&destination)?;

    if backups.len() <= keep {
        println!(
            "Nothing to prune. {} backup(s) found, keeping {}.",
            backups.len(),
            keep
        );
        return Ok(());
    }

    let to_delete = backups.len() - keep;

    if !yes {
        println!(
            "This will delete {} backup(s), keeping the {} most recent:",
            to_delete, keep
        );
        println!();
        for backup in backups.iter().skip(keep) {
            println!("  - {} ({})", backup.name, format_size(backup.size));
        }
        println!();
        print!("Continue? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    let policy = RetentionPolicy::keep_last(keep);
    let result =
        Database::prune_backups(&destination, &policy).context("Failed to prune backups")?;

    println!("✅ Pruned {} backup(s)", result.deleted_count);
    println!("   Freed: {}", format_size(result.bytes_freed));
    println!("   Remaining: {} backup(s)", result.retained_count);

    if !result.deleted_names.is_empty() {
        println!();
        println!("Deleted:");
        for name in &result.deleted_names {
            println!("  - {}", name);
        }
    }

    Ok(())
}

/// Format a byte size as human-readable
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

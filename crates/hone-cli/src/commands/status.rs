//! Status-related command implementations (status, dashboard, accounts, alerts, reset)

use std::path::Path;

use anyhow::{Context, Result};

use super::open_db;

pub fn cmd_status(db_path: &Path, no_encrypt: bool) -> Result<()> {
    use hone_core::db::DB_KEY_ENV;
    use std::fs;

    println!();
    println!("📊 Hone Status");
    println!("   ─────────────────────────────────────────────────────────────");

    // Database path
    println!("   Database: {}", db_path.display());

    // Check if database file exists and get size
    if db_path.exists() {
        if let Ok(metadata) = fs::metadata(db_path) {
            let size_kb = metadata.len() as f64 / 1024.0;
            if size_kb < 1024.0 {
                println!("   Size: {:.1} KB", size_kb);
            } else {
                println!("   Size: {:.1} MB", size_kb / 1024.0);
            }
        }
    } else {
        println!("   Size: (database not initialized)");
    }

    // Check encryption status
    let has_key = std::env::var(DB_KEY_ENV).is_ok();
    if no_encrypt {
        println!("   ⚠️  Encryption: DISABLED (--no-encrypt)");
    } else if has_key {
        println!("   🔒 Encryption: ENABLED ({}=***)", DB_KEY_ENV);
    } else {
        println!("   ❌ Encryption: REQUIRED but {} not set", DB_KEY_ENV);
    }

    // Try to open the database and show stats
    if db_path.exists() {
        match open_db(db_path, no_encrypt) {
            Ok(db) => {
                if let Ok(stats) = db.get_dashboard_stats() {
                    println!();
                    println!("   Accounts: {}", stats.total_accounts);
                    println!("   Transactions: {}", stats.total_transactions);
                    println!("   Subscriptions: {}", stats.active_subscriptions);
                }
            }
            Err(e) => {
                println!();
                println!("   ❌ Error opening database: {}", e);
                if !no_encrypt && !has_key {
                    println!("      Set {} or use --no-encrypt", DB_KEY_ENV);
                } else if has_key {
                    println!("      (Check if {} is correct)", DB_KEY_ENV);
                }
            }
        }
    }

    println!();
    Ok(())
}

pub fn cmd_dashboard(db_path: &Path, no_encrypt: bool) -> Result<()> {
    let db = open_db(db_path, no_encrypt)?;
    let stats = db.get_dashboard_stats()?;

    println!();
    println!("╭─────────────────────────────────────────╮");
    println!("│           💰 Hone Dashboard             │");
    println!("╰─────────────────────────────────────────╯");
    println!();
    println!("  Accounts:        {}", stats.total_accounts);
    println!("  Transactions:    {}", stats.total_transactions);
    if stats.untagged_transactions > 0 {
        println!("  🏷️  Untagged:       {}", stats.untagged_transactions);
    }
    println!();
    println!("  📋 Active Subscriptions: {}", stats.active_subscriptions);
    println!("     Monthly Cost: ${:.2}", stats.monthly_subscription_cost);
    println!();
    println!("  ⚠️  Active Alerts: {}", stats.active_alerts);
    println!(
        "  💸 Potential Savings: ${:.2}/mo",
        stats.potential_monthly_savings
    );
    println!();

    if stats.active_alerts > 0 {
        println!("  Run 'hone alerts' to see what needs attention.");
    }

    Ok(())
}

pub fn cmd_accounts(db_path: &Path, no_encrypt: bool) -> Result<()> {
    let db = open_db(db_path, no_encrypt)?;
    let accounts = db.list_accounts()?;

    if accounts.is_empty() {
        println!("No accounts found. Import transactions with:");
        println!("  hone import --file statement.csv");
        return Ok(());
    }

    println!();
    println!("📁 Accounts");
    println!("   ─────────────────────────────");

    for account in accounts {
        println!("   {} ({})", account.name, account.bank);
    }

    Ok(())
}

pub fn cmd_alerts(db_path: &Path, include_dismissed: bool, no_encrypt: bool) -> Result<()> {
    let db = open_db(db_path, no_encrypt)?;
    let alerts = db.list_alerts(include_dismissed)?;

    let active: Vec<_> = alerts.iter().filter(|a| !a.dismissed).collect();

    if active.is_empty() && !include_dismissed {
        println!("✅ No active alerts. Your spending looks good!");
        return Ok(());
    }

    println!();
    println!("⚠️  Alerts");
    println!("   ─────────────────────────────────────────────────────────────");

    for alert in &alerts {
        if !include_dismissed && alert.dismissed {
            continue;
        }

        let type_icon = match alert.alert_type {
            hone_core::models::AlertType::Zombie => "🧟",
            hone_core::models::AlertType::PriceIncrease => "📈",
            hone_core::models::AlertType::Duplicate => "👯",
            hone_core::models::AlertType::Resume => "🔄",
            hone_core::models::AlertType::SpendingAnomaly => "📊",
            hone_core::models::AlertType::TipDiscrepancy => "💸",
        };

        let dismissed_mark = if alert.dismissed { " (dismissed)" } else { "" };

        println!(
            "   {} {}{}",
            type_icon,
            alert.alert_type.label(),
            dismissed_mark
        );
        if let Some(msg) = &alert.message {
            println!("      {}", msg);
        }
        println!();
    }

    Ok(())
}

/// Reset the database (soft or hard)
pub fn cmd_reset(db_path: &Path, soft: bool, yes: bool, no_encrypt: bool) -> Result<()> {
    use std::fs;
    use std::io::{self, Write};

    if soft {
        // Soft reset: clear data tables but keep config
        if !db_path.exists() {
            anyhow::bail!("Database not found: {}", db_path.display());
        }

        if !yes {
            print!("⚠️  This will delete all transactions, subscriptions, alerts, and receipts.\n");
            print!("   Tags, rules, accounts, and entities will be preserved.\n\n");
            print!("Are you sure? [y/N] ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        let db = open_db(db_path, no_encrypt)?;
        db.soft_reset()?;

        println!("✅ Database soft reset complete.");
        println!("   Cleared: transactions, subscriptions, alerts, receipts");
        println!("   Preserved: accounts, tags, rules, entities");
    } else {
        // Hard reset: delete and re-initialize
        if !yes {
            print!("⚠️  This will DELETE the entire database and start fresh.\n");
            print!("   All data including tags and rules will be lost.\n\n");
            print!("Are you sure? [y/N] ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        // Delete database file if it exists
        if db_path.exists() {
            fs::remove_file(db_path)
                .with_context(|| format!("Failed to delete database: {}", db_path.display()))?;
            // Also remove WAL and journal files if present
            let wal_path = db_path.with_extension("db-wal");
            let shm_path = db_path.with_extension("db-shm");
            let journal_path = db_path.with_extension("db-journal");
            let _ = fs::remove_file(wal_path);
            let _ = fs::remove_file(shm_path);
            let _ = fs::remove_file(journal_path);
        }

        // Re-initialize
        super::cmd_init(db_path, no_encrypt)?;

        println!("\n✅ Database hard reset complete.");
    }

    Ok(())
}

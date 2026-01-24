//! Transaction command implementations

use anyhow::Result;
use hone_core::db::Database;

use super::truncate;

pub fn cmd_transactions_list(db: &Database, limit: i64) -> Result<()> {
    let transactions = db.list_transactions(None, limit, 0)?;

    if transactions.is_empty() {
        println!("No transactions found. Import some with:");
        println!("  hone import --file statement.csv");
        return Ok(());
    }

    println!();
    println!("📝 Recent Transactions");
    println!("   ─────────────────────────────────────────────────────────────");

    for tx in transactions {
        let amount_str = if tx.amount < 0.0 {
            format!("\x1b[31m${:.2}\x1b[0m", tx.amount.abs()) // Red for expenses
        } else {
            format!("\x1b[32m+${:.2}\x1b[0m", tx.amount) // Green for income
        };

        println!(
            "   {} │ {:>10} │ {}",
            tx.date,
            amount_str,
            truncate(&tx.description, 40)
        );
    }

    Ok(())
}

pub fn cmd_transactions_archived(db: &Database, limit: i64) -> Result<()> {
    let transactions = db.list_archived_transactions(limit, 0)?;

    if transactions.is_empty() {
        println!("No archived transactions.");
        return Ok(());
    }

    let count = db.count_archived_transactions()?;

    println!();
    println!("📦 Archived Transactions ({} total)", count);
    println!("   ─────────────────────────────────────────────────────────────");

    for tx in transactions {
        let amount_str = if tx.amount < 0.0 {
            format!("\x1b[31m${:.2}\x1b[0m", tx.amount.abs())
        } else {
            format!("\x1b[32m+${:.2}\x1b[0m", tx.amount)
        };

        println!(
            "   [{}] {} │ {:>10} │ {}",
            tx.id,
            tx.date,
            amount_str,
            truncate(&tx.description, 35)
        );
    }

    println!();
    println!("   Use 'hone transactions unarchive <id>' to restore a transaction.");

    Ok(())
}

pub fn cmd_transactions_archive(db: &Database, id: i64) -> Result<()> {
    // Verify transaction exists
    let tx = db
        .get_transaction(id)?
        .ok_or_else(|| anyhow::anyhow!("Transaction {} not found", id))?;

    if tx.archived {
        println!("Transaction {} is already archived.", id);
        return Ok(());
    }

    db.archive_transaction(id)?;

    println!("✅ Archived transaction {}:", id);
    println!(
        "   {} │ ${:.2} │ {}",
        tx.date,
        tx.amount.abs(),
        truncate(&tx.description, 40)
    );
    println!();
    println!("   This transaction is now hidden from reports and lists.");
    println!("   Use 'hone transactions unarchive {}' to restore it.", id);

    Ok(())
}

pub fn cmd_transactions_unarchive(db: &Database, id: i64) -> Result<()> {
    // Verify transaction exists
    let tx = db
        .get_transaction(id)?
        .ok_or_else(|| anyhow::anyhow!("Transaction {} not found", id))?;

    if !tx.archived {
        println!("Transaction {} is not archived.", id);
        return Ok(());
    }

    db.unarchive_transaction(id)?;

    println!("✅ Restored transaction {}:", id);
    println!(
        "   {} │ ${:.2} │ {}",
        tx.date,
        tx.amount.abs(),
        truncate(&tx.description, 40)
    );
    println!();
    println!("   This transaction will now appear in reports and lists.");

    Ok(())
}

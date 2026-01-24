//! Receipt workflow CLI commands

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use hone_core::ai::{AIBackend, AIClient};
use hone_core::db::Database;
use hone_core::models::{NewReceipt, ReceiptRole, ReceiptStatus};
use sha2::{Digest, Sha256};

/// List receipts by status
pub fn cmd_receipts_list(db: &Database, status: &str) -> Result<()> {
    let status: ReceiptStatus = status.parse().map_err(|e: String| anyhow!(e))?;

    let receipts = db.get_receipts_by_status(status)?;

    if receipts.is_empty() {
        println!("No receipts with status '{}'", status.as_str());
        return Ok(());
    }

    println!(
        "\n{} Receipts ({})",
        match status {
            ReceiptStatus::Pending => "⏳ Pending",
            ReceiptStatus::Matched => "✓ Matched",
            ReceiptStatus::ManualReview => "⚠️  Manual Review",
            ReceiptStatus::Orphaned => "❓ Orphaned",
        },
        receipts.len()
    );
    println!("{}", "─".repeat(70));

    for receipt in &receipts {
        let merchant = receipt.receipt_merchant.as_deref().unwrap_or("Unknown");
        let total = receipt
            .receipt_total
            .map(|t| format!("${:.2}", t))
            .unwrap_or_else(|| "N/A".to_string());
        let date = receipt
            .receipt_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        println!("  #{:<5} {} - {} ({})", receipt.id, merchant, total, date);

        if let Some(path) = &receipt.image_path {
            println!("         📷 {}", path);
        }

        if let Some(tx_id) = receipt.transaction_id {
            println!("         🔗 Linked to transaction #{}", tx_id);
        }
    }

    println!();
    Ok(())
}

/// Upload a receipt image
pub async fn cmd_receipts_add(db: &Database, file: &Path) -> Result<()> {
    // Verify file exists
    if !file.exists() {
        return Err(anyhow!("File not found: {}", file.display()));
    }

    // Read file data
    let image_data = std::fs::read(file).context("Failed to read receipt file")?;

    // Compute content hash
    let mut hasher = Sha256::new();
    hasher.update(&image_data);
    let content_hash = format!("{:x}", hasher.finalize());

    // Check for duplicate
    if let Some(existing) = db.get_receipt_by_hash(&content_hash)? {
        println!("Receipt already exists with ID #{}", existing.id);
        println!("Use 'hone receipts list' to see pending receipts");
        return Ok(());
    }

    // Create receipts directory
    let receipts_dir = std::path::Path::new("receipts");
    if !receipts_dir.exists() {
        std::fs::create_dir_all(receipts_dir).context("Failed to create receipts directory")?;
    }

    // Generate filename
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let filename = format!("receipt_pending_{}.{}", timestamp, extension);
    let image_path = receipts_dir.join(&filename);

    // Save file
    std::fs::write(&image_path, &image_data).context("Failed to save receipt image")?;

    let path_str = image_path.to_string_lossy().to_string();

    // Try to parse with AI backend if available
    println!("Uploading receipt...");

    let parsed = if let Some(ai) = AIClient::from_env() {
        println!("Parsing receipt with AI...");
        match ai.parse_receipt(&image_data, None).await {
            Ok(p) => {
                println!("  Merchant: {}", p.merchant.as_deref().unwrap_or("Unknown"));
                println!("  Date:     {}", p.date.as_deref().unwrap_or("Unknown"));
                println!("  Total:    ${:.2}", p.total.unwrap_or(0.0));
                if !p.items.is_empty() {
                    println!("  Items:    {} line items found", p.items.len());
                }
                Some(p)
            }
            Err(e) => {
                println!("  ⚠️  Could not parse receipt: {}", e);
                None
            }
        }
    } else {
        println!("  ℹ️  AI backend not configured - receipt saved without parsing");
        println!("     Set OLLAMA_HOST environment variable to enable AI parsing");
        None
    };

    // Create receipt record
    let new_receipt = NewReceipt {
        transaction_id: None,
        image_path: Some(path_str.clone()),
        image_data: None,
        status: ReceiptStatus::Pending,
        role: ReceiptRole::Primary,
        receipt_date: parsed.as_ref().and_then(|p| {
            p.date
                .as_ref()
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        }),
        receipt_total: parsed.as_ref().and_then(|p| p.total),
        receipt_merchant: parsed.as_ref().and_then(|p| p.merchant.clone()),
        content_hash: Some(content_hash),
    };

    let receipt_id = db.create_receipt_full(&new_receipt)?;

    // Store parsed JSON if available
    if let Some(ref p) = parsed {
        let json = serde_json::to_string(p)?;
        db.update_receipt_parsed(receipt_id, &json)?;
    }

    println!("\n✓ Receipt #{} created (status: pending)", receipt_id);
    println!("  Saved to: {}", path_str);
    println!("\nNext steps:");
    println!("  - Import bank transactions: hone import --file CSV");
    println!(
        "  - Match manually:          hone receipts match {} <transaction_id>",
        receipt_id
    );

    Ok(())
}

/// Link a receipt to a transaction
pub fn cmd_receipts_match(db: &Database, receipt_id: i64, transaction_id: i64) -> Result<()> {
    // Verify receipt exists
    let receipt = db
        .get_receipt(receipt_id)?
        .ok_or_else(|| anyhow!("Receipt #{} not found", receipt_id))?;

    if receipt.status != ReceiptStatus::Pending && receipt.status != ReceiptStatus::ManualReview {
        return Err(anyhow!(
            "Receipt #{} is already matched or cannot be linked (status: {})",
            receipt_id,
            receipt.status.as_str()
        ));
    }

    // Verify transaction exists
    let tx = db
        .get_transaction(transaction_id)?
        .ok_or_else(|| anyhow!("Transaction #{} not found", transaction_id))?;

    // Link them
    db.link_receipt_to_transaction(receipt_id, transaction_id)?;

    println!(
        "✓ Receipt #{} linked to transaction #{}",
        receipt_id, transaction_id
    );
    println!(
        "  Transaction: {} - ${:.2}",
        tx.description,
        tx.amount.abs()
    );

    if let Some(merchant) = &receipt.receipt_merchant {
        println!(
            "  Receipt:     {} - ${:.2}",
            merchant,
            receipt.receipt_total.unwrap_or(0.0)
        );
    }

    Ok(())
}

/// Update receipt status
pub fn cmd_receipts_status(db: &Database, receipt_id: i64, status: &str) -> Result<()> {
    // Verify receipt exists
    db.get_receipt(receipt_id)?
        .ok_or_else(|| anyhow!("Receipt #{} not found", receipt_id))?;

    // Parse status
    let status: ReceiptStatus = status.parse().map_err(|e: String| anyhow!(e))?;

    // Update
    db.update_receipt_status(receipt_id, status)?;

    println!(
        "✓ Receipt #{} status updated to '{}'",
        receipt_id,
        status.as_str()
    );

    Ok(())
}

/// Delete a receipt
pub fn cmd_receipts_dismiss(db: &Database, receipt_id: i64) -> Result<()> {
    // Verify receipt exists and get path
    let receipt = db
        .get_receipt(receipt_id)?
        .ok_or_else(|| anyhow!("Receipt #{} not found", receipt_id))?;

    // Delete image file if exists
    if let Some(path) = &receipt.image_path {
        let _ = std::fs::remove_file(path); // Ignore errors
    }

    // Delete receipt
    db.delete_receipt(receipt_id)?;

    println!("✓ Receipt #{} dismissed", receipt_id);

    Ok(())
}

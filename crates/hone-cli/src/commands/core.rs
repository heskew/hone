//! Core command implementations and shared utilities
//!
//! This module contains:
//! - `open_db` - Shared utility to open the database
//! - `cmd_init` - Initialize the database
//! - `cmd_detect` - Run waste detection

use std::path::Path;

use anyhow::{Context, Result};
use hone_core::ai::{orchestrator::AIOrchestrator, AIClient};
use hone_core::{db::Database, detect::WasteDetector};

/// Open database with encryption by default, or unencrypted if --no-encrypt
pub fn open_db(db_path: &Path, no_encrypt: bool) -> Result<Database> {
    let path_str = db_path.to_str().unwrap();
    if no_encrypt {
        Database::new_unencrypted(path_str).context("Failed to open database (unencrypted)")
    } else {
        Database::new(path_str).context("Failed to open database")
    }
}

pub fn cmd_init(db_path: &Path, no_encrypt: bool) -> Result<()> {
    println!("🔧 Initializing database at {}...", db_path.display());

    let db = open_db(db_path, no_encrypt)?;

    // Seed root tags
    db.seed_root_tags().context("Failed to seed root tags")?;
    println!("   Seeded default tags");

    if no_encrypt {
        println!("   ⚠️  Encryption: DISABLED (--no-encrypt)");
    } else {
        println!("   🔒 Encryption: ENABLED");
    }

    println!("✅ Database initialized successfully!");
    println!();
    println!("Next steps:");
    println!("  1. Import transactions: hone import --file statement.csv");
    println!("  2. Start web UI: hone serve");

    Ok(())
}

pub async fn cmd_detect(db_path: &Path, kind: &str, no_encrypt: bool) -> Result<()> {
    println!("🔍 Running waste detection...");

    let db = open_db(db_path, no_encrypt)?;

    // Try to create AI orchestrator for agentic analysis (tool-calling)
    let orchestrator = AIOrchestrator::from_env(db.clone());

    // Try to create AI client for smart subscription detection
    let ai = AIClient::from_env();

    // Build detector with best available AI capabilities
    let detector = match (&orchestrator, &ai) {
        (Some(ref orch), Some(ref client)) => {
            println!("   🤖 AI backend enabled (full: classification + agentic analysis)");
            WasteDetector::with_ai_and_orchestrator(&db, client, orch)
        }
        (Some(ref orch), None) => {
            println!("   🤖 AI backend enabled (agentic analysis only)");
            WasteDetector::with_orchestrator(&db, orch)
        }
        (None, Some(ref client)) => {
            println!("   🤖 AI backend enabled (classification only)");
            WasteDetector::with_ai(&db, client)
        }
        (None, None) => {
            println!("   💡 Tip: Set OLLAMA_HOST for smart subscription detection");
            println!("   💡 Tip: Set ANTHROPIC_COMPATIBLE_HOST for agentic analysis");
            WasteDetector::new(&db)
        }
    };

    let results = match kind {
        "zombies" => {
            println!("   Mode: Zombie subscriptions only");
            detector.detect_zombies_only().await?
        }
        "increases" => {
            println!("   Mode: Price increases only");
            detector.detect_increases_only().await?
        }
        "duplicates" => {
            println!("   Mode: Duplicate services only");
            detector.detect_duplicates_only().await?
        }
        _ => {
            println!("   Mode: All detection types");
            detector.detect_all().await?
        }
    };

    println!();
    println!("📊 Detection Results");
    println!("   ─────────────────────────────");
    println!(
        "   Subscriptions identified: {}",
        results.subscriptions_found
    );
    println!("   🧟 Zombie subscriptions: {}", results.zombies_detected);
    println!(
        "   📈 Price increases: {}",
        results.price_increases_detected
    );
    println!("   👯 Duplicate services: {}", results.duplicates_detected);

    let total =
        results.zombies_detected + results.price_increases_detected + results.duplicates_detected;
    if total > 0 {
        println!();
        println!(
            "⚠️  {} potential issues found. Run 'hone alerts' to see details.",
            total
        );
    } else {
        println!();
        println!("✅ No waste detected. Your spending looks good!");
    }

    Ok(())
}

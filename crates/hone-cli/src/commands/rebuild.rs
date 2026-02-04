//! Rebuild command - re-process transactions with current AI backend/rules
//!
//! Clears auto-generated data (tags, merchant normalizations) and re-runs:
//! - Tagging pipeline (learned, rules, patterns, AI, bank category, fallback)
//! - Merchant normalization via AI backend
//! - Waste detection algorithms

use anyhow::{Context, Result};
use std::io::{self, Write};

use hone_core::ai::{orchestrator::AIOrchestrator, AIBackend, AIClient};
use hone_core::detect::WasteDetector;
use hone_core::models::ImportTaggingBreakdown;
use hone_core::tags::TagAssigner;
use hone_core::Database;

/// Re-process transactions with current Ollama models and rules
pub async fn cmd_rebuild(db: &Database, session_id: Option<i64>, yes: bool) -> Result<()> {
    // Get transaction count
    let (transactions, scope_description) = if let Some(session_id) = session_id {
        let session_with_account = db
            .get_import_session(session_id)?
            .context(format!("Import session {} not found", session_id))?;
        let count = db.count_import_session_transactions(session_id)?;
        let txs = db.get_import_session_transactions(session_id, count, 0)?;
        let filename = session_with_account
            .session
            .filename
            .as_deref()
            .unwrap_or("unknown");
        (
            txs,
            format!(
                "import session {} ({} from {})",
                session_id, filename, session_with_account.account_name
            ),
        )
    } else {
        let txs = db.list_transactions(None, 100000, 0)?;
        (txs, "all transactions".to_string())
    };

    if transactions.is_empty() {
        println!("No transactions to rebuild.");
        return Ok(());
    }

    // Confirm unless --yes
    if !yes {
        println!("This will re-process {} for:", scope_description);
        println!("  - {} transactions", transactions.len());
        println!();
        println!("Operations:");
        println!("  1. Clear auto-assigned tags (manual tags preserved)");
        println!("  2. Clear merchant normalizations");
        println!("  3. Re-run tagging pipeline");
        println!("  4. Re-run merchant normalization (if Ollama available)");
        println!("  5. Re-run waste detection");
        println!();
        print!("Proceed? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let transaction_ids: Vec<i64> = transactions.iter().map(|tx| tx.id).collect();

    // 1. Clear auto-assigned tags
    println!("Clearing auto-assigned tags...");
    let tags_cleared = db.clear_auto_tags_for_transactions(&transaction_ids)?;
    println!("  Cleared {} auto-assigned tags", tags_cleared);

    // 2. Clear merchant normalizations
    println!("Clearing merchant normalizations...");
    let merchants_cleared = db.clear_merchant_normalized_for_transactions(&transaction_ids)?;
    println!("  Cleared {} merchant normalizations", merchants_cleared);

    // Initialize AI backend if available
    let ai = AIClient::from_env();
    let ai_available = ai.is_some();
    if ai_available {
        println!("AI backend available - will use for classification and normalization");
    } else {
        println!("AI backend not available - using pattern/rule-based processing only");
    }

    // 3. Re-run tagging
    println!("Re-running tagging pipeline...");
    let assigner = TagAssigner::new(db, ai.as_ref());
    let backfill = if let Some(session_id) = session_id {
        assigner.backfill_tags_for_session(session_id).await?
    } else {
        // For all transactions, use a high limit
        assigner.backfill_tags(100000).await?
    };
    println!(
        "  Tagged {}/{} transactions",
        backfill.transactions_tagged, backfill.transactions_processed
    );
    println!(
        "    Learned: {}, Rules: {}, Patterns: {}, Ollama: {}, Bank: {}, Other: {}",
        backfill.by_learned,
        backfill.by_rule,
        backfill.by_pattern,
        backfill.by_ollama,
        backfill.by_bank_category,
        backfill.fallback_to_other
    );

    // 4. Re-run merchant normalization
    if let Some(ref client) = ai {
        println!("Re-running merchant normalization...");
        let mut normalized_count = 0;

        // Get unique descriptions that need normalization
        let mut descriptions: std::collections::HashMap<String, Vec<i64>> =
            std::collections::HashMap::new();
        for tx in &transactions {
            descriptions
                .entry(tx.description.clone())
                .or_default()
                .push(tx.id);
        }

        for (description, tx_ids) in descriptions {
            match client.normalize_merchant(&description, None).await {
                Ok(normalized) => {
                    // Update all transactions with this description
                    for tx_id in &tx_ids {
                        let _ = db.update_merchant_normalized(*tx_id, &normalized);
                    }
                    normalized_count += tx_ids.len();
                }
                Err(e) => {
                    tracing::debug!("Failed to normalize '{}': {}", description, e);
                }
            }
        }
        println!("  Normalized {} transactions", normalized_count);
    }

    // 5. Re-run waste detection
    println!("Re-running waste detection...");

    // Try to create AI orchestrator for agentic analysis
    let orchestrator = AIOrchestrator::from_env(db.clone());

    // Build detector with best available AI capabilities
    let detector = match (&orchestrator, &ai) {
        (Some(ref orch), Some(ref client)) => {
            WasteDetector::with_ai_and_orchestrator(db, client, orch)
        }
        (Some(ref orch), None) => WasteDetector::with_orchestrator(db, orch),
        (None, Some(ref client)) => WasteDetector::with_ai(db, client),
        (None, None) => WasteDetector::new(db),
    };
    let detection = detector.detect_all().await?;
    println!("  Found {} subscriptions", detection.subscriptions_found);
    println!(
        "  Detected {} zombie subscriptions",
        detection.zombies_detected
    );
    println!(
        "  Detected {} price increases",
        detection.price_increases_detected
    );
    println!(
        "  Detected {} duplicate services",
        detection.duplicates_detected
    );
    println!(
        "  Auto-cancelled {} subscriptions",
        detection.auto_cancelled
    );
    println!(
        "  Detected {} resumed subscriptions",
        detection.resumes_detected
    );
    println!(
        "  Detected {} spending anomalies",
        detection.spending_anomalies_detected
    );

    // Update import session if rebuilding a specific session
    if let Some(session_id) = session_id {
        let tagging_breakdown = ImportTaggingBreakdown {
            by_learned: backfill.by_learned,
            by_rule: backfill.by_rule,
            by_pattern: backfill.by_pattern,
            by_ollama: backfill.by_ollama,
            by_bank_category: backfill.by_bank_category,
            fallback: backfill.fallback_to_other,
        };

        // Get current session to preserve imported/skipped counts
        if let Ok(Some(current)) = db.get_import_session(session_id) {
            db.update_import_session_results(
                session_id,
                current.session.imported_count,
                current.session.skipped_count,
                &tagging_breakdown,
                detection.subscriptions_found as i64,
                detection.zombies_detected as i64,
                detection.price_increases_detected as i64,
                detection.duplicates_detected as i64,
                current.session.receipts_matched,
                detection.spending_anomalies_detected as i64,
                detection.tip_discrepancies_detected as i64,
            )?;
            println!(
                "\nUpdated import session {} with new processing results",
                session_id
            );
        }
    }

    println!("\nRebuild complete!");

    Ok(())
}

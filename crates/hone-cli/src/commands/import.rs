//! Import and export command implementations

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use hone_core::{
    ai::{orchestrator::AIOrchestrator, AIClient},
    db::Database,
    detect::WasteDetector,
    export::TransactionExportOptions,
    import::{detect_bank_format, parse_csv},
    models::Bank,
    tags::TagAssigner,
};

use super::ollama::normalize_merchants;
use super::open_db;

pub async fn cmd_import(
    db_path: &Path,
    file: &Path,
    bank_str: Option<&str>,
    account_name: Option<String>,
    skip_tagging: bool,
    skip_detection: bool,
    no_encrypt: bool,
) -> Result<()> {
    // Open file and read first line for auto-detection
    let csv_file =
        File::open(file).with_context(|| format!("Failed to open file: {}", file.display()))?;
    let mut buf_reader = BufReader::new(csv_file);

    let mut header_line = String::new();
    buf_reader
        .read_line(&mut header_line)
        .with_context(|| "Failed to read CSV header")?;

    // Determine bank format
    let bank: Bank = if let Some(bank_str) = bank_str {
        bank_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Unknown bank format: {}", bank_str))?
    } else {
        detect_bank_format(&header_line).ok_or_else(|| {
            anyhow::anyhow!(
                "Could not auto-detect bank format from CSV header.\n\
                 Specify --bank with one of: chase, bofa, amex, capitalone"
            )
        })?
    };

    let account_name =
        account_name.unwrap_or_else(|| format!("{} Account", bank.as_str().to_uppercase()));

    println!("📥 Importing {} from {}...", bank, file.display());

    let db = open_db(db_path, no_encrypt)?;

    // Re-open file to parse from beginning (including header)
    let csv_file =
        File::open(file).with_context(|| format!("Failed to open file: {}", file.display()))?;
    let transactions = parse_csv(csv_file, bank)?;

    println!("   Found {} transactions", transactions.len());

    // Create/get account
    let account_id = db.upsert_account(&account_name, bank, None)?;

    // Import transactions
    let mut imported = 0;
    let mut skipped = 0;

    for tx in &transactions {
        match db.insert_transaction(account_id, tx)? {
            Some(_) => imported += 1,
            None => skipped += 1,
        }
    }

    println!("✅ Import complete!");
    println!("   Imported: {}", imported);
    println!("   Skipped (duplicates): {}", skipped);

    // Create AI client if configured (used for both tagging and normalization)
    let ai = AIClient::from_env();

    // Auto-tag imported transactions (unless --no-tag)
    if imported > 0 && !skip_tagging {
        println!();
        println!("🏷️  Auto-tagging transactions...");

        let assigner = TagAssigner::new(&db, ai.as_ref());
        let backfill = assigner.backfill_tags(imported).await?;

        println!("   Tagged: {} transactions", backfill.transactions_tagged);
        if backfill.by_rule > 0 {
            println!("   - By rule: {}", backfill.by_rule);
        }
        if backfill.by_pattern > 0 {
            println!("   - By pattern: {}", backfill.by_pattern);
        }
        if backfill.by_bank_category > 0 {
            println!("   - By bank category: {}", backfill.by_bank_category);
        }
        if backfill.by_ollama > 0 || backfill.by_ollama_cached > 0 {
            let total_ollama = backfill.by_ollama + backfill.by_ollama_cached;
            if backfill.by_ollama_cached > 0 {
                println!(
                    "   - By Ollama: {} ({} cached)",
                    total_ollama, backfill.by_ollama_cached
                );
            } else {
                println!("   - By Ollama: {}", total_ollama);
            }
        }
        if backfill.fallback_to_other > 0 {
            println!("   - Uncategorized: {}", backfill.fallback_to_other);
        }
    }

    // Normalize merchant names via AI backend (separate from tagging)
    if imported > 0 {
        if let Some(ref client) = ai {
            println!();
            println!("✨ Normalizing merchant names...");

            let normalized = normalize_merchants(&db, client, imported).await?;
            println!("   Normalized: {} merchants", normalized);
        } else {
            println!();
            println!("💡 Tip: Set OLLAMA_HOST to enable merchant name normalization");
        }
    }

    // Auto-detect subscriptions and waste (unless --no-detect)
    if !skip_detection {
        println!();
        println!("🔍 Running waste detection...");

        // Try to create AI orchestrator for agentic analysis (tool-calling)
        let orchestrator = AIOrchestrator::from_env(db.clone());

        // Build detector with best available AI capabilities
        let detector = match (&orchestrator, &ai) {
            (Some(ref orch), Some(ref client)) => {
                WasteDetector::with_ai_and_orchestrator(&db, client, orch)
            }
            (Some(ref orch), None) => WasteDetector::with_orchestrator(&db, orch),
            (None, Some(ref client)) => WasteDetector::with_ai(&db, client),
            (None, None) => WasteDetector::new(&db),
        };
        let results = detector.detect_all().await?;

        println!(
            "   Subscriptions identified: {}",
            results.subscriptions_found
        );
        if results.zombies_detected > 0 {
            println!("   🧟 Zombie subscriptions: {}", results.zombies_detected);
        }
        if results.price_increases_detected > 0 {
            println!(
                "   📈 Price increases: {}",
                results.price_increases_detected
            );
        }
        if results.duplicates_detected > 0 {
            println!("   👯 Duplicate services: {}", results.duplicates_detected);
        }

        let total = results.zombies_detected
            + results.price_increases_detected
            + results.duplicates_detected;
        if total > 0 {
            println!();
            println!(
                "⚠️  {} potential issues found. Run 'hone alerts' to see details.",
                total
            );
        }
    }

    Ok(())
}

/// Export transactions to CSV
pub fn cmd_export_transactions(
    db: &Database,
    output: Option<PathBuf>,
    from: Option<String>,
    to: Option<String>,
    tags: Option<String>,
    include_children: bool,
) -> Result<()> {
    // Parse date options
    let from_date = from
        .map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .context("Invalid --from date format (use YYYY-MM-DD)")?;

    let to_date = to
        .map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .context("Invalid --to date format (use YYYY-MM-DD)")?;

    // Parse tag IDs
    let tag_ids = tags.map(|s| {
        s.split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok())
            .collect::<Vec<_>>()
    });

    let opts = TransactionExportOptions {
        from: from_date,
        to: to_date,
        tag_ids,
        include_children,
    };

    let csv = db.export_transactions_csv(&opts)?;

    match output {
        Some(path) => {
            let mut file = File::create(&path)
                .with_context(|| format!("Failed to create output file: {}", path.display()))?;
            file.write_all(csv.as_bytes())?;

            let lines = csv.lines().count() - 1; // Subtract header
            println!("✅ Exported {} transactions to {}", lines, path.display());
        }
        None => {
            // Write to stdout
            print!("{}", csv);
        }
    }

    Ok(())
}

/// Export full database backup to JSON
pub fn cmd_export_full(db: &Database, output: &Path) -> Result<()> {
    // Check output doesn't already exist
    if output.exists() {
        anyhow::bail!(
            "Output file already exists: {}\nUse a different filename or remove the existing file.",
            output.display()
        );
    }

    println!("📦 Exporting full database backup...");

    let backup = db.export_full_backup()?;

    // Serialize to JSON
    let json =
        serde_json::to_string_pretty(&backup).context("Failed to serialize backup to JSON")?;

    // Write to file
    let mut file = File::create(output)
        .with_context(|| format!("Failed to create output file: {}", output.display()))?;
    file.write_all(json.as_bytes())?;

    println!("✅ Full backup exported to: {}", output.display());
    println!("   Version: {}", backup.metadata.version);
    println!("   Total records: {}", backup.metadata.total_records);
    println!();
    println!("   Accounts: {}", backup.accounts.len());
    println!("   Transactions: {}", backup.transactions.len());
    println!("   Tags: {}", backup.tags.len());
    println!("   Subscriptions: {}", backup.subscriptions.len());
    println!("   Entities: {}", backup.entities.len());
    println!("   Receipts: {}", backup.receipts.len());

    Ok(())
}

/// Import a full JSON backup
pub fn cmd_import_full(
    db_path: &Path,
    input: &Path,
    clear: bool,
    yes: bool,
    no_encrypt: bool,
) -> Result<()> {
    use std::io;

    // Verify input file exists
    if !input.exists() {
        anyhow::bail!("Backup file not found: {}", input.display());
    }

    // Read and parse the backup file
    let mut file = File::open(input)
        .with_context(|| format!("Failed to open backup file: {}", input.display()))?;
    let mut json = String::new();
    file.read_to_string(&mut json)
        .with_context(|| "Failed to read backup file")?;

    let backup: hone_core::export::FullBackup =
        serde_json::from_str(&json).with_context(|| "Failed to parse backup file as JSON")?;

    println!("📦 Importing full backup from: {}", input.display());
    println!("   Version: {}", backup.metadata.version);
    println!("   Created: {}", backup.metadata.created_at);
    println!("   Total records: {}", backup.metadata.total_records);
    println!();

    // Confirmation for clear
    if clear && !yes {
        print!("⚠️  This will DELETE all existing data before importing.\n");
        print!("   Accounts: {} → {}\n", "?", backup.accounts.len());
        print!("   Transactions: {} → {}\n", "?", backup.transactions.len());
        print!("\nAre you sure? [y/N] ");
        io::stdout().flush()?;

        let mut input_str = String::new();
        io::stdin().read_line(&mut input_str)?;
        if !input_str.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    } else if !clear && !yes {
        print!("⚠️  Importing into existing database. This may cause conflicts if IDs overlap.\n");
        print!("   Use --clear to replace all existing data instead.\n\n");
        print!("Continue? [y/N] ");
        io::stdout().flush()?;

        let mut input_str = String::new();
        io::stdin().read_line(&mut input_str)?;
        if !input_str.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let db = open_db(db_path, no_encrypt)?;
    let stats = db.import_full_backup(&backup, clear)?;

    println!();
    println!("✅ Import complete!");
    println!("   Accounts: {}", stats.accounts);
    println!("   Locations: {}", stats.locations);
    println!("   Entities: {}", stats.entities);
    println!("   Tags: {}", stats.tags);
    println!("   Tag rules: {}", stats.tag_rules);
    println!("   Subscriptions: {}", stats.subscriptions);
    println!("   Transactions: {}", stats.transactions);
    println!("   Transaction tags: {}", stats.transaction_tags);
    println!("   Splits: {}", stats.transaction_splits);
    println!("   Receipts: {}", stats.receipts);
    println!("   Alerts: {}", stats.alerts);

    Ok(())
}

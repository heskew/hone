//! Prompts-related command implementations

use anyhow::Result;
use hone_core::prompts::{default_prompts_dir, PromptId, PromptLibrary};

/// List all available prompts and their override status
pub fn cmd_prompts_list() -> Result<()> {
    let mut library = PromptLibrary::new();
    let prompts = library.list();

    println!("Available Prompts:\n");

    // Header
    println!(
        "{:<35} {:>7}  {:<20}  {}",
        "ID", "VERSION", "TASK TYPE", "OVERRIDE"
    );
    println!("{}", "-".repeat(80));

    for info in prompts {
        let override_status = if info.has_override {
            "✓ Custom"
        } else {
            "Default"
        };

        println!(
            "{:<35} {:>7}  {:<20}  {}",
            info.id, info.version, info.task_type, override_status
        );
    }

    println!();
    println!(
        "Override directory: {}",
        default_prompts_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not available)".to_string())
    );

    println!();
    println!("To customize a prompt:");
    println!("  1. Copy the default to the override directory");
    println!("  2. Edit the file with your changes");
    println!("  3. Restart the server to use the new prompt");

    Ok(())
}

/// Show the content of a specific prompt
pub fn cmd_prompts_show(prompt_id: &str) -> Result<()> {
    let mut library = PromptLibrary::new();

    // Find matching prompt ID
    let id = match prompt_id {
        "classify_merchant" => PromptId::ClassifyMerchant,
        "normalize_merchant" => PromptId::NormalizeMerchant,
        "normalize_merchant_with_context" => PromptId::NormalizeMerchantWithContext,
        "parse_receipt" => PromptId::ParseReceipt,
        "suggest_entity" => PromptId::SuggestEntity,
        "classify_subscription" => PromptId::ClassifySubscription,
        "suggest_split" => PromptId::SuggestSplit,
        "evaluate_receipt_match" => PromptId::EvaluateReceiptMatch,
        "analyze_duplicates" => PromptId::AnalyzeDuplicates,
        "explain_spending" => PromptId::ExplainSpending,
        _ => {
            eprintln!("Unknown prompt ID: {}", prompt_id);
            eprintln!();
            eprintln!("Available prompts:");
            for id in PromptId::all() {
                eprintln!("  - {}", id.as_str());
            }
            return Ok(());
        }
    };

    let prompt = library.get(id)?;

    println!("Prompt: {}", prompt.metadata.id);
    println!("Version: {}", prompt.metadata.version);
    println!("Task Type: {}", prompt.metadata.task_type);
    println!(
        "Source: {}",
        if prompt.is_override {
            "Override"
        } else {
            "Default"
        }
    );

    if let Some(ref path) = prompt.override_path {
        println!("Override Path: {}", path.display());
    }

    println!();
    println!("--- Content ---");
    println!("{}", prompt.content);

    Ok(())
}

/// Show the path where prompt overrides should be placed
pub fn cmd_prompts_path() -> Result<()> {
    match default_prompts_dir() {
        Some(path) => {
            println!("{}", path.display());

            // Check if directory exists
            if !path.exists() {
                eprintln!();
                eprintln!("Note: This directory does not exist yet.");
                eprintln!("Create it to start adding custom prompts.");
            }
        }
        None => {
            eprintln!("Could not determine prompts directory.");
            eprintln!("The data directory is not available on this system.");
        }
    }

    Ok(())
}

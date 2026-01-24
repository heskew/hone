//! Ollama-related command implementations

use std::path::Path;

use anyhow::{Context, Result};
use hone_core::ai::{AIBackend, AIClient, OllamaBackend};
use hone_core::db::Database;

/// Test Ollama connection and AI features
pub async fn cmd_ollama_test(
    merchant: Option<&str>,
    receipt: Option<&Path>,
    vision_model: Option<&str>,
) -> Result<()> {
    println!("🔍 Testing Ollama connection...\n");

    // Check environment variables
    let host = std::env::var("OLLAMA_HOST");
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".to_string());

    match &host {
        Ok(h) => println!("  OLLAMA_HOST: {}", h),
        Err(_) => {
            println!("  ⚠️  OLLAMA_HOST not set (defaulting to http://localhost:11434)");
        }
    }
    println!("  OLLAMA_MODEL: {} (text)", model);
    let vision = vision_model.unwrap_or("llama3.2-vision:11b");
    println!("  Vision model: {}\n", vision);

    let host = host.unwrap_or_else(|_| "http://localhost:11434".to_string());
    let client = OllamaBackend::new(&host, &model);

    // Health check
    print!("Checking Ollama availability... ");
    if client.health_check().await {
        println!("✅ Connected");
    } else {
        println!("❌ Failed");
        println!("\n⚠️  Could not connect to Ollama at {}", host);
        println!("\nTo set up Ollama:");
        println!("  1. Install Ollama: https://ollama.ai/download");
        println!("  2. Start the server: ollama serve");
        println!("  3. Pull the model: ollama pull {}", model);
        println!("  4. Set environment variable: export OLLAMA_HOST={}", host);
        return Ok(());
    }

    // Test merchant classification
    let test_merchants = merchant.map(|m| vec![m.to_string()]).unwrap_or_else(|| {
        vec![
            "NETFLIX.COM*1234".to_string(),
            "SHELL OIL 12345 AUSTIN TX".to_string(),
            "TARGET #1234 AUSTIN TX".to_string(),
            "DOORDASH*THAI KITCHEN".to_string(),
            "SPOTIFY USA".to_string(),
        ]
    });

    println!("\n📋 Testing merchant classification...\n");

    for merchant_name in &test_merchants {
        print!("  \"{}\" → ", merchant_name);
        match client.classify_merchant(merchant_name).await {
            Ok(result) => {
                println!("{} ({})", result.merchant, result.category);
            }
            Err(e) => {
                println!("❌ Error: {}", e);
            }
        }
    }

    // Test split recommendation
    println!("\n🔀 Testing split recommendations...\n");
    let split_test_merchants = ["Target", "Netflix", "Costco", "Starbucks"];
    for merchant_name in split_test_merchants {
        print!("  \"{}\" → ", merchant_name);
        match client.should_suggest_split(merchant_name).await {
            Ok(result) => {
                if result.should_split {
                    println!(
                        "✂️  Yes ({})",
                        if result.typical_categories.is_empty() {
                            result.reason.clone()
                        } else {
                            result.typical_categories.join(", ")
                        }
                    );
                } else {
                    println!("📝 No - {}", result.reason);
                }
            }
            Err(e) => {
                println!("❌ Error: {}", e);
            }
        }
    }

    // Test entity suggestion
    println!("\n👤 Testing entity suggestions...\n");
    let entities = vec![
        "Rex (dog)".to_string(),
        "Kids".to_string(),
        "Honda Civic".to_string(),
    ];
    let entity_tests = [
        ("PETCO", "Pet Care"),
        ("GAMESTOP", "Entertainment"),
        ("JIFFY LUBE", "Auto"),
        ("WHOLE FOODS", "Groceries"),
    ];

    for (merchant_name, category) in entity_tests {
        print!("  {} ({}) → ", merchant_name, category);
        match client
            .suggest_entity(merchant_name, category, &entities)
            .await
        {
            Ok(Some(entity)) => println!("{}", entity),
            Ok(None) => println!("(household/general)"),
            Err(e) => println!("❌ Error: {}", e),
        }
    }

    // Test receipt parsing if image provided
    if let Some(receipt_path) = receipt {
        println!("\n📷 Testing receipt parsing...\n");

        if !receipt_path.exists() {
            println!("  ⚠️  File not found: {}", receipt_path.display());
        } else {
            let image_data = std::fs::read(receipt_path)
                .with_context(|| format!("Failed to read image: {}", receipt_path.display()))?;

            print!("  Parsing {}... ", receipt_path.display());
            match client.parse_receipt(&image_data, Some(vision)).await {
                Ok(result) => {
                    println!("✅\n");
                    if let Some(merchant) = &result.merchant {
                        println!("  Merchant: {}", merchant);
                    }
                    if let Some(date) = &result.date {
                        println!("  Date: {}", date);
                    }
                    println!("\n  Items:");
                    for item in &result.items {
                        println!(
                            "    ${:.2} - {} [{}]{}",
                            item.amount,
                            item.description,
                            item.split_type,
                            item.category_hint
                                .as_ref()
                                .map(|c| format!(" → {}", c))
                                .unwrap_or_default()
                        );
                    }
                    if let Some(total) = result.total {
                        println!("\n  Total: ${:.2}", total);
                    }
                }
                Err(e) => {
                    println!("❌ Error: {}", e);
                    println!("\n  Make sure you have a vision-capable model installed:");
                    println!("    ollama pull {}", vision);
                }
            }
        }
    } else {
        println!("\n💡 Tip: Test receipt parsing with:");
        println!("   hone ollama test --receipt /path/to/receipt.jpg");
    }

    println!("\n✅ Ollama test complete!");
    Ok(())
}

/// Normalize merchant names for recently imported transactions via AI backend
pub(crate) async fn normalize_merchants(db: &Database, ai: &AIClient, limit: i64) -> Result<i64> {
    use std::collections::HashMap;

    // Get transactions without normalized merchant names
    let transactions = db.get_unnormalized_transactions(limit)?;
    if transactions.is_empty() {
        return Ok(0);
    }

    // Collect unique descriptions to avoid duplicate API calls
    let mut unique_descriptions: HashMap<String, Vec<i64>> = HashMap::new();
    for tx in &transactions {
        unique_descriptions
            .entry(tx.description.clone())
            .or_default()
            .push(tx.id);
    }

    let mut normalized_count = 0;

    for (description, tx_ids) in unique_descriptions {
        // Get the category hint from the first transaction's tag
        let category_hint = tx_ids.first().and_then(|&tx_id| {
            db.get_transaction_tags(tx_id)
                .ok()
                .and_then(|tags| tags.first().map(|t| t.tag_id))
                .and_then(|tag_id| db.get_tag(tag_id).ok().flatten())
                .map(|tag| tag.name)
        });

        match ai
            .normalize_merchant(&description, category_hint.as_deref())
            .await
        {
            Ok(normalized) => {
                // Update all transactions with this description
                for tx_id in &tx_ids {
                    if let Err(e) = db.update_merchant_normalized(*tx_id, &normalized) {
                        tracing::warn!(
                            "Failed to update merchant_normalized for tx {}: {}",
                            tx_id,
                            e
                        );
                    } else {
                        normalized_count += 1;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to normalize '{}': {}", description, e);
            }
        }
    }

    Ok(normalized_count)
}

/// CLI command: Normalize merchant names for existing transactions
pub async fn cmd_ollama_normalize(db: &Database, limit: i64) -> Result<()> {
    let ai = AIClient::from_env().ok_or_else(|| {
        anyhow::anyhow!("AI backend not configured. Set OLLAMA_HOST environment variable.")
    })?;

    println!("✨ Normalizing merchant names via AI backend...");
    println!("   Host: {}", ai.host());
    println!("   Model: {}", ai.model());
    println!();

    let transactions = db.get_unnormalized_transactions(limit)?;
    if transactions.is_empty() {
        println!("   No transactions need normalization.");
        return Ok(());
    }

    println!(
        "   Found {} transactions without normalized names",
        transactions.len()
    );

    let normalized = normalize_merchants(db, &ai, limit).await?;

    println!();
    println!("✅ Normalized {} merchant names", normalized);

    Ok(())
}

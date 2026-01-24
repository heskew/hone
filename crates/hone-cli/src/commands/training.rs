//! Training data and model training command implementations

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use hone_core::db::Database;
use hone_core::training::{TrainingDataGenerator, TrainingTask};
use hone_core::training_pipeline::{ExperimentStatus, PipelineConfig, TrainingPipeline};

/// Export training data for a specific task
pub fn cmd_export_training_data(
    db: &Database,
    task: &str,
    output: Option<&Path>,
    min_confidence: f64,
) -> Result<()> {
    let task = TrainingTask::from_str(task)
        .ok_or_else(|| anyhow::anyhow!("Unknown task: {}. Valid tasks: classify_merchant, normalize_merchant, classify_subscription", task))?;

    println!("📊 Exporting training data for: {}", task);
    println!();

    let generator = TrainingDataGenerator::new(db);

    // Generate examples first to show stats
    let examples = generator.generate(task)?;

    if examples.is_empty() {
        println!("⚠️  No training data found for this task.");
        println!();
        println!("Training data is generated from:");
        match task {
            TrainingTask::ClassifyMerchant => {
                println!("  - Manual tag assignments (merchant_tag_cache)");
                println!("  - Tag corrections (ollama_corrections)");
            }
            TrainingTask::NormalizeMerchant => {
                println!("  - Merchant name corrections (merchant_name_cache)");
                println!("  - Uncorrected Ollama normalizations (ollama_metrics)");
            }
            TrainingTask::ClassifySubscription => {
                println!("  - Subscription exclusions (merchant_subscription_cache)");
            }
        }
        return Ok(());
    }

    // Filter by confidence
    let filtered: Vec<_> = examples
        .iter()
        .filter(|e| e.confidence >= min_confidence)
        .collect();

    println!(
        "Found {} examples ({} meet confidence threshold >= {:.0}%)",
        examples.len(),
        filtered.len(),
        min_confidence * 100.0
    );

    // Count by source
    let user_count = filtered
        .iter()
        .filter(|e| {
            e.source == "user" || e.source == "user_correction" || e.source == "user_override"
        })
        .count();
    let ollama_count = filtered.len() - user_count;

    println!("  - User corrections: {}", user_count);
    println!("  - Ollama confirmed: {}", ollama_count);
    println!();

    // Write output
    match output {
        Some(path) => {
            let file = File::create(path)
                .with_context(|| format!("Failed to create output file: {}", path.display()))?;
            let mut writer = BufWriter::new(file);

            let stats = generator.export_jsonl(task, &mut writer)?;
            writer.flush()?;

            println!(
                "✅ Exported {} examples to: {}",
                stats.total_examples,
                path.display()
            );
            println!();
            println!("To fine-tune with Ollama:");
            println!(
                "  ollama create hone-{} -f Modelfile",
                task.as_str().replace('_', "-")
            );
            println!();
            println!("Example Modelfile:");
            println!("  FROM gemma3");
            println!("  ADAPTER {}", path.display());
        }
        None => {
            // Write to stdout
            let mut stdout = std::io::stdout().lock();
            let stats = generator.export_jsonl(task, &mut stdout)?;
            eprintln!();
            eprintln!("# Exported {} examples", stats.total_examples);
        }
    }

    Ok(())
}

/// List available training tasks and their data counts
pub fn cmd_list_training_tasks(db: &Database) -> Result<()> {
    println!("📊 Available Training Tasks");
    println!();

    let generator = TrainingDataGenerator::new(db);

    for task in TrainingTask::all() {
        let examples = generator.generate(task)?;
        let user_count = examples
            .iter()
            .filter(|e| {
                e.source == "user" || e.source == "user_correction" || e.source == "user_override"
            })
            .count();

        let status = if examples.is_empty() {
            "❌ No data"
        } else if user_count < 10 {
            "⚠️  Limited data"
        } else {
            "✅ Ready"
        };

        println!("  {} {}", task, status);
        if !examples.is_empty() {
            println!(
                "      {} examples ({} user, {} ollama)",
                examples.len(),
                user_count,
                examples.len() - user_count
            );
        }
        println!();
    }

    println!("Export training data with:");
    println!("  hone training export --task <task> --output <file.jsonl>");

    Ok(())
}

/// Show training data statistics
pub fn cmd_training_stats(db: &Database) -> Result<()> {
    println!("📊 Training Data Statistics");
    println!();

    let generator = TrainingDataGenerator::new(db);

    let mut total_examples = 0;
    let mut total_user = 0;

    for task in TrainingTask::all() {
        let examples = generator.generate(task)?;
        let user_count = examples
            .iter()
            .filter(|e| {
                e.source == "user" || e.source == "user_correction" || e.source == "user_override"
            })
            .count();

        total_examples += examples.len();
        total_user += user_count;

        if !examples.is_empty() {
            println!("{}:", task);
            println!("  Total examples: {}", examples.len());
            println!(
                "  User corrections: {} ({:.1}%)",
                user_count,
                user_count as f64 / examples.len() as f64 * 100.0
            );
            println!("  Ollama confirmed: {}", examples.len() - user_count);

            // Show confidence distribution
            let high_conf = examples.iter().filter(|e| e.confidence >= 0.9).count();
            let med_conf = examples
                .iter()
                .filter(|e| e.confidence >= 0.7 && e.confidence < 0.9)
                .count();
            let low_conf = examples.iter().filter(|e| e.confidence < 0.7).count();

            println!(
                "  Confidence: {} high, {} medium, {} low",
                high_conf, med_conf, low_conf
            );
            println!();
        }
    }

    println!("─────────────────────────────");
    println!(
        "Total: {} examples ({} user corrections)",
        total_examples, total_user
    );

    if total_user < 50 {
        println!();
        println!("💡 Tip: More user corrections improve fine-tuning quality.");
        println!("   Correct Ollama mistakes in the UI to build training data.");
    }

    Ok(())
}

/// Create a new training experiment
pub fn cmd_training_create(
    db: &Database,
    task: &str,
    branch: &str,
    base_model: Option<&str>,
    notes: Option<&str>,
) -> Result<()> {
    let task =
        TrainingTask::from_str(task).ok_or_else(|| anyhow::anyhow!("Unknown task: {}", task))?;

    println!("🧪 Creating training experiment...");
    println!("   Task: {}", task);
    println!("   Branch: {}", branch);
    if let Some(model) = base_model {
        println!("   Base model: {}", model);
    }
    println!();

    let pipeline = TrainingPipeline::new(db);

    match pipeline.create_experiment(task, branch, base_model, None, notes) {
        Ok(exp) => {
            println!("✅ Experiment created: #{}", exp.id);
            println!("   Model name: {}", exp.model_name);
            println!("   Training examples: {}", exp.training_examples);
            println!();
            println!("Next steps:");
            println!(
                "  1. hone training prepare --id {}   # Export training data",
                exp.id
            );
            println!(
                "  2. hone training train --id {}     # Run fine-tuning",
                exp.id
            );
            println!(
                "  3. hone training promote --id {}   # Promote to production",
                exp.id
            );
        }
        Err(e) => {
            println!("❌ Failed to create experiment: {}", e);
            println!();
            println!("Possible reasons:");
            println!("  - Insufficient training data (minimum 50 examples)");
            println!("  - Build more training data by correcting Ollama in the UI");
        }
    }

    Ok(())
}

/// Prepare training data for an experiment
pub fn cmd_training_prepare(db: &Database, experiment_id: i64) -> Result<()> {
    println!(
        "📦 Preparing training data for experiment #{}...",
        experiment_id
    );

    let pipeline = TrainingPipeline::new(db);

    let (path, stats) = pipeline.prepare_training_data(experiment_id)?;

    println!("✅ Training data prepared:");
    println!("   Path: {}", path.display());
    println!("   Examples: {}", stats.total_examples);
    println!("   User corrections: {}", stats.user_corrections);
    println!("   Ollama confirmed: {}", stats.ollama_confirmed);
    println!();
    println!("Ready to train. Run:");
    println!("  hone training train --id {}", experiment_id);

    Ok(())
}

/// Run training for an experiment (MLX on Mac)
pub fn cmd_training_train(db: &Database, experiment_id: i64, skip_mlx: bool) -> Result<()> {
    println!("🚀 Starting training for experiment #{}...", experiment_id);

    let pipeline = TrainingPipeline::new(db);

    // First prepare data if not already done
    let experiment = db
        .get_training_experiment(experiment_id)?
        .ok_or_else(|| anyhow::anyhow!("Experiment not found"))?;

    if experiment.training_data_path.is_none() {
        println!("   Preparing training data first...");
        pipeline.prepare_training_data(experiment_id)?;
    }

    if skip_mlx {
        println!();
        println!("⚠️  MLX training skipped (--skip-mlx flag)");
        println!();
        println!("To train manually:");
        println!("  1. Install mlx-lm: pip install mlx-lm");
        println!(
            "  2. Run: mlx_lm.lora --model {} --train --data <training_data.jsonl>",
            experiment.base_model
        );
        println!(
            "  3. Update experiment: hone training update --id {} --adapter <path>",
            experiment_id
        );
        return Ok(());
    }

    match pipeline.run_mlx_finetuning(experiment_id) {
        Ok(()) => {
            println!("✅ Training completed!");
            println!();
            println!("Next: Create Ollama model and test:");
            println!("  hone training create-model --id {}", experiment_id);
        }
        Err(e) => {
            println!("❌ Training failed: {}", e);
            println!();
            println!("Troubleshooting:");
            println!("  - Ensure mlx-lm is installed: pip install mlx-lm");
            println!("  - Check you have enough RAM for the base model");
        }
    }

    Ok(())
}

/// Create Ollama model from trained adapter
pub fn cmd_training_create_model(db: &Database, experiment_id: i64) -> Result<()> {
    println!(
        "🔧 Creating Ollama model for experiment #{}...",
        experiment_id
    );

    let pipeline = TrainingPipeline::new(db);

    match pipeline.create_ollama_model(experiment_id) {
        Ok(model_name) => {
            println!("✅ Model created: {}", model_name);
            println!();
            println!("Test with:");
            println!("  OLLAMA_MODEL={} hone ollama test", model_name);
            println!();
            println!("To promote to production:");
            println!("  hone training promote --id {}", experiment_id);
        }
        Err(e) => {
            println!("❌ Failed to create model: {}", e);
        }
    }

    Ok(())
}

/// Promote an experiment to production
pub fn cmd_training_promote(db: &Database, experiment_id: i64) -> Result<()> {
    println!(
        "🚀 Promoting experiment #{} to production...",
        experiment_id
    );

    let pipeline = TrainingPipeline::new(db);

    let experiment = db
        .get_training_experiment(experiment_id)?
        .ok_or_else(|| anyhow::anyhow!("Experiment not found"))?;

    pipeline.promote_experiment(experiment_id)?;

    println!("✅ Experiment promoted!");
    println!();
    println!("To use the fine-tuned model:");
    println!("  export OLLAMA_MODEL={}", experiment.model_name);
    println!();
    println!("Or add to your config/models.toml:");
    println!("  [tasks.{}]", experiment.task);
    println!("  model = \"{}\"", experiment.model_name);

    Ok(())
}

/// List training experiments
pub fn cmd_training_experiments(
    db: &Database,
    task: Option<&str>,
    branch: Option<&str>,
) -> Result<()> {
    println!("🧪 Training Experiments");
    println!();

    let pipeline = TrainingPipeline::new(db);
    let experiments = pipeline.list_experiments(task, branch)?;

    if experiments.is_empty() {
        println!("No experiments found.");
        println!();
        println!("Create one with:");
        println!("  hone training create --task classify_merchant --branch main");
        return Ok(());
    }

    for exp in &experiments {
        let status_icon = match exp.status {
            ExperimentStatus::Pending => "⏳",
            ExperimentStatus::Training => "🔄",
            ExperimentStatus::Completed => "✅",
            ExperimentStatus::Failed => "❌",
            ExperimentStatus::Promoted => "🚀",
            ExperimentStatus::Archived => "📦",
        };

        println!("{} #{} [{}] {}", status_icon, exp.id, exp.branch, exp.task);
        println!("   Model: {} → {}", exp.base_model, exp.model_name);
        println!("   Examples: {}", exp.training_examples);
        println!("   Created: {}", exp.created_at.format("%Y-%m-%d %H:%M"));
        if let Some(ref notes) = exp.notes {
            println!("   Notes: {}", notes);
        }
        println!();
    }

    Ok(())
}

/// Run training agent (automated monitoring and recommendations)
pub fn cmd_training_agent(db: &Database, check_only: bool) -> Result<()> {
    println!("🤖 Training Agent Status Report");
    println!("================================");
    println!();

    let generator = TrainingDataGenerator::new(db);
    let pipeline = TrainingPipeline::new(db);
    let config = PipelineConfig::default();

    let mut recommendations = Vec::new();

    for task in TrainingTask::all() {
        let examples = generator.generate(task)?;
        let user_count = examples
            .iter()
            .filter(|e| {
                e.source == "user" || e.source == "user_correction" || e.source == "user_override"
            })
            .count();

        println!("📊 {}", task);

        // Check if we have enough data
        if examples.len() < config.min_training_examples {
            println!(
                "   ⚠️  Insufficient data: {} examples (need {})",
                examples.len(),
                config.min_training_examples
            );
            println!("   💡 Add more corrections in the UI");
        } else {
            println!(
                "   ✅ Data ready: {} examples ({} user)",
                examples.len(),
                user_count
            );

            // Check for promoted experiment
            let promoted = pipeline.get_promoted_experiment(task.as_str())?;

            if let Some(exp) = promoted {
                // Check if we have significantly more data than when trained
                let data_growth = examples.len() as f64 / exp.training_examples as f64;
                if data_growth > 1.2 {
                    println!(
                        "   🔄 Promoted: #{} ({} examples)",
                        exp.id, exp.training_examples
                    );
                    println!(
                        "   📈 Data growth: +{:.0}% since last training",
                        (data_growth - 1.0) * 100.0
                    );
                    recommendations.push(format!(
                        "Consider retraining {} - data grew by {:.0}%",
                        task,
                        (data_growth - 1.0) * 100.0
                    ));
                } else {
                    println!("   🚀 Promoted: #{} (up to date)", exp.id);
                }
            } else {
                println!("   ⏳ No promoted model");
                recommendations.push(format!(
                    "Create first experiment for {}: hone training create --task {} --branch main",
                    task,
                    task.as_str()
                ));
            }
        }
        println!();
    }

    // Show recommendations
    if !recommendations.is_empty() {
        println!("📋 Recommendations");
        println!("─────────────────");
        for (i, rec) in recommendations.iter().enumerate() {
            println!("{}. {}", i + 1, rec);
        }
        println!();

        if !check_only {
            println!("Run with --check-only to skip action suggestions.");
        }
    } else {
        println!("✅ All models are up to date!");
    }

    Ok(())
}

/// Branch from an existing experiment
pub fn cmd_training_branch(db: &Database, experiment_id: i64, branch_name: &str) -> Result<()> {
    println!(
        "🌿 Creating branch '{}' from experiment #{}...",
        branch_name, experiment_id
    );

    let pipeline = TrainingPipeline::new(db);

    match pipeline.branch_experiment(experiment_id, branch_name) {
        Ok(exp) => {
            println!("✅ Branch created: #{}", exp.id);
            println!("   Task: {}", exp.task);
            println!("   Parent: #{}", experiment_id);
            println!();
            println!("Continue with:");
            println!("  hone training prepare --id {}", exp.id);
        }
        Err(e) => {
            println!("❌ Failed to create branch: {}", e);
        }
    }

    Ok(())
}

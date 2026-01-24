//! Hone CLI - Personal finance waste detector
//!
//! Usage:
//!   hone init                 Initialize database
//!   hone import --file CSV    Import transactions (auto-detects bank format)
//!   hone detect --kind all    Run waste detection
//!   hone serve --port 3000    Start web server

mod cli;
mod commands;

#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use cli::*;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging
    // Priority: RUST_LOG env var > --verbose flag > default (info)
    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false).compact())
        .init();

    match cli.command {
        Commands::Init => commands::cmd_init(&cli.db, cli.no_encrypt),
        Commands::Import {
            file,
            bank,
            account,
            no_tag,
            no_detect,
        } => {
            commands::cmd_import(
                &cli.db,
                &file,
                bank.as_deref(),
                account,
                no_tag,
                no_detect,
                cli.no_encrypt,
            )
            .await
        }
        Commands::Detect { kind } => commands::cmd_detect(&cli.db, &kind, cli.no_encrypt).await,
        Commands::Serve {
            port,
            host,
            no_auth,
            static_dir,
            mcp_port,
        } => {
            commands::cmd_serve(
                &cli.db,
                &host,
                port,
                no_auth,
                cli.no_encrypt,
                static_dir.as_deref(),
                mcp_port,
            )
            .await
        }
        Commands::Dashboard => commands::cmd_dashboard(&cli.db, cli.no_encrypt),
        Commands::Status => commands::cmd_status(&cli.db, cli.no_encrypt),
        Commands::Accounts => commands::cmd_accounts(&cli.db, cli.no_encrypt),
        Commands::Transactions { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                None | Some(TransactionsAction::List { limit: 20 }) => {
                    commands::cmd_transactions_list(&db, 20)
                }
                Some(TransactionsAction::List { limit }) => {
                    commands::cmd_transactions_list(&db, limit)
                }
                Some(TransactionsAction::Archived { limit }) => {
                    commands::cmd_transactions_archived(&db, limit)
                }
                Some(TransactionsAction::Archive { id }) => {
                    commands::cmd_transactions_archive(&db, id)
                }
                Some(TransactionsAction::Unarchive { id }) => {
                    commands::cmd_transactions_unarchive(&db, id)
                }
            }
        }
        Commands::Subscriptions { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                Some(SubscriptionsAction::Cancel { name_or_id, date }) => {
                    commands::cmd_subscriptions_cancel(&db, &name_or_id, date.as_deref())
                }
                None => commands::cmd_subscriptions_list(&db),
            }
        }
        Commands::Alerts { all } => commands::cmd_alerts(&cli.db, all, cli.no_encrypt),
        Commands::Tags { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                None => commands::cmd_tags_list(&db),
                Some(TagsAction::Add {
                    path,
                    color,
                    patterns,
                }) => commands::cmd_tags_add(&db, &path, color.as_deref(), patterns.as_deref()),
                Some(TagsAction::Rename { old_name, new_name }) => {
                    commands::cmd_tags_rename(&db, &old_name, &new_name)
                }
                Some(TagsAction::Move { tag, to }) => commands::cmd_tags_move(&db, &tag, &to),
                Some(TagsAction::Delete {
                    tag,
                    force,
                    to_parent,
                }) => commands::cmd_tags_delete(&db, &tag, force, to_parent),
                Some(TagsAction::Merge { source, into }) => {
                    commands::cmd_tags_merge(&db, &source, &into)
                }
            }
        }
        Commands::Rules { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                None => commands::cmd_rules_list(&db),
                Some(RulesAction::Add {
                    tag,
                    pattern,
                    pattern_type,
                    priority,
                }) => commands::cmd_rules_add(&db, &tag, &pattern, &pattern_type, priority),
                Some(RulesAction::Delete { id }) => commands::cmd_rules_delete(&db, id),
                Some(RulesAction::Test { description }) => {
                    commands::cmd_rules_test(&db, &description)
                }
            }
        }
        Commands::Tag {
            transaction_id,
            tag,
        } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            commands::cmd_tag(&db, transaction_id, &tag)
        }
        Commands::Untag {
            transaction_id,
            tag,
        } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            commands::cmd_untag(&db, transaction_id, &tag)
        }
        Commands::Report { report_type } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match report_type {
                ReportType::Spending {
                    period,
                    from,
                    to,
                    tag,
                    expand,
                } => {
                    let (from_date, to_date) =
                        commands::resolve_period(&period, from.as_deref(), to.as_deref())?;
                    commands::cmd_report_spending(&db, from_date, to_date, tag.as_deref(), expand)
                }
                ReportType::Trends {
                    granularity,
                    period,
                    tag,
                } => {
                    let (from_date, to_date) = commands::resolve_period(&period, None, None)?;
                    let granularity: hone_core::models::Granularity = granularity
                        .parse()
                        .map_err(|e: String| anyhow::anyhow!(e))?;
                    commands::cmd_report_trends(
                        &db,
                        from_date,
                        to_date,
                        granularity,
                        tag.as_deref(),
                    )
                }
                ReportType::Merchants { limit, period, tag } => {
                    let (from_date, to_date) = commands::resolve_period(&period, None, None)?;
                    commands::cmd_report_merchants(&db, from_date, to_date, limit, tag.as_deref())
                }
                ReportType::Subscriptions => commands::cmd_report_subscriptions(&db),
                ReportType::Savings => commands::cmd_report_savings(&db),
                ReportType::ByTag { depth, from, to } => {
                    use anyhow::Context;
                    let from_date = from
                        .as_deref()
                        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                        .transpose()
                        .context("Invalid --from date format (use YYYY-MM-DD)")?;
                    let to_date = to
                        .as_deref()
                        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
                        .transpose()
                        .context("Invalid --to date format (use YYYY-MM-DD)")?;
                    commands::cmd_report_by_tag(&db, depth, from_date, to_date)
                }
            }
        }
        Commands::Entities { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                None => commands::cmd_entities_list(&db, false),
                Some(EntitiesAction::List { entity_type, all }) => {
                    if let Some(ref etype) = entity_type {
                        commands::cmd_entities_list_type(&db, etype)
                    } else {
                        commands::cmd_entities_list(&db, all)
                    }
                }
                Some(EntitiesAction::Add {
                    name,
                    entity_type,
                    icon,
                    color,
                }) => commands::cmd_entities_add(
                    &db,
                    &name,
                    &entity_type,
                    icon.as_deref(),
                    color.as_deref(),
                ),
                Some(EntitiesAction::Update {
                    id,
                    name,
                    icon,
                    color,
                }) => commands::cmd_entities_update(
                    &db,
                    id,
                    name.as_deref(),
                    icon.as_deref(),
                    color.as_deref(),
                ),
                Some(EntitiesAction::Archive { id }) => commands::cmd_entities_archive(&db, id),
                Some(EntitiesAction::Unarchive { id }) => commands::cmd_entities_unarchive(&db, id),
                Some(EntitiesAction::Delete { id, force }) => {
                    commands::cmd_entities_delete(&db, id, force)
                }
            }
        }
        Commands::Backup { action } => match action {
            BackupAction::Create { name, dir } => {
                let db = commands::open_db(&cli.db, cli.no_encrypt)?;
                commands::cmd_backup_create(&db, name.as_deref(), dir)
            }
            BackupAction::List { dir } => commands::cmd_backup_list(dir),
            BackupAction::Restore { name, dir, force } => {
                commands::cmd_backup_restore(&cli.db, &name, dir, force, cli.no_encrypt)
            }
            BackupAction::Prune { keep, dir, yes } => commands::cmd_backup_prune(keep, dir, yes),
        },
        Commands::Ollama { action } => match action {
            OllamaAction::Test {
                merchant,
                receipt,
                vision_model,
            } => {
                commands::cmd_ollama_test(
                    merchant.as_deref(),
                    receipt.as_deref(),
                    vision_model.as_deref(),
                )
                .await
            }
            OllamaAction::Normalize { limit } => {
                let db = commands::open_db(&cli.db, cli.no_encrypt)?;
                commands::cmd_ollama_normalize(&db, limit).await
            }
        },
        Commands::Receipts { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                None => commands::cmd_receipts_list(&db, "pending"),
                Some(ReceiptsAction::Add { file, account: _ }) => {
                    commands::cmd_receipts_add(&db, &file).await
                }
                Some(ReceiptsAction::List { status }) => commands::cmd_receipts_list(&db, &status),
                Some(ReceiptsAction::Match {
                    receipt_id,
                    transaction_id,
                }) => commands::cmd_receipts_match(&db, receipt_id, transaction_id),
                Some(ReceiptsAction::Status { receipt_id, status }) => {
                    commands::cmd_receipts_status(&db, receipt_id, &status)
                }
                Some(ReceiptsAction::Dismiss {
                    receipt_id,
                    reason: _,
                }) => commands::cmd_receipts_dismiss(&db, receipt_id),
            }
        }
        Commands::Reset { soft, yes } => commands::cmd_reset(&cli.db, soft, yes, cli.no_encrypt),
        Commands::Export { export_type } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match export_type {
                ExportType::Transactions {
                    output,
                    from,
                    to,
                    tags,
                    include_children,
                } => {
                    commands::cmd_export_transactions(&db, output, from, to, tags, include_children)
                }
                ExportType::Full { output } => commands::cmd_export_full(&db, &output),
            }
        }
        Commands::ImportFull { file, clear, yes } => {
            commands::cmd_import_full(&cli.db, &file, clear, yes, cli.no_encrypt)
        }
        Commands::Prompts { action } => match action {
            None | Some(PromptsAction::List) => commands::cmd_prompts_list(),
            Some(PromptsAction::Show { prompt_id }) => commands::cmd_prompts_show(&prompt_id),
            Some(PromptsAction::Path) => commands::cmd_prompts_path(),
        },
        Commands::Rebuild { session, yes } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            commands::cmd_rebuild(&db, session, yes).await
        }
        Commands::Training { action } => {
            let db = commands::open_db(&cli.db, cli.no_encrypt)?;
            match action {
                TrainingAction::List => commands::cmd_list_training_tasks(&db),
                TrainingAction::Export {
                    task,
                    output,
                    min_confidence,
                } => commands::cmd_export_training_data(
                    &db,
                    &task,
                    output.as_deref(),
                    min_confidence,
                ),
                TrainingAction::Stats => commands::cmd_training_stats(&db),
                TrainingAction::Create {
                    task,
                    branch,
                    base_model,
                    notes,
                } => commands::cmd_training_create(
                    &db,
                    &task,
                    &branch,
                    base_model.as_deref(),
                    notes.as_deref(),
                ),
                TrainingAction::Prepare { id } => commands::cmd_training_prepare(&db, id),
                TrainingAction::Train { id, skip_mlx } => {
                    commands::cmd_training_train(&db, id, skip_mlx)
                }
                TrainingAction::CreateModel { id } => commands::cmd_training_create_model(&db, id),
                TrainingAction::Promote { id } => commands::cmd_training_promote(&db, id),
                TrainingAction::Experiments { task, branch } => {
                    commands::cmd_training_experiments(&db, task.as_deref(), branch.as_deref())
                }
                TrainingAction::Branch { id, name } => {
                    commands::cmd_training_branch(&db, id, &name)
                }
                TrainingAction::Agent { check_only } => {
                    commands::cmd_training_agent(&db, check_only)
                }
            }
        }
    }
}

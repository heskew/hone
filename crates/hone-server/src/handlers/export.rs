//! Export and full backup/import handlers

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, Response, StatusCode},
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{get_user_email, AppError, AppState};
use hone_core::export::{FullBackup, ImportStats, TransactionExportOptions};

/// Query parameters for transaction export
#[derive(Debug, Deserialize)]
pub struct TransactionExportQuery {
    /// Output format (default: csv)
    #[serde(default = "default_format")]
    pub format: String,
    /// Start date (YYYY-MM-DD)
    pub from: Option<String>,
    /// End date (YYYY-MM-DD)
    pub to: Option<String>,
    /// Filter by tag IDs (comma-separated)
    pub tags: Option<String>,
    /// Include child tags when filtering
    #[serde(default)]
    pub include_children: bool,
}

fn default_format() -> String {
    "csv".to_string()
}

/// GET /api/export/transactions - Export transactions to CSV or JSON
pub async fn export_transactions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<TransactionExportQuery>,
) -> Result<Response<Body>, AppError> {
    let user_email = get_user_email(&headers);

    // Parse date options
    let from_date = params
        .from
        .map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::bad_request("Invalid 'from' date format (use YYYY-MM-DD)"))?;

    let to_date = params
        .to
        .map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::bad_request("Invalid 'to' date format (use YYYY-MM-DD)"))?;

    // Parse tag IDs
    let tag_ids = params.tags.map(|s| {
        s.split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok())
            .collect::<Vec<_>>()
    });

    let opts = TransactionExportOptions {
        from: from_date,
        to: to_date,
        tag_ids,
        include_children: params.include_children,
    };

    // Audit log
    state.db.log_audit(
        &user_email,
        "export_transactions",
        Some("transaction"),
        None,
        Some(&format!(
            "format={}, from={:?}, to={:?}, tags={:?}",
            params.format, from_date, to_date, opts.tag_ids
        )),
    )?;

    match params.format.as_str() {
        "csv" => {
            let csv = state.db.export_transactions_csv(&opts)?;
            let lines = csv.lines().count().saturating_sub(1);
            info!("Exported {} transactions to CSV", lines);

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"transactions.csv\"",
                )
                .body(Body::from(csv))
                .map_err(|e| AppError::internal(&e.to_string()))
        }
        "json" => {
            let transactions = state.db.export_transactions(&opts)?;
            let json = serde_json::to_string_pretty(&transactions)
                .map_err(|e| AppError::internal(&e.to_string()))?;
            info!("Exported {} transactions to JSON", transactions.len());

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"transactions.json\"",
                )
                .body(Body::from(json))
                .map_err(|e| AppError::internal(&e.to_string()))
        }
        _ => Err(AppError::bad_request("Invalid format. Use 'csv' or 'json'")),
    }
}

/// GET /api/export/full - Export full database backup as JSON
pub async fn export_full(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Response<Body>, AppError> {
    let user_email = get_user_email(&headers);

    info!("Exporting full database backup");
    let backup = state.db.export_full_backup()?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "export_full",
        None,
        None,
        Some(&format!(
            "version={}, total_records={}",
            backup.metadata.version, backup.metadata.total_records
        )),
    )?;

    let json =
        serde_json::to_string_pretty(&backup).map_err(|e| AppError::internal(&e.to_string()))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"hone-backup-{}.json\"",
                chrono::Utc::now().format("%Y-%m-%d")
            ),
        )
        .body(Body::from(json))
        .map_err(|e| AppError::internal(&e.to_string()))
}

/// Query parameters for full import
#[derive(Debug, Deserialize)]
pub struct ImportFullQuery {
    /// Clear existing data before import (required)
    #[serde(default)]
    pub clear: bool,
}

/// Response for full import
#[derive(Serialize)]
pub struct ImportFullResponse {
    pub success: bool,
    pub stats: ImportStats,
}

/// POST /api/import/full - Import full database backup from JSON
pub async fn import_full(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<ImportFullQuery>,
    body: String,
) -> Result<Json<ImportFullResponse>, AppError> {
    let user_email = get_user_email(&headers);

    // Parse the backup JSON
    let backup: FullBackup = serde_json::from_str(&body)
        .map_err(|e| AppError::bad_request(&format!("Invalid JSON: {}", e)))?;

    info!(
        "Importing full backup: version={}, records={}",
        backup.metadata.version, backup.metadata.total_records
    );

    // Import the backup
    let stats = state.db.import_full_backup(&backup, params.clear)?;

    // Audit log
    state.db.log_audit(
        &user_email,
        "import_full",
        None,
        None,
        Some(&format!(
            "clear={}, accounts={}, transactions={}, tags={}",
            params.clear, stats.accounts, stats.transactions, stats.tags
        )),
    )?;

    Ok(Json(ImportFullResponse {
        success: true,
        stats,
    }))
}

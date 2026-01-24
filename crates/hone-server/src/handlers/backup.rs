//! Backup API handlers

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use hone_core::backup::{default_backup_dir, BackupDestination, LocalDestination, RetentionPolicy};
use hone_core::Database;

use crate::{get_user_email, AppError, AppState};

/// Create backup request
#[derive(Debug, Deserialize)]
pub struct CreateBackupRequest {
    /// Optional backup name (defaults to timestamped name)
    pub name: Option<String>,
}

/// Create backup response
#[derive(Debug, Serialize)]
pub struct CreateBackupResponse {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub accounts: i64,
    pub transactions: i64,
    pub subscriptions: i64,
    pub encrypted: bool,
    pub compressed: bool,
}

/// List backups response
#[derive(Debug, Serialize)]
pub struct BackupInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub created_at: String,
    pub encrypted: bool,
    pub compressed: bool,
}

/// Prune request
#[derive(Debug, Deserialize)]
pub struct PruneBackupsRequest {
    /// Number of backups to keep (default: 7)
    pub keep: Option<usize>,
}

/// Prune response
#[derive(Debug, Serialize)]
pub struct PruneBackupsResponse {
    pub deleted_count: usize,
    pub deleted_names: Vec<String>,
    pub retained_count: usize,
    pub bytes_freed: u64,
}

/// Get backup directory from state or default
fn get_backup_dir(state: &AppState) -> std::path::PathBuf {
    state.backup_dir.clone().unwrap_or_else(default_backup_dir)
}

/// Create a backup
pub async fn create_backup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateBackupRequest>,
) -> Result<Json<CreateBackupResponse>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);
    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    let result = state
        .db
        .create_backup(&destination, req.name.as_deref())
        .map_err(|e| AppError::internal(&format!("Failed to create backup: {}", e)))?;

    // Log audit
    state.db.log_audit(
        &user_email,
        "backup_created",
        Some("backup"),
        None,
        Some(&format!("name={}", result.info.name)),
    )?;

    Ok(Json(CreateBackupResponse {
        name: result.info.name,
        path: result.info.path,
        size: result.info.size,
        accounts: result.accounts,
        transactions: result.transactions,
        subscriptions: result.subscriptions,
        encrypted: result.info.encrypted,
        compressed: result.info.compressed,
    }))
}

/// List available backups
pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<BackupInfo>>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);

    // Check if backup directory exists
    if !backup_dir.exists() {
        return Ok(Json(vec![]));
    }

    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    let backups = hone_core::Database::list_backups(&destination)
        .map_err(|e| AppError::internal(&format!("Failed to list backups: {}", e)))?;

    // Log audit
    state.db.log_audit(
        &user_email,
        "backup_list",
        Some("backup"),
        None,
        Some(&format!("count={}", backups.len())),
    )?;

    let response: Vec<BackupInfo> = backups
        .into_iter()
        .map(|b| BackupInfo {
            name: b.name,
            path: b.path,
            size: b.size,
            created_at: b.created_at.to_rfc3339(),
            encrypted: b.encrypted,
            compressed: b.compressed,
        })
        .collect();

    Ok(Json(response))
}

/// Prune old backups
pub async fn prune_backups(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<PruneBackupsRequest>,
) -> Result<Json<PruneBackupsResponse>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);
    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    let keep = req.keep.unwrap_or(7);
    let policy = RetentionPolicy::keep_last(keep);

    let result = hone_core::Database::prune_backups(&destination, &policy)
        .map_err(|e| AppError::internal(&format!("Failed to prune backups: {}", e)))?;

    // Log audit
    state.db.log_audit(
        &user_email,
        "backup_prune",
        Some("backup"),
        None,
        Some(&format!("keep={}, deleted={}", keep, result.deleted_count)),
    )?;

    Ok(Json(PruneBackupsResponse {
        deleted_count: result.deleted_count,
        deleted_names: result.deleted_names,
        retained_count: result.retained_count,
        bytes_freed: result.bytes_freed,
    }))
}

/// Get a specific backup by name
pub async fn get_backup(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<Json<BackupInfo>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);

    if !backup_dir.exists() {
        return Err(AppError::not_found("Backup not found"));
    }

    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    let backups = Database::list_backups(&destination)
        .map_err(|e| AppError::internal(&format!("Failed to list backups: {}", e)))?;

    let backup = backups
        .into_iter()
        .find(|b| b.name == name)
        .ok_or_else(|| AppError::not_found("Backup not found"))?;

    state.db.log_audit(
        &user_email,
        "backup_view",
        Some("backup"),
        None,
        Some(&format!("name={}", backup.name)),
    )?;

    Ok(Json(BackupInfo {
        name: backup.name,
        path: backup.path,
        size: backup.size,
        created_at: backup.created_at.to_rfc3339(),
        encrypted: backup.encrypted,
        compressed: backup.compressed,
    }))
}

/// Restore backup request
#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    /// Force overwrite if database exists
    #[serde(default)]
    pub force: bool,
}

/// Restore backup response
#[derive(Debug, Serialize)]
pub struct RestoreBackupResponse {
    pub success: bool,
    pub message: String,
    pub backup_name: String,
    /// Stats from restored database (if verification succeeded)
    pub accounts: Option<i64>,
    pub transactions: Option<i64>,
    pub subscriptions: Option<i64>,
}

/// POST /api/backup/:name/restore - Restore from a backup
///
/// WARNING: This replaces the current database with the backup.
/// The server will need to be restarted after restore to pick up the new database.
pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(req): Json<RestoreBackupRequest>,
) -> Result<Json<RestoreBackupResponse>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);

    if !backup_dir.exists() {
        return Err(AppError::not_found("Backup not found"));
    }

    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    // Verify backup exists
    let backups = Database::list_backups(&destination)
        .map_err(|e| AppError::internal(&format!("Failed to list backups: {}", e)))?;

    let backup = backups
        .iter()
        .find(|b| b.name == name)
        .ok_or_else(|| AppError::not_found("Backup not found"))?;

    // Get the database path from the current database
    let db_path = state.db.path();

    // Check if database exists and force flag
    if std::path::Path::new(&db_path).exists() && !req.force {
        return Err(AppError::conflict(
            "Database already exists. Set force=true to overwrite.",
        ));
    }

    info!(
        "Restoring backup {} to {} (force={})",
        name, db_path, req.force
    );

    // Perform the restore
    Database::restore_backup(
        &destination,
        &name,
        std::path::Path::new(&db_path),
        req.force,
    )
    .map_err(|e| AppError::internal(&format!("Failed to restore backup: {}", e)))?;

    // Try to verify the restored database
    let (accounts, transactions, subscriptions) = match Database::new(&db_path) {
        Ok(restored_db) => match restored_db.get_dashboard_stats() {
            Ok(stats) => (
                Some(stats.total_accounts),
                Some(stats.total_transactions),
                Some(stats.active_subscriptions),
            ),
            Err(_) => (None, None, None),
        },
        Err(_) => (None, None, None),
    };

    // Log audit
    state.db.log_audit(
        &user_email,
        "backup_restored",
        Some("backup"),
        None,
        Some(&format!(
            "name={}, force={}, accounts={:?}",
            backup.name, req.force, accounts
        )),
    )?;

    Ok(Json(RestoreBackupResponse {
        success: true,
        message: "Backup restored. Restart the server to use the restored database.".to_string(),
        backup_name: backup.name.clone(),
        accounts,
        transactions,
        subscriptions,
    }))
}

/// DELETE /api/backup/:name - Delete a specific backup
pub async fn delete_backup(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<Json<crate::SuccessResponse>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);

    if !backup_dir.exists() {
        return Err(AppError::not_found("Backup not found"));
    }

    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    // Verify backup exists
    let backups = Database::list_backups(&destination)
        .map_err(|e| AppError::internal(&format!("Failed to list backups: {}", e)))?;

    if !backups.iter().any(|b| b.name == name) {
        return Err(AppError::not_found("Backup not found"));
    }

    // Delete the backup
    destination
        .delete(&name)
        .map_err(|e| AppError::internal(&format!("Failed to delete backup: {}", e)))?;

    // Log audit
    state.db.log_audit(
        &user_email,
        "backup_deleted",
        Some("backup"),
        None,
        Some(&format!("name={}", name)),
    )?;

    Ok(Json(crate::SuccessResponse { success: true }))
}

/// Verify backup request
#[derive(Debug, Deserialize)]
pub struct VerifyBackupRequest {
    /// Name of the backup to verify (optional - verifies most recent if not specified)
    pub name: Option<String>,
}

/// Verify backup response
#[derive(Debug, Serialize)]
pub struct VerifyBackupResponse {
    pub valid: bool,
    pub backup_name: String,
    pub message: String,
    /// Stats from verified backup (if valid)
    pub accounts: Option<i64>,
    pub transactions: Option<i64>,
    pub subscriptions: Option<i64>,
}

/// POST /api/backup/verify - Verify a backup can be opened
///
/// This tests that a backup is valid by:
/// 1. Decompressing it to a temp location
/// 2. Opening it as a database
/// 3. Running basic queries to verify data
/// 4. Cleaning up the temp file
pub async fn verify_backup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<VerifyBackupRequest>,
) -> Result<Json<VerifyBackupResponse>, AppError> {
    let user_email = get_user_email(&headers);

    let backup_dir = get_backup_dir(&state);

    if !backup_dir.exists() {
        return Err(AppError::not_found("No backups found"));
    }

    let destination = LocalDestination::new(&backup_dir)
        .map_err(|e| AppError::internal(&format!("Failed to access backup directory: {}", e)))?;

    let backups = Database::list_backups(&destination)
        .map_err(|e| AppError::internal(&format!("Failed to list backups: {}", e)))?;

    if backups.is_empty() {
        return Err(AppError::not_found("No backups found"));
    }

    // Find the backup to verify
    let backup = match &req.name {
        Some(name) => backups
            .iter()
            .find(|b| &b.name == name)
            .ok_or_else(|| AppError::not_found("Backup not found"))?,
        None => backups.first().unwrap(), // Already checked not empty
    };

    info!("Verifying backup: {}", backup.name);

    // Create temp directory for verification
    let temp_dir = tempfile::TempDir::new()
        .map_err(|e| AppError::internal(&format!("Failed to create temp directory: {}", e)))?;
    let temp_db_path = temp_dir.path().join("verify.db");

    // Restore to temp location
    if let Err(e) = destination.retrieve(&backup.name, &temp_db_path) {
        state.db.log_audit(
            &user_email,
            "backup_verify_failed",
            Some("backup"),
            None,
            Some(&format!("name={}, error=retrieve_failed", backup.name)),
        )?;

        return Ok(Json(VerifyBackupResponse {
            valid: false,
            backup_name: backup.name.clone(),
            message: format!("Failed to decompress backup: {}", e),
            accounts: None,
            transactions: None,
            subscriptions: None,
        }));
    }

    // Try to open and verify the database
    let verification_result = match Database::new(temp_db_path.to_str().unwrap()) {
        Ok(db) => match db.get_dashboard_stats() {
            Ok(stats) => Ok((
                stats.total_accounts,
                stats.total_transactions,
                stats.active_subscriptions,
            )),
            Err(e) => Err(format!("Failed to read database stats: {}", e)),
        },
        Err(e) => Err(format!("Failed to open database: {}", e)),
    };

    // Log audit
    let (valid, message, accounts, transactions, subscriptions) = match verification_result {
        Ok((a, t, s)) => (
            true,
            format!(
                "Backup verified successfully: {} accounts, {} transactions, {} subscriptions",
                a, t, s
            ),
            Some(a),
            Some(t),
            Some(s),
        ),
        Err(msg) => (false, msg, None, None, None),
    };

    state.db.log_audit(
        &user_email,
        if valid {
            "backup_verified"
        } else {
            "backup_verify_failed"
        },
        Some("backup"),
        None,
        Some(&format!("name={}, valid={}", backup.name, valid)),
    )?;

    // Temp directory automatically cleaned up when dropped

    Ok(Json(VerifyBackupResponse {
        valid,
        backup_name: backup.name.clone(),
        message,
        accounts,
        transactions,
        subscriptions,
    }))
}

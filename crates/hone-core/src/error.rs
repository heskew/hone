//! Error types for Hone

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Database pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Import error: {0}")]
    Import(String),

    #[error("Unsupported bank format: {0}")]
    UnsupportedBank(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Tag error: {0}")]
    Tag(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Backup error: {0}")]
    Backup(String),

    #[error("Training error: {0}")]
    Training(String),
}

pub type Result<T> = std::result::Result<T, Error>;

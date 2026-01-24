//! Hone Core Library
//!
//! Shared functionality for the Hone personal finance tool:
//! - Database access and migrations
//! - CSV import parsers for various banks
//! - Waste detection algorithms
//! - Pluggable local AI backends (Ollama, llama.cpp, etc.)
//! - Model router for task-based model selection
//! - Prompt library for customizable AI prompts
//! - Context assembler for LLM prompt context
//! - Tag assignment engine for automatic categorization
//! - Backup system with pluggable destinations

pub mod ai;
pub mod backup;
pub mod context;
pub mod db;
pub mod detect;
pub mod error;
pub mod export;
pub mod import;
pub mod insights;
pub mod model_router;
pub mod models;
pub mod ollama;
pub mod prompts;
pub mod tags;
pub mod tools;
pub mod training;
pub mod training_pipeline;

/// Test utilities including mock Ollama server
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub use ai::{
    AIBackend, AIClient, AIOrchestrator, AnthropicCompatBackend, DuplicateAnalysis,
    MerchantClassification, MerchantContext, MockBackend, OllamaBackend, OpenAICompatibleBackend,
    ParsedReceipt, ParsedReceiptItem, ReceiptMatchEvaluation, RouterInfo, ServiceFeature,
    SplitRecommendation, SubscriptionClassification,
};
pub use backup::{
    BackupDestination, BackupInfo, BackupResult, LocalDestination, PruneResult, RetentionPolicy,
};
pub use context::{BaselineStats, Context, ContextAssembler, ContextType};
pub use db::{AuditEntry, Database};
pub use error::{Error, Result};
pub use export::{ExportFormat, FullBackup, ImportStats, TransactionExportOptions};
pub use model_router::{ModelRouter, RouterConfig, TaskConfig, TaskType};
pub use prompts::{Prompt, PromptId, PromptInfo, PromptLibrary};
pub use tags::{BackfillResult, TagAssigner, TagAssignment};
pub use training::{TrainingDataGenerator, TrainingExample, TrainingExportStats, TrainingTask};
pub use training_pipeline::{
    ExperimentStatus, PipelineConfig, TrainingExperiment, TrainingPipeline,
};

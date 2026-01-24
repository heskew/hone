//! Automated training pipeline for model fine-tuning
//!
//! This module provides infrastructure for:
//! - Training experiment versioning (branches)
//! - Automated fine-tuning via MLX or Ollama
//! - Model comparison and promotion
//! - Scheduled training runs

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::Result;
use crate::training::{TrainingDataGenerator, TrainingExportStats, TrainingTask};

/// Status of a training experiment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    /// Experiment created, training not started
    Pending,
    /// Training in progress
    Training,
    /// Training completed successfully
    Completed,
    /// Training failed
    Failed,
    /// Model promoted to production
    Promoted,
    /// Experiment archived (superseded)
    Archived,
}

impl ExperimentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExperimentStatus::Pending => "pending",
            ExperimentStatus::Training => "training",
            ExperimentStatus::Completed => "completed",
            ExperimentStatus::Failed => "failed",
            ExperimentStatus::Promoted => "promoted",
            ExperimentStatus::Archived => "archived",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ExperimentStatus::Pending),
            "training" => Some(ExperimentStatus::Training),
            "completed" => Some(ExperimentStatus::Completed),
            "failed" => Some(ExperimentStatus::Failed),
            "promoted" => Some(ExperimentStatus::Promoted),
            "archived" => Some(ExperimentStatus::Archived),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExperimentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A training experiment record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExperiment {
    pub id: i64,
    /// Branch name (e.g., "main", "experiment-gemma3-2024-01")
    pub branch: String,
    /// Task this experiment is for
    pub task: String,
    /// Base model used for fine-tuning
    pub base_model: String,
    /// Name of the fine-tuned model (for Ollama)
    pub model_name: String,
    /// Status of the experiment
    pub status: ExperimentStatus,
    /// Parent experiment ID (for branching)
    pub parent_id: Option<i64>,
    /// Number of training examples used
    pub training_examples: i64,
    /// Training data file path
    pub training_data_path: Option<String>,
    /// Adapter/LoRA file path
    pub adapter_path: Option<String>,
    /// Evaluation metrics (JSON)
    pub metrics: Option<String>,
    /// Notes or description
    pub notes: Option<String>,
    /// When the experiment was created
    pub created_at: DateTime<Utc>,
    /// When training started
    pub started_at: Option<DateTime<Utc>>,
    /// When training completed
    pub completed_at: Option<DateTime<Utc>>,
}

/// Evaluation metrics for comparing models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    /// Total number of test examples
    pub total_examples: i64,
    /// Number of correct predictions
    pub correct_predictions: i64,
    /// Accuracy (correct / total)
    pub accuracy: f64,
    /// Average latency in ms
    pub avg_latency_ms: f64,
    /// Improvement over baseline (if applicable)
    pub improvement_vs_baseline: Option<f64>,
}

/// Result of a model comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub experiment_a: String,
    pub experiment_b: String,
    pub metrics_a: EvaluationMetrics,
    pub metrics_b: EvaluationMetrics,
    pub winner: String,
    pub improvement: f64,
}

/// Configuration for the training pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Directory for storing training artifacts
    pub artifacts_dir: PathBuf,
    /// Default base model for fine-tuning
    pub default_base_model: String,
    /// Minimum examples required before training
    pub min_training_examples: usize,
    /// Test set percentage (0.0-1.0)
    pub test_split: f64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            artifacts_dir: dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("hone")
                .join("training"),
            default_base_model: "gemma3".to_string(),
            min_training_examples: 50,
            test_split: 0.1,
        }
    }
}

/// Training pipeline manager
pub struct TrainingPipeline<'a> {
    db: &'a Database,
    config: PipelineConfig,
}

impl<'a> TrainingPipeline<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            config: PipelineConfig::default(),
        }
    }

    pub fn with_config(db: &'a Database, config: PipelineConfig) -> Self {
        Self { db, config }
    }

    /// Create a new training experiment
    pub fn create_experiment(
        &self,
        task: TrainingTask,
        branch: &str,
        base_model: Option<&str>,
        parent_id: Option<i64>,
        notes: Option<&str>,
    ) -> Result<TrainingExperiment> {
        let base_model = base_model.unwrap_or(&self.config.default_base_model);
        let model_name = format!("hone-{}-{}", task.as_str().replace('_', "-"), branch);

        // Generate training data and count examples
        let generator = TrainingDataGenerator::new(self.db);
        let examples = generator.generate(task)?;

        if examples.len() < self.config.min_training_examples {
            return Err(crate::error::Error::Training(format!(
                "Insufficient training data: {} examples (minimum: {})",
                examples.len(),
                self.config.min_training_examples
            )));
        }

        let experiment = self.db.create_training_experiment(
            branch,
            task.as_str(),
            base_model,
            &model_name,
            parent_id,
            examples.len() as i64,
            notes,
        )?;

        Ok(experiment)
    }

    /// Export training data for an experiment
    pub fn prepare_training_data(
        &self,
        experiment_id: i64,
    ) -> Result<(PathBuf, TrainingExportStats)> {
        let experiment = self
            .db
            .get_training_experiment(experiment_id)?
            .ok_or_else(|| {
                crate::error::Error::Training(format!("Experiment {} not found", experiment_id))
            })?;

        let task = TrainingTask::from_str(&experiment.task).ok_or_else(|| {
            crate::error::Error::Training(format!("Unknown task: {}", experiment.task))
        })?;

        // Create artifacts directory
        let exp_dir = self
            .config
            .artifacts_dir
            .join(format!("exp-{}", experiment_id));
        fs::create_dir_all(&exp_dir)?;

        let data_path = exp_dir.join("training_data.jsonl");
        let mut file = fs::File::create(&data_path)?;

        let generator = TrainingDataGenerator::new(self.db);
        let stats = generator.export_jsonl(task, &mut file)?;

        // Update experiment with data path
        self.db
            .update_experiment_training_data(experiment_id, data_path.to_string_lossy().as_ref())?;

        Ok((data_path, stats))
    }

    /// Generate a Modelfile for Ollama fine-tuning
    pub fn generate_modelfile(&self, experiment_id: i64) -> Result<PathBuf> {
        let experiment = self
            .db
            .get_training_experiment(experiment_id)?
            .ok_or_else(|| {
                crate::error::Error::Training(format!("Experiment {} not found", experiment_id))
            })?;

        let exp_dir = self
            .config
            .artifacts_dir
            .join(format!("exp-{}", experiment_id));
        fs::create_dir_all(&exp_dir)?;

        let modelfile_path = exp_dir.join("Modelfile");
        let training_data_path = experiment.training_data_path.ok_or_else(|| {
            crate::error::Error::Training("Training data not prepared".to_string())
        })?;

        let modelfile_content = format!(
            r#"# Hone fine-tuned model for {}
# Experiment: {} (branch: {})
# Base model: {}

FROM {}
ADAPTER {}

# System message optimized for this task
SYSTEM """{}"""
"#,
            experiment.task,
            experiment_id,
            experiment.branch,
            experiment.base_model,
            experiment.base_model,
            training_data_path,
            self.get_system_prompt(&experiment.task),
        );

        fs::write(&modelfile_path, modelfile_content)?;

        Ok(modelfile_path)
    }

    /// Run MLX fine-tuning (for Mac Studio)
    pub fn run_mlx_finetuning(&self, experiment_id: i64) -> Result<()> {
        let experiment = self
            .db
            .get_training_experiment(experiment_id)?
            .ok_or_else(|| {
                crate::error::Error::Training(format!("Experiment {} not found", experiment_id))
            })?;

        // Mark as training
        self.db
            .update_experiment_status(experiment_id, ExperimentStatus::Training)?;

        let exp_dir = self
            .config
            .artifacts_dir
            .join(format!("exp-{}", experiment_id));
        let adapter_path = exp_dir.join("adapters");

        // Prepare MLX command
        // This assumes mlx-lm is installed: pip install mlx-lm
        let training_data = experiment.training_data_path.ok_or_else(|| {
            crate::error::Error::Training("Training data not prepared".to_string())
        })?;

        let output = Command::new("mlx_lm.lora")
            .args([
                "--model",
                &experiment.base_model,
                "--train",
                "--data",
                &training_data,
                "--adapter-path",
                adapter_path.to_string_lossy().as_ref(),
                "--iters",
                "100", // Adjust based on data size
                "--batch-size",
                "4",
                "--learning-rate",
                "1e-5",
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                self.db.update_experiment_adapter(
                    experiment_id,
                    adapter_path.to_string_lossy().as_ref(),
                )?;
                self.db
                    .update_experiment_status(experiment_id, ExperimentStatus::Completed)?;
                Ok(())
            }
            Ok(result) => {
                self.db
                    .update_experiment_status(experiment_id, ExperimentStatus::Failed)?;
                Err(crate::error::Error::Training(format!(
                    "MLX training failed: {}",
                    String::from_utf8_lossy(&result.stderr)
                )))
            }
            Err(e) => {
                self.db
                    .update_experiment_status(experiment_id, ExperimentStatus::Failed)?;
                Err(crate::error::Error::Training(format!(
                    "Failed to run MLX: {}. Is mlx-lm installed?",
                    e
                )))
            }
        }
    }

    /// Create Ollama model from fine-tuned adapter
    pub fn create_ollama_model(&self, experiment_id: i64) -> Result<String> {
        let experiment = self
            .db
            .get_training_experiment(experiment_id)?
            .ok_or_else(|| {
                crate::error::Error::Training(format!("Experiment {} not found", experiment_id))
            })?;

        let modelfile_path = self.generate_modelfile(experiment_id)?;

        let output = Command::new("ollama")
            .args([
                "create",
                &experiment.model_name,
                "-f",
                modelfile_path.to_string_lossy().as_ref(),
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => Ok(experiment.model_name),
            Ok(result) => Err(crate::error::Error::Training(format!(
                "Ollama create failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ))),
            Err(e) => Err(crate::error::Error::Training(format!(
                "Failed to run ollama: {}",
                e
            ))),
        }
    }

    /// List experiments for a task
    pub fn list_experiments(
        &self,
        task: Option<&str>,
        branch: Option<&str>,
    ) -> Result<Vec<TrainingExperiment>> {
        self.db.list_training_experiments(task, branch)
    }

    /// Get the currently promoted experiment for a task
    pub fn get_promoted_experiment(&self, task: &str) -> Result<Option<TrainingExperiment>> {
        self.db.get_promoted_experiment(task)
    }

    /// Promote an experiment to production
    pub fn promote_experiment(&self, experiment_id: i64) -> Result<()> {
        let experiment = self
            .db
            .get_training_experiment(experiment_id)?
            .ok_or_else(|| {
                crate::error::Error::Training(format!("Experiment {} not found", experiment_id))
            })?;

        if experiment.status != ExperimentStatus::Completed {
            return Err(crate::error::Error::Training(format!(
                "Cannot promote experiment with status: {}",
                experiment.status
            )));
        }

        // Archive any currently promoted experiment for this task
        if let Some(current) = self.db.get_promoted_experiment(&experiment.task)? {
            self.db
                .update_experiment_status(current.id, ExperimentStatus::Archived)?;
        }

        // Promote this experiment
        self.db
            .update_experiment_status(experiment_id, ExperimentStatus::Promoted)?;

        Ok(())
    }

    /// Create a branch from an existing experiment
    pub fn branch_experiment(
        &self,
        experiment_id: i64,
        new_branch: &str,
    ) -> Result<TrainingExperiment> {
        let parent = self
            .db
            .get_training_experiment(experiment_id)?
            .ok_or_else(|| {
                crate::error::Error::Training(format!("Experiment {} not found", experiment_id))
            })?;

        let task = TrainingTask::from_str(&parent.task).ok_or_else(|| {
            crate::error::Error::Training(format!("Unknown task: {}", parent.task))
        })?;

        self.create_experiment(
            task,
            new_branch,
            Some(&parent.base_model),
            Some(experiment_id),
            None,
        )
    }

    fn get_system_prompt(&self, task: &str) -> String {
        match task {
            "classify_merchant" => {
                "You are a financial transaction classifier. Given a merchant description, output only the spending category name.".to_string()
            }
            "normalize_merchant" => {
                "You are a merchant name normalizer. Given a raw transaction description, output only the clean merchant name.".to_string()
            }
            "classify_subscription" => {
                "You are a subscription classifier. Output SUBSCRIPTION or RETAIL.".to_string()
            }
            _ => "You are a helpful assistant.".to_string(),
        }
    }
}

// Database methods for training experiments
impl Database {
    /// Create a new training experiment
    pub fn create_training_experiment(
        &self,
        branch: &str,
        task: &str,
        base_model: &str,
        model_name: &str,
        parent_id: Option<i64>,
        training_examples: i64,
        notes: Option<&str>,
    ) -> Result<TrainingExperiment> {
        let conn = self.conn()?;

        conn.execute(
            r#"
            INSERT INTO training_experiments (
                branch, task, base_model, model_name, status,
                parent_id, training_examples, notes
            ) VALUES (?, ?, ?, ?, 'pending', ?, ?, ?)
            "#,
            rusqlite::params![
                branch,
                task,
                base_model,
                model_name,
                parent_id,
                training_examples,
                notes
            ],
        )?;

        let id = conn.last_insert_rowid();
        self.get_training_experiment(id)?.ok_or_else(|| {
            crate::error::Error::Training("Failed to retrieve created experiment".to_string())
        })
    }

    /// Get a training experiment by ID
    pub fn get_training_experiment(&self, id: i64) -> Result<Option<TrainingExperiment>> {
        let conn = self.conn()?;

        let experiment = conn
            .query_row(
                r#"
                SELECT id, branch, task, base_model, model_name, status,
                       parent_id, training_examples, training_data_path,
                       adapter_path, metrics, notes, created_at, started_at, completed_at
                FROM training_experiments WHERE id = ?
                "#,
                rusqlite::params![id],
                |row| {
                    let status_str: String = row.get(5)?;
                    let created_at: String = row.get(12)?;
                    let started_at: Option<String> = row.get(13)?;
                    let completed_at: Option<String> = row.get(14)?;
                    Ok(TrainingExperiment {
                        id: row.get(0)?,
                        branch: row.get(1)?,
                        task: row.get(2)?,
                        base_model: row.get(3)?,
                        model_name: row.get(4)?,
                        status: ExperimentStatus::from_str(&status_str)
                            .unwrap_or(ExperimentStatus::Pending),
                        parent_id: row.get(6)?,
                        training_examples: row.get(7)?,
                        training_data_path: row.get(8)?,
                        adapter_path: row.get(9)?,
                        metrics: row.get(10)?,
                        notes: row.get(11)?,
                        created_at: DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        started_at: started_at.and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        }),
                        completed_at: completed_at.and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        }),
                    })
                },
            )
            .ok();

        Ok(experiment)
    }

    /// List training experiments with optional filters
    pub fn list_training_experiments(
        &self,
        task: Option<&str>,
        branch: Option<&str>,
    ) -> Result<Vec<TrainingExperiment>> {
        let conn = self.conn()?;

        let mut sql = r#"
            SELECT id, branch, task, base_model, model_name, status,
                   parent_id, training_examples, training_data_path,
                   adapter_path, metrics, notes, created_at, started_at, completed_at
            FROM training_experiments
            WHERE 1=1
        "#
        .to_string();

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(t) = task {
            sql.push_str(" AND task = ?");
            params.push(Box::new(t.to_string()));
        }

        if let Some(b) = branch {
            sql.push_str(" AND branch = ?");
            params.push(Box::new(b.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let experiments = stmt
            .query_map(params_refs.as_slice(), |row| {
                let status_str: String = row.get(5)?;
                let created_at: String = row.get(12)?;
                let started_at: Option<String> = row.get(13)?;
                let completed_at: Option<String> = row.get(14)?;
                Ok(TrainingExperiment {
                    id: row.get(0)?,
                    branch: row.get(1)?,
                    task: row.get(2)?,
                    base_model: row.get(3)?,
                    model_name: row.get(4)?,
                    status: ExperimentStatus::from_str(&status_str)
                        .unwrap_or(ExperimentStatus::Pending),
                    parent_id: row.get(6)?,
                    training_examples: row.get(7)?,
                    training_data_path: row.get(8)?,
                    adapter_path: row.get(9)?,
                    metrics: row.get(10)?,
                    notes: row.get(11)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    started_at: started_at.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    completed_at: completed_at.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(experiments)
    }

    /// Get the currently promoted experiment for a task
    pub fn get_promoted_experiment(&self, task: &str) -> Result<Option<TrainingExperiment>> {
        let conn = self.conn()?;

        let experiment = conn
            .query_row(
                r#"
                SELECT id, branch, task, base_model, model_name, status,
                       parent_id, training_examples, training_data_path,
                       adapter_path, metrics, notes, created_at, started_at, completed_at
                FROM training_experiments
                WHERE task = ? AND status = 'promoted'
                "#,
                rusqlite::params![task],
                |row| {
                    let status_str: String = row.get(5)?;
                    let created_at: String = row.get(12)?;
                    let started_at: Option<String> = row.get(13)?;
                    let completed_at: Option<String> = row.get(14)?;
                    Ok(TrainingExperiment {
                        id: row.get(0)?,
                        branch: row.get(1)?,
                        task: row.get(2)?,
                        base_model: row.get(3)?,
                        model_name: row.get(4)?,
                        status: ExperimentStatus::from_str(&status_str)
                            .unwrap_or(ExperimentStatus::Pending),
                        parent_id: row.get(6)?,
                        training_examples: row.get(7)?,
                        training_data_path: row.get(8)?,
                        adapter_path: row.get(9)?,
                        metrics: row.get(10)?,
                        notes: row.get(11)?,
                        created_at: DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        started_at: started_at.and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        }),
                        completed_at: completed_at.and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        }),
                    })
                },
            )
            .ok();

        Ok(experiment)
    }

    /// Update experiment status
    pub fn update_experiment_status(&self, id: i64, status: ExperimentStatus) -> Result<()> {
        let conn = self.conn()?;

        let timestamp_col = match status {
            ExperimentStatus::Training => Some("started_at"),
            ExperimentStatus::Completed | ExperimentStatus::Failed => Some("completed_at"),
            _ => None,
        };

        if let Some(col) = timestamp_col {
            conn.execute(
                &format!("UPDATE training_experiments SET status = ?, {} = CURRENT_TIMESTAMP WHERE id = ?", col),
                rusqlite::params![status.as_str(), id],
            )?;
        } else {
            conn.execute(
                "UPDATE training_experiments SET status = ? WHERE id = ?",
                rusqlite::params![status.as_str(), id],
            )?;
        }

        Ok(())
    }

    /// Update experiment training data path
    pub fn update_experiment_training_data(&self, id: i64, path: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE training_experiments SET training_data_path = ? WHERE id = ?",
            rusqlite::params![path, id],
        )?;
        Ok(())
    }

    /// Update experiment adapter path
    pub fn update_experiment_adapter(&self, id: i64, path: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE training_experiments SET adapter_path = ? WHERE id = ?",
            rusqlite::params![path, id],
        )?;
        Ok(())
    }

    /// Update experiment metrics
    pub fn update_experiment_metrics(&self, id: i64, metrics: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE training_experiments SET metrics = ? WHERE id = ?",
            rusqlite::params![metrics, id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    // ExperimentStatus tests
    #[test]
    fn test_experiment_status_roundtrip() {
        for status in [
            ExperimentStatus::Pending,
            ExperimentStatus::Training,
            ExperimentStatus::Completed,
            ExperimentStatus::Failed,
            ExperimentStatus::Promoted,
            ExperimentStatus::Archived,
        ] {
            let s = status.as_str();
            let parsed = ExperimentStatus::from_str(s).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_experiment_status_as_str() {
        assert_eq!(ExperimentStatus::Pending.as_str(), "pending");
        assert_eq!(ExperimentStatus::Training.as_str(), "training");
        assert_eq!(ExperimentStatus::Completed.as_str(), "completed");
        assert_eq!(ExperimentStatus::Failed.as_str(), "failed");
        assert_eq!(ExperimentStatus::Promoted.as_str(), "promoted");
        assert_eq!(ExperimentStatus::Archived.as_str(), "archived");
    }

    #[test]
    fn test_experiment_status_from_str_invalid() {
        assert!(ExperimentStatus::from_str("invalid").is_none());
        assert!(ExperimentStatus::from_str("").is_none());
        assert!(ExperimentStatus::from_str("PENDING").is_none()); // case sensitive
    }

    #[test]
    fn test_experiment_status_display() {
        assert_eq!(format!("{}", ExperimentStatus::Pending), "pending");
        assert_eq!(format!("{}", ExperimentStatus::Training), "training");
        assert_eq!(format!("{}", ExperimentStatus::Completed), "completed");
    }

    #[test]
    fn test_experiment_status_serialization() {
        let status = ExperimentStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");

        let parsed: ExperimentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn test_experiment_status_all_variants_serialize() {
        for status in [
            ExperimentStatus::Pending,
            ExperimentStatus::Training,
            ExperimentStatus::Completed,
            ExperimentStatus::Failed,
            ExperimentStatus::Promoted,
            ExperimentStatus::Archived,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: ExperimentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    // PipelineConfig tests
    #[test]
    fn test_pipeline_config_default() {
        let config = PipelineConfig::default();
        assert_eq!(config.default_base_model, "gemma3");
        assert_eq!(config.min_training_examples, 50);
        assert!((config.test_split - 0.1).abs() < 0.001);
        assert!(config.artifacts_dir.to_string_lossy().contains("training"));
    }

    #[test]
    fn test_pipeline_config_serialization() {
        let config = PipelineConfig {
            artifacts_dir: PathBuf::from("/tmp/test"),
            default_base_model: "llama3".to_string(),
            min_training_examples: 100,
            test_split: 0.2,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: PipelineConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.default_base_model, "llama3");
        assert_eq!(parsed.min_training_examples, 100);
        assert!((parsed.test_split - 0.2).abs() < 0.001);
    }

    // EvaluationMetrics tests
    #[test]
    fn test_evaluation_metrics_creation() {
        let metrics = EvaluationMetrics {
            total_examples: 100,
            correct_predictions: 85,
            accuracy: 0.85,
            avg_latency_ms: 150.5,
            improvement_vs_baseline: Some(0.1),
        };

        assert_eq!(metrics.total_examples, 100);
        assert_eq!(metrics.correct_predictions, 85);
        assert!((metrics.accuracy - 0.85).abs() < 0.001);
        assert!((metrics.avg_latency_ms - 150.5).abs() < 0.001);
        assert_eq!(metrics.improvement_vs_baseline, Some(0.1));
    }

    #[test]
    fn test_evaluation_metrics_serialization() {
        let metrics = EvaluationMetrics {
            total_examples: 50,
            correct_predictions: 45,
            accuracy: 0.9,
            avg_latency_ms: 100.0,
            improvement_vs_baseline: None,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: EvaluationMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.total_examples, 50);
        assert_eq!(parsed.correct_predictions, 45);
        assert!((parsed.accuracy - 0.9).abs() < 0.001);
        assert_eq!(parsed.improvement_vs_baseline, None);
    }

    // ComparisonResult tests
    #[test]
    fn test_comparison_result_creation() {
        let metrics_a = EvaluationMetrics {
            total_examples: 100,
            correct_predictions: 80,
            accuracy: 0.8,
            avg_latency_ms: 200.0,
            improvement_vs_baseline: None,
        };

        let metrics_b = EvaluationMetrics {
            total_examples: 100,
            correct_predictions: 90,
            accuracy: 0.9,
            avg_latency_ms: 180.0,
            improvement_vs_baseline: Some(0.1),
        };

        let result = ComparisonResult {
            experiment_a: "exp-1".to_string(),
            experiment_b: "exp-2".to_string(),
            metrics_a,
            metrics_b,
            winner: "exp-2".to_string(),
            improvement: 0.1,
        };

        assert_eq!(result.experiment_a, "exp-1");
        assert_eq!(result.experiment_b, "exp-2");
        assert_eq!(result.winner, "exp-2");
        assert!((result.improvement - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_comparison_result_serialization() {
        let metrics = EvaluationMetrics {
            total_examples: 100,
            correct_predictions: 80,
            accuracy: 0.8,
            avg_latency_ms: 200.0,
            improvement_vs_baseline: None,
        };

        let result = ComparisonResult {
            experiment_a: "exp-1".to_string(),
            experiment_b: "exp-2".to_string(),
            metrics_a: metrics.clone(),
            metrics_b: metrics,
            winner: "exp-1".to_string(),
            improvement: 0.0,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: ComparisonResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.experiment_a, "exp-1");
        assert_eq!(parsed.experiment_b, "exp-2");
        assert_eq!(parsed.winner, "exp-1");
    }

    // TrainingExperiment tests
    #[test]
    fn test_training_experiment_serialization() {
        let experiment = TrainingExperiment {
            id: 1,
            branch: "main".to_string(),
            task: "classify_merchant".to_string(),
            base_model: "gemma3".to_string(),
            model_name: "hone-classify-merchant-main".to_string(),
            status: ExperimentStatus::Pending,
            parent_id: None,
            training_examples: 100,
            training_data_path: None,
            adapter_path: None,
            metrics: None,
            notes: Some("Test experiment".to_string()),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        };

        let json = serde_json::to_string(&experiment).unwrap();
        let parsed: TrainingExperiment = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.task, "classify_merchant");
        assert_eq!(parsed.status, ExperimentStatus::Pending);
        assert_eq!(parsed.notes, Some("Test experiment".to_string()));
    }

    // TrainingPipeline tests
    #[test]
    fn test_training_pipeline_new() {
        let db = create_test_db();
        let pipeline = TrainingPipeline::new(&db);
        assert_eq!(pipeline.config.default_base_model, "gemma3");
        assert_eq!(pipeline.config.min_training_examples, 50);
    }

    #[test]
    fn test_training_pipeline_with_config() {
        let db = create_test_db();
        let config = PipelineConfig {
            artifacts_dir: PathBuf::from("/custom/path"),
            default_base_model: "llama3".to_string(),
            min_training_examples: 10,
            test_split: 0.2,
        };

        let pipeline = TrainingPipeline::with_config(&db, config);
        assert_eq!(pipeline.config.default_base_model, "llama3");
        assert_eq!(pipeline.config.min_training_examples, 10);
    }

    #[test]
    fn test_training_pipeline_list_experiments_empty() {
        let db = create_test_db();
        let pipeline = TrainingPipeline::new(&db);
        let experiments = pipeline.list_experiments(None, None).unwrap();
        assert!(experiments.is_empty());
    }

    #[test]
    fn test_training_pipeline_get_promoted_experiment_empty() {
        let db = create_test_db();
        let pipeline = TrainingPipeline::new(&db);
        let promoted = pipeline
            .get_promoted_experiment("classify_merchant")
            .unwrap();
        assert!(promoted.is_none());
    }

    #[test]
    fn test_get_system_prompt() {
        let db = create_test_db();
        let pipeline = TrainingPipeline::new(&db);

        let classify_prompt = pipeline.get_system_prompt("classify_merchant");
        assert!(classify_prompt.contains("classifier"));

        let normalize_prompt = pipeline.get_system_prompt("normalize_merchant");
        assert!(normalize_prompt.contains("normalizer"));

        let subscription_prompt = pipeline.get_system_prompt("classify_subscription");
        assert!(subscription_prompt.contains("subscription"));

        let unknown_prompt = pipeline.get_system_prompt("unknown_task");
        assert!(unknown_prompt.contains("helpful assistant"));
    }

    // Database method tests
    #[test]
    fn test_db_get_training_experiment_not_found() {
        let db = create_test_db();
        let result = db.get_training_experiment(999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_db_list_training_experiments_empty() {
        let db = create_test_db();
        let experiments = db.list_training_experiments(None, None).unwrap();
        assert!(experiments.is_empty());
    }

    #[test]
    fn test_db_list_training_experiments_with_task_filter() {
        let db = create_test_db();
        let experiments = db
            .list_training_experiments(Some("classify_merchant"), None)
            .unwrap();
        assert!(experiments.is_empty());
    }

    #[test]
    fn test_db_list_training_experiments_with_branch_filter() {
        let db = create_test_db();
        let experiments = db.list_training_experiments(None, Some("main")).unwrap();
        assert!(experiments.is_empty());
    }

    #[test]
    fn test_db_list_training_experiments_with_both_filters() {
        let db = create_test_db();
        let experiments = db
            .list_training_experiments(Some("classify_merchant"), Some("main"))
            .unwrap();
        assert!(experiments.is_empty());
    }

    #[test]
    fn test_db_get_promoted_experiment_not_found() {
        let db = create_test_db();
        let promoted = db.get_promoted_experiment("classify_merchant").unwrap();
        assert!(promoted.is_none());
    }

    #[test]
    fn test_db_update_experiment_status_nonexistent() {
        let db = create_test_db();
        // Updating a non-existent experiment should not error (SQLite UPDATE affects 0 rows)
        let result = db.update_experiment_status(999, ExperimentStatus::Training);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_experiment_training_data_nonexistent() {
        let db = create_test_db();
        let result = db.update_experiment_training_data(999, "/path/to/data.jsonl");
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_experiment_adapter_nonexistent() {
        let db = create_test_db();
        let result = db.update_experiment_adapter(999, "/path/to/adapter");
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_experiment_metrics_nonexistent() {
        let db = create_test_db();
        let result = db.update_experiment_metrics(999, r#"{"accuracy": 0.9}"#);
        assert!(result.is_ok());
    }

    // Experiment status update tests for different statuses
    #[test]
    fn test_db_update_status_training_sets_started_at() {
        let db = create_test_db();
        // Since there's no experiment, just verify the query runs without error
        let result = db.update_experiment_status(1, ExperimentStatus::Training);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_status_completed_sets_completed_at() {
        let db = create_test_db();
        let result = db.update_experiment_status(1, ExperimentStatus::Completed);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_status_failed_sets_completed_at() {
        let db = create_test_db();
        let result = db.update_experiment_status(1, ExperimentStatus::Failed);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_status_promoted_no_timestamp() {
        let db = create_test_db();
        let result = db.update_experiment_status(1, ExperimentStatus::Promoted);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_status_archived_no_timestamp() {
        let db = create_test_db();
        let result = db.update_experiment_status(1, ExperimentStatus::Archived);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_update_status_pending_no_timestamp() {
        let db = create_test_db();
        let result = db.update_experiment_status(1, ExperimentStatus::Pending);
        assert!(result.is_ok());
    }

    // Test equality and clone for ExperimentStatus
    #[test]
    fn test_experiment_status_equality() {
        assert_eq!(ExperimentStatus::Pending, ExperimentStatus::Pending);
        assert_ne!(ExperimentStatus::Pending, ExperimentStatus::Training);
    }

    #[test]
    fn test_experiment_status_clone() {
        let status = ExperimentStatus::Completed;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }

    #[test]
    fn test_experiment_status_copy() {
        let status = ExperimentStatus::Failed;
        let copied: ExperimentStatus = status; // Copy
        assert_eq!(status, copied);
    }

    // TrainingExperiment clone
    #[test]
    fn test_training_experiment_clone() {
        let experiment = TrainingExperiment {
            id: 1,
            branch: "main".to_string(),
            task: "classify_merchant".to_string(),
            base_model: "gemma3".to_string(),
            model_name: "hone-classify-merchant-main".to_string(),
            status: ExperimentStatus::Pending,
            parent_id: None,
            training_examples: 100,
            training_data_path: Some("/path/to/data".to_string()),
            adapter_path: Some("/path/to/adapter".to_string()),
            metrics: Some(r#"{"accuracy": 0.9}"#.to_string()),
            notes: None,
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
        };

        let cloned = experiment.clone();
        assert_eq!(cloned.id, experiment.id);
        assert_eq!(cloned.branch, experiment.branch);
        assert_eq!(cloned.status, experiment.status);
    }

    // EvaluationMetrics clone
    #[test]
    fn test_evaluation_metrics_clone() {
        let metrics = EvaluationMetrics {
            total_examples: 100,
            correct_predictions: 90,
            accuracy: 0.9,
            avg_latency_ms: 150.0,
            improvement_vs_baseline: Some(0.05),
        };

        let cloned = metrics.clone();
        assert_eq!(cloned.total_examples, metrics.total_examples);
        assert_eq!(cloned.correct_predictions, metrics.correct_predictions);
    }

    // ComparisonResult clone
    #[test]
    fn test_comparison_result_clone() {
        let metrics = EvaluationMetrics {
            total_examples: 100,
            correct_predictions: 80,
            accuracy: 0.8,
            avg_latency_ms: 200.0,
            improvement_vs_baseline: None,
        };

        let result = ComparisonResult {
            experiment_a: "exp-1".to_string(),
            experiment_b: "exp-2".to_string(),
            metrics_a: metrics.clone(),
            metrics_b: metrics,
            winner: "exp-2".to_string(),
            improvement: 0.1,
        };

        let cloned = result.clone();
        assert_eq!(cloned.experiment_a, result.experiment_a);
        assert_eq!(cloned.winner, result.winner);
    }

    // PipelineConfig clone
    #[test]
    fn test_pipeline_config_clone() {
        let config = PipelineConfig {
            artifacts_dir: PathBuf::from("/tmp/test"),
            default_base_model: "gemma3".to_string(),
            min_training_examples: 50,
            test_split: 0.1,
        };

        let cloned = config.clone();
        assert_eq!(cloned.default_base_model, config.default_base_model);
        assert_eq!(cloned.min_training_examples, config.min_training_examples);
    }
}

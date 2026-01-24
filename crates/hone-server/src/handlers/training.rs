//! Training data export API handlers
//!
//! Endpoints for exporting training data to be used for fine-tuning models.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use hone_core::training::{TrainingDataGenerator, TrainingTask};
use hone_core::training_pipeline::{PipelineConfig, TrainingPipeline};

use crate::{AppError, AppState};

/// Query params for training export
#[derive(Debug, Deserialize)]
pub struct TrainingExportQuery {
    /// Task to export: classify_merchant, normalize_merchant, classify_subscription
    pub task: String,
}

/// Training task info
#[derive(Debug, Serialize)]
pub struct TaskInfo {
    pub task: String,
    pub total_examples: usize,
    pub user_corrections: usize,
    pub ollama_confirmed: usize,
    pub ready: bool,
    pub min_required: usize,
}

/// Training tasks list response
#[derive(Debug, Serialize)]
pub struct TrainingTasksResponse {
    pub tasks: Vec<TaskInfo>,
}

/// Training agent status response
#[derive(Debug, Serialize)]
pub struct TrainingAgentResponse {
    pub tasks: Vec<TaskStatus>,
    pub recommendations: Vec<String>,
    pub all_up_to_date: bool,
}

/// Per-task status for agent report
#[derive(Debug, Serialize)]
pub struct TaskStatus {
    pub task: String,
    pub total_examples: usize,
    pub user_corrections: usize,
    pub ready: bool,
    pub promoted_experiment_id: Option<i64>,
    pub promoted_examples: Option<i64>,
    pub data_growth_percent: Option<f64>,
    pub needs_retraining: bool,
}

/// List available training tasks with data counts
///
/// GET /api/training/tasks
pub async fn training_tasks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TrainingTasksResponse>, AppError> {
    let generator = TrainingDataGenerator::new(&state.db);
    let config = PipelineConfig::default();

    let mut tasks = Vec::new();

    for task in TrainingTask::all() {
        let examples = generator.generate(task)?;
        let user_count = examples
            .iter()
            .filter(|e| {
                e.source == "user" || e.source == "user_correction" || e.source == "user_override"
            })
            .count();

        tasks.push(TaskInfo {
            task: task.as_str().to_string(),
            total_examples: examples.len(),
            user_corrections: user_count,
            ollama_confirmed: examples.len() - user_count,
            ready: examples.len() >= config.min_training_examples,
            min_required: config.min_training_examples,
        });
    }

    Ok(Json(TrainingTasksResponse { tasks }))
}

/// Export training data as JSONL
///
/// GET /api/training/export?task=classify_merchant
pub async fn training_export(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TrainingExportQuery>,
) -> Result<Response, AppError> {
    let task = TrainingTask::from_str(&params.task).ok_or_else(|| {
        AppError::bad_request(&format!(
            "Unknown task: {}. Valid tasks: classify_merchant, normalize_merchant, classify_subscription",
            params.task
        ))
    })?;

    let generator = TrainingDataGenerator::new(&state.db);

    // Generate JSONL
    let mut output = Vec::new();
    let stats = generator.export_jsonl(task, &mut output)?;

    if stats.total_examples == 0 {
        return Err(AppError::not_found(&format!(
            "No training data available for task: {}",
            params.task
        )));
    }

    // Return as downloadable JSONL file
    let filename = format!("{}_training.jsonl", params.task);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        output,
    )
        .into_response())
}

/// Get training agent status report
///
/// GET /api/training/agent
pub async fn training_agent(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TrainingAgentResponse>, AppError> {
    let generator = TrainingDataGenerator::new(&state.db);
    let pipeline = TrainingPipeline::new(&state.db);
    let config = PipelineConfig::default();

    let mut tasks = Vec::new();
    let mut recommendations = Vec::new();

    for task in TrainingTask::all() {
        let examples = generator.generate(task)?;
        let user_count = examples
            .iter()
            .filter(|e| {
                e.source == "user" || e.source == "user_correction" || e.source == "user_override"
            })
            .count();

        let ready = examples.len() >= config.min_training_examples;

        let (promoted_id, promoted_examples, data_growth, needs_retraining) = if ready {
            if let Some(exp) = pipeline.get_promoted_experiment(task.as_str())? {
                let growth = examples.len() as f64 / exp.training_examples as f64;
                let needs_retrain = growth > 1.2;

                if needs_retrain {
                    recommendations.push(format!(
                        "Consider retraining {} - data grew by {:.0}%",
                        task,
                        (growth - 1.0) * 100.0
                    ));
                }

                (
                    Some(exp.id),
                    Some(exp.training_examples),
                    Some((growth - 1.0) * 100.0),
                    needs_retrain,
                )
            } else {
                recommendations.push(format!(
                    "Create first experiment for {}: fetch training data and run fine-tuning",
                    task
                ));
                (None, None, None, false)
            }
        } else {
            (None, None, None, false)
        };

        tasks.push(TaskStatus {
            task: task.as_str().to_string(),
            total_examples: examples.len(),
            user_corrections: user_count,
            ready,
            promoted_experiment_id: promoted_id,
            promoted_examples,
            data_growth_percent: data_growth,
            needs_retraining,
        });
    }

    let all_up_to_date = recommendations.is_empty()
        && tasks
            .iter()
            .all(|t| t.ready && t.promoted_experiment_id.is_some());

    Ok(Json(TrainingAgentResponse {
        tasks,
        recommendations,
        all_up_to_date,
    }))
}

/// Get training data stats
///
/// GET /api/training/stats
pub async fn training_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TrainingTasksResponse>, AppError> {
    // Same as tasks endpoint for now
    training_tasks(State(state)).await
}

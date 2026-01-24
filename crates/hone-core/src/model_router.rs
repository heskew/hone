//! Model Router for task-based model selection
//!
//! Routes different types of tasks to appropriate models based on configuration.
//! Supports:
//! - Task-based routing (different models for classification vs reasoning)
//! - Fallback on failure
//! - Health tracking (consecutive failures trigger fallback)
//! - Config-driven customization via override files
//!
//! ## Configuration Resolution
//!
//! Config is loaded with a two-layer resolution:
//! 1. Check for override in data dir (~/.local/share/hone/config/models.toml)
//! 2. Fall back to embedded defaults (compiled into binary)

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Embedded default config (compiled into binary)
const DEFAULT_CONFIG: &str = include_str!("../../../config/models.toml");

/// Task types for model routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskType {
    /// Quick decisions (merchant category, subscription detection)
    FastClassification,
    /// JSON output (receipt parsing)
    StructuredExtraction,
    /// Analysis requiring explanation (spending anomalies, duplicates)
    Reasoning,
    /// Image processing (receipt OCR)
    Vision,
    /// Text generation (reports, summaries)
    Narrative,
}

impl TaskType {
    /// Get the config key for this task type
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FastClassification => "fast_classification",
            Self::StructuredExtraction => "structured_extraction",
            Self::Reasoning => "reasoning",
            Self::Vision => "vision",
            Self::Narrative => "narrative",
        }
    }

    /// Get all task types
    pub fn all() -> &'static [TaskType] {
        &[
            Self::FastClassification,
            Self::StructuredExtraction,
            Self::Reasoning,
            Self::Vision,
            Self::Narrative,
        ]
    }
}

/// Configuration for a specific task type
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// Model to use for this task
    pub model: String,
    /// Timeout for API calls
    pub timeout: Duration,
    /// Number of retries on failure
    pub max_retries: u32,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            model: "gemma3".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 2,
        }
    }
}

/// Health status of a model
#[derive(Debug)]
struct ModelHealth {
    /// Number of consecutive failures
    failures: AtomicU32,
    /// When the model was marked unhealthy
    unhealthy_since: Option<Instant>,
}

impl ModelHealth {
    fn new() -> Self {
        Self {
            failures: AtomicU32::new(0),
            unhealthy_since: None,
        }
    }

    fn record_success(&self) {
        self.failures.store(0, Ordering::SeqCst);
    }

    fn record_failure(&self) -> u32 {
        self.failures.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn failure_count(&self) -> u32 {
        self.failures.load(Ordering::SeqCst)
    }
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Default model for all task types
    pub default_model: String,
    /// Default timeout
    pub default_timeout: Duration,
    /// Default retries
    pub default_retries: u32,
    /// Fallback model if primary fails
    pub fallback_model: Option<String>,
    /// Per-task configurations
    pub tasks: HashMap<TaskType, TaskConfig>,
    /// Failure threshold before marking unhealthy
    pub failure_threshold: u32,
    /// Time to wait before retrying unhealthy model
    pub recovery_wait: Duration,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            default_model: "gemma3".to_string(),
            default_timeout: Duration::from_secs(30),
            default_retries: 2,
            fallback_model: Some("llama3.2".to_string()),
            tasks: HashMap::new(),
            failure_threshold: 3,
            recovery_wait: Duration::from_secs(300),
        }
    }
}

/// Model Router for task-based model selection
pub struct ModelRouter {
    config: RouterConfig,
    health: HashMap<String, ModelHealth>,
    config_path: Option<PathBuf>,
}

impl ModelRouter {
    /// Create a new model router with default configuration
    pub fn new() -> Result<Self> {
        let config = load_config(None)?;
        Ok(Self {
            config,
            health: HashMap::new(),
            config_path: default_config_path(),
        })
    }

    /// Create with a custom config path
    pub fn with_config_path(path: PathBuf) -> Result<Self> {
        let config = load_config(Some(&path))?;
        Ok(Self {
            config,
            health: HashMap::new(),
            config_path: Some(path),
        })
    }

    /// Create with an explicit configuration (for testing)
    pub fn with_config(config: RouterConfig) -> Self {
        Self {
            config,
            health: HashMap::new(),
            config_path: None,
        }
    }

    /// Get the model to use for a task type
    pub fn model_for_task(&self, task: TaskType) -> &str {
        // Check for task-specific config
        if let Some(task_config) = self.config.tasks.get(&task) {
            // Check if model is healthy
            if self.is_healthy(&task_config.model) {
                return &task_config.model;
            }
        }

        // Check if default model is healthy
        if self.is_healthy(&self.config.default_model) {
            return &self.config.default_model;
        }

        // Try fallback
        if let Some(ref fallback) = self.config.fallback_model {
            if self.is_healthy(fallback) {
                return fallback;
            }
        }

        // Return default even if unhealthy (let it fail)
        &self.config.default_model
    }

    /// Get the full task configuration
    pub fn config_for_task(&self, task: TaskType) -> TaskConfig {
        self.config
            .tasks
            .get(&task)
            .cloned()
            .unwrap_or_else(|| TaskConfig {
                model: self.config.default_model.clone(),
                timeout: self.config.default_timeout,
                max_retries: self.config.default_retries,
            })
    }

    /// Get the timeout for a task
    pub fn timeout_for_task(&self, task: TaskType) -> Duration {
        self.config
            .tasks
            .get(&task)
            .map(|c| c.timeout)
            .unwrap_or(self.config.default_timeout)
    }

    /// Get the retry count for a task
    pub fn retries_for_task(&self, task: TaskType) -> u32 {
        self.config
            .tasks
            .get(&task)
            .map(|c| c.max_retries)
            .unwrap_or(self.config.default_retries)
    }

    /// Record a successful call
    pub fn record_success(&mut self, model: &str) {
        self.health
            .entry(model.to_string())
            .or_insert_with(ModelHealth::new)
            .record_success();
    }

    /// Record a failed call, returns true if model is now unhealthy
    pub fn record_failure(&mut self, model: &str) -> bool {
        let health = self
            .health
            .entry(model.to_string())
            .or_insert_with(ModelHealth::new);

        let failures = health.record_failure();
        failures >= self.config.failure_threshold
    }

    /// Check if a model is considered healthy
    pub fn is_healthy(&self, model: &str) -> bool {
        if let Some(health) = self.health.get(model) {
            if health.failure_count() >= self.config.failure_threshold {
                // Check if enough time has passed for recovery
                if let Some(unhealthy_since) = health.unhealthy_since {
                    return unhealthy_since.elapsed() >= self.config.recovery_wait;
                }
                return false;
            }
        }
        true
    }

    /// Get the configured fallback model
    pub fn fallback_model(&self) -> Option<&str> {
        self.config.fallback_model.as_deref()
    }

    /// Get the router configuration
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Get the config path (if using file-based config)
    pub fn config_path(&self) -> Option<&PathBuf> {
        self.config_path.as_ref()
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<()> {
        self.config = load_config(self.config_path.as_ref())?;
        Ok(())
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self::with_config(RouterConfig::default()))
    }
}

/// Default config override path
pub fn default_config_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("hone").join("config").join("models.toml"))
}

/// Load configuration (override first, then default)
fn load_config(override_path: Option<&PathBuf>) -> Result<RouterConfig> {
    // Try override path first
    let content = if let Some(path) = override_path {
        if path.exists() {
            fs::read_to_string(path)
                .map_err(|e| Error::InvalidData(format!("Failed to read config: {}", e)))?
        } else {
            DEFAULT_CONFIG.to_string()
        }
    } else {
        // Check default override location
        if let Some(default_path) = default_config_path() {
            if default_path.exists() {
                fs::read_to_string(&default_path)
                    .map_err(|e| Error::InvalidData(format!("Failed to read config: {}", e)))?
            } else {
                DEFAULT_CONFIG.to_string()
            }
        } else {
            DEFAULT_CONFIG.to_string()
        }
    };

    parse_config(&content)
}

/// Raw config structure for TOML parsing
#[derive(Debug, Deserialize)]
struct RawConfig {
    defaults: Option<RawDefaults>,
    models: Option<HashMap<String, RawTaskConfig>>,
    health: Option<RawHealth>,
}

#[derive(Debug, Deserialize)]
struct RawDefaults {
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_retries: Option<u32>,
    fallback_model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTaskConfig {
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_retries: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawHealth {
    failure_threshold: Option<u32>,
    recovery_wait_secs: Option<u64>,
}

/// Parse config from TOML content
fn parse_config(content: &str) -> Result<RouterConfig> {
    let raw: RawConfig = toml::from_str(content)
        .map_err(|e| Error::InvalidData(format!("Invalid config TOML: {}", e)))?;

    let mut config = RouterConfig::default();

    // Apply defaults
    if let Some(defaults) = raw.defaults {
        if let Some(model) = defaults.model {
            config.default_model = model;
        }
        if let Some(timeout) = defaults.timeout_secs {
            config.default_timeout = Duration::from_secs(timeout);
        }
        if let Some(retries) = defaults.max_retries {
            config.default_retries = retries;
        }
        if let Some(fallback) = defaults.fallback_model {
            config.fallback_model = Some(fallback);
        }
    }

    // Apply task-specific configs
    if let Some(models) = raw.models {
        for (task_name, task_config) in models {
            let task = match task_name.as_str() {
                "fast_classification" => TaskType::FastClassification,
                "structured_extraction" => TaskType::StructuredExtraction,
                "reasoning" => TaskType::Reasoning,
                "vision" => TaskType::Vision,
                "narrative" => TaskType::Narrative,
                _ => continue, // Skip unknown task types
            };

            config.tasks.insert(
                task,
                TaskConfig {
                    model: task_config
                        .model
                        .unwrap_or_else(|| config.default_model.clone()),
                    timeout: task_config
                        .timeout_secs
                        .map(Duration::from_secs)
                        .unwrap_or(config.default_timeout),
                    max_retries: task_config.max_retries.unwrap_or(config.default_retries),
                },
            );
        }
    }

    // Apply health config
    if let Some(health) = raw.health {
        if let Some(threshold) = health.failure_threshold {
            config.failure_threshold = threshold;
        }
        if let Some(wait) = health.recovery_wait_secs {
            config.recovery_wait = Duration::from_secs(wait);
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_config() {
        let config = parse_config(DEFAULT_CONFIG).unwrap();
        assert_eq!(config.default_model, "gemma3");
        assert!(config.fallback_model.is_some());
    }

    #[test]
    fn test_task_type_as_str() {
        assert_eq!(TaskType::FastClassification.as_str(), "fast_classification");
        assert_eq!(TaskType::Vision.as_str(), "vision");
    }

    #[test]
    fn test_router_model_selection() {
        let mut tasks = HashMap::new();
        tasks.insert(
            TaskType::Vision,
            TaskConfig {
                model: "llama3.2-vision".to_string(),
                timeout: Duration::from_secs(120),
                max_retries: 1,
            },
        );

        let config = RouterConfig {
            default_model: "gemma3".to_string(),
            tasks,
            ..Default::default()
        };

        let router = ModelRouter::with_config(config);

        assert_eq!(
            router.model_for_task(TaskType::FastClassification),
            "gemma3"
        );
        assert_eq!(router.model_for_task(TaskType::Vision), "llama3.2-vision");
    }

    #[test]
    fn test_health_tracking() {
        let mut router = ModelRouter::with_config(RouterConfig {
            failure_threshold: 2,
            ..Default::default()
        });

        assert!(router.is_healthy("test-model"));

        router.record_failure("test-model");
        assert!(router.is_healthy("test-model")); // 1 failure, threshold is 2

        router.record_failure("test-model");
        assert!(!router.is_healthy("test-model")); // 2 failures, now unhealthy

        router.record_success("test-model");
        assert!(router.is_healthy("test-model")); // Reset by success
    }

    #[test]
    fn test_fallback_on_unhealthy() {
        let config = RouterConfig {
            default_model: "primary".to_string(),
            fallback_model: Some("fallback".to_string()),
            failure_threshold: 1,
            ..Default::default()
        };

        let mut router = ModelRouter::with_config(config);

        assert_eq!(
            router.model_for_task(TaskType::FastClassification),
            "primary"
        );

        router.record_failure("primary");
        assert_eq!(
            router.model_for_task(TaskType::FastClassification),
            "fallback"
        );
    }

    #[test]
    fn test_config_for_task() {
        let router = ModelRouter::new().unwrap();
        let config = router.config_for_task(TaskType::Vision);

        assert!(config.timeout > Duration::from_secs(30)); // Vision has longer timeout
    }
}

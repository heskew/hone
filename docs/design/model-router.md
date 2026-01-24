---
title: Model Router
description: LLM model selection and routing
date: 2026-01-24
---

## Overview

The Model Router abstracts LLM model selection from application code. Instead of hardcoding model names, code declares what *capability* it needs, and the router selects the appropriate model based on configuration.

## Goals

1. **Decouple code from models** — Swap models without code changes
2. **Right-size for the task** — Use small fast models for simple tasks, larger models for complex reasoning
3. **Easy experimentation** — A/B test models, compare performance
4. **Graceful evolution** — New models drop constantly; make adoption frictionless
5. **Observable** — Track latency, quality, cost per task type

## Current State

Today, Ollama integration uses environment variables:

```bash
OLLAMA_HOST=http://localhost:11434
OLLAMA_MODEL=gemma3           # Used for everything text
OLLAMA_VISION_MODEL=llama3.2-vision  # Used for receipts
```

Problems:
- One model for all text tasks (classification, explanation, extraction)
- No way to use different models for different needs
- Adding a new model requires code changes
- No visibility into which model is best for which task

## Task Types

Different LLM tasks have different requirements:

| Task Type | Characteristics | Example Tasks |
|-----------|-----------------|---------------|
| `FastClassification` | Simple, high volume, latency-sensitive | Merchant category, subscription vs retail |
| `StructuredExtraction` | Reliable JSON output, schema adherence | Receipt parsing, entity extraction |
| `Reasoning` | Complex analysis, multi-step thinking | Spending explanations, anomaly analysis |
| `Vision` | Image understanding | Receipt OCR, document parsing |
| `Conversational` | Multi-turn, context retention | Follow-up questions, clarifications |
| `Narrative` | Natural, engaging prose | Insight summaries, reports |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application Code                          │
│                                                                  │
│    ollama.with_task(TaskType::Reasoning)                        │
│          .prompt("Explain why spending increased...")            │
│          .call()                                                 │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Model Router                             │
├─────────────────────────────────────────────────────────────────┤
│  1. Look up task → model mapping from config                     │
│  2. Check model health/availability                              │
│  3. Apply fallback chain if primary unavailable                  │
│  4. Route to selected model                                      │
│  5. Record metrics (latency, success, tokens)                    │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Ollama API Client                           │
│                   POST /api/generate                             │
│                   POST /api/chat                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Configuration

### Model Registry

Define available models and their characteristics:

```toml
# config/models.toml

[models.gemma3]
size = "4b"
strengths = ["instructions", "json", "fast"]
context_window = 8192
avg_latency_ms = 800

[models.llama3-2-3b]
size = "3b"
strengths = ["fast", "general"]
context_window = 4096
avg_latency_ms = 400

[models.llama3-1-8b]
size = "8b"
strengths = ["reasoning", "narrative", "context"]
context_window = 8192
avg_latency_ms = 2500

[models.qwen2-5-7b]
size = "7b"
strengths = ["reasoning", "code", "json"]
context_window = 32768
avg_latency_ms = 2000

[models.llama3-2-vision-11b]
size = "11b"
strengths = ["vision", "ocr"]
context_window = 4096
avg_latency_ms = 5000
```

### Task Routing

Map task types to models:

```toml
# config/routing.toml

[routing]
FastClassification = "llama3-2-3b"
StructuredExtraction = "gemma3"
Reasoning = "llama3-1-8b"
Vision = "llama3-2-vision-11b"
Conversational = "llama3-1-8b"
Narrative = "llama3-1-8b"

# Fallback chains (try in order)
[fallbacks]
Reasoning = ["llama3-1-8b", "qwen2-5-7b", "gemma3"]
FastClassification = ["llama3-2-3b", "gemma3"]
```

### Environment Override

For quick testing without config file changes:

```bash
# Override specific task routing
HONE_MODEL_REASONING=mistral-7b
HONE_MODEL_FAST_CLASSIFICATION=phi3

# Or override everything (legacy behavior)
OLLAMA_MODEL=gemma3
```

## Implementation

### Core Types

```rust
/// Categories of LLM tasks with different requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Quick classification, high volume (merchant category, yes/no questions)
    FastClassification,
    
    /// Reliable structured output (JSON, specific formats)
    StructuredExtraction,
    
    /// Complex analysis requiring multi-step thinking
    Reasoning,
    
    /// Image understanding (receipts, documents)
    Vision,
    
    /// Multi-turn dialogue with context
    Conversational,
    
    /// Natural, engaging prose generation
    Narrative,
}

/// Model capabilities and metadata
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: String,
    pub strengths: Vec<String>,
    pub context_window: usize,
    pub avg_latency_ms: u32,
}

/// Router configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RouterConfig {
    pub models: HashMap<String, ModelInfo>,
    pub routing: HashMap<TaskType, String>,
    pub fallbacks: HashMap<TaskType, Vec<String>>,
}
```

### Model Router

```rust
pub struct ModelRouter {
    config: RouterConfig,
    client: OllamaClient,
    metrics: Arc<Mutex<RouterMetrics>>,
    model_health: Arc<Mutex<HashMap<String, ModelHealth>>>,
}

impl ModelRouter {
    pub fn new(config: RouterConfig, ollama_host: &str) -> Self {
        Self {
            config,
            client: OllamaClient::new(ollama_host),
            metrics: Arc::new(Mutex::new(RouterMetrics::default())),
            model_health: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Select the best available model for a task
    pub fn select_model(&self, task: TaskType) -> Result<String> {
        // Get primary model for this task
        let primary = self.config.routing
            .get(&task)
            .ok_or_else(|| anyhow!("No model configured for task {:?}", task))?;
        
        // Check if it's healthy
        if self.is_model_healthy(primary) {
            return Ok(primary.clone());
        }
        
        // Try fallbacks
        if let Some(fallbacks) = self.config.fallbacks.get(&task) {
            for model in fallbacks {
                if self.is_model_healthy(model) {
                    tracing::info!(
                        task = ?task, 
                        primary = %primary, 
                        fallback = %model, 
                        "Using fallback model"
                    );
                    return Ok(model.clone());
                }
            }
        }
        
        // Last resort: return primary anyway, let it fail explicitly
        Ok(primary.clone())
    }
    
    /// Create a request builder for a specific task
    pub fn with_task(&self, task: TaskType) -> TaskRequest {
        let model = self.select_model(task).unwrap_or_else(|_| "gemma3".to_string());
        
        TaskRequest {
            router: self,
            task,
            model,
            prompt: None,
            system: None,
            images: vec![],
        }
    }
    
    /// Check if a model is responding
    fn is_model_healthy(&self, model: &str) -> bool {
        let health = self.model_health.lock().unwrap();
        health.get(model)
            .map(|h| h.is_healthy())
            .unwrap_or(true) // Assume healthy if unknown
    }
    
    /// Record the result of a model call
    fn record_result(&self, task: TaskType, model: &str, result: &CallResult) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.record(task, model, result);
        
        let mut health = self.model_health.lock().unwrap();
        health.entry(model.to_string())
            .or_insert_with(ModelHealth::new)
            .update(result.success);
    }
}

/// Fluent builder for LLM requests
pub struct TaskRequest<'a> {
    router: &'a ModelRouter,
    task: TaskType,
    model: String,
    prompt: Option<String>,
    system: Option<String>,
    images: Vec<Vec<u8>>,
}

impl<'a> TaskRequest<'a> {
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }
    
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }
    
    pub fn image(mut self, image_data: Vec<u8>) -> Self {
        self.images.push(image_data);
        self
    }
    
    pub async fn call(self) -> Result<String> {
        let start = Instant::now();
        
        let result = self.router.client
            .generate(&self.model, self.prompt.as_deref().unwrap_or(""), self.system.as_deref())
            .await;
        
        let call_result = CallResult {
            success: result.is_ok(),
            latency: start.elapsed(),
            tokens: result.as_ref().map(|r| r.len() / 4).unwrap_or(0), // rough estimate
        };
        
        self.router.record_result(self.task, &self.model, &call_result);
        
        result
    }
    
    pub async fn call_json<T: DeserializeOwned>(self) -> Result<T> {
        let response = self.call().await?;
        serde_json::from_str(&response)
            .map_err(|e| anyhow!("Failed to parse JSON response: {}", e))
    }
}
```

### Health Tracking

```rust
#[derive(Debug)]
pub struct ModelHealth {
    recent_calls: VecDeque<bool>, // Last N successes/failures
    last_failure: Option<Instant>,
    consecutive_failures: u32,
}

impl ModelHealth {
    const WINDOW_SIZE: usize = 10;
    const UNHEALTHY_THRESHOLD: u32 = 3; // Consecutive failures
    const RECOVERY_DELAY: Duration = Duration::from_secs(60);
    
    pub fn new() -> Self {
        Self {
            recent_calls: VecDeque::with_capacity(Self::WINDOW_SIZE),
            last_failure: None,
            consecutive_failures: 0,
        }
    }
    
    pub fn update(&mut self, success: bool) {
        if self.recent_calls.len() >= Self::WINDOW_SIZE {
            self.recent_calls.pop_front();
        }
        self.recent_calls.push_back(success);
        
        if success {
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            self.last_failure = Some(Instant::now());
        }
    }
    
    pub fn is_healthy(&self) -> bool {
        // Unhealthy if too many consecutive failures
        if self.consecutive_failures >= Self::UNHEALTHY_THRESHOLD {
            // Allow recovery attempt after delay
            if let Some(last) = self.last_failure {
                if last.elapsed() < Self::RECOVERY_DELAY {
                    return false;
                }
            }
        }
        true
    }
    
    pub fn success_rate(&self) -> f32 {
        if self.recent_calls.is_empty() {
            return 1.0;
        }
        let successes = self.recent_calls.iter().filter(|&&s| s).count();
        successes as f32 / self.recent_calls.len() as f32
    }
}
```

### Metrics Collection

```rust
#[derive(Debug, Default)]
pub struct RouterMetrics {
    calls: HashMap<(TaskType, String), TaskMetrics>,
}

#[derive(Debug, Default)]
pub struct TaskMetrics {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub total_latency_ms: u64,
    pub total_tokens: u64,
}

impl RouterMetrics {
    pub fn record(&mut self, task: TaskType, model: &str, result: &CallResult) {
        let key = (task, model.to_string());
        let metrics = self.calls.entry(key).or_default();
        
        metrics.total_calls += 1;
        if result.success {
            metrics.successful_calls += 1;
        }
        metrics.total_latency_ms += result.latency.as_millis() as u64;
        metrics.total_tokens += result.tokens as u64;
    }
    
    pub fn summary(&self) -> Vec<MetricsSummary> {
        self.calls.iter().map(|((task, model), m)| {
            MetricsSummary {
                task: *task,
                model: model.clone(),
                calls: m.total_calls,
                success_rate: m.successful_calls as f32 / m.total_calls as f32,
                avg_latency_ms: m.total_latency_ms / m.total_calls,
                avg_tokens: m.total_tokens / m.total_calls,
            }
        }).collect()
    }
}
```

## Usage Examples

### Before (Current Code)

```rust
// Hardcoded model, no task differentiation
let response = ollama.generate("gemma3", &prompt, Some(&system)).await?;
```

### After (With Router)

```rust
// Task-based selection
let response = router
    .with_task(TaskType::FastClassification)
    .system("Classify this merchant into a category")
    .prompt(&format!("Merchant: {}", merchant_name))
    .call()
    .await?;

// JSON extraction with appropriate model
let receipt: ParsedReceipt = router
    .with_task(TaskType::StructuredExtraction)
    .system(RECEIPT_PARSING_PROMPT)
    .prompt(&receipt_text)
    .call_json()
    .await?;

// Complex reasoning with larger model
let explanation = router
    .with_task(TaskType::Reasoning)
    .system("You are a financial analyst explaining spending patterns")
    .prompt(&format!(
        "Explain why spending increased by {}% this month. Data: {:?}",
        increase_pct, spending_data
    ))
    .call()
    .await?;

// Vision task (receipt image)
let ocr_result = router
    .with_task(TaskType::Vision)
    .system("Extract text and structure from this receipt image")
    .image(receipt_image_bytes)
    .call()
    .await?;
```

## API Endpoints

```
GET  /api/ollama/models          # List available models
GET  /api/ollama/routing         # Current task → model mapping
PUT  /api/ollama/routing         # Update routing (admin)
GET  /api/ollama/metrics         # Per-task/model metrics
GET  /api/ollama/health          # Model health status
POST /api/ollama/test            # Test a specific model
```

### Metrics Response Example

```json
{
  "metrics": [
    {
      "task": "fast_classification",
      "model": "llama3-2-3b",
      "calls": 1523,
      "success_rate": 0.98,
      "avg_latency_ms": 420,
      "avg_tokens": 45
    },
    {
      "task": "reasoning",
      "model": "llama3-1-8b",
      "calls": 87,
      "success_rate": 0.95,
      "avg_latency_ms": 2340,
      "avg_tokens": 380
    }
  ],
  "model_health": {
    "llama3-2-3b": { "status": "healthy", "success_rate": 0.98 },
    "llama3-1-8b": { "status": "healthy", "success_rate": 0.95 },
    "gemma3": { "status": "degraded", "success_rate": 0.72 }
  }
}
```

## A/B Testing

Route a percentage of requests to alternative models:

```toml
[routing]
Reasoning = "llama3-1-8b"

[experiments]
Reasoning = [
    { model = "llama3-1-8b", weight = 80 },
    { model = "qwen2-5-7b", weight = 20 },
]
```

Compare metrics to determine which performs better for your data.

## Migration Path

### Phase 1: Introduce Router (Backward Compatible)

1. Add `ModelRouter` with default config
2. If `OLLAMA_MODEL` set, use it for all tasks (legacy behavior)
3. Existing code continues to work

### Phase 2: Migrate Existing Calls

1. Update `ollama.rs` to use `ModelRouter` internally
2. Annotate existing calls with appropriate `TaskType`
3. No external API changes

### Phase 3: Enable Task-Based Routing

1. Ship default `routing.toml` with sensible defaults
2. Document task types and model recommendations
3. Enable metrics collection

### Phase 4: Advanced Features

1. A/B testing support
2. Auto-tuning based on metrics
3. Cost tracking (if using cloud models)

## Integration with Insight Engine

The Insight Engine uses the Model Router for all LLM calls:

```rust
impl SpendingExplainerInsight {
    fn explain(&self, finding: &Finding, router: &ModelRouter) -> Result<String> {
        // Use reasoning model for explanations
        router
            .with_task(TaskType::Reasoning)
            .system(EXPLAINER_SYSTEM_PROMPT)
            .prompt(&format!("Explain this spending change: {:?}", finding.data))
            .call()
    }
}

impl PriceComparisonInsight {
    fn match_items(&self, items: &[Item], router: &ModelRouter) -> Result<Vec<ItemMatch>> {
        // Use fast classification for high-volume matching
        for item in items {
            let category = router
                .with_task(TaskType::FastClassification)
                .prompt(&format!("Categorize: {}", item.description))
                .call_json()?;
        }
    }
}
```

## CLI Support

```bash
# Show current routing configuration
hone ollama routing

# Test a specific task type
hone ollama test --task reasoning --prompt "Why did spending increase?"

# Show metrics
hone ollama metrics

# Override routing temporarily
hone ollama --model-reasoning=mistral-7b test --task reasoning
```

## Future: Quality Scoring

Track output quality, not just latency:

```rust
pub struct QualityMetrics {
    // User feedback on insight helpfulness
    pub helpful_rate: f32,
    
    // JSON parsing success rate (for structured extraction)
    pub parse_success_rate: f32,
    
    // Response coherence score (self-evaluated)
    pub coherence_score: f32,
}
```

Use quality + latency + cost to auto-select optimal model per task.
//! Training data generation for fine-tuning task-specific models
//!
//! This module exports training data from user corrections and Ollama interactions
//! in formats suitable for fine-tuning LLMs (JSONL chat format for Ollama/llama.cpp).

use std::collections::HashMap;
use std::io::Write;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::Result;

/// Supported training tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingTask {
    /// Merchant → category classification
    ClassifyMerchant,
    /// Raw description → clean merchant name
    NormalizeMerchant,
    /// Merchant → subscription/retail classification
    ClassifySubscription,
}

impl TrainingTask {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrainingTask::ClassifyMerchant => "classify_merchant",
            TrainingTask::NormalizeMerchant => "normalize_merchant",
            TrainingTask::ClassifySubscription => "classify_subscription",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "classify_merchant" => Some(TrainingTask::ClassifyMerchant),
            "normalize_merchant" => Some(TrainingTask::NormalizeMerchant),
            "classify_subscription" => Some(TrainingTask::ClassifySubscription),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            TrainingTask::ClassifyMerchant,
            TrainingTask::NormalizeMerchant,
            TrainingTask::ClassifySubscription,
        ]
    }
}

impl std::fmt::Display for TrainingTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for TrainingTask {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        TrainingTask::from_str(s).ok_or_else(|| format!("Unknown training task: {}", s))
    }
}

/// A single training example in chat format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    /// The input (user message)
    pub input: String,
    /// The expected output (assistant message)
    pub output: String,
    /// Source of this example (user_correction, ollama_confirmed, etc.)
    pub source: String,
    /// Confidence score (1.0 for user corrections, lower for ollama-derived)
    pub confidence: f64,
    /// When this example was created
    pub created_at: Option<NaiveDateTime>,
}

/// Chat message format for JSONL export (Ollama/llama.cpp compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Training example in chat format (ready for export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTrainingExample {
    pub messages: Vec<ChatMessage>,
}

/// Statistics about exported training data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExportStats {
    pub task: String,
    pub total_examples: usize,
    pub user_corrections: usize,
    pub ollama_confirmed: usize,
    pub unique_inputs: usize,
}

/// Training data generator
pub struct TrainingDataGenerator<'a> {
    db: &'a Database,
}

impl<'a> TrainingDataGenerator<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Generate training examples for a specific task
    pub fn generate(&self, task: TrainingTask) -> Result<Vec<TrainingExample>> {
        match task {
            TrainingTask::ClassifyMerchant => self.generate_classify_merchant(),
            TrainingTask::NormalizeMerchant => self.generate_normalize_merchant(),
            TrainingTask::ClassifySubscription => self.generate_classify_subscription(),
        }
    }

    /// Generate merchant classification training data
    fn generate_classify_merchant(&self) -> Result<Vec<TrainingExample>> {
        let mut examples = Vec::new();

        // Source 1: merchant_tag_cache (user corrections have highest confidence)
        let cache_examples = self.db.get_merchant_tag_training_data()?;
        for (merchant_pattern, tag_name, source, confidence) in cache_examples {
            examples.push(TrainingExample {
                input: merchant_pattern,
                output: tag_name,
                source,
                confidence,
                created_at: None,
            });
        }

        // Source 2: ollama_corrections (explicit user corrections)
        let corrections = self.db.get_ollama_corrections_training_data()?;
        for (description, corrected_tag, confidence) in corrections {
            examples.push(TrainingExample {
                input: description,
                output: corrected_tag,
                source: "user_correction".to_string(),
                confidence,
                created_at: None,
            });
        }

        // Deduplicate by input (keep highest confidence)
        examples = Self::deduplicate_examples(examples);

        Ok(examples)
    }

    /// Generate merchant name normalization training data
    fn generate_normalize_merchant(&self) -> Result<Vec<TrainingExample>> {
        let mut examples = Vec::new();

        // Source: merchant_name_cache (user corrections)
        let name_corrections = self.db.get_merchant_name_training_data()?;
        for (raw_name, corrected_name, source) in name_corrections {
            let confidence = if source == "user" { 1.0 } else { 0.8 };
            examples.push(TrainingExample {
                input: raw_name,
                output: corrected_name,
                source,
                confidence,
                created_at: None,
            });
        }

        // Source: ollama_metrics successful normalizations (if user didn't correct them)
        let ollama_examples = self.db.get_ollama_normalize_training_data()?;
        for (input, output) in ollama_examples {
            // Only include if not already overridden by user
            if !examples.iter().any(|e| e.input == input) {
                examples.push(TrainingExample {
                    input,
                    output,
                    source: "ollama_confirmed".to_string(),
                    confidence: 0.7,
                    created_at: None,
                });
            }
        }

        examples = Self::deduplicate_examples(examples);
        Ok(examples)
    }

    /// Generate subscription classification training data
    fn generate_classify_subscription(&self) -> Result<Vec<TrainingExample>> {
        let mut examples = Vec::new();

        // Source: merchant_subscription_cache
        let cache = self.db.get_subscription_classification_training_data()?;
        for (merchant, is_subscription, source) in cache {
            let output = if is_subscription {
                "SUBSCRIPTION"
            } else {
                "RETAIL"
            };
            let confidence = if source == "user_override" { 1.0 } else { 0.8 };
            examples.push(TrainingExample {
                input: merchant,
                output: output.to_string(),
                source,
                confidence,
                created_at: None,
            });
        }

        examples = Self::deduplicate_examples(examples);
        Ok(examples)
    }

    /// Deduplicate examples by input, keeping highest confidence
    fn deduplicate_examples(examples: Vec<TrainingExample>) -> Vec<TrainingExample> {
        let mut by_input: HashMap<String, TrainingExample> = HashMap::new();

        for example in examples {
            let key = example.input.clone();
            if let Some(existing) = by_input.get(&key) {
                if example.confidence > existing.confidence {
                    by_input.insert(key, example);
                }
            } else {
                by_input.insert(key, example);
            }
        }

        by_input.into_values().collect()
    }

    /// Export training data in JSONL chat format
    pub fn export_jsonl<W: Write>(
        &self,
        task: TrainingTask,
        writer: &mut W,
    ) -> Result<TrainingExportStats> {
        let examples = self.generate(task)?;
        let system_prompt = self.get_system_prompt(task);

        let mut user_corrections = 0;
        let mut ollama_confirmed = 0;
        let mut unique_inputs = std::collections::HashSet::new();

        for example in &examples {
            unique_inputs.insert(example.input.clone());

            if example.source == "user_correction" || example.source == "user" {
                user_corrections += 1;
            } else {
                ollama_confirmed += 1;
            }

            let chat_example = ChatTrainingExample {
                messages: vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: system_prompt.clone(),
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: example.input.clone(),
                    },
                    ChatMessage {
                        role: "assistant".to_string(),
                        content: example.output.clone(),
                    },
                ],
            };

            let json = serde_json::to_string(&chat_example)?;
            writeln!(writer, "{}", json)?;
        }

        Ok(TrainingExportStats {
            task: task.to_string(),
            total_examples: examples.len(),
            user_corrections,
            ollama_confirmed,
            unique_inputs: unique_inputs.len(),
        })
    }

    /// Get the system prompt for a task
    fn get_system_prompt(&self, task: TrainingTask) -> String {
        match task {
            TrainingTask::ClassifyMerchant => {
                "You are a financial transaction classifier. Given a merchant description from a bank statement, output the spending category. Categories: Income, Housing, Utilities, Groceries, Dining, Transport, Healthcare, Shopping, Entertainment, Subscriptions, Travel, Personal, Education, Pets, Gifts, Financial, Other. Output only the category name.".to_string()
            }
            TrainingTask::NormalizeMerchant => {
                "You are a merchant name normalizer. Given a raw transaction description from a bank statement, extract and output only the clean, human-readable merchant name. Remove transaction IDs, location codes, and payment prefixes. Preserve proper capitalization and apostrophes in brand names.".to_string()
            }
            TrainingTask::ClassifySubscription => {
                "You are a subscription classifier. Given a merchant name, determine if it is a subscription service or a retail store. Output either SUBSCRIPTION or RETAIL.".to_string()
            }
        }
    }
}

// Database methods for extracting training data
impl Database {
    /// Get merchant→tag associations for training (from merchant_tag_cache)
    pub fn get_merchant_tag_training_data(&self) -> Result<Vec<(String, String, String, f64)>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT mtc.merchant_pattern, t.name, mtc.source, mtc.confidence
            FROM merchant_tag_cache mtc
            INNER JOIN tags t ON mtc.tag_id = t.id
            ORDER BY mtc.confidence DESC
            "#,
        )?;

        let results = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Get Ollama corrections for training
    pub fn get_ollama_corrections_training_data(&self) -> Result<Vec<(String, String, f64)>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT t.description, tags.name, 1.0 as confidence
            FROM ollama_corrections oc
            INNER JOIN transactions t ON oc.transaction_id = t.id
            INNER JOIN tags ON oc.corrected_tag_id = tags.id
            WHERE oc.original_tag_id != oc.corrected_tag_id
            ORDER BY oc.corrected_at DESC
            "#,
        )?;

        let results = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Get merchant name corrections for training
    pub fn get_merchant_name_training_data(&self) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT description, merchant_name, source
            FROM merchant_name_cache
            ORDER BY updated_at DESC
            "#,
        )?;

        let results = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Get successful Ollama normalizations (not corrected by user) for training
    pub fn get_ollama_normalize_training_data(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn()?;

        // Get successful normalizations that weren't corrected
        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT input_text, result_text
            FROM ollama_metrics
            WHERE operation = 'normalize_merchant'
              AND success = 1
              AND input_text IS NOT NULL
              AND result_text IS NOT NULL
              AND input_text NOT IN (SELECT description FROM merchant_name_cache WHERE source = 'user')
            ORDER BY started_at DESC
            LIMIT 1000
            "#,
        )?;

        let results = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Get subscription classifications for training
    pub fn get_subscription_classification_training_data(
        &self,
    ) -> Result<Vec<(String, bool, String)>> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT merchant_pattern, is_subscription, source
            FROM merchant_subscription_cache
            ORDER BY created_at DESC
            "#,
        )?;

        let results = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn test_training_task_roundtrip() {
        for task in TrainingTask::all() {
            let s = task.as_str();
            let parsed = TrainingTask::from_str(s).unwrap();
            assert_eq!(task, parsed);
        }
    }

    #[test]
    fn test_training_task_from_str_invalid() {
        let result = TrainingTask::from_str("invalid_task");
        assert!(result.is_none());
    }

    #[test]
    fn test_training_task_display() {
        assert_eq!(
            TrainingTask::ClassifyMerchant.to_string(),
            "classify_merchant"
        );
        assert_eq!(
            TrainingTask::NormalizeMerchant.to_string(),
            "normalize_merchant"
        );
        assert_eq!(
            TrainingTask::ClassifySubscription.to_string(),
            "classify_subscription"
        );
    }

    #[test]
    fn test_training_task_from_str_trait() {
        let task: std::result::Result<TrainingTask, _> = "classify_merchant".parse();
        assert!(task.is_ok());
        assert_eq!(task.unwrap(), TrainingTask::ClassifyMerchant);

        let invalid: std::result::Result<TrainingTask, _> = "invalid".parse();
        assert!(invalid.is_err());
    }

    #[test]
    fn test_training_task_all() {
        let all = TrainingTask::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&TrainingTask::ClassifyMerchant));
        assert!(all.contains(&TrainingTask::NormalizeMerchant));
        assert!(all.contains(&TrainingTask::ClassifySubscription));
    }

    #[test]
    fn test_deduplicate_examples() {
        let examples = vec![
            TrainingExample {
                input: "NETFLIX".to_string(),
                output: "Entertainment".to_string(),
                source: "ollama".to_string(),
                confidence: 0.7,
                created_at: None,
            },
            TrainingExample {
                input: "NETFLIX".to_string(),
                output: "Subscriptions".to_string(),
                source: "user".to_string(),
                confidence: 1.0,
                created_at: None,
            },
        ];

        let deduped = TrainingDataGenerator::deduplicate_examples(examples);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].output, "Subscriptions"); // Higher confidence wins
    }

    #[test]
    fn test_deduplicate_examples_keeps_higher_confidence() {
        let examples = vec![
            TrainingExample {
                input: "AMAZON".to_string(),
                output: "Shopping".to_string(),
                source: "rule".to_string(),
                confidence: 0.8,
                created_at: None,
            },
            TrainingExample {
                input: "AMAZON".to_string(),
                output: "Entertainment".to_string(),
                source: "ollama".to_string(),
                confidence: 0.7, // Lower confidence, should NOT replace
                created_at: None,
            },
        ];

        let deduped = TrainingDataGenerator::deduplicate_examples(examples);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].output, "Shopping");
    }

    #[test]
    fn test_deduplicate_examples_unique_inputs() {
        let examples = vec![
            TrainingExample {
                input: "NETFLIX".to_string(),
                output: "Subscriptions".to_string(),
                source: "user".to_string(),
                confidence: 1.0,
                created_at: None,
            },
            TrainingExample {
                input: "SPOTIFY".to_string(),
                output: "Subscriptions".to_string(),
                source: "user".to_string(),
                confidence: 1.0,
                created_at: None,
            },
        ];

        let deduped = TrainingDataGenerator::deduplicate_examples(examples);
        assert_eq!(deduped.len(), 2); // Both unique inputs preserved
    }

    #[test]
    fn test_chat_example_serialization() {
        let example = ChatTrainingExample {
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are a classifier.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "NETFLIX".to_string(),
                },
                ChatMessage {
                    role: "assistant".to_string(),
                    content: "Subscriptions".to_string(),
                },
            ],
        };

        let json = serde_json::to_string(&example).unwrap();
        assert!(json.contains("system"));
        assert!(json.contains("NETFLIX"));
        assert!(json.contains("Subscriptions"));
    }

    #[test]
    fn test_chat_example_deserialization() {
        let json = r#"{"messages":[{"role":"system","content":"Test"},{"role":"user","content":"Input"},{"role":"assistant","content":"Output"}]}"#;
        let example: ChatTrainingExample = serde_json::from_str(json).unwrap();
        assert_eq!(example.messages.len(), 3);
        assert_eq!(example.messages[0].role, "system");
        assert_eq!(example.messages[1].content, "Input");
        assert_eq!(example.messages[2].content, "Output");
    }

    #[test]
    fn test_training_example_creation() {
        let example = TrainingExample {
            input: "WHOLE FOODS".to_string(),
            output: "Groceries".to_string(),
            source: "user_correction".to_string(),
            confidence: 1.0,
            created_at: None,
        };

        assert_eq!(example.input, "WHOLE FOODS");
        assert_eq!(example.output, "Groceries");
        assert_eq!(example.confidence, 1.0);
    }

    #[test]
    fn test_training_export_stats() {
        let stats = TrainingExportStats {
            task: "classify_merchant".to_string(),
            total_examples: 100,
            user_corrections: 30,
            ollama_confirmed: 70,
            unique_inputs: 95,
        };

        assert_eq!(stats.task, "classify_merchant");
        assert_eq!(stats.total_examples, 100);
        assert_eq!(stats.user_corrections, 30);
        assert_eq!(stats.ollama_confirmed, 70);
        assert_eq!(stats.unique_inputs, 95);
    }

    #[test]
    fn test_training_data_generator_new() {
        let db = create_test_db();
        let generator = TrainingDataGenerator::new(&db);
        // Just test that we can create it
        let _ = generator;
    }

    #[test]
    fn test_generate_classify_merchant_empty() {
        let db = create_test_db();
        let generator = TrainingDataGenerator::new(&db);
        let examples = generator.generate(TrainingTask::ClassifyMerchant).unwrap();
        assert_eq!(examples.len(), 0);
    }

    #[test]
    fn test_generate_normalize_merchant_empty() {
        let db = create_test_db();
        let generator = TrainingDataGenerator::new(&db);
        let examples = generator.generate(TrainingTask::NormalizeMerchant).unwrap();
        assert_eq!(examples.len(), 0);
    }

    #[test]
    fn test_generate_classify_subscription_empty() {
        let db = create_test_db();
        let generator = TrainingDataGenerator::new(&db);
        let examples = generator
            .generate(TrainingTask::ClassifySubscription)
            .unwrap();
        assert_eq!(examples.len(), 0);
    }

    #[test]
    fn test_export_jsonl_empty() {
        let db = create_test_db();
        let generator = TrainingDataGenerator::new(&db);
        let mut output = Vec::new();
        let stats = generator
            .export_jsonl(TrainingTask::ClassifyMerchant, &mut output)
            .unwrap();

        assert_eq!(stats.total_examples, 0);
        assert_eq!(stats.user_corrections, 0);
        assert_eq!(stats.ollama_confirmed, 0);
        assert_eq!(stats.unique_inputs, 0);
        assert!(output.is_empty());
    }

    #[test]
    fn test_db_get_merchant_tag_training_data_empty() {
        let db = create_test_db();
        let data = db.get_merchant_tag_training_data().unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_db_get_ollama_corrections_training_data_empty() {
        let db = create_test_db();
        let data = db.get_ollama_corrections_training_data().unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_db_get_merchant_name_training_data_empty() {
        let db = create_test_db();
        let data = db.get_merchant_name_training_data().unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_db_get_ollama_normalize_training_data_empty() {
        let db = create_test_db();
        let data = db.get_ollama_normalize_training_data().unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_db_get_subscription_classification_training_data_empty() {
        let db = create_test_db();
        let data = db.get_subscription_classification_training_data().unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_training_task_serialization() {
        let task = TrainingTask::ClassifyMerchant;
        let json = serde_json::to_string(&task).unwrap();
        assert_eq!(json, "\"classify_merchant\"");

        let parsed: TrainingTask = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, task);
    }

    #[test]
    fn test_chat_message_creation() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "test content".to_string(),
        };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "test content");
    }
}

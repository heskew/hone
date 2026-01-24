//! Prompt Library for Ollama integration
//!
//! Prompts are loaded with a two-layer resolution:
//! 1. Check for override in data dir (~/.local/share/hone/prompts/overrides/)
//! 2. Fall back to embedded defaults (compiled into binary)
//!
//! This allows users to customize prompts without modifying the source,
//! while automatically getting new default prompts on upgrade.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{Error, Result};

/// Embedded default prompts (compiled into binary)
mod defaults {
    pub const CLASSIFY_MERCHANT: &str = include_str!("../../../prompts/classify_merchant.md");
    pub const NORMALIZE_MERCHANT: &str = include_str!("../../../prompts/normalize_merchant.md");
    pub const NORMALIZE_MERCHANT_WITH_CONTEXT: &str =
        include_str!("../../../prompts/normalize_merchant_with_context.md");
    pub const PARSE_RECEIPT: &str = include_str!("../../../prompts/parse_receipt.md");
    pub const SUGGEST_ENTITY: &str = include_str!("../../../prompts/suggest_entity.md");
    pub const CLASSIFY_SUBSCRIPTION: &str =
        include_str!("../../../prompts/classify_subscription.md");
    pub const SUGGEST_SPLIT: &str = include_str!("../../../prompts/suggest_split.md");
    pub const EVALUATE_RECEIPT_MATCH: &str =
        include_str!("../../../prompts/evaluate_receipt_match.md");
    pub const ANALYZE_DUPLICATES: &str = include_str!("../../../prompts/analyze_duplicates.md");
    pub const EXPLAIN_SPENDING: &str = include_str!("../../../prompts/explain_spending.md");
    pub const SPENDING_ANALYSIS_AGENT: &str =
        include_str!("../../../prompts/spending_analysis_agent.md");
    pub const DUPLICATE_ANALYSIS_AGENT: &str =
        include_str!("../../../prompts/duplicate_analysis_agent.md");
    pub const EXPLORE_AGENT: &str = include_str!("../../../prompts/explore_agent.md");
}

/// Known prompt IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptId {
    ClassifyMerchant,
    NormalizeMerchant,
    NormalizeMerchantWithContext,
    ParseReceipt,
    SuggestEntity,
    ClassifySubscription,
    SuggestSplit,
    EvaluateReceiptMatch,
    AnalyzeDuplicates,
    ExplainSpending,
    /// Agentic prompt for spending anomaly analysis with tool calling
    SpendingAnalysisAgent,
    /// Agentic prompt for duplicate subscription analysis with tool calling
    DuplicateAnalysisAgent,
    /// Agentic prompt for explore mode conversational queries
    ExploreAgent,
}

impl PromptId {
    /// Get the string identifier for this prompt
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClassifyMerchant => "classify_merchant",
            Self::NormalizeMerchant => "normalize_merchant",
            Self::NormalizeMerchantWithContext => "normalize_merchant_with_context",
            Self::ParseReceipt => "parse_receipt",
            Self::SuggestEntity => "suggest_entity",
            Self::ClassifySubscription => "classify_subscription",
            Self::SuggestSplit => "suggest_split",
            Self::EvaluateReceiptMatch => "evaluate_receipt_match",
            Self::AnalyzeDuplicates => "analyze_duplicates",
            Self::ExplainSpending => "explain_spending",
            Self::SpendingAnalysisAgent => "spending_analysis_agent",
            Self::DuplicateAnalysisAgent => "duplicate_analysis_agent",
            Self::ExploreAgent => "explore_agent",
        }
    }

    /// Get all known prompt IDs
    pub fn all() -> &'static [PromptId] {
        &[
            Self::ClassifyMerchant,
            Self::NormalizeMerchant,
            Self::NormalizeMerchantWithContext,
            Self::ParseReceipt,
            Self::SuggestEntity,
            Self::ClassifySubscription,
            Self::SuggestSplit,
            Self::EvaluateReceiptMatch,
            Self::AnalyzeDuplicates,
            Self::ExplainSpending,
            Self::SpendingAnalysisAgent,
            Self::DuplicateAnalysisAgent,
            Self::ExploreAgent,
        ]
    }

    /// Get the default embedded content for this prompt
    fn default_content(&self) -> &'static str {
        match self {
            Self::ClassifyMerchant => defaults::CLASSIFY_MERCHANT,
            Self::NormalizeMerchant => defaults::NORMALIZE_MERCHANT,
            Self::NormalizeMerchantWithContext => defaults::NORMALIZE_MERCHANT_WITH_CONTEXT,
            Self::ParseReceipt => defaults::PARSE_RECEIPT,
            Self::SuggestEntity => defaults::SUGGEST_ENTITY,
            Self::ClassifySubscription => defaults::CLASSIFY_SUBSCRIPTION,
            Self::SuggestSplit => defaults::SUGGEST_SPLIT,
            Self::EvaluateReceiptMatch => defaults::EVALUATE_RECEIPT_MATCH,
            Self::AnalyzeDuplicates => defaults::ANALYZE_DUPLICATES,
            Self::ExplainSpending => defaults::EXPLAIN_SPENDING,
            Self::SpendingAnalysisAgent => defaults::SPENDING_ANALYSIS_AGENT,
            Self::DuplicateAnalysisAgent => defaults::DUPLICATE_ANALYSIS_AGENT,
            Self::ExploreAgent => defaults::EXPLORE_AGENT,
        }
    }
}

/// Prompt frontmatter metadata
#[derive(Debug, Clone, Deserialize)]
pub struct PromptMetadata {
    /// Unique identifier
    pub id: String,
    /// Version number for tracking changes
    pub version: u32,
    /// Task type for model routing (fast_classification, reasoning, vision, etc.)
    pub task_type: String,
}

/// A loaded prompt with metadata and content
#[derive(Debug, Clone)]
pub struct Prompt {
    /// Metadata from frontmatter
    pub metadata: PromptMetadata,
    /// The prompt content (system + user sections)
    pub content: String,
    /// Whether this came from an override file
    pub is_override: bool,
    /// Path to override file (if any)
    pub override_path: Option<PathBuf>,
}

impl Prompt {
    /// Get the system section of the prompt
    pub fn system_section(&self) -> Option<&str> {
        extract_section(&self.content, "# System")
    }

    /// Get the user section of the prompt
    pub fn user_section(&self) -> Option<&str> {
        extract_section(&self.content, "# User")
    }

    /// Render the prompt with template variables replaced
    pub fn render(&self, vars: &HashMap<&str, &str>) -> String {
        let mut result = self.content.clone();

        // Simple mustache-style replacement: {{var}}
        for (key, value) in vars {
            let pattern = format!("{{{{{}}}}}", key);
            result = result.replace(&pattern, value);
        }

        // Also handle conditional blocks: {{#if var}}...{{/if}}
        // For simplicity, we remove unmatched conditionals
        result = remove_unmatched_conditionals(&result, vars);

        result
    }

    /// Render just the user section with variables
    pub fn render_user(&self, vars: &HashMap<&str, &str>) -> String {
        if let Some(user) = self.user_section() {
            let mut result = user.to_string();
            for (key, value) in vars {
                let pattern = format!("{{{{{}}}}}", key);
                result = result.replace(&pattern, value);
            }
            remove_unmatched_conditionals(&result, vars)
        } else {
            self.render(vars)
        }
    }
}

/// Prompt library for loading and caching prompts
pub struct PromptLibrary {
    /// Override directory path
    override_dir: Option<PathBuf>,
    /// Cached parsed prompts
    cache: HashMap<PromptId, Prompt>,
}

impl PromptLibrary {
    /// Create a new prompt library with default paths
    pub fn new() -> Self {
        let override_dir = default_prompts_dir();
        Self {
            override_dir,
            cache: HashMap::new(),
        }
    }

    /// Create a prompt library with a custom override directory
    pub fn with_override_dir(path: PathBuf) -> Self {
        Self {
            override_dir: Some(path),
            cache: HashMap::new(),
        }
    }

    /// Create a prompt library with no override directory (embedded only)
    pub fn embedded_only() -> Self {
        Self {
            override_dir: None,
            cache: HashMap::new(),
        }
    }

    /// Get a prompt by ID, loading from override or default
    pub fn get(&mut self, id: PromptId) -> Result<&Prompt> {
        if !self.cache.contains_key(&id) {
            let prompt = self.load(id)?;
            self.cache.insert(id, prompt);
        }
        Ok(self.cache.get(&id).unwrap())
    }

    /// Load a prompt (checking override first, then default)
    fn load(&self, id: PromptId) -> Result<Prompt> {
        // Check for override
        if let Some(ref override_dir) = self.override_dir {
            let override_path = override_dir.join(format!("{}.md", id.as_str()));
            if override_path.exists() {
                let content = fs::read_to_string(&override_path).map_err(|e| {
                    Error::InvalidData(format!("Failed to read prompt override: {}", e))
                })?;
                let (metadata, body) = parse_prompt(&content)?;
                return Ok(Prompt {
                    metadata,
                    content: body,
                    is_override: true,
                    override_path: Some(override_path),
                });
            }
        }

        // Use embedded default
        let content = id.default_content();
        let (metadata, body) = parse_prompt(content)?;
        Ok(Prompt {
            metadata,
            content: body,
            is_override: false,
            override_path: None,
        })
    }

    /// List all prompts with their override status
    pub fn list(&mut self) -> Vec<PromptInfo> {
        PromptId::all()
            .iter()
            .map(|&id| {
                let has_override = self.has_override(id);
                let prompt = self.get(id).ok();
                PromptInfo {
                    id: id.as_str().to_string(),
                    version: prompt.map(|p| p.metadata.version).unwrap_or(0),
                    task_type: prompt
                        .map(|p| p.metadata.task_type.clone())
                        .unwrap_or_default(),
                    has_override,
                    override_path: if has_override {
                        self.override_dir
                            .as_ref()
                            .map(|d| d.join(format!("{}.md", id.as_str())))
                    } else {
                        None
                    },
                }
            })
            .collect()
    }

    /// Check if a prompt has an override file
    pub fn has_override(&self, id: PromptId) -> bool {
        if let Some(ref override_dir) = self.override_dir {
            override_dir.join(format!("{}.md", id.as_str())).exists()
        } else {
            false
        }
    }

    /// Get the override directory path
    pub fn override_dir(&self) -> Option<&PathBuf> {
        self.override_dir.as_ref()
    }

    /// Clear the cache (useful after editing override files)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl Default for PromptLibrary {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a prompt for listing
#[derive(Debug, Clone)]
pub struct PromptInfo {
    /// Prompt identifier
    pub id: String,
    /// Version from metadata
    pub version: u32,
    /// Task type for model routing
    pub task_type: String,
    /// Whether an override exists
    pub has_override: bool,
    /// Path to override file (if exists)
    pub override_path: Option<PathBuf>,
}

/// Default prompts override directory
pub fn default_prompts_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("hone").join("prompts").join("overrides"))
}

/// Parse a prompt file into metadata and body
fn parse_prompt(content: &str) -> Result<(PromptMetadata, String)> {
    let content = content.trim();

    // Check for YAML frontmatter
    if !content.starts_with("---") {
        return Err(Error::InvalidData(
            "Prompt must start with YAML frontmatter (---)".into(),
        ));
    }

    // Find end of frontmatter
    let rest = &content[3..];
    let end = rest.find("---").ok_or_else(|| {
        Error::InvalidData("Prompt frontmatter not closed (missing second ---)".into())
    })?;

    let frontmatter = &rest[..end].trim();
    let body = &rest[end + 3..].trim();

    // Parse frontmatter as YAML
    let metadata: PromptMetadata = serde_yaml::from_str(frontmatter)
        .map_err(|e| Error::InvalidData(format!("Invalid prompt frontmatter: {}", e)))?;

    Ok((metadata, body.to_string()))
}

/// Extract a section from the prompt content
fn extract_section<'a>(content: &'a str, header: &str) -> Option<&'a str> {
    let start = content.find(header)?;
    let after_header = &content[start + header.len()..];

    // Find the next header or end of content
    let end = after_header.find("\n# ").unwrap_or(after_header.len());

    Some(after_header[..end].trim())
}

/// Remove unmatched conditional blocks from the template
fn remove_unmatched_conditionals(content: &str, vars: &HashMap<&str, &str>) -> String {
    let mut result = content.to_string();

    // Find all {{#if var}}...{{/if}} blocks
    loop {
        if let Some(if_start) = result.find("{{#if ") {
            let var_start = if_start + 6;
            if let Some(var_end) = result[var_start..].find("}}") {
                let var_name = &result[var_start..var_start + var_end];
                let block_start = var_start + var_end + 2;

                // Find matching {{/if}}
                if let Some(endif_pos) = result[block_start..].find("{{/if}}") {
                    let block_content = &result[block_start..block_start + endif_pos];
                    let full_end = block_start + endif_pos + 7;

                    // Check if variable is present and non-empty
                    let should_include = vars.get(var_name).is_some_and(|v| !v.is_empty());

                    if should_include {
                        // Keep block content, remove markers
                        result = format!(
                            "{}{}{}",
                            &result[..if_start],
                            block_content,
                            &result[full_end..]
                        );
                    } else {
                        // Remove entire block
                        result = format!("{}{}", &result[..if_start], &result[full_end..]);
                    }
                    continue;
                }
            }
        }
        break;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt() {
        let content = r#"---
id: test_prompt
version: 1
task_type: fast_classification
---

# System
Test system prompt.

# User
Test user prompt with {{variable}}.
"#;

        let (metadata, body) = parse_prompt(content).unwrap();
        assert_eq!(metadata.id, "test_prompt");
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.task_type, "fast_classification");
        assert!(body.contains("# System"));
        assert!(body.contains("# User"));
    }

    #[test]
    fn test_extract_section() {
        let content = r#"# System
System content here.

# User
User content here."#;

        assert_eq!(
            extract_section(content, "# System"),
            Some("System content here.")
        );
        assert_eq!(
            extract_section(content, "# User"),
            Some("User content here.")
        );
    }

    #[test]
    fn test_prompt_render() {
        let content = r#"---
id: test
version: 1
task_type: test
---

Hello {{name}}, your value is {{value}}."#;

        let (metadata, body) = parse_prompt(content).unwrap();
        let prompt = Prompt {
            metadata,
            content: body,
            is_override: false,
            override_path: None,
        };

        let mut vars = HashMap::new();
        vars.insert("name", "World");
        vars.insert("value", "42");

        let rendered = prompt.render(&vars);
        assert!(rendered.contains("Hello World"));
        assert!(rendered.contains("your value is 42"));
    }

    #[test]
    fn test_conditional_blocks() {
        let content = "Start{{#if category}}\nCategory: {{category}}{{/if}}\nEnd";

        let mut vars = HashMap::new();
        vars.insert("category", "Groceries");
        let result = remove_unmatched_conditionals(content, &vars);
        assert!(result.contains("Category: {{category}}"));

        let empty_vars: HashMap<&str, &str> = HashMap::new();
        let result = remove_unmatched_conditionals(content, &empty_vars);
        assert!(!result.contains("Category:"));
        assert!(result.contains("Start"));
        assert!(result.contains("End"));
    }

    #[test]
    fn test_prompt_library_embedded() {
        let mut lib = PromptLibrary::embedded_only();

        // Should load all embedded prompts
        for id in PromptId::all() {
            let prompt = lib.get(*id).unwrap();
            assert!(!prompt.is_override);
            assert!(prompt.override_path.is_none());
        }
    }

    #[test]
    fn test_prompt_id_all() {
        let all = PromptId::all();
        assert_eq!(all.len(), 13);
    }

    #[test]
    fn test_default_prompts_parse() {
        // Verify all default prompts parse correctly
        for id in PromptId::all() {
            let content = id.default_content();
            let result = parse_prompt(content);
            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                id.as_str(),
                result.err()
            );

            let (metadata, _) = result.unwrap();
            assert_eq!(
                metadata.id,
                id.as_str(),
                "Prompt ID mismatch for {}",
                id.as_str()
            );
        }
    }
}

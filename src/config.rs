use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DepthLevel {
    Quick,
    Standard,
    Comprehensive,
    Continuous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub path: PathBuf,
    pub depth: DepthLevel,
    pub query_count: usize,
    pub output_dir: PathBuf,
    pub model: String,
    pub resume: bool,
    pub force: bool,
    pub rate_limit: u32,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl Config {
    /// Maximum tokens to use in context window
    pub fn max_context_tokens(&self) -> usize {
        // Conservative limits based on model
        if self.model.contains("claude") {
            180_000 // Claude has 200k window, leave buffer
        } else if self.model.contains("gpt-4") {
            120_000 // GPT-4 has 128k window
        } else {
            30_000 // Conservative default
        }
    }

    /// Tokens reserved for response
    pub fn response_buffer_tokens(&self) -> usize {
        4_096
    }

    /// Get available context tokens after response buffer
    pub fn available_context_tokens(&self) -> usize {
        self.max_context_tokens() - self.response_buffer_tokens()
    }

    /// Check if a path should be included based on patterns
    pub fn should_include_path(&self, path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check exclude patterns first
        for pattern in &self.exclude_patterns {
            if glob::Pattern::new(pattern)
                .map(|p| p.matches(&path_str))
                .unwrap_or(false)
            {
                return false;
            }
        }

        // If no include patterns, include everything not excluded
        if self.include_patterns.is_empty() {
            return true;
        }

        // Check include patterns
        for pattern in &self.include_patterns {
            if glob::Pattern::new(pattern)
                .map(|p| p.matches(&path_str))
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }
}
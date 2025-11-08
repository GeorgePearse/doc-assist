use crate::planner::{Query, ContextSpec, QueryPriority};
use crate::state::GenerationState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    pub query_id: usize,
    pub prompt_summary: String,
    pub response_summary: String,
    pub tokens: usize,
    pub priority: QueryPriority,
}

pub struct ContextManager {
    max_context_tokens: usize,
    entries: HashMap<usize, ContextEntry>,
    recent_history: VecDeque<usize>,
    code_snippets: HashMap<String, String>,
}

impl ContextManager {
    pub fn new(max_context_tokens: usize) -> Self {
        Self {
            max_context_tokens,
            entries: HashMap::new(),
            recent_history: VecDeque::new(),
            code_snippets: HashMap::new(),
        }
    }

    pub async fn get_context_for_query(&self, query: &Query) -> String {
        let mut context = String::new();
        let mut token_budget = self.max_context_tokens / 2; // Use half the context for previous responses

        match &query.context_spec {
            ContextSpec::None => {
                // No context needed
                debug!("Query {} requires no context", query.id);
            }
            ContextSpec::WithPrevious(query_ids) => {
                // Include specific previous queries
                for id in query_ids {
                    if let Some(entry) = self.entries.get(id) {
                        let entry_text = self.format_context_entry(entry);
                        let entry_tokens = entry_text.len() / 4; // Rough estimate

                        if token_budget >= entry_tokens {
                            context.push_str(&entry_text);
                            context.push_str("\n\n---\n\n");
                            token_budget -= entry_tokens;
                        }
                    }
                }
            }
            ContextSpec::Full => {
                // Include as much recent context as possible
                for query_id in self.recent_history.iter().rev() {
                    if let Some(entry) = self.entries.get(query_id) {
                        let entry_text = self.format_context_entry(entry);
                        let entry_tokens = entry_text.len() / 4;

                        if token_budget < entry_tokens {
                            break;
                        }

                        context.push_str(&entry_text);
                        context.push_str("\n\n---\n\n");
                        token_budget -= entry_tokens;
                    }
                }
            }
            ContextSpec::WithCode { paths, max_lines } => {
                // Include code snippets
                for path in paths {
                    if let Some(code) = self.code_snippets.get(path) {
                        let lines: Vec<&str> = code.lines().take(*max_lines).collect();
                        context.push_str(&format!("Code from {}:\n```\n{}\n```\n\n", path, lines.join("\n")));
                    }
                }
            }
        }

        // Add any high-priority context that should always be included
        self.add_persistent_context(&mut context, token_budget);

        context
    }

    pub fn add_response(&mut self, query_id: usize, response: &str) {
        // Create summary of response for context management
        let response_summary = self.summarize_text(response, 500);

        let entry = ContextEntry {
            query_id,
            prompt_summary: String::new(), // Will be set from query
            response_summary,
            tokens: response.len() / 4, // Rough estimate
            priority: QueryPriority::Medium,
        };

        self.entries.insert(query_id, entry);
        self.recent_history.push_back(query_id);

        // Maintain a reasonable history size
        if self.recent_history.len() > 50 {
            self.recent_history.pop_front();
        }

        debug!("Added response for query {} to context", query_id);
    }

    pub fn add_code_snippet(&mut self, path: String, code: String) {
        self.code_snippets.insert(path, code);
    }

    pub fn restore_from_state(&mut self, state: &GenerationState) {
        self.entries = state.context_entries.clone();
        self.recent_history = state.completed_queries.clone().into();
        info!("Restored context with {} entries", self.entries.len());
    }

    pub fn get_state_snapshot(&self) -> HashMap<usize, ContextEntry> {
        self.entries.clone()
    }

    fn format_context_entry(&self, entry: &ContextEntry) -> String {
        format!(
            "Query {}: {}\n\nResponse Summary:\n{}",
            entry.query_id,
            entry.prompt_summary,
            entry.response_summary
        )
    }

    fn summarize_text(&self, text: &str, max_chars: usize) -> String {
        // Simple summarization: take first and last parts, key sections
        let lines: Vec<&str> = text.lines().collect();

        if text.len() <= max_chars {
            return text.to_string();
        }

        let mut summary = String::new();
        let section_size = max_chars / 3;

        // Take beginning
        for line in lines.iter().take(5) {
            summary.push_str(line);
            summary.push('\n');
            if summary.len() >= section_size {
                break;
            }
        }

        summary.push_str("\n[... content trimmed ...]\n\n");

        // Look for key sections (headers, important keywords)
        let important_keywords = ["function", "class", "struct", "API", "example", "usage", "error", "return"];
        let mut important_lines = Vec::new();

        for line in &lines {
            for keyword in &important_keywords {
                if line.to_lowercase().contains(keyword) {
                    important_lines.push(*line);
                    break;
                }
            }
        }

        for line in important_lines.iter().take(3) {
            summary.push_str(line);
            summary.push('\n');
        }

        // Take end
        summary.push_str("\n[... content trimmed ...]\n\n");

        for line in lines.iter().rev().take(3).rev() {
            summary.push_str(line);
            summary.push('\n');
        }

        if summary.len() > max_chars {
            summary.truncate(max_chars);
            summary.push_str("...");
        }

        summary
    }

    fn add_persistent_context(&self, context: &mut String, token_budget: usize) {
        // Add important context that should persist across queries
        let persistent = vec![
            "Project Overview: This is a comprehensive documentation generation task.",
            "Documentation Standards: Use clear headings, code examples, and explanations.",
            "Target Audience: Developers who need to understand and use this codebase.",
        ];

        for item in persistent {
            if token_budget > item.len() / 4 {
                context.push_str(item);
                context.push_str("\n");
            }
        }
    }

    pub fn optimize_context(&mut self) {
        // Remove low-priority old entries if we're running out of space
        let total_tokens: usize = self.entries.values().map(|e| e.tokens).sum();

        if total_tokens > self.max_context_tokens * 2 {
            info!("Optimizing context: {} tokens exceeds limit", total_tokens);

            // Remove old low-priority entries
            let mut entries_to_remove = Vec::new();
            for (id, entry) in &self.entries {
                if entry.priority == QueryPriority::Low && !self.recent_history.contains(id) {
                    entries_to_remove.push(*id);
                }
            }

            for id in entries_to_remove {
                self.entries.remove(&id);
                debug!("Removed low-priority entry {}", id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_manager_basic() {
        let mut manager = ContextManager::new(10000);

        manager.add_response(1, "This is a test response about functions and APIs");
        manager.add_response(2, "Another response about classes and structures");

        assert_eq!(manager.entries.len(), 2);
        assert!(manager.recent_history.contains(&1));
        assert!(manager.recent_history.contains(&2));
    }

    #[tokio::test]
    async fn test_context_retrieval() {
        let mut manager = ContextManager::new(10000);

        manager.add_response(1, "Response about project overview");
        manager.add_response(2, "Response about architecture");

        let query = Query {
            id: 3,
            description: "Test query".to_string(),
            prompt: "Test prompt".to_string(),
            context_spec: ContextSpec::WithPrevious(vec![1, 2]),
            priority: QueryPriority::Medium,
            estimated_tokens: 100,
            target: crate::planner::QueryTarget::Overview,
        };

        let context = manager.get_context_for_query(&query).await;
        assert!(context.contains("Query 1"));
        assert!(context.contains("Query 2"));
    }

    #[tokio::test]
    async fn test_summarization() {
        let manager = ContextManager::new(10000);
        let long_text = "a".repeat(1000);
        let summary = manager.summarize_text(&long_text, 100);

        assert!(summary.len() <= 103); // 100 + "..."
        assert!(summary.contains("trimmed"));
    }
}
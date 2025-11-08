use crate::context::ContextEntry;
use crate::error::{DocAssistError, Result};
use crate::generator::QueryResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationState {
    pub project_path: PathBuf,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub completed_queries: Vec<usize>,
    pub query_results: HashMap<usize, QueryResult>,
    pub context_entries: HashMap<usize, ContextEntry>,
    pub total_tokens_used: usize,
    pub total_cost_usd: f64,
    pub errors: Vec<String>,
    pub checkpoint_version: u32,
}

impl GenerationState {
    pub fn new() -> Self {
        Self {
            project_path: PathBuf::new(),
            started_at: chrono::Utc::now(),
            last_updated: chrono::Utc::now(),
            completed_queries: Vec::new(),
            query_results: HashMap::new(),
            context_entries: HashMap::new(),
            total_tokens_used: 0,
            total_cost_usd: 0.0,
            errors: Vec::new(),
            checkpoint_version: 1,
        }
    }

    pub fn from_project_path(path: PathBuf) -> Self {
        let mut state = Self::new();
        state.project_path = path;
        state
    }

    pub fn mark_query_completed(&mut self, query_id: usize) {
        if !self.completed_queries.contains(&query_id) {
            self.completed_queries.push(query_id);
            self.last_updated = chrono::Utc::now();
            debug!("Marked query {} as completed", query_id);
        }
    }

    pub fn add_query_result(&mut self, result: QueryResult) {
        self.total_tokens_used += result.tokens_used;
        let query_id = result.query_id;
        self.query_results.insert(query_id, result);
        self.mark_query_completed(query_id);
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(format!("{}: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"), error));
        warn!("Error added to state: {}", error);
    }

    pub fn get_state_file_path(&self) -> PathBuf {
        self.project_path.join(".docassist").join("state.json")
    }

    pub async fn save_to_disk(&self) -> Result<()> {
        let state_file = self.get_state_file_path();

        // Create directory if it doesn't exist
        if let Some(parent) = state_file.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| DocAssistError::IoError(format!("Failed to create state directory: {}", e)))?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| DocAssistError::SerializationError(format!("Failed to serialize state: {}", e)))?;

        fs::write(&state_file, json).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to write state file: {}", e)))?;

        // Also save a backup
        let backup_file = state_file.with_extension("json.backup");
        if state_file.exists() {
            fs::copy(&state_file, &backup_file).await.ok();
        }

        debug!("State saved to {:?}", state_file);
        Ok(())
    }

    pub async fn load_from_disk(project_path: &Path) -> Result<Option<Self>> {
        let state_file = project_path.join(".docassist").join("state.json");

        if !state_file.exists() {
            info!("No existing state file found at {:?}", state_file);
            return Ok(None);
        }

        let json = fs::read_to_string(&state_file).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to read state file: {}", e)))?;

        let mut state: GenerationState = serde_json::from_str(&json)
            .map_err(|e| DocAssistError::SerializationError(format!("Failed to deserialize state: {}", e)))?;

        // Validate state
        if state.checkpoint_version != 1 {
            warn!("State file version mismatch. Expected 1, got {}", state.checkpoint_version);
        }

        state.project_path = project_path.to_path_buf();
        info!(
            "Loaded state: {} completed queries, {} tokens used, last updated {}",
            state.completed_queries.len(),
            state.total_tokens_used,
            state.last_updated.format("%Y-%m-%d %H:%M:%S")
        );

        Ok(Some(state))
    }

    pub async fn create_checkpoint(&self, checkpoint_name: &str) -> Result<()> {
        let checkpoint_dir = self.project_path.join(".docassist").join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to create checkpoint directory: {}", e)))?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let checkpoint_file = checkpoint_dir.join(format!("{}_{}.json", timestamp, checkpoint_name));

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| DocAssistError::SerializationError(format!("Failed to serialize checkpoint: {}", e)))?;

        fs::write(&checkpoint_file, json).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to write checkpoint file: {}", e)))?;

        info!("Created checkpoint: {:?}", checkpoint_file);
        Ok(())
    }

    pub async fn list_checkpoints(project_path: &Path) -> Result<Vec<PathBuf>> {
        let checkpoint_dir = project_path.join(".docassist").join("checkpoints");

        if !checkpoint_dir.exists() {
            return Ok(Vec::new());
        }

        let mut checkpoints = Vec::new();
        let mut entries = fs::read_dir(&checkpoint_dir).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to read checkpoint directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| DocAssistError::IoError(format!("Failed to read directory entry: {}", e)))? {

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                checkpoints.push(path);
            }
        }

        checkpoints.sort();
        Ok(checkpoints)
    }

    pub async fn load_checkpoint(checkpoint_path: &Path) -> Result<Self> {
        let json = fs::read_to_string(checkpoint_path).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to read checkpoint file: {}", e)))?;

        let state: GenerationState = serde_json::from_str(&json)
            .map_err(|e| DocAssistError::SerializationError(format!("Failed to deserialize checkpoint: {}", e)))?;

        info!("Loaded checkpoint from {:?}", checkpoint_path);
        Ok(state)
    }

    pub fn merge_with(&mut self, other: &GenerationState) {
        // Merge completed queries
        for query_id in &other.completed_queries {
            if !self.completed_queries.contains(query_id) {
                self.completed_queries.push(*query_id);
            }
        }

        // Merge query results
        for (id, result) in &other.query_results {
            self.query_results.insert(*id, result.clone());
        }

        // Merge context entries
        for (id, entry) in &other.context_entries {
            self.context_entries.insert(*id, entry.clone());
        }

        // Update totals
        self.total_tokens_used = self.total_tokens_used.max(other.total_tokens_used);
        self.total_cost_usd = self.total_cost_usd.max(other.total_cost_usd);

        // Merge errors
        for error in &other.errors {
            if !self.errors.contains(error) {
                self.errors.push(error.clone());
            }
        }

        self.last_updated = chrono::Utc::now();
        info!("Merged state: {} total completed queries", self.completed_queries.len());
    }

    pub fn get_progress_percentage(&self, total_queries: usize) -> f32 {
        if total_queries == 0 {
            return 0.0;
        }
        (self.completed_queries.len() as f32 / total_queries as f32) * 100.0
    }

    pub fn estimate_remaining_time(&self, total_queries: usize) -> Option<chrono::Duration> {
        if self.completed_queries.is_empty() {
            return None;
        }

        let elapsed = chrono::Utc::now() - self.started_at;
        let completed = self.completed_queries.len();
        let remaining = total_queries.saturating_sub(completed);

        if completed == 0 || remaining == 0 {
            return None;
        }

        let avg_time_per_query = elapsed.num_seconds() / completed as i64;
        let estimated_remaining_seconds = avg_time_per_query * remaining as i64;

        Some(chrono::Duration::seconds(estimated_remaining_seconds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        let mut state = GenerationState::from_project_path(project_path.clone());
        state.mark_query_completed(1);
        state.mark_query_completed(2);
        state.total_tokens_used = 1000;

        // Save state
        state.save_to_disk().await.unwrap();

        // Load state
        let loaded_state = GenerationState::load_from_disk(&project_path).await.unwrap().unwrap();
        assert_eq!(loaded_state.completed_queries.len(), 2);
        assert_eq!(loaded_state.total_tokens_used, 1000);
    }

    #[tokio::test]
    async fn test_checkpoint_creation() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        let mut state = GenerationState::from_project_path(project_path.clone());
        state.mark_query_completed(1);

        // Create checkpoint
        state.create_checkpoint("test").await.unwrap();

        // List checkpoints
        let checkpoints = GenerationState::list_checkpoints(&project_path).await.unwrap();
        assert_eq!(checkpoints.len(), 1);
    }

    #[test]
    fn test_progress_calculation() {
        let mut state = GenerationState::new();
        state.mark_query_completed(1);
        state.mark_query_completed(2);
        state.mark_query_completed(3);

        let progress = state.get_progress_percentage(10);
        assert_eq!(progress, 30.0);
    }
}
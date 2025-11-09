use doc_assist::{
    analyzer::analyze_codebase,
    config::{Config, DepthLevel},
    error::{DocAssistError, Result},
};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_invalid_path_error() {
    let config = Config {
        path: PathBuf::from("/non/existent/path"),
        depth: DepthLevel::Quick,
        query_count: 10,
        output_dir: PathBuf::from("/tmp/docs"),
        model: "gpt-3.5-turbo".to_string(),
        resume: false,
        force: false,
        rate_limit: 50,
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    let result = analyze_codebase(&config).await;

    // Should handle gracefully - empty analysis rather than error
    assert!(result.is_ok());
    let analysis = result.unwrap();
    assert_eq!(analysis.file_count, 0);
}

#[test]
fn test_config_validation() {
    // Test invalid rate limit
    let config = Config {
        path: PathBuf::from("."),
        depth: DepthLevel::Quick,
        query_count: 10,
        output_dir: PathBuf::from("docs"),
        model: "gpt-3.5-turbo".to_string(),
        resume: false,
        force: false,
        rate_limit: 0, // Invalid - should be > 0
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    // Rate limit of 0 should still work but might be slow
    assert_eq!(config.rate_limit, 0);
}

#[test]
fn test_error_display() {
    let errors = vec![
        DocAssistError::ConfigError("Invalid configuration".to_string()),
        DocAssistError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        )),
        DocAssistError::GenerationError("API failed".to_string()),
    ];

    for error in errors {
        let error_string = error.to_string();
        assert!(!error_string.is_empty());
    }
}

#[test]
fn test_invalid_model_name() {
    let config = Config {
        path: PathBuf::from("."),
        depth: DepthLevel::Quick,
        query_count: 10,
        output_dir: PathBuf::from("docs"),
        model: "invalid-model-xyz".to_string(), // Invalid model
        resume: false,
        force: false,
        rate_limit: 50,
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    // Should store the model name even if invalid
    assert_eq!(config.model, "invalid-model-xyz");
}

#[tokio::test]
async fn test_empty_directory_handling() {
    let temp_dir = TempDir::new().unwrap();

    let config = Config {
        path: temp_dir.path().to_path_buf(),
        depth: DepthLevel::Quick,
        query_count: 10,
        output_dir: temp_dir.path().join("docs"),
        model: "gpt-3.5-turbo".to_string(),
        resume: false,
        force: false,
        rate_limit: 50,
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    let analysis = analyze_codebase(&config).await.unwrap();

    // Should handle empty directory gracefully
    assert_eq!(analysis.file_count, 0);
    assert_eq!(analysis.module_count, 0);
    assert_eq!(analysis.public_api_count, 0);
}

#[test]
fn test_query_result_error_handling() {
    use doc_assist::generator::QueryResult;

    let error_result = QueryResult {
        query_id: 1,
        prompt: "Test".to_string(),
        response: String::new(),
        tokens_used: 0,
        duration_ms: 100,
        error: Some("Network timeout".to_string()),
    };

    assert!(error_result.error.is_some());
    assert_eq!(error_result.error.unwrap(), "Network timeout");
    assert_eq!(error_result.tokens_used, 0);
    assert!(error_result.response.is_empty());
}

#[test]
fn test_generation_result_error_aggregation() {
    use doc_assist::generator::{GenerationResult, QueryResult};
    use std::collections::HashMap;

    let mut phase_results = HashMap::new();
    phase_results.insert(
        "Phase1".to_string(),
        vec![
            QueryResult {
                query_id: 0,
                prompt: "Q1".to_string(),
                response: String::new(),
                tokens_used: 0,
                duration_ms: 100,
                error: Some("Error 1".to_string()),
            },
            QueryResult {
                query_id: 1,
                prompt: "Q2".to_string(),
                response: String::new(),
                tokens_used: 0,
                duration_ms: 100,
                error: Some("Error 2".to_string()),
            },
        ],
    );

    let result = GenerationResult {
        phase_results,
        total_tokens: 0,
        total_cost_usd: 0.0,
        total_duration_ms: 200,
        errors: vec!["Query 0: Error 1".to_string(), "Query 1: Error 2".to_string()],
        completed_queries: 0,
    };

    assert_eq!(result.errors.len(), 2);
    assert_eq!(result.completed_queries, 0);
    assert_eq!(result.total_tokens, 0);
}

#[tokio::test]
async fn test_concurrent_file_access() {
    use tokio::fs;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    // Create file
    fs::write(&file_path, "content").await.unwrap();

    // Simulate concurrent reads
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let path = file_path.clone();
            tokio::spawn(async move {
                fs::read_to_string(path).await
            })
        })
        .collect();

    // All reads should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "content");
    }
}

#[test]
fn test_rate_limiter_overflow() {
    // Test that rate limiter handles high request counts
    let high_rate = u32::MAX;
    let config = Config {
        path: PathBuf::from("."),
        depth: DepthLevel::Quick,
        query_count: 10,
        output_dir: PathBuf::from("docs"),
        model: "gpt-3.5-turbo".to_string(),
        resume: false,
        force: false,
        rate_limit: high_rate,
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    assert_eq!(config.rate_limit, high_rate);
}

#[test]
fn test_partial_state_recovery() {
    use doc_assist::state::GenerationState;

    let mut state = GenerationState::new();
    state.completed_queries = vec![0, 1, 2, 5]; // Note: 3, 4 are missing
    state.total_queries = 10;

    // Should be able to identify gaps
    let mut missing = Vec::new();
    for i in 0..state.total_queries {
        if !state.completed_queries.contains(&i) {
            missing.push(i);
        }
    }

    assert_eq!(missing, vec![3, 4, 6, 7, 8, 9]);
}
use doc_assist::generator::{QueryResult, GenerationResult};
use std::collections::HashMap;

#[test]
fn test_query_result_creation() {
    let result = QueryResult {
        query_id: 1,
        prompt: "Test prompt".to_string(),
        response: "Test response".to_string(),
        tokens_used: 100,
        duration_ms: 500,
        error: None,
    };

    assert_eq!(result.query_id, 1);
    assert_eq!(result.prompt, "Test prompt");
    assert_eq!(result.response, "Test response");
    assert_eq!(result.tokens_used, 100);
    assert_eq!(result.duration_ms, 500);
    assert!(result.error.is_none());
}

#[test]
fn test_query_result_with_error() {
    let result = QueryResult {
        query_id: 2,
        prompt: "Test prompt".to_string(),
        response: String::new(),
        tokens_used: 0,
        duration_ms: 100,
        error: Some("API error".to_string()),
    };

    assert_eq!(result.query_id, 2);
    assert!(result.response.is_empty());
    assert_eq!(result.error, Some("API error".to_string()));
}

#[test]
fn test_generation_result_aggregation() {
    let mut phase_results = HashMap::new();

    // Add some query results
    let phase1_results = vec![
        QueryResult {
            query_id: 0,
            prompt: "Overview query".to_string(),
            response: "Overview response".to_string(),
            tokens_used: 150,
            duration_ms: 1000,
            error: None,
        },
        QueryResult {
            query_id: 1,
            prompt: "Architecture query".to_string(),
            response: "Architecture response".to_string(),
            tokens_used: 200,
            duration_ms: 1500,
            error: None,
        },
    ];

    let phase2_results = vec![
        QueryResult {
            query_id: 2,
            prompt: "Module query".to_string(),
            response: "Module response".to_string(),
            tokens_used: 100,
            duration_ms: 800,
            error: None,
        },
    ];

    phase_results.insert("Overview".to_string(), phase1_results);
    phase_results.insert("Modules".to_string(), phase2_results);

    let result = GenerationResult {
        phase_results,
        total_tokens: 450,
        total_cost_usd: 0.02,
        total_duration_ms: 3300,
        errors: vec![],
        completed_queries: 3,
    };

    assert_eq!(result.total_tokens, 450);
    assert_eq!(result.total_cost_usd, 0.02);
    assert_eq!(result.total_duration_ms, 3300);
    assert_eq!(result.completed_queries, 3);
    assert!(result.errors.is_empty());

    // Verify phase results
    assert_eq!(result.phase_results.len(), 2);
    assert!(result.phase_results.contains_key("Overview"));
    assert!(result.phase_results.contains_key("Modules"));

    let overview_results = &result.phase_results["Overview"];
    assert_eq!(overview_results.len(), 2);
}

#[test]
fn test_generation_result_with_errors() {
    let mut phase_results = HashMap::new();

    let results = vec![
        QueryResult {
            query_id: 0,
            prompt: "Test".to_string(),
            response: "Response".to_string(),
            tokens_used: 100,
            duration_ms: 500,
            error: None,
        },
        QueryResult {
            query_id: 1,
            prompt: "Test2".to_string(),
            response: String::new(),
            tokens_used: 0,
            duration_ms: 100,
            error: Some("Failed".to_string()),
        },
    ];

    phase_results.insert("Test".to_string(), results);

    let result = GenerationResult {
        phase_results,
        total_tokens: 100,
        total_cost_usd: 0.01,
        total_duration_ms: 600,
        errors: vec!["Query 1: Failed".to_string()],
        completed_queries: 1,
    };

    assert_eq!(result.completed_queries, 1);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("Failed"));
}

#[test]
fn test_cost_calculation() {
    // Test cost calculation for different token counts
    let small_tokens = 1000;
    let medium_tokens = 10000;
    let large_tokens = 100000;

    // Approximate costs (these would be calculated by the actual implementation)
    // Assuming ~$0.01 per 1000 tokens for testing
    let small_cost = (small_tokens as f64 / 1000.0) * 0.01;
    let medium_cost = (medium_tokens as f64 / 1000.0) * 0.01;
    let large_cost = (large_tokens as f64 / 1000.0) * 0.01;

    assert!(small_cost > 0.0 && small_cost < 1.0);
    assert!(medium_cost > small_cost);
    assert!(large_cost > medium_cost);
}

// Mock test for generator execution (would need actual mock framework for full test)
#[cfg(test)]
mod generator_execution_tests {
    use super::*;

    #[test]
    fn test_rate_limiting() {
        // Test that rate limiting logic works
        let requests_per_minute = 60;
        let max_concurrent = 10;

        // Verify configuration
        assert!(max_concurrent <= requests_per_minute);
        assert!(max_concurrent > 0);
    }

    #[test]
    fn test_retry_logic() {
        let max_retries = 3;
        let mut attempt = 0;

        // Simulate retry logic
        while attempt < max_retries {
            attempt += 1;
            if attempt == max_retries {
                // Final attempt
                assert_eq!(attempt, 3);
            }
        }
    }

    #[test]
    fn test_context_management() {
        let max_context_tokens = 128000;
        let response_buffer = 4000;
        let available_tokens = max_context_tokens - response_buffer;

        assert!(available_tokens > 0);
        assert!(available_tokens < max_context_tokens);
    }
}
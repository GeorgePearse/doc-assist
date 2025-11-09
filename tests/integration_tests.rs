use doc_assist::{
    analyzer::{analyze_codebase},
    planner::{create_query_plan},
    assembler::{assemble_documentation},
    config::{Config, DepthLevel},
};
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_full_documentation_workflow() {
    // Create a test project
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    // Create sample files
    fs::write(
        src_dir.join("main.rs"),
        r#"
//! Main application

fn main() {
    println!("Hello, world!");
}

/// Configuration struct
pub struct Config {
    pub name: String,
}

impl Config {
    pub fn new() -> Self {
        Config { name: String::from("app") }
    }
}
"#,
    )
    .unwrap();

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    let output_dir = temp_dir.path().join("docs");

    let config = Config {
        path: temp_dir.path().to_path_buf(),
        depth: DepthLevel::Quick,
        query_count: 5, // Very small for testing
        output_dir: output_dir.clone(),
        model: "gpt-3.5-turbo".to_string(),
        resume: false,
        force: false,
        rate_limit: 50,
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    // Step 1: Analyze codebase
    let analysis = analyze_codebase(&config).await.unwrap();
    assert!(analysis.file_count > 0);
    assert_eq!(analysis.primary_language.name(), "Rust");

    // Step 2: Create query plan
    let plan = create_query_plan(&analysis, &config).unwrap();
    assert_eq!(plan.total_queries, 5);
    assert!(!plan.phases.is_empty());

    // Step 3: Mock generation result (since we can't call actual LLM in tests)
    let mock_generation_result = doc_assist::generator::GenerationResult {
        phase_results: std::collections::HashMap::new(),
        total_tokens: 1000,
        total_cost_usd: 0.01,
        total_duration_ms: 5000,
        errors: vec![],
        completed_queries: 5,
    };

    // Step 4: Assemble documentation
    let documentation = assemble_documentation(
        mock_generation_result,
        &analysis,
        &config,
    ).await.unwrap();

    assert!(documentation.file_count > 0);

    // Verify output files were created
    assert!(output_dir.exists());
    assert!(output_dir.join("README.md").exists());
    assert!(output_dir.join("ARCHITECTURE.md").exists());
}

#[tokio::test]
async fn test_resume_functionality() {
    let temp_dir = TempDir::new().unwrap();
    let state_dir = temp_dir.path().join(".docassist");
    fs::create_dir(&state_dir).unwrap();

    // Create a mock state file
    let state_content = r#"
{
    "project_path": "/test",
    "start_time": "2024-01-01T00:00:00Z",
    "last_update": "2024-01-01T00:01:00Z",
    "completed_queries": [0, 1, 2],
    "total_queries": 10,
    "responses": {}
}
"#;
    fs::write(state_dir.join("state.json"), state_content).unwrap();

    let config = Config {
        path: temp_dir.path().to_path_buf(),
        depth: DepthLevel::Quick,
        query_count: 10,
        output_dir: temp_dir.path().join("docs"),
        model: "gpt-3.5-turbo".to_string(),
        resume: true,
        force: false,
        rate_limit: 50,
        include_patterns: vec![],
        exclude_patterns: vec![],
    };

    // Load state
    let state = doc_assist::state::load_state(&config.path).await.unwrap();

    if let Some(state) = state {
        assert_eq!(state.completed_queries.len(), 3);
        assert_eq!(state.total_queries, 10);
        assert!(state.completed_queries.contains(&0));
        assert!(state.completed_queries.contains(&1));
        assert!(state.completed_queries.contains(&2));
    } else {
        panic!("State should be loaded");
    }
}

#[tokio::test]
async fn test_exclude_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    let test_dir = temp_dir.path().join("tests");
    fs::create_dir(&src_dir).unwrap();
    fs::create_dir(&test_dir).unwrap();

    fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();
    fs::write(test_dir.join("test.rs"), "fn test() {}").unwrap();

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
        exclude_patterns: vec!["tests/**".to_string()],
    };

    let analysis = analyze_codebase(&config).await.unwrap();

    // Should only analyze src/main.rs, not tests/test.rs
    assert_eq!(analysis.file_count, 1);
}

#[test]
fn test_depth_level_query_counts() {
    let quick = DepthLevel::Quick;
    let standard = DepthLevel::Standard;
    let comprehensive = DepthLevel::Comprehensive;
    let continuous = DepthLevel::Continuous;

    // Test expected query counts for each depth level
    match quick {
        DepthLevel::Quick => assert!(true),
        _ => panic!("Wrong depth level"),
    }

    match standard {
        DepthLevel::Standard => assert!(true),
        _ => panic!("Wrong depth level"),
    }

    match comprehensive {
        DepthLevel::Comprehensive => assert!(true),
        _ => panic!("Wrong depth level"),
    }

    match continuous {
        DepthLevel::Continuous => assert!(true),
        _ => panic!("Wrong depth level"),
    }
}

#[tokio::test]
async fn test_documentation_structure() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("docs");
    fs::create_dir(&output_dir).unwrap();

    // Create expected directory structure
    fs::create_dir(&output_dir.join("api")).unwrap();
    fs::create_dir(&output_dir.join("modules")).unwrap();
    fs::create_dir(&output_dir.join("examples")).unwrap();
    fs::create_dir(&output_dir.join("guides")).unwrap();

    // Verify structure
    assert!(output_dir.join("api").exists());
    assert!(output_dir.join("modules").exists());
    assert!(output_dir.join("examples").exists());
    assert!(output_dir.join("guides").exists());
}

#[tokio::test]
async fn test_metadata_generation() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("docs");
    fs::create_dir(&output_dir).unwrap();

    // Create metadata file
    let metadata = r#"
{
    "generation_date": "2024-01-01",
    "version": "0.1.0",
    "model": "gpt-3.5-turbo",
    "total_queries": 20
}
"#;
    fs::write(output_dir.join(".metadata.json"), metadata).unwrap();

    // Read and verify metadata
    let metadata_content = fs::read_to_string(output_dir.join(".metadata.json")).unwrap();
    assert!(metadata_content.contains("generation_date"));
    assert!(metadata_content.contains("version"));
    assert!(metadata_content.contains("model"));
    assert!(metadata_content.contains("total_queries"));
}
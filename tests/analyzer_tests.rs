use doc_assist::analyzer::{analyze_codebase, CodebaseAnalysis, Module, ApiItem, ApiKind};
use doc_assist::config::{Config, DepthLevel};
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_analyze_rust_codebase() {
    // Create a temporary directory with sample Rust code
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    // Create main.rs
    fs::write(
        src_dir.join("main.rs"),
        r#"
//! Main application entry point

use std::collections::HashMap;

/// Main function that starts the application
fn main() {
    println!("Hello, world!");
    let config = Config::new();
    run_app(config);
}

/// Configuration struct for the application
#[derive(Debug)]
pub struct Config {
    pub name: String,
    pub version: u32,
}

impl Config {
    /// Creates a new config with defaults
    pub fn new() -> Self {
        Config {
            name: String::from("app"),
            version: 1,
        }
    }
}

/// Runs the application with the given configuration
pub fn run_app(config: Config) {
    println!("Running {} v{}", config.name, config.version);
}

mod utils {
    /// Helper function to format strings
    pub fn format_string(s: &str) -> String {
        s.to_uppercase()
    }
}
"#,
    )
    .unwrap();

    // Create lib.rs
    fs::write(
        src_dir.join("lib.rs"),
        r#"
//! Library module for the application

pub mod analyzer;
pub mod generator;

/// Main library struct
pub struct DocAssist {
    name: String,
}

impl DocAssist {
    /// Creates a new instance
    pub fn new(name: String) -> Self {
        DocAssist { name }
    }

    /// Gets the name
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Public trait for documentation
pub trait Documentable {
    fn document(&self) -> String;
}
"#,
    )
    .unwrap();

    // Create Cargo.toml
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
"#,
    )
    .unwrap();

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

    // Verify analysis results
    assert_eq!(analysis.primary_language.name(), "Rust");
    assert!(analysis.file_count >= 2);
    assert!(analysis.module_count >= 1);
    assert!(analysis.public_api_count >= 5); // At least 5 public functions/structs
    assert!(analysis.total_lines > 50);

    // Verify some modules were found
    assert!(!analysis.modules.is_empty());

    // Verify some APIs were found
    assert!(!analysis.api_items.is_empty());

    // Check for specific API items
    let has_config_struct = analysis.api_items.iter().any(|api|
        api.name == "Config" && matches!(api.kind, ApiKind::Struct)
    );
    assert!(has_config_struct, "Should find Config struct");

    let has_main_fn = analysis.api_items.iter().any(|api|
        api.name == "main" && matches!(api.kind, ApiKind::Function)
    );
    assert!(has_main_fn, "Should find main function");
}

#[tokio::test]
async fn test_analyze_empty_directory() {
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

    assert_eq!(analysis.file_count, 0);
    assert_eq!(analysis.module_count, 0);
    assert_eq!(analysis.public_api_count, 0);
    assert_eq!(analysis.total_lines, 0);
}

#[tokio::test]
async fn test_analyze_with_exclude_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    // Create files
    fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();
    fs::write(src_dir.join("test.rs"), "fn test() {}").unwrap();
    fs::write(src_dir.join("bench.rs"), "fn bench() {}").unwrap();

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
        exclude_patterns: vec!["**/test.rs".to_string(), "**/bench.rs".to_string()],
    };

    let analysis = analyze_codebase(&config).await.unwrap();

    // Should only find main.rs
    assert_eq!(analysis.file_count, 1);
}

#[test]
fn test_codebase_analysis_methods() {
    let mut analysis = CodebaseAnalysis::new(
        doc_assist::analyzer::Language::Rust,
    );

    // Add a module
    let module = Module {
        name: "test_module".to_string(),
        path: PathBuf::from("src/test.rs"),
        public_items: 5,
        private_items: 3,
        documentation: Some("Test module".to_string()),
        dependencies: vec![],
    };
    analysis.modules.push(module);

    // Add an API item
    let api = ApiItem {
        name: "test_function".to_string(),
        kind: ApiKind::Function,
        module: "test_module".to_string(),
        path: PathBuf::from("src/test.rs"),
        signature: "fn test_function() -> String".to_string(),
        doc_comment: Some("Test function".to_string()),
        line_number: 20,
    };
    analysis.api_items.push(api);

    // Update counts
    analysis.file_count = 1;
    analysis.module_count = 1;
    analysis.public_api_count = 1;
    analysis.total_lines = 100;

    // Verify
    assert_eq!(analysis.modules.len(), 1);
    assert_eq!(analysis.api_items.len(), 1);
    assert_eq!(analysis.file_count, 1);
    assert_eq!(analysis.module_count, 1);
    assert_eq!(analysis.public_api_count, 1);
}
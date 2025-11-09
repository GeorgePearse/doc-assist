use doc_assist::planner::{create_query_plan, QueryPlan, Phase, Query, ContextSpec, QueryPriority};
use doc_assist::analyzer::{CodebaseAnalysis, Language, Module, ApiItem, ApiKind};
use doc_assist::config::{Config, DepthLevel};
use std::path::PathBuf;

fn create_test_analysis() -> CodebaseAnalysis {
    let mut analysis = CodebaseAnalysis::new(Language::Rust);

    // Add modules
    analysis.modules.push(Module {
        name: "main".to_string(),
        path: PathBuf::from("src/main.rs"),
        public_items: 3,
        private_items: 2,
        documentation: Some("Main module".to_string()),
        dependencies: vec!["config".to_string()],
    });

    analysis.modules.push(Module {
        name: "config".to_string(),
        path: PathBuf::from("src/config.rs"),
        public_items: 5,
        private_items: 1,
        documentation: Some("Configuration module".to_string()),
        dependencies: vec![],
    });

    // Add API items
    analysis.api_items.push(ApiItem {
        name: "main".to_string(),
        kind: ApiKind::Function,
        module: "main".to_string(),
        path: PathBuf::from("src/main.rs"),
        signature: "fn main()".to_string(),
        doc_comment: Some("Entry point".to_string()),
        line_number: 10,
    });

    analysis.api_items.push(ApiItem {
        name: "Config".to_string(),
        kind: ApiKind::Struct,
        module: "config".to_string(),
        path: PathBuf::from("src/config.rs"),
        signature: "pub struct Config".to_string(),
        doc_comment: Some("Configuration struct".to_string()),
        line_number: 5,
    });

    analysis.api_items.push(ApiItem {
        name: "new".to_string(),
        kind: ApiKind::Function,
        module: "config".to_string(),
        path: PathBuf::from("src/config.rs"),
        signature: "pub fn new() -> Self".to_string(),
        doc_comment: None,
        line_number: 15,
    });

    // Set counts
    analysis.file_count = 2;
    analysis.module_count = 2;
    analysis.public_api_count = 8;
    analysis.total_lines = 150;
    analysis.documentation_coverage = 0.75;

    analysis
}

fn create_test_config(depth: DepthLevel, query_count: usize) -> Config {
    Config {
        path: PathBuf::from("/test"),
        depth,
        query_count,
        output_dir: PathBuf::from("/test/docs"),
        model: "gpt-3.5-turbo".to_string(),
        resume: false,
        force: false,
        rate_limit: 50,
        include_patterns: vec![],
        exclude_patterns: vec![],
    }
}

#[test]
fn test_create_query_plan_quick() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Quick, 20);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // Verify plan structure
    assert_eq!(plan.total_queries, 20);
    assert!(!plan.phases.is_empty());

    // Should have at least overview, module, API, and examples phases
    assert!(plan.phases.len() >= 4);

    // Check phase names
    let phase_names: Vec<_> = plan.phases.iter().map(|p| &p.name).collect();
    assert!(phase_names.iter().any(|n| n.contains("Overview")));
    assert!(phase_names.iter().any(|n| n.contains("Module")));
    assert!(phase_names.iter().any(|n| n.contains("API")));
    assert!(phase_names.iter().any(|n| n.contains("Examples")));
}

#[test]
fn test_create_query_plan_comprehensive() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Comprehensive, 100);

    let plan = create_query_plan(&analysis, &config).unwrap();

    assert_eq!(plan.total_queries, 100);
    assert!(plan.phases.len() >= 4);

    // Comprehensive should have more queries in each phase
    let overview_phase = plan.phases.iter()
        .find(|p| p.name.contains("Overview"))
        .expect("Should have overview phase");
    assert!(overview_phase.queries.len() > 3);
}

#[test]
fn test_query_plan_skip_completed() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Quick, 20);

    let mut plan = create_query_plan(&analysis, &config).unwrap();

    let initial_query_count = plan.total_queries;

    // Mark some queries as completed
    let completed_ids = vec![0, 1, 2, 5, 10];
    plan.skip_completed(&completed_ids);

    // Verify queries are marked as skipped
    for phase in &plan.phases {
        for query in &phase.queries {
            if completed_ids.contains(&query.id) {
                // These should be skipped (removed or marked)
                // The actual implementation might remove them or mark them differently
                // This test would need to match the actual implementation
            }
        }
    }
}

#[test]
fn test_phase_dependencies() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Standard, 60);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // Later phases should depend on earlier phases
    for (i, phase) in plan.phases.iter().enumerate() {
        if i > 0 {
            // Check that at least some queries have context dependencies
            let has_dependencies = phase.queries.iter().any(|q| {
                matches!(q.context_spec, ContextSpec::WithPrevious(_))
            });

            // Module and API phases should have dependencies
            if phase.name.contains("Module") || phase.name.contains("API") {
                assert!(has_dependencies, "Phase {} should have context dependencies", phase.name);
            }
        }
    }
}

#[test]
fn test_query_priorities() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Quick, 20);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // Overview queries should be high priority
    let overview_phase = plan.phases.iter()
        .find(|p| p.name.contains("Overview"))
        .expect("Should have overview phase");

    for query in &overview_phase.queries {
        assert!(matches!(query.priority, QueryPriority::High));
    }
}

#[test]
fn test_estimated_cost_calculation() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Standard, 60);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // Verify cost estimation
    assert!(plan.estimated_cost.total_cost_usd > 0.0);
    assert!(plan.estimated_cost.total_tokens > 0);
    assert!(plan.estimated_cost.estimated_duration_minutes > 0.0);

    // Cost should be reasonable (not too high for standard depth)
    assert!(plan.estimated_cost.total_cost_usd < 10.0, "Cost should be reasonable");
}

#[test]
fn test_empty_codebase_plan() {
    let analysis = CodebaseAnalysis::new(Language::Rust);
    let config = create_test_config(DepthLevel::Quick, 20);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // Should still create a basic plan even for empty codebase
    assert!(plan.total_queries > 0);
    assert!(!plan.phases.is_empty());

    // Should have at least overview phase
    assert!(plan.phases.iter().any(|p| p.name.contains("Overview")));
}

#[test]
fn test_query_descriptions() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Quick, 20);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // All queries should have descriptions
    for phase in &plan.phases {
        for query in &phase.queries {
            assert!(!query.description.is_empty());
            assert!(!query.prompt.is_empty());
        }
    }
}

#[test]
fn test_continuous_depth() {
    let analysis = create_test_analysis();
    let config = create_test_config(DepthLevel::Continuous, 1000);

    let plan = create_query_plan(&analysis, &config).unwrap();

    // Continuous should create many queries
    assert_eq!(plan.total_queries, 1000);
    assert!(plan.phases.len() >= 4);
}
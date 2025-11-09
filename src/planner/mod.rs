use crate::analyzer::{CodebaseAnalysis, ApiItem, ApiKind};
use crate::config::Config;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    pub total_queries: usize,
    pub phases: Vec<Phase>,
    pub estimated_cost: EstimatedCost,
    pub estimated_duration: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub name: String,
    pub description: String,
    pub queries: Vec<Query>,
    pub dependencies: Vec<usize>, // Phase indices this depends on
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub id: usize,
    pub description: String,
    pub prompt: String,
    pub context_spec: ContextSpec,
    pub priority: QueryPriority,
    pub estimated_tokens: usize,
    pub target: QueryTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextSpec {
    /// Include full context history
    Full,
    /// Include specific previous queries by ID
    WithPrevious(Vec<usize>),
    /// No previous context needed
    None,
    /// Include code snippets from specific files
    WithCode { paths: Vec<String>, max_lines: usize },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueryPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryTarget {
    Overview,
    Module(String),
    Api(String),
    Example(String),
    Integration,
    Troubleshooting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedCost {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_cost_usd: f64,
}

impl QueryPlan {
    /// Skip queries that have already been completed (for resuming)
    pub fn skip_completed(&mut self, completed_ids: &[usize]) {
        for phase in &mut self.phases {
            phase.queries.retain(|q| !completed_ids.contains(&q.id));
        }
        self.total_queries = self.phases.iter().map(|p| p.queries.len()).sum();
    }
}

/// Create a comprehensive query plan based on the codebase analysis
pub fn create_query_plan(analysis: &CodebaseAnalysis, config: &Config) -> Result<QueryPlan> {
    info!("Creating query plan for {} queries", config.query_count);

    let mut query_id_counter = 0;
    let mut phases = Vec::new();

    // Phase 1: Overview and Architecture (10-15% of queries)
    let overview_count = (config.query_count as f32 * 0.15).round() as usize;
    let overview_phase = create_overview_phase(
        analysis,
        overview_count,
        &mut query_id_counter,
    )?;
    phases.push(overview_phase);

    // Phase 2: Module Documentation (30-40% of queries)
    let module_count = (config.query_count as f32 * 0.35).round() as usize;
    let module_phase = create_module_phase(
        analysis,
        module_count,
        &mut query_id_counter,
    )?;
    phases.push(module_phase);

    // Phase 3: API Documentation (30-40% of queries)
    let api_count = (config.query_count as f32 * 0.35).round() as usize;
    let api_phase = create_api_phase(
        analysis,
        api_count,
        &mut query_id_counter,
    )?;
    phases.push(api_phase);

    // Phase 4: Examples and Deep Dives (15-20% of queries)
    let remaining = config.query_count.saturating_sub(query_id_counter);
    let examples_phase = create_examples_phase(
        analysis,
        remaining,
        &mut query_id_counter,
    )?;
    phases.push(examples_phase);

    // Calculate estimated cost
    let estimated_cost = calculate_estimated_cost(&phases, &config.model);

    // Calculate estimated duration (3 seconds per query average)
    let estimated_duration = std::time::Duration::from_secs((config.query_count * 3) as u64);

    Ok(QueryPlan {
        total_queries: query_id_counter,
        phases,
        estimated_cost,
        estimated_duration,
    })
}

fn create_overview_phase(
    analysis: &CodebaseAnalysis,
    query_count: usize,
    id_counter: &mut usize,
) -> Result<Phase> {
    let mut queries = Vec::new();

    // Query 1: Project purpose and overview
    queries.push(Query {
        id: next_id(id_counter),
        description: "Analyze project purpose and goals".to_string(),
        prompt: format!(
            "Analyze this codebase and provide a comprehensive overview:\n\n\
             Project structure:\n\
             - Primary language: {}\n\
             - Total files: {}\n\
             - Modules: {}\n\
             - Public APIs: {}\n\
             - Lines of code: {}\n\n\
             Key dependencies: {:?}\n\n\
             Provide:\n\
             1. The high-level purpose and goals of this project\n\
             2. The target users or use cases\n\
             3. Key features and capabilities\n\
             4. Project maturity and development status",
            analysis.primary_language,
            analysis.file_count,
            analysis.module_count,
            analysis.public_api_count,
            analysis.total_lines,
            analysis.dependencies.iter().take(10).map(|d| &d.name).collect::<Vec<_>>()
        ),
        context_spec: ContextSpec::None,
        priority: QueryPriority::Critical,
        estimated_tokens: 3000,
        target: QueryTarget::Overview,
    });

    // Query 2: Architecture patterns
    queries.push(Query {
        id: next_id(id_counter),
        description: "Identify architectural patterns".to_string(),
        prompt: format!(
            "Analyze the architectural patterns and design decisions in this codebase:\n\n\
             Module structure: {} modules found\n\
             File organization pattern detected\n\n\
             Identify and explain:\n\
             1. Main architectural patterns used (MVC, layered, microservices, etc.)\n\
             2. Design patterns employed\n\
             3. Code organization strategy\n\
             4. Separation of concerns approach\n\
             5. Key architectural decisions and trade-offs",
            analysis.module_count
        ),
        context_spec: ContextSpec::WithPrevious(vec![0]),
        priority: QueryPriority::High,
        estimated_tokens: 3500,
        target: QueryTarget::Overview,
    });

    // Query 3: Entry points and main flows
    queries.push(Query {
        id: next_id(id_counter),
        description: "Document entry points and flows".to_string(),
        prompt: "Identify and document the main entry points and execution flows:\n\
                 1. Application entry points (main functions, CLI commands, API endpoints)\n\
                 2. Primary execution flows\n\
                 3. Initialization and setup procedures\n\
                 4. Core processing loops or pipelines\n\
                 5. How different components interact".to_string(),
        context_spec: ContextSpec::WithPrevious(vec![0, 1]),
        priority: QueryPriority::High,
        estimated_tokens: 3000,
        target: QueryTarget::Overview,
    });

    // Additional overview queries based on available quota
    if query_count > 3 {
        // Dependencies and external integrations
        queries.push(Query {
            id: next_id(id_counter),
            description: "Analyze dependencies and integrations".to_string(),
            prompt: format!(
                "Analyze the project dependencies and external integrations:\n\n\
                 Dependencies found: {:?}\n\n\
                 Document:\n\
                 1. Purpose of key dependencies\n\
                 2. External services or APIs integrated\n\
                 3. Database or storage systems used\n\
                 4. Third-party libraries and their roles\n\
                 5. Dependency management approach",
                analysis.dependencies.iter().take(20).map(|d| &d.name).collect::<Vec<_>>()
            ),
            context_spec: ContextSpec::WithPrevious(vec![0]),
            priority: QueryPriority::Medium,
            estimated_tokens: 2500,
            target: QueryTarget::Overview,
        });
    }

    if query_count > 4 {
        // Configuration and deployment
        queries.push(Query {
            id: next_id(id_counter),
            description: "Document configuration and deployment".to_string(),
            prompt: "Analyze and document the configuration and deployment aspects:\n\
                     1. Configuration management approach\n\
                     2. Environment variables and settings\n\
                     3. Build process and requirements\n\
                     4. Deployment strategies and targets\n\
                     5. Docker/container setup if present\n\
                     6. CI/CD pipeline configuration".to_string(),
            context_spec: ContextSpec::WithPrevious(vec![0]),
            priority: QueryPriority::Medium,
            estimated_tokens: 2500,
            target: QueryTarget::Overview,
        });
    }

    // Add more overview queries if needed to reach target count
    let remaining = query_count.saturating_sub(queries.len());
    for i in 0..remaining.min(5) {
        let topics = [
            "Security considerations and authentication approach",
            "Performance characteristics and optimization strategies",
            "Testing strategy and coverage",
            "Error handling and logging approach",
            "Data models and database schema",
        ];

        queries.push(Query {
            id: next_id(id_counter),
            description: format!("Analyze {}", topics[i].to_lowercase()),
            prompt: format!(
                "Analyze and document the following aspect of the codebase:\n\n{}\n\n\
                 Provide specific details and examples from the code.",
                topics[i]
            ),
            context_spec: ContextSpec::WithPrevious(vec![0, 1]),
            priority: QueryPriority::Low,
            estimated_tokens: 2000,
            target: QueryTarget::Overview,
        });
    }

    Ok(Phase {
        name: "Overview & Architecture".to_string(),
        description: "High-level project documentation".to_string(),
        queries: queries.into_iter().take(query_count).collect(),
        dependencies: vec![],
    })
}

fn create_module_phase(
    analysis: &CodebaseAnalysis,
    query_count: usize,
    id_counter: &mut usize,
) -> Result<Phase> {
    let mut queries = Vec::new();

    // Sort modules by importance (public items count and size)
    let mut modules = analysis.modules.clone();
    modules.sort_by_key(|m| (m.public_items.len(), m.line_count));
    modules.reverse();

    // Calculate queries per module
    let module_count = modules.len().min(query_count);
    let queries_per_module = if module_count > 0 {
        query_count / module_count
    } else {
        0
    };

    for module in modules.iter().take(module_count) {
        // Primary module query
        queries.push(Query {
            id: next_id(id_counter),
            description: format!("Document module '{}'", module.name),
            prompt: format!(
                "Document the '{}' module in detail:\n\n\
                 Module location: {}\n\
                 Public items: {} items\n\
                 Lines of code: {}\n\
                 Exported items: {:?}\n\n\
                 Provide:\n\
                 1. Module purpose and responsibilities\n\
                 2. Key functionality provided\n\
                 3. Main data structures and types\n\
                 4. How this module fits into the overall architecture\n\
                 5. Dependencies and interactions with other modules\n\
                 6. Usage examples",
                module.name,
                module.path.display(),
                module.public_items.len(),
                module.line_count,
                module.public_items.iter().take(10).collect::<Vec<_>>()
            ),
            context_spec: ContextSpec::WithPrevious(vec![0, 1]),
            priority: QueryPriority::High,
            estimated_tokens: 3500,
            target: QueryTarget::Module(module.name.clone()),
        });

        // Additional detailed query if we have quota
        if queries_per_module > 1 && !module.public_items.is_empty() {
            queries.push(Query {
                id: next_id(id_counter),
                description: format!("Deep dive into '{}' internals", module.name),
                prompt: format!(
                    "Provide a deep dive into the '{}' module implementation:\n\n\
                     Focus on:\n\
                     1. Internal architecture and design patterns\n\
                     2. Key algorithms and data structures\n\
                     3. Performance considerations\n\
                     4. Error handling strategies\n\
                     5. Thread safety and concurrency aspects\n\
                     6. Testing approach for this module",
                    module.name
                ),
                context_spec: ContextSpec::WithPrevious(vec![*id_counter - 1]),
                priority: QueryPriority::Medium,
                estimated_tokens: 3000,
                target: QueryTarget::Module(module.name.clone()),
            });
        }

        if queries.len() >= query_count {
            break;
        }
    }

    Ok(Phase {
        name: "Module Documentation".to_string(),
        description: "Detailed documentation for each module".to_string(),
        queries: queries.into_iter().take(query_count).collect(),
        dependencies: vec![0], // Depends on overview phase
    })
}

fn create_api_phase(
    analysis: &CodebaseAnalysis,
    query_count: usize,
    id_counter: &mut usize,
) -> Result<Phase> {
    let mut queries = Vec::new();

    // Group APIs by module
    let mut api_groups: HashMap<String, Vec<&ApiItem>> = HashMap::new();
    for api in &analysis.public_apis {
        api_groups.entry(api.module.clone()).or_default().push(api);
    }

    // Sort APIs by priority (functions and classes first)
    let mut prioritized_apis: Vec<&ApiItem> = analysis.public_apis.iter().collect();
    prioritized_apis.sort_by_key(|api| match api.kind {
        ApiKind::Class | ApiKind::Struct => 0,
        ApiKind::Function | ApiKind::Method => 1,
        ApiKind::Trait | ApiKind::Interface => 2,
        _ => 3,
    });

    // Document APIs in batches
    let apis_per_query = 5; // Document 5 APIs per query for efficiency
    let mut api_chunks: Vec<Vec<&ApiItem>> = Vec::new();

    for chunk in prioritized_apis.chunks(apis_per_query) {
        api_chunks.push(chunk.to_vec());
        if api_chunks.len() >= query_count {
            break;
        }
    }

    for (i, api_chunk) in api_chunks.iter().enumerate().take(query_count) {
        let api_descriptions: Vec<String> = api_chunk.iter().map(|api| {
            format!("- {} '{}' in {} ({})",
                match api.kind {
                    ApiKind::Function => "Function",
                    ApiKind::Struct => "Struct",
                    ApiKind::Class => "Class",
                    ApiKind::Enum => "Enum",
                    ApiKind::Trait => "Trait",
                    ApiKind::Interface => "Interface",
                    ApiKind::Method => "Method",
                    ApiKind::Constant => "Constant",
                    ApiKind::Type => "Type",
                },
                api.name,
                api.module,
                api.signature
            )
        }).collect();

        queries.push(Query {
            id: next_id(id_counter),
            description: format!("Document API batch {}", i + 1),
            prompt: format!(
                "Document the following public APIs in detail:\n\n{}\n\n\
                 For each API, provide:\n\
                 1. Purpose and functionality\n\
                 2. Parameters/arguments and their types\n\
                 3. Return values and types\n\
                 4. Usage examples\n\
                 5. Error conditions and handling\n\
                 6. Performance characteristics\n\
                 7. Related APIs or alternatives",
                api_descriptions.join("\n")
            ),
            context_spec: ContextSpec::WithPrevious(vec![0, 1]),
            priority: if i < 5 { QueryPriority::High } else { QueryPriority::Medium },
            estimated_tokens: 4000,
            target: QueryTarget::Api(format!("batch_{}", i)),
        });
    }

    // Add specialized API documentation queries if we have remaining quota
    let remaining = query_count.saturating_sub(queries.len());
    if remaining > 0 && !analysis.public_apis.is_empty() {
        // Document API relationships and patterns
        queries.push(Query {
            id: next_id(id_counter),
            description: "Document API relationships".to_string(),
            prompt: "Analyze and document the relationships between public APIs:\n\
                     1. Common usage patterns\n\
                     2. API composition and chaining\n\
                     3. Typical workflows using multiple APIs\n\
                     4. Design patterns in the API structure\n\
                     5. Best practices for using the APIs together".to_string(),
            context_spec: ContextSpec::Full,
            priority: QueryPriority::Medium,
            estimated_tokens: 3000,
            target: QueryTarget::Api("relationships".to_string()),
        });
    }

    Ok(Phase {
        name: "API Reference".to_string(),
        description: "Detailed API documentation".to_string(),
        queries: queries.into_iter().take(query_count).collect(),
        dependencies: vec![0, 1], // Depends on overview and modules
    })
}

fn create_examples_phase(
    analysis: &CodebaseAnalysis,
    query_count: usize,
    id_counter: &mut usize,
) -> Result<Phase> {
    let mut queries = Vec::new();

    // Query 1: Basic usage examples
    if query_count > 0 {
        queries.push(Query {
            id: next_id(id_counter),
            description: "Create basic usage examples".to_string(),
            prompt: "Create comprehensive usage examples for this codebase:\n\
                     1. Getting started example\n\
                     2. Basic usage patterns\n\
                     3. Common use cases\n\
                     4. Step-by-step tutorials\n\
                     5. Code snippets for typical scenarios\n\
                     Include complete, runnable examples with explanations.".to_string(),
            context_spec: ContextSpec::Full,
            priority: QueryPriority::High,
            estimated_tokens: 4000,
            target: QueryTarget::Example("basic".to_string()),
        });
    }

    // Query 2: Advanced examples
    if query_count > 1 {
        queries.push(Query {
            id: next_id(id_counter),
            description: "Create advanced usage examples".to_string(),
            prompt: "Create advanced usage examples and patterns:\n\
                     1. Complex integration scenarios\n\
                     2. Performance optimization techniques\n\
                     3. Advanced configuration options\n\
                     4. Custom extensions or plugins\n\
                     5. Production deployment examples\n\
                     6. Scaling strategies".to_string(),
            context_spec: ContextSpec::Full,
            priority: QueryPriority::Medium,
            estimated_tokens: 4000,
            target: QueryTarget::Example("advanced".to_string()),
        });
    }

    // Query 3: Integration guide
    if query_count > 2 {
        queries.push(Query {
            id: next_id(id_counter),
            description: "Create integration guide".to_string(),
            prompt: "Create a comprehensive integration guide:\n\
                     1. How to integrate with other systems\n\
                     2. API integration examples\n\
                     3. Database setup and configuration\n\
                     4. Third-party service connections\n\
                     5. Webhook and event handling\n\
                     6. Authentication and authorization setup".to_string(),
            context_spec: ContextSpec::Full,
            priority: QueryPriority::Medium,
            estimated_tokens: 3500,
            target: QueryTarget::Integration,
        });
    }

    // Query 4: Troubleshooting guide
    if query_count > 3 {
        queries.push(Query {
            id: next_id(id_counter),
            description: "Create troubleshooting guide".to_string(),
            prompt: "Create a comprehensive troubleshooting guide:\n\
                     1. Common errors and solutions\n\
                     2. Debugging techniques\n\
                     3. Performance troubleshooting\n\
                     4. Configuration issues\n\
                     5. Compatibility problems\n\
                     6. FAQ section\n\
                     7. How to report issues".to_string(),
            context_spec: ContextSpec::Full,
            priority: QueryPriority::Low,
            estimated_tokens: 3500,
            target: QueryTarget::Troubleshooting,
        });
    }

    // Additional example queries for specific modules
    let remaining = query_count.saturating_sub(queries.len());
    for (_i, module) in analysis.modules.iter().take(remaining).enumerate() {
        queries.push(Query {
            id: next_id(id_counter),
            description: format!("Examples for '{}'", module.name),
            prompt: format!(
                "Create specific examples for the '{}' module:\n\
                 1. Basic usage of module functionality\n\
                 2. Common patterns and idioms\n\
                 3. Integration with other modules\n\
                 4. Testing strategies\n\
                 5. Edge cases and error handling\n\
                 Provide complete, runnable code examples.",
                module.name
            ),
            context_spec: ContextSpec::Full,
            priority: QueryPriority::Low,
            estimated_tokens: 3000,
            target: QueryTarget::Example(module.name.clone()),
        });

        if queries.len() >= query_count {
            break;
        }
    }

    Ok(Phase {
        name: "Examples & Guides".to_string(),
        description: "Usage examples and integration guides".to_string(),
        queries: queries.into_iter().take(query_count).collect(),
        dependencies: vec![0, 1, 2], // Depends on all previous phases
    })
}

fn next_id(counter: &mut usize) -> usize {
    let id = *counter;
    *counter += 1;
    id
}

fn calculate_estimated_cost(phases: &[Phase], model: &str) -> EstimatedCost {
    let total_input_tokens: usize = phases.iter()
        .flat_map(|p| &p.queries)
        .map(|q| q.estimated_tokens)
        .sum();

    let total_output_tokens = total_input_tokens / 3; // Rough estimate

    // Pricing per million tokens (approximate)
    let (input_price, output_price) = if model.contains("claude") {
        (3.0, 15.0) // Claude 3.5 Sonnet pricing
    } else if model.contains("gpt-4") {
        (10.0, 30.0) // GPT-4 pricing
    } else {
        (0.5, 1.5) // Cheaper model fallback
    };

    let input_cost = (total_input_tokens as f64 / 1_000_000.0) * input_price;
    let output_cost = (total_output_tokens as f64 / 1_000_000.0) * output_price;

    EstimatedCost {
        input_tokens: total_input_tokens,
        output_tokens: total_output_tokens,
        total_cost_usd: input_cost + output_cost,
    }
}
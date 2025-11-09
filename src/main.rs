use anyhow::Result;
use clap::{Parser, ValueEnum};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use tracing::error;
use tracing_subscriber;

mod analyzer;
mod assembler;
mod config;
mod context;
mod error;
mod generator;
mod planner;
mod state;

use crate::config::Config;
use crate::error::DocAssistError;

#[derive(Parser)]
#[command(name = "docassist")]
#[command(author, version, about = "Automatic documentation generator using LLMs", long_about = None)]
struct Cli {
    /// Path to the codebase to document
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Documentation depth level
    #[arg(short, long, value_enum, default_value = "standard")]
    depth: CliDepthLevel,

    /// Custom query count (overrides depth level)
    #[arg(short = 'q', long)]
    queries: Option<usize>,

    /// Output directory for generated documentation
    #[arg(short, long, default_value = "docs")]
    output: PathBuf,

    /// LLM model to use
    #[arg(
        short,
        long,
        env = "DOCASSIST_MODEL",
        default_value = "gpt-3.5-turbo"  // Change default to a known working model
    )]
    model: String,

    /// Resume from previous run
    #[arg(short, long)]
    resume: bool,

    /// Force regeneration (ignore cache)
    #[arg(short, long)]
    force: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Dry run - show query plan without executing
    #[arg(long)]
    dry_run: bool,

    /// Maximum queries per minute (rate limiting)
    #[arg(long, default_value = "50")]
    rate_limit: u32,

    /// Include patterns (glob patterns for files to include)
    #[arg(long)]
    include: Vec<String>,

    /// Exclude patterns (glob patterns for files to exclude)
    #[arg(long)]
    exclude: Vec<String>,
}

// DepthLevel wrapper for CLI
#[derive(Clone, Debug, ValueEnum)]
enum CliDepthLevel {
    /// Quick pass - 20 queries for basic documentation
    Quick,
    /// Standard depth - 60 queries for comprehensive docs
    Standard,
    /// Comprehensive - 100+ queries for exhaustive documentation
    Comprehensive,
    /// Continuous - Keep adding detail until manually stopped
    Continuous,
}

impl CliDepthLevel {
    fn to_depth_level(&self) -> crate::config::DepthLevel {
        match self {
            CliDepthLevel::Quick => crate::config::DepthLevel::Quick,
            CliDepthLevel::Standard => crate::config::DepthLevel::Standard,
            CliDepthLevel::Comprehensive => crate::config::DepthLevel::Comprehensive,
            CliDepthLevel::Continuous => crate::config::DepthLevel::Continuous,
        }
    }

    fn to_query_count(&self) -> usize {
        match self {
            CliDepthLevel::Quick => 20,
            CliDepthLevel::Standard => 60,
            CliDepthLevel::Comprehensive => 100,
            CliDepthLevel::Continuous => 1000, // Will be limited by user interruption
        }
    }

    fn description(&self) -> &str {
        match self {
            CliDepthLevel::Quick => "Quick overview with basic API docs",
            CliDepthLevel::Standard => "Comprehensive documentation with examples",
            CliDepthLevel::Comprehensive => "Exhaustive documentation with deep dives",
            CliDepthLevel::Continuous => "Continuous generation until stopped",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        "doc_assist=debug,info"
    } else {
        "doc_assist=info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    // Print header
    println!();
    println!("{}", "╔══════════════════════════════════════╗".bright_blue());
    println!("{}", "║        doc-assist v0.1.0             ║".bright_blue());
    println!("{}", "║  Automatic Documentation Generator   ║".bright_blue());
    println!("{}", "╚══════════════════════════════════════╝".bright_blue());
    println!();

    // Validate path exists
    if !cli.path.exists() {
        error!("Path does not exist: {}", cli.path.display());
        std::process::exit(1);
    }

    if !cli.path.is_dir() {
        error!("Path is not a directory: {}", cli.path.display());
        std::process::exit(1);
    }

    // Create configuration
    let query_count = cli.queries.unwrap_or_else(|| cli.depth.to_query_count());

    let config = Config {
        path: cli.path.clone(),
        depth: cli.depth.to_depth_level(),
        query_count,
        output_dir: cli.output.clone(),
        model: cli.model.clone(),
        resume: cli.resume,
        force: cli.force,
        rate_limit: cli.rate_limit,
        include_patterns: cli.include,
        exclude_patterns: cli.exclude,
    };

    // Print configuration
    println!("{}", "Configuration:".bold());
    println!("  {} {}", "Path:".cyan(), config.path.display());
    println!("  {} {} ({})", "Depth:".cyan(), format!("{:?}", cli.depth), cli.depth.description());
    println!("  {} {} queries", "Target:".cyan(), query_count);
    println!("  {} {}", "Model:".cyan(), config.model);
    println!("  {} {}", "Output:".cyan(), config.output_dir.display());
    if cli.resume {
        println!("  {} {}", "Mode:".cyan(), "Resuming from previous run".yellow());
    }
    println!();

    // Check for API key and validate model configuration
    let api_key_validation = validate_api_configuration(&config.model);
    if let Err(e) = api_key_validation {
        error!("{}", e);
        println!("\n{}", "Configuration Help:".bold());
        println!("  1. Set your API key:");
        if config.model.contains("claude") {
            println!("     export ANTHROPIC_API_KEY=your-api-key");
        } else if config.model.contains("gpt") {
            println!("     export OPENAI_API_KEY=your-api-key");
        } else {
            println!("     export ANTHROPIC_API_KEY=your-api-key  # for Claude models");
            println!("     export OPENAI_API_KEY=your-api-key     # for GPT models");
        }
        println!("\n  2. Verify your model name is correct:");
        println!("     - Claude models: claude-3-opus, claude-3-sonnet, etc.");
        println!("     - OpenAI models: gpt-4, gpt-3.5-turbo, etc.");
        println!("\n  3. Run docassist with a valid model:");
        println!("     docassist --model gpt-3.5-turbo .");
        std::process::exit(1);
    }

    // Execute main workflow
    match run_documentation_generation(config, cli.dry_run).await {
        Ok(_) => {
            println!();
            println!("{} Documentation generated successfully!", "✓".green().bold());
            println!("  View your docs at: {}", cli.output.display());
            Ok(())
        }
        Err(e) => {
            // Print error to both stderr and stdout for visibility
            println!();
            eprintln!("{} Documentation generation failed!", "✗".red().bold());
            eprintln!("{}", e.to_string().red());

            // Also print to stdout for better test capture
            println!("{} Documentation generation failed!", "✗".red().bold());
            println!("{}", e);

            std::process::exit(1);
        }
    }
}

async fn run_documentation_generation(config: Config, dry_run: bool) -> Result<()> {
    // Phase 1: Analysis
    println!("{} Analyzing codebase...", "►".cyan().bold());
    let pb = create_spinner("Scanning files...");
    let analysis = analyzer::analyze_codebase(&config).await?;
    pb.finish_with_message(format!(
        "✓ Found {} files, {} modules",
        analysis.file_count, analysis.module_count
    ));

    // Print analysis summary
    println!("\n{}", "Analysis Summary:".bold());
    println!("  {} {}", "Primary language:".cyan(), analysis.primary_language.name());
    println!("  {} {} files", "Total files:".cyan(), analysis.file_count);
    println!("  {} {} modules", "Modules found:".cyan(), analysis.module_count);
    println!("  {} {} public APIs", "Public APIs:".cyan(), analysis.public_api_count);
    if !analysis.existing_docs.is_empty() {
        println!("  {} {} existing docs found", "Documentation:".cyan(), analysis.existing_docs.len());
    }
    println!();

    // Phase 2: Planning
    println!("{} Creating documentation plan...", "►".cyan().bold());
    let pb = create_spinner("Generating query plan...");
    let plan = planner::create_query_plan(&analysis, &config)?;
    pb.finish_with_message(format!("✓ Created plan with {} queries", plan.total_queries));

    // Print plan summary
    println!("\n{}", "Query Plan:".bold());
    for (i, phase) in plan.phases.iter().enumerate() {
        println!(
            "  {} {} ({} queries)",
            format!("Phase {}:", i + 1).cyan(),
            phase.name,
            phase.queries.len()
        );
    }

    if dry_run {
        println!("\n{}", "Dry run mode - Query details:".yellow().bold());
        for phase in &plan.phases {
            println!("\n{}", phase.name.bold());
            for (i, query) in phase.queries.iter().take(3).enumerate() {
                println!("  Query {}: {}", i + 1, query.description.dimmed());
            }
            if phase.queries.len() > 3 {
                println!("  ... and {} more", phase.queries.len() - 3);
            }
        }
        println!("\n{} Dry run complete. Use without --dry-run to execute.", "✓".green().bold());
        return Ok(());
    }

    // Check for resume state
    let existing_state = if config.resume {
        state::load_state(&config.path).await?
    } else {
        None
    };

    if let Some(ref state) = existing_state {
        println!(
            "{} Resuming from previous run ({}/{} queries completed)",
            "►".yellow().bold(),
            state.completed_queries.len(),
            state.total_queries
        );
    }

    // Phase 3: Generation
    println!("\n{} Generating documentation...", "►".cyan().bold());
    println!("{}", "This may take several minutes...".dimmed());

    let responses = generator::execute_queries(
        &plan,
        &config,
        existing_state.as_ref(),
    ).await?;

    // Phase 4: Assembly
    println!("\n{} Assembling documentation...", "►".cyan().bold());
    let pb = create_spinner("Creating markdown files...");
    let documentation = assembler::assemble_documentation(
        responses,
        &analysis,
        &config,
    ).await?;
    pb.finish_with_message(format!(
        "✓ Generated {} documentation files",
        documentation.file_count
    ));

    // Clean up state if successful
    if config.resume {
        state::clear_state(&config.path).await?;
    }

    // Print statistics
    println!("\n{}", "Generation Statistics:".bold());
    println!("  {} {}", "Total queries:".cyan(), plan.total_queries);
    println!("  {} ${:.2}", "Estimated cost:".cyan(), plan.estimated_cost.total_cost_usd);
    println!("  {} {}", "Files generated:".cyan(), documentation.file_count);

    Ok(())
}

fn validate_api_configuration(model: &str) -> Result<()> {
    let supported_models = [
        // OpenAI models
        ("gpt-4", "OPENAI_API_KEY"),
        ("gpt-4-turbo", "OPENAI_API_KEY"),
        ("gpt-3.5-turbo", "OPENAI_API_KEY"),
        ("gpt-4o", "OPENAI_API_KEY"),
        ("gpt-4o-mini", "OPENAI_API_KEY"),
        // Anthropic models
        ("claude-3-opus", "ANTHROPIC_API_KEY"),
        ("claude-3-sonnet", "ANTHROPIC_API_KEY"),
        ("claude-3-haiku", "ANTHROPIC_API_KEY"),
        ("claude-2", "ANTHROPIC_API_KEY"),
        ("claude-instant", "ANTHROPIC_API_KEY"),
    ];

    // Determine which API key is needed
    let required_key = if model.contains("claude") {
        "ANTHROPIC_API_KEY"
    } else if model.contains("gpt") {
        "OPENAI_API_KEY"
    } else {
        // Check if it's a known model
        let known_model = supported_models.iter()
            .find(|(m, _)| model.contains(m));

        if let Some((_, key)) = known_model {
            key
        } else {
            return Err(anyhow::anyhow!(
                "Unknown model '{}'. Supported models include: gpt-4, gpt-3.5-turbo, claude-3-opus, claude-3-sonnet",
                model
            ));
        }
    };

    // Check if the required API key is set
    if std::env::var(required_key).is_err() {
        return Err(anyhow::anyhow!(
            "API key '{}' not found for model '{}'. Please set it as an environment variable.",
            required_key, model
        ));
    }

    // Validate the API key format (basic check)
    let api_key = std::env::var(required_key).unwrap();
    if api_key.is_empty() || api_key == "your-api-key" {
        return Err(anyhow::anyhow!(
            "Invalid API key for '{}'. The key appears to be empty or a placeholder.",
            required_key
        ));
    }

    Ok(())
}

fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(message.to_string());
    pb
}
use crate::error::{DocAssistError, Result};
use crate::generator::{GenerationResult, QueryResult};
// QueryCategory removed as it's not used in this module
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationStructure {
    pub readme: String,
    pub architecture: String,
    pub modules: HashMap<String, ModuleDoc>,
    pub api_reference: String,
    pub guides: HashMap<String, String>,
    pub examples: Vec<Example>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDoc {
    pub name: String,
    pub overview: String,
    pub api_docs: Vec<ApiDoc>,
    pub examples: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDoc {
    pub name: String,
    pub signature: String,
    pub description: String,
    pub parameters: String,
    pub returns: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub title: String,
    pub description: String,
    pub code: String,
    pub explanation: String,
}

pub struct DocumentAssembler {
    output_dir: PathBuf,
    project_name: String,
}

impl DocumentAssembler {
    pub fn new(output_dir: PathBuf, project_name: String) -> Self {
        Self {
            output_dir,
            project_name,
        }
    }

    pub async fn assemble_documentation(&self, generation_result: &GenerationResult) -> Result<()> {
        info!("Assembling documentation from {} completed queries", generation_result.completed_queries);

        // Validate we have actual content to work with
        if generation_result.completed_queries == 0 {
            return Err(DocAssistError::GenerationError(
                "Cannot assemble documentation: No queries completed successfully".to_string()
            ));
        }

        // Validate that we have non-empty responses
        let non_empty_responses = generation_result.phase_results.values()
            .flat_map(|results| results.iter())
            .filter(|r| r.error.is_none() && !r.response.is_empty())
            .count();

        if non_empty_responses == 0 {
            return Err(DocAssistError::GenerationError(
                "Cannot assemble documentation: All responses are empty or contain errors".to_string()
            ));
        }

        warn!("Found {} non-empty responses out of {} total queries",
              non_empty_responses,
              generation_result.phase_results.values()
                  .map(|r| r.len())
                  .sum::<usize>());

        // Create output directory structure
        self.create_directory_structure().await?;

        // Group results by category, filtering out empty responses
        let grouped_results = self.group_valid_results_by_category(generation_result);

        if grouped_results.is_empty() {
            return Err(DocAssistError::GenerationError(
                "Cannot assemble documentation: No valid content to include".to_string()
            ));
        }

        // Generate all documents in parallel using tokio::join!
        let (readme_result, architecture_result, module_result, api_result, guide_result, example_result) = tokio::join!(
            async {
                let readme = self.generate_readme(&grouped_results).await?;
                if !Self::is_content_meaningful(&readme) {
                    warn!("README content appears to be mostly template/placeholder text");
                }
                self.write_file("README.md", &readme).await
            },
            async {
                let architecture = self.generate_architecture_doc(&grouped_results).await?;
                if !Self::is_content_meaningful(&architecture) {
                    warn!("Architecture documentation appears to be mostly template text");
                }
                self.write_file("ARCHITECTURE.md", &architecture).await
            },
            self.generate_module_docs(&grouped_results),
            async {
                let api_reference = self.generate_api_reference(&grouped_results).await?;
                self.write_file("api/README.md", &api_reference).await
            },
            self.generate_guides(&grouped_results),
            self.generate_examples(&grouped_results)
        );

        // Check results from parallel execution
        readme_result?;
        architecture_result?;
        module_result?;
        api_result?;
        guide_result?;
        example_result?;

        // Generate index/table of contents
        let toc = self.generate_table_of_contents().await?;
        self.write_file("SUMMARY.md", &toc).await?;

        // Generate metadata file
        self.generate_metadata(generation_result).await?;

        info!("Documentation assembly complete. Output written to {:?}", self.output_dir);
        Ok(())
    }

    async fn create_directory_structure(&self) -> Result<()> {
        let dirs = ["api", "modules", "guides", "examples", "assets"];

        // Create all directories in parallel
        let tasks: Vec<_> = dirs.iter().map(|dir| {
            let dir_path = self.output_dir.join(dir);
            async move {
                fs::create_dir_all(&dir_path).await
                    .map_err(|e| DocAssistError::IoError(format!("Failed to create directory {:?}: {}", dir_path, e)))
            }
        }).collect();

        // Wait for all directories to be created
        for task in tasks {
            task.await?;
        }

        Ok(())
    }

    fn group_valid_results_by_category(&self, generation_result: &GenerationResult) -> HashMap<String, Vec<QueryResult>> {
        let mut grouped = HashMap::new();

        for (phase_name, phase_results) in &generation_result.phase_results {
            for result in phase_results {
                // Only include results with actual content
                if result.error.is_none() && !result.response.is_empty() {
                    let category = self.determine_category(phase_name, &result.prompt);
                    grouped.entry(category).or_insert_with(Vec::new).push(result.clone());
                }
            }
        }

        grouped
    }

    fn group_results_by_category(&self, generation_result: &GenerationResult) -> HashMap<String, Vec<QueryResult>> {
        let mut grouped = HashMap::new();

        for (phase_name, phase_results) in &generation_result.phase_results {
            for result in phase_results {
                let category = self.determine_category(phase_name, &result.prompt);
                grouped.entry(category).or_insert_with(Vec::new).push(result.clone());
            }
        }

        grouped
    }

    fn is_content_meaningful(content: &str) -> bool {
        // Check if content has actual documentation beyond template text
        let meaningful_lines = content.lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() &&
                !trimmed.starts_with('#') && // Ignore headers
                !trimmed.starts_with("TODO") &&
                !trimmed.starts_with("- Feature") &&
                !trimmed.contains("Add installation instructions") &&
                !trimmed.contains("Add usage examples")
            })
            .count();

        // Content should have at least 10 lines of meaningful text
        meaningful_lines >= 10
    }

    fn determine_category(&self, phase_name: &str, prompt: &str) -> String {
        if phase_name.contains("Overview") || prompt.contains("overview") {
            "overview".to_string()
        } else if phase_name.contains("Architecture") || prompt.contains("architecture") {
            "architecture".to_string()
        } else if phase_name.contains("Module") || prompt.contains("module") {
            "modules".to_string()
        } else if phase_name.contains("API") || prompt.contains("API") {
            "api".to_string()
        } else if phase_name.contains("Example") || prompt.contains("example") {
            "examples".to_string()
        } else if phase_name.contains("Guide") || prompt.contains("guide") {
            "guides".to_string()
        } else {
            "other".to_string()
        }
    }

    async fn generate_readme(&self, grouped_results: &HashMap<String, Vec<QueryResult>>) -> Result<String> {
        let mut readme = String::new();

        // Title and badges
        readme.push_str(&format!("# {}\n\n", self.project_name));
        readme.push_str("![Documentation](https://img.shields.io/badge/docs-auto--generated-blue)\n");
        readme.push_str(&format!("![Generated](https://img.shields.io/badge/generated-{}-green)\n\n", chrono::Utc::now().format("%Y--%m--%d")));

        // Overview section
        if let Some(overview_results) = grouped_results.get("overview") {
            readme.push_str("## Overview\n\n");
            for result in overview_results.iter().take(2) {
                readme.push_str(&result.response);
                readme.push_str("\n\n");
            }
        }

        // Quick Start
        readme.push_str("## Quick Start\n\n");
        readme.push_str("```bash\n# Installation\n");
        readme.push_str("# TODO: Add installation instructions\n\n");
        readme.push_str("# Basic Usage\n");
        readme.push_str("# TODO: Add usage examples\n```\n\n");

        // Features
        readme.push_str("## Features\n\n");
        readme.push_str("- Feature 1\n");
        readme.push_str("- Feature 2\n");
        readme.push_str("- Feature 3\n\n");

        // Documentation Structure
        readme.push_str("## Documentation\n\n");
        readme.push_str("- [Architecture](./ARCHITECTURE.md) - System architecture and design\n");
        readme.push_str("- [API Reference](./api/README.md) - Complete API documentation\n");
        readme.push_str("- [Modules](./modules/README.md) - Module documentation\n");
        readme.push_str("- [Guides](./guides/README.md) - How-to guides and tutorials\n");
        readme.push_str("- [Examples](./examples/README.md) - Code examples\n\n");

        // Contributing
        readme.push_str("## Contributing\n\n");
        readme.push_str("Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.\n\n");

        // License
        readme.push_str("## License\n\n");
        readme.push_str("See [LICENSE](./LICENSE) for details.\n\n");

        // Footer
        readme.push_str("---\n\n");
        readme.push_str("*This documentation was automatically generated using [doc-assist](https://github.com/yourusername/doc-assist)*\n");

        Ok(readme)
    }

    async fn generate_architecture_doc(&self, grouped_results: &HashMap<String, Vec<QueryResult>>) -> Result<String> {
        let mut doc = String::new();

        doc.push_str("# Architecture Documentation\n\n");
        doc.push_str(&format!("## {} Architecture Overview\n\n", self.project_name));

        if let Some(arch_results) = grouped_results.get("architecture") {
            for (i, result) in arch_results.iter().enumerate() {
                if i > 0 {
                    doc.push_str("\n---\n\n");
                }
                doc.push_str(&result.response);
                doc.push_str("\n\n");
            }
        }

        // Add diagrams section
        doc.push_str("## System Diagrams\n\n");
        doc.push_str("```mermaid\ngraph TB\n");
        doc.push_str("    A[Client] --> B[API Gateway]\n");
        doc.push_str("    B --> C[Service Layer]\n");
        doc.push_str("    C --> D[Data Layer]\n");
        doc.push_str("```\n\n");

        // Add design decisions
        doc.push_str("## Design Decisions\n\n");
        doc.push_str("### Key Architectural Choices\n\n");
        doc.push_str("- **Pattern**: Description of pattern used\n");
        doc.push_str("- **Rationale**: Why this was chosen\n");
        doc.push_str("- **Trade-offs**: What we gained and lost\n\n");

        Ok(doc)
    }

    async fn generate_module_docs(&self, grouped_results: &HashMap<String, Vec<QueryResult>>) -> Result<()> {
        if let Some(module_results) = grouped_results.get("modules") {
            // Create index for modules
            let mut index = String::new();
            index.push_str("# Module Documentation\n\n");
            index.push_str("This directory contains detailed documentation for each module.\n\n");
            index.push_str("## Available Modules\n\n");

            // Group module results by module name
            let mut modules_map: HashMap<String, Vec<&QueryResult>> = HashMap::new();
            for result in module_results {
                let module_name = self.extract_module_name(&result.prompt);
                modules_map.entry(module_name.clone()).or_insert_with(Vec::new).push(result);
            }

            // Generate documentation for each module
            for (module_name, results) in modules_map {
                let module_doc = self.generate_single_module_doc(&module_name, &results).await?;
                let file_name = format!("{}.md", module_name.to_lowercase().replace(' ', "_"));
                self.write_file(&format!("modules/{}", file_name), &module_doc).await?;

                index.push_str(&format!("- [{}](./{})\n", module_name, file_name));
            }

            self.write_file("modules/README.md", &index).await?;
        }

        Ok(())
    }

    async fn generate_single_module_doc(&self, module_name: &str, results: &[&QueryResult]) -> Result<String> {
        let mut doc = String::new();

        doc.push_str(&format!("# {} Module\n\n", module_name));

        for result in results {
            doc.push_str(&result.response);
            doc.push_str("\n\n");
        }

        Ok(doc)
    }

    async fn generate_api_reference(&self, grouped_results: &HashMap<String, Vec<QueryResult>>) -> Result<String> {
        let mut doc = String::new();

        doc.push_str("# API Reference\n\n");
        doc.push_str("Complete API documentation for all public interfaces.\n\n");

        if let Some(api_results) = grouped_results.get("api") {
            doc.push_str("## Table of Contents\n\n");

            // Group by API type
            let mut functions = Vec::new();
            let mut classes = Vec::new();
            let mut other = Vec::new();

            for result in api_results {
                if result.response.contains("function") || result.response.contains("Function") {
                    functions.push(result);
                } else if result.response.contains("class") || result.response.contains("Class") {
                    classes.push(result);
                } else {
                    other.push(result);
                }
            }

            // Generate sections
            if !functions.is_empty() {
                doc.push_str("## Functions\n\n");
                for func in functions {
                    doc.push_str(&func.response);
                    doc.push_str("\n\n---\n\n");
                }
            }

            if !classes.is_empty() {
                doc.push_str("## Classes\n\n");
                for class in classes {
                    doc.push_str(&class.response);
                    doc.push_str("\n\n---\n\n");
                }
            }

            if !other.is_empty() {
                doc.push_str("## Other APIs\n\n");
                for api in other {
                    doc.push_str(&api.response);
                    doc.push_str("\n\n---\n\n");
                }
            }
        }

        Ok(doc)
    }

    async fn generate_guides(&self, grouped_results: &HashMap<String, Vec<QueryResult>>) -> Result<()> {
        let guides = vec![
            ("getting-started", "Getting Started Guide"),
            ("installation", "Installation Guide"),
            ("configuration", "Configuration Guide"),
            ("deployment", "Deployment Guide"),
            ("troubleshooting", "Troubleshooting Guide"),
        ];

        let mut index = String::new();
        index.push_str("# Guides\n\n");
        index.push_str("Step-by-step guides for common tasks.\n\n");

        for (filename, title) in guides {
            let mut guide = String::new();
            guide.push_str(&format!("# {}\n\n", title));

            // Add relevant content from results
            if let Some(guide_results) = grouped_results.get("guides") {
                for result in guide_results {
                    if result.prompt.to_lowercase().contains(&filename.replace('-', " ")) {
                        guide.push_str(&result.response);
                        guide.push_str("\n\n");
                    }
                }
            }

            if guide.len() > title.len() + 10 {
                self.write_file(&format!("guides/{}.md", filename), &guide).await?;
                index.push_str(&format!("- [{}](./{}.md)\n", title, filename));
            }
        }

        self.write_file("guides/README.md", &index).await?;
        Ok(())
    }

    async fn generate_examples(&self, grouped_results: &HashMap<String, Vec<QueryResult>>) -> Result<()> {
        let mut index = String::new();
        index.push_str("# Examples\n\n");
        index.push_str("Code examples demonstrating usage.\n\n");

        if let Some(example_results) = grouped_results.get("examples") {
            for (i, result) in example_results.iter().enumerate() {
                let filename = format!("example_{:03}.md", i + 1);
                self.write_file(&format!("examples/{}", filename), &result.response).await?;
                index.push_str(&format!("- [Example {}](./{})\n", i + 1, filename));
            }
        }

        self.write_file("examples/README.md", &index).await?;
        Ok(())
    }

    async fn generate_table_of_contents(&self) -> Result<String> {
        let mut toc = String::new();

        toc.push_str("# Documentation Summary\n\n");
        toc.push_str("## Table of Contents\n\n");
        toc.push_str("- [Overview](./README.md)\n");
        toc.push_str("- [Architecture](./ARCHITECTURE.md)\n");
        toc.push_str("- [API Reference](./api/README.md)\n");
        toc.push_str("- [Modules](./modules/README.md)\n");
        toc.push_str("- [Guides](./guides/README.md)\n");
        toc.push_str("- [Examples](./examples/README.md)\n");

        Ok(toc)
    }

    async fn generate_metadata(&self, generation_result: &GenerationResult) -> Result<()> {
        let metadata = serde_json::json!({
            "project_name": self.project_name,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "total_queries": generation_result.completed_queries,
            "total_tokens": generation_result.total_tokens,
            "total_cost_usd": generation_result.total_cost_usd,
            "duration_ms": generation_result.total_duration_ms,
            "errors": generation_result.errors,
            "doc_assist_version": env!("CARGO_PKG_VERSION"),
        });

        let json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| DocAssistError::SerializationError(format!("Failed to serialize metadata: {}", e)))?;

        self.write_file(".metadata.json", &json).await?;
        Ok(())
    }

    async fn write_file(&self, relative_path: &str, content: &str) -> Result<()> {
        let file_path = self.output_dir.join(relative_path);

        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| DocAssistError::IoError(format!("Failed to create directory: {}", e)))?;
        }

        fs::write(&file_path, content).await
            .map_err(|e| DocAssistError::IoError(format!("Failed to write file {:?}: {}", file_path, e)))?;

        debug!("Wrote {} bytes to {:?}", content.len(), file_path);
        Ok(())
    }

    fn extract_module_name(&self, prompt: &str) -> String {
        // Try to extract module name from prompt
        if let Some(start) = prompt.find("module '") {
            if let Some(end) = prompt[start + 8..].find('\'') {
                return prompt[start + 8..start + 8 + end].to_string();
            }
        }

        if let Some(start) = prompt.find("Document the ") {
            let text = &prompt[start + 13..];
            if let Some(end) = text.find(" module") {
                return text[..end].to_string();
            }
        }

        "Unknown".to_string()
    }
}

/// Assemble documentation from generation results
pub async fn assemble_documentation(
    generation_result: GenerationResult,
    analysis: &crate::analyzer::CodebaseAnalysis,
    config: &crate::config::Config,
) -> Result<DocumentationOutput> {
    let assembler = DocumentAssembler::new(
        config.output_dir.clone(),
        analysis.name.clone(),
    );

    assembler.assemble_documentation(&generation_result).await?;

    Ok(DocumentationOutput {
        file_count: count_files(&config.output_dir).await?,
        output_dir: config.output_dir.clone(),
    })
}

#[derive(Debug)]
pub struct DocumentationOutput {
    pub file_count: usize,
    pub output_dir: std::path::PathBuf,
}

async fn count_files(dir: &std::path::Path) -> Result<usize> {
    let mut count = 0;
    let mut entries = fs::read_dir(dir).await
        .map_err(|e| DocAssistError::IoError(format!("Failed to read output directory: {}", e)))?;

    while let Some(entry) = entries.next_entry().await
        .map_err(|e| DocAssistError::IoError(format!("Failed to read directory entry: {}", e)))? {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("md") {
            count += 1;
        }
    }

    Ok(count)
}
pub mod analyzer;
pub mod assembler;
pub mod config;
pub mod context;
pub mod error;
pub mod generator;
pub mod planner;
pub mod state;

#[cfg(test)]
mod test_llm;


pub use config::{Config, DepthLevel};
pub use error::{DocAssistError, Result};

use crate::assembler::DocumentationStructure;

/// Main entry point for documentation generation
pub async fn generate_documentation(_config: Config) -> Result<DocumentationStructure> {
    // TODO: Implement the main workflow
    // For now, return a placeholder
    Ok(DocumentationStructure {
        readme: String::new(),
        architecture: String::new(),
        modules: std::collections::HashMap::new(),
        api_reference: String::new(),
        guides: std::collections::HashMap::new(),
        examples: vec![],
    })
}
use crate::config::Config;
use crate::error::{DocAssistError, Result};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

mod language;
mod parser;

pub use language::{Language, PrimaryLanguage};
use parser::CodeParser;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseAnalysis {
    pub name: String,
    pub root_path: PathBuf,
    pub primary_language: PrimaryLanguage,
    pub file_count: usize,
    pub module_count: usize,
    pub public_api_count: usize,
    pub modules: Vec<Module>,
    pub public_apis: Vec<ApiItem>,
    pub dependencies: Vec<Dependency>,
    pub existing_docs: Vec<ExistingDoc>,
    pub coverage_gaps: Vec<CoverageGap>,
    pub file_tree: FileTree,
    pub total_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
    pub public_items: Vec<String>,
    pub dependencies: Vec<String>,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiItem {
    pub name: String,
    pub kind: ApiKind,
    pub module: String,
    pub path: PathBuf,
    pub signature: String,
    pub doc_comment: Option<String>,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKind {
    Function,
    Struct,
    Class,
    Enum,
    Trait,
    Interface,
    Method,
    Constant,
    Type,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyKind {
    Runtime,
    Development,
    Build,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingDoc {
    pub path: PathBuf,
    pub kind: DocKind,
    pub content: String,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocKind {
    Readme,
    ApiDoc,
    Guide,
    Docstring,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub item: String,
    pub path: PathBuf,
    pub severity: GapSeverity,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTree {
    pub root: FileNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub kind: FileNodeKind,
    pub children: Vec<FileNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileNodeKind {
    Directory,
    SourceFile(Language),
    DocFile,
    ConfigFile,
    Other,
}

/// Main entry point for codebase analysis
pub async fn analyze_codebase(config: &Config) -> Result<CodebaseAnalysis> {
    info!("Starting codebase analysis at: {}", config.path.display());

    // Discover all files
    let files = discover_files(&config.path, config)?;
    debug!("Discovered {} files", files.len());

    if files.is_empty() {
        return Err(DocAssistError::NoSourceFiles);
    }

    // Detect primary language
    let primary_language = detect_primary_language(&files)?;
    info!("Detected primary language: {:?}", primary_language);

    // Create file tree
    let file_tree = build_file_tree(&config.path, &files)?;

    // Parse source files
    let parser = CodeParser::new(primary_language.clone());
    let mut modules = Vec::new();
    let mut public_apis = Vec::new();
    let mut total_lines = 0;

    for file_path in &files {
        if let Some(language) = language_from_path(file_path) {
            match parser.parse_file(file_path, language).await {
                Ok(parsed) => {
                    modules.push(parsed.module);
                    public_apis.extend(parsed.apis);
                    total_lines += parsed.line_count;
                }
                Err(e) => {
                    debug!("Failed to parse {}: {}", file_path.display(), e);
                }
            }
        }
    }

    // Find existing documentation
    let existing_docs = find_existing_docs(&config.path)?;

    // Analyze dependencies
    let dependencies = analyze_dependencies(&config.path, &primary_language)?;

    // Identify coverage gaps
    let coverage_gaps = identify_coverage_gaps(&modules, &public_apis, &existing_docs);

    let project_name = config.path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let analysis = CodebaseAnalysis {
        name: project_name,
        root_path: config.path.clone(),
        primary_language,
        file_count: files.len(),
        module_count: modules.len(),
        public_api_count: public_apis.len(),
        modules,
        public_apis,
        dependencies,
        existing_docs,
        coverage_gaps,
        file_tree,
        total_lines,
    };

    Ok(analysis)
}

/// Discover all relevant files in the codebase
fn discover_files(root: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let walker = WalkBuilder::new(root)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(false)
        .build();

    for entry in walker {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            // Apply include/exclude patterns
            if !config.should_include_path(path) {
                continue;
            }

            // Check if it's a source file or doc file
            if is_source_file(path) || is_doc_file(path) {
                files.push(path.to_path_buf());
            }
        }
    }

    Ok(files)
}

/// Check if a file is a source code file
fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "go" | "java" | "cpp" | "c" | "h" | "hpp" | "cs" | "rb" | "swift" | "kt")
    )
}

/// Check if a file is a documentation file
fn is_doc_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("md" | "rst" | "txt" | "adoc")
    ) || matches!(
        path.file_name().and_then(|s| s.to_str()),
        Some("README" | "LICENSE" | "CHANGELOG" | "CONTRIBUTING")
    )
}

/// Detect the primary language of the codebase
fn detect_primary_language(files: &[PathBuf]) -> Result<PrimaryLanguage> {
    let mut language_counts: HashMap<Language, usize> = HashMap::new();

    for file in files {
        if let Some(lang) = language_from_path(file) {
            *language_counts.entry(lang).or_insert(0) += 1;
        }
    }

    let primary = language_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(lang, _)| lang)
        .ok_or(DocAssistError::NoSourceFiles)?;

    Ok(primary.into())
}

/// Determine language from file path
fn language_from_path(path: &Path) -> Option<Language> {
    match path.extension()?.to_str()? {
        "rs" => Some(Language::Rust),
        "py" => Some(Language::Python),
        "js" | "mjs" => Some(Language::JavaScript),
        "ts" | "tsx" => Some(Language::TypeScript),
        "go" => Some(Language::Go),
        "java" => Some(Language::Java),
        "cpp" | "cc" | "cxx" => Some(Language::Cpp),
        "c" | "h" => Some(Language::C),
        "cs" => Some(Language::CSharp),
        "rb" => Some(Language::Ruby),
        "swift" => Some(Language::Swift),
        "kt" | "kts" => Some(Language::Kotlin),
        _ => None,
    }
}

/// Build a hierarchical file tree
fn build_file_tree(root: &Path, files: &[PathBuf]) -> Result<FileTree> {
    let mut root_node = FileNode {
        name: root.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("."))
            .to_string_lossy()
            .to_string(),
        path: root.to_path_buf(),
        kind: FileNodeKind::Directory,
        children: Vec::new(),
    };

    for file in files {
        if let Ok(relative) = file.strip_prefix(root) {
            add_to_tree(&mut root_node, relative, file)?;
        }
    }

    Ok(FileTree { root: root_node })
}

/// Add a file to the tree structure
fn add_to_tree(node: &mut FileNode, relative_path: &Path, full_path: &Path) -> Result<()> {
    let components: Vec<_> = relative_path.components().collect();

    if components.is_empty() {
        return Ok(());
    }

    if components.len() == 1 {
        // This is a file in the current directory
        let name = relative_path.to_string_lossy().to_string();
        let kind = if let Some(lang) = language_from_path(full_path) {
            FileNodeKind::SourceFile(lang)
        } else if is_doc_file(full_path) {
            FileNodeKind::DocFile
        } else {
            FileNodeKind::Other
        };

        node.children.push(FileNode {
            name,
            path: full_path.to_path_buf(),
            kind,
            children: Vec::new(),
        });
    } else {
        // Navigate deeper into the tree
        let first = components[0].as_os_str().to_string_lossy().to_string();
        let rest = components[1..].iter().collect::<PathBuf>();

        let child_node = node.children.iter_mut().find(|n| n.name == first);

        if let Some(child) = child_node {
            add_to_tree(child, &rest, full_path)?;
        } else {
            // Create new directory node
            let mut new_node = FileNode {
                name: first,
                path: node.path.join(components[0]),
                kind: FileNodeKind::Directory,
                children: Vec::new(),
            };
            add_to_tree(&mut new_node, &rest, full_path)?;
            node.children.push(new_node);
        }
    }

    Ok(())
}

/// Find existing documentation files
fn find_existing_docs(root: &Path) -> Result<Vec<ExistingDoc>> {
    let mut docs = Vec::new();

    // Common documentation file names
    let doc_files = [
        "README.md",
        "README.rst",
        "README.txt",
        "CONTRIBUTING.md",
        "ARCHITECTURE.md",
        "DESIGN.md",
        "API.md",
    ];

    for doc_file in &doc_files {
        let path = root.join(doc_file);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let word_count = content.split_whitespace().count();
            docs.push(ExistingDoc {
                path,
                kind: DocKind::Readme,
                content,
                word_count,
            });
        }
    }

    // Check for docs directory
    let docs_dir = root.join("docs");
    if docs_dir.exists() && docs_dir.is_dir() {
        for entry in std::fs::read_dir(docs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if is_doc_file(&path) {
                let content = std::fs::read_to_string(&path)?;
                let word_count = content.split_whitespace().count();
                docs.push(ExistingDoc {
                    path,
                    kind: DocKind::Guide,
                    content,
                    word_count,
                });
            }
        }
    }

    Ok(docs)
}

/// Analyze project dependencies
fn analyze_dependencies(root: &Path, language: &PrimaryLanguage) -> Result<Vec<Dependency>> {
    let mut dependencies = Vec::new();

    match language {
        PrimaryLanguage::Rust => {
            let cargo_toml = root.join("Cargo.toml");
            if cargo_toml.exists() {
                dependencies.extend(parse_cargo_dependencies(&cargo_toml)?);
            }
        }
        PrimaryLanguage::Python => {
            let requirements = root.join("requirements.txt");
            if requirements.exists() {
                dependencies.extend(parse_requirements(&requirements)?);
            }
            let pyproject = root.join("pyproject.toml");
            if pyproject.exists() {
                dependencies.extend(parse_pyproject(&pyproject)?);
            }
        }
        PrimaryLanguage::JavaScript | PrimaryLanguage::TypeScript => {
            let package_json = root.join("package.json");
            if package_json.exists() {
                dependencies.extend(parse_package_json(&package_json)?);
            }
        }
        _ => {}
    }

    Ok(dependencies)
}

/// Parse Cargo.toml for Rust dependencies
fn parse_cargo_dependencies(path: &Path) -> Result<Vec<Dependency>> {
    let content = std::fs::read_to_string(path)?;
    let toml: toml::Value = toml::from_str(&content)?;
    let mut deps = Vec::new();

    if let Some(dependencies) = toml.get("dependencies").and_then(|v| v.as_table()) {
        for (name, value) in dependencies {
            let version = if let Some(v) = value.as_str() {
                Some(v.to_string())
            } else if let Some(table) = value.as_table() {
                table.get("version").and_then(|v| v.as_str()).map(String::from)
            } else {
                None
            };

            deps.push(Dependency {
                name: name.to_string(),
                version,
                kind: DependencyKind::Runtime,
            });
        }
    }

    if let Some(dev_deps) = toml.get("dev-dependencies").and_then(|v| v.as_table()) {
        for (name, value) in dev_deps {
            let version = if let Some(v) = value.as_str() {
                Some(v.to_string())
            } else {
                None
            };

            deps.push(Dependency {
                name: name.to_string(),
                version,
                kind: DependencyKind::Development,
            });
        }
    }

    Ok(deps)
}

/// Parse requirements.txt for Python dependencies
fn parse_requirements(path: &Path) -> Result<Vec<Dependency>> {
    let content = std::fs::read_to_string(path)?;
    let mut deps = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split("==").collect();
        let name = parts[0].trim().to_string();
        let version = parts.get(1).map(|v| v.trim().to_string());

        deps.push(Dependency {
            name,
            version,
            kind: DependencyKind::Runtime,
        });
    }

    Ok(deps)
}

/// Parse pyproject.toml for Python dependencies
fn parse_pyproject(path: &Path) -> Result<Vec<Dependency>> {
    let content = std::fs::read_to_string(path)?;
    let toml: toml::Value = toml::from_str(&content)?;
    let mut deps = Vec::new();

    // Check for dependencies in [project] section (PEP 621)
    if let Some(project) = toml.get("project") {
        if let Some(dependencies) = project.get("dependencies").and_then(|v| v.as_array()) {
            for dep in dependencies {
                if let Some(dep_str) = dep.as_str() {
                    let parts: Vec<&str> = dep_str.split("==").collect();
                    let name = parts[0].trim().to_string();
                    let version = parts.get(1).map(|v| v.trim().to_string());

                    deps.push(Dependency {
                        name,
                        version,
                        kind: DependencyKind::Runtime,
                    });
                }
            }
        }
    }

    Ok(deps)
}

/// Parse package.json for JavaScript/TypeScript dependencies
fn parse_package_json(path: &Path) -> Result<Vec<Dependency>> {
    let content = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    let mut deps = Vec::new();

    if let Some(dependencies) = json.get("dependencies").and_then(|v| v.as_object()) {
        for (name, version) in dependencies {
            deps.push(Dependency {
                name: name.to_string(),
                version: version.as_str().map(String::from),
                kind: DependencyKind::Runtime,
            });
        }
    }

    if let Some(dev_deps) = json.get("devDependencies").and_then(|v| v.as_object()) {
        for (name, version) in dev_deps {
            deps.push(Dependency {
                name: name.to_string(),
                version: version.as_str().map(String::from),
                kind: DependencyKind::Development,
            });
        }
    }

    Ok(deps)
}

/// Identify gaps in documentation coverage
fn identify_coverage_gaps(
    modules: &[Module],
    apis: &[ApiItem],
    existing_docs: &[ExistingDoc],
) -> Vec<CoverageGap> {
    let mut gaps = Vec::new();

    // Check for undocumented public APIs
    for api in apis {
        if api.doc_comment.is_none() {
            gaps.push(CoverageGap {
                item: api.name.clone(),
                path: api.path.clone(),
                severity: match api.kind {
                    ApiKind::Function | ApiKind::Method => GapSeverity::High,
                    ApiKind::Class | ApiKind::Struct => GapSeverity::Critical,
                    _ => GapSeverity::Medium,
                },
                suggestion: format!("Add documentation for {} '{}'",
                    match api.kind {
                        ApiKind::Function => "function",
                        ApiKind::Struct => "struct",
                        ApiKind::Class => "class",
                        ApiKind::Enum => "enum",
                        ApiKind::Trait => "trait",
                        ApiKind::Interface => "interface",
                        ApiKind::Method => "method",
                        ApiKind::Constant => "constant",
                        ApiKind::Type => "type",
                    },
                    api.name
                ),
            });
        }
    }

    // Check for modules without documentation
    for module in modules {
        if module.description.is_none() && !module.public_items.is_empty() {
            gaps.push(CoverageGap {
                item: module.name.clone(),
                path: module.path.clone(),
                severity: GapSeverity::High,
                suggestion: format!("Add module-level documentation for '{}'", module.name),
            });
        }
    }

    // Check for missing README
    let has_readme = existing_docs.iter().any(|d| matches!(d.kind, DocKind::Readme));
    if !has_readme {
        gaps.push(CoverageGap {
            item: "README".to_string(),
            path: PathBuf::from("README.md"),
            severity: GapSeverity::Critical,
            suggestion: "Add a README.md file with project overview".to_string(),
        });
    }

    gaps
}
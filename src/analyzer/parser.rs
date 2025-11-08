use crate::analyzer::{ApiItem, ApiKind, Module};
use crate::error::Result;
use std::path::Path;
use syn::{Item, ItemFn, Visibility};
use tracing::debug;

use super::{Language, PrimaryLanguage};

pub struct CodeParser {
    primary_language: PrimaryLanguage,
}

pub struct ParsedFile {
    pub module: Module,
    pub apis: Vec<ApiItem>,
    pub line_count: usize,
}

impl CodeParser {
    pub fn new(primary_language: PrimaryLanguage) -> Self {
        Self { primary_language }
    }

    pub async fn parse_file(&self, path: &Path, language: Language) -> Result<ParsedFile> {
        match language {
            Language::Rust => self.parse_rust_file(path).await,
            Language::Python => self.parse_python_file(path).await,
            Language::JavaScript | Language::TypeScript => self.parse_js_ts_file(path).await,
            _ => {
                // For unsupported languages, return basic info
                let content = std::fs::read_to_string(path)?;
                let line_count = content.lines().count();

                Ok(ParsedFile {
                    module: Module {
                        name: path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        path: path.to_path_buf(),
                        description: None,
                        public_items: Vec::new(),
                        dependencies: Vec::new(),
                        line_count,
                    },
                    apis: Vec::new(),
                    line_count,
                })
            }
        }
    }

    async fn parse_rust_file(&self, path: &Path) -> Result<ParsedFile> {
        let content = std::fs::read_to_string(path)?;
        let line_count = content.lines().count();

        let file = match syn::parse_file(&content) {
            Ok(f) => f,
            Err(e) => {
                debug!("Failed to parse Rust file {}: {}", path.display(), e);
                return Ok(ParsedFile {
                    module: Module {
                        name: path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        path: path.to_path_buf(),
                        description: None,
                        public_items: Vec::new(),
                        dependencies: Vec::new(),
                        line_count,
                    },
                    apis: Vec::new(),
                    line_count,
                });
            }
        };

        let mut public_items = Vec::new();
        let mut apis = Vec::new();
        let module_name = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        for item in &file.items {
            match item {
                Item::Fn(func) if is_public(&func.vis) => {
                    let name = func.sig.ident.to_string();
                    public_items.push(name.clone());

                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Function,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: format_rust_fn_signature(func),
                        doc_comment: extract_rust_doc_comment(&func.attrs),
                        line_number: 0, // Would need more complex parsing for accurate line numbers
                    });
                }
                Item::Struct(s) if is_public(&s.vis) => {
                    let name = s.ident.to_string();
                    public_items.push(name.clone());

                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Struct,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: format!("struct {}", name),
                        doc_comment: extract_rust_doc_comment(&s.attrs),
                        line_number: 0,
                    });
                }
                Item::Enum(e) if is_public(&e.vis) => {
                    let name = e.ident.to_string();
                    public_items.push(name.clone());

                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Enum,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: format!("enum {}", name),
                        doc_comment: extract_rust_doc_comment(&e.attrs),
                        line_number: 0,
                    });
                }
                Item::Trait(t) if is_public(&t.vis) => {
                    let name = t.ident.to_string();
                    public_items.push(name.clone());

                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Trait,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: format!("trait {}", name),
                        doc_comment: extract_rust_doc_comment(&t.attrs),
                        line_number: 0,
                    });
                }
                _ => {}
            }
        }

        // Extract module-level documentation
        let module_doc = file.attrs.iter()
            .filter_map(|attr| {
                if attr.path().is_ident("doc") {
                    attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ParsedFile {
            module: Module {
                name: module_name,
                path: path.to_path_buf(),
                description: if module_doc.is_empty() { None } else { Some(module_doc) },
                public_items,
                dependencies: extract_rust_dependencies(&file),
                line_count,
            },
            apis,
            line_count,
        })
    }

    async fn parse_python_file(&self, path: &Path) -> Result<ParsedFile> {
        let content = std::fs::read_to_string(path)?;
        let line_count = content.lines().count();

        let mut public_items = Vec::new();
        let mut apis = Vec::new();
        let module_name = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Simple Python parsing - look for class and function definitions
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("def ") && !trimmed.contains("def _") {
                // Public function (doesn't start with _)
                if let Some(name) = extract_python_function_name(trimmed) {
                    public_items.push(name.clone());
                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Function,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: trimmed.to_string(),
                        doc_comment: extract_python_docstring(&content, line_num),
                        line_number: line_num + 1,
                    });
                }
            } else if trimmed.starts_with("class ") {
                if let Some(name) = extract_python_class_name(trimmed) {
                    public_items.push(name.clone());
                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Class,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: trimmed.to_string(),
                        doc_comment: extract_python_docstring(&content, line_num),
                        line_number: line_num + 1,
                    });
                }
            }
        }

        Ok(ParsedFile {
            module: Module {
                name: module_name,
                path: path.to_path_buf(),
                description: extract_python_module_docstring(&content),
                public_items,
                dependencies: extract_python_imports(&content),
                line_count,
            },
            apis,
            line_count,
        })
    }

    async fn parse_js_ts_file(&self, path: &Path) -> Result<ParsedFile> {
        let content = std::fs::read_to_string(path)?;
        let line_count = content.lines().count();

        let mut public_items = Vec::new();
        let mut apis = Vec::new();
        let module_name = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Simple JS/TS parsing - look for exported functions and classes
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("export function ") || trimmed.starts_with("export const ") || trimmed.starts_with("export async function ") {
                if let Some(name) = extract_js_function_name(trimmed) {
                    public_items.push(name.clone());
                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Function,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: trimmed.to_string(),
                        doc_comment: extract_js_jsdoc(&content, line_num),
                        line_number: line_num + 1,
                    });
                }
            } else if trimmed.starts_with("export class ") {
                if let Some(name) = extract_js_class_name(trimmed) {
                    public_items.push(name.clone());
                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: ApiKind::Class,
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: trimmed.to_string(),
                        doc_comment: extract_js_jsdoc(&content, line_num),
                        line_number: line_num + 1,
                    });
                }
            } else if trimmed.starts_with("export interface ") || trimmed.starts_with("export type ") {
                if let Some(name) = extract_ts_type_name(trimmed) {
                    public_items.push(name.clone());
                    apis.push(ApiItem {
                        name: name.clone(),
                        kind: if trimmed.starts_with("export interface") {
                            ApiKind::Interface
                        } else {
                            ApiKind::Type
                        },
                        module: module_name.clone(),
                        path: path.to_path_buf(),
                        signature: trimmed.to_string(),
                        doc_comment: extract_js_jsdoc(&content, line_num),
                        line_number: line_num + 1,
                    });
                }
            }
        }

        Ok(ParsedFile {
            module: Module {
                name: module_name,
                path: path.to_path_buf(),
                description: None,
                public_items,
                dependencies: extract_js_imports(&content),
                line_count,
            },
            apis,
            line_count,
        })
    }
}

// Helper functions

fn is_public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn format_rust_fn_signature(func: &ItemFn) -> String {
    format!("fn {}()", func.sig.ident)
}

fn extract_rust_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let doc_comments: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value())
            } else {
                None
            }
        })
        .collect();

    if doc_comments.is_empty() {
        None
    } else {
        Some(doc_comments.join("\n"))
    }
}

fn extract_rust_dependencies(file: &syn::File) -> Vec<String> {
    file.items
        .iter()
        .filter_map(|item| {
            if let Item::Use(use_item) = item {
                Some(format!("{:?}", use_item.tree))
            } else {
                None
            }
        })
        .collect()
}

fn extract_python_function_name(line: &str) -> Option<String> {
    let after_def = line.trim_start().strip_prefix("def ")?;
    let name_end = after_def.find('(')?;
    Some(after_def[..name_end].to_string())
}

fn extract_python_class_name(line: &str) -> Option<String> {
    let after_class = line.trim_start().strip_prefix("class ")?;
    let name_end = after_class.find(|c: char| c == '(' || c == ':')
        .unwrap_or(after_class.len());
    Some(after_class[..name_end].trim().to_string())
}

fn extract_python_docstring(content: &str, line_num: usize) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if line_num + 1 < lines.len() {
        let next_line = lines[line_num + 1].trim();
        if next_line.starts_with("\"\"\"") || next_line.starts_with("'''") {
            // Simple docstring extraction - would need proper parsing for multi-line
            Some(next_line.trim_matches('"').trim_matches('\'').to_string())
        } else {
            None
        }
    } else {
        None
    }
}

fn extract_python_module_docstring(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
        let quote = if trimmed.starts_with("\"\"\"") { "\"\"\"" } else { "'''" };
        let end_pos = trimmed[3..].find(quote)?;
        Some(trimmed[3..3 + end_pos].to_string())
    } else {
        None
    }
}

fn extract_python_imports(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("import ") || trimmed.starts_with("from ")
        })
        .map(|line| line.trim().to_string())
        .collect()
}

fn extract_js_function_name(line: &str) -> Option<String> {
    if line.contains("function ") {
        let after_func = line.split("function ").nth(1)?;
        let name_end = after_func.find('(')?;
        Some(after_func[..name_end].trim().to_string())
    } else if line.contains("const ") {
        let after_const = line.split("const ").nth(1)?;
        let name_end = after_const.find(|c: char| c == ' ' || c == '=' || c == ':')?;
        Some(after_const[..name_end].trim().to_string())
    } else {
        None
    }
}

fn extract_js_class_name(line: &str) -> Option<String> {
    let after_class = line.split("class ").nth(1)?;
    let name_end = after_class.find(|c: char| c == ' ' || c == '{')
        .unwrap_or(after_class.len());
    Some(after_class[..name_end].trim().to_string())
}

fn extract_ts_type_name(line: &str) -> Option<String> {
    let after_keyword = if line.contains("interface ") {
        line.split("interface ").nth(1)?
    } else {
        line.split("type ").nth(1)?
    };
    let name_end = after_keyword.find(|c: char| c == ' ' || c == '<' || c == '=' || c == '{')
        .unwrap_or(after_keyword.len());
    Some(after_keyword[..name_end].trim().to_string())
}

fn extract_js_jsdoc(content: &str, line_num: usize) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if line_num > 0 {
        let prev_line = lines[line_num - 1].trim();
        if prev_line.ends_with("*/") {
            // Look backwards for JSDoc comment
            let mut doc_lines = Vec::new();
            let mut i = line_num - 1;
            while i > 0 {
                let line = lines[i].trim();
                doc_lines.push(line);
                if line.starts_with("/**") {
                    break;
                }
                i -= 1;
            }
            doc_lines.reverse();
            let doc = doc_lines.join("\n");
            Some(doc.replace("/**", "").replace("*/", "").replace("*", "").trim().to_string())
        } else {
            None
        }
    } else {
        None
    }
}

fn extract_js_imports(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("import ") || trimmed.starts_with("const ") && trimmed.contains(" require(")
        })
        .map(|line| line.trim().to_string())
        .collect()
}
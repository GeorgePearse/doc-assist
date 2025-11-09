use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    Cpp,
    C,
    CSharp,
    Ruby,
    Swift,
    Kotlin,
}

impl Language {
    pub fn file_extensions(&self) -> &[&str] {
        match self {
            Language::Rust => &["rs"],
            Language::Python => &["py", "pyw"],
            Language::JavaScript => &["js", "mjs", "cjs"],
            Language::TypeScript => &["ts", "tsx"],
            Language::Go => &["go"],
            Language::Java => &["java"],
            Language::Cpp => &["cpp", "cxx", "cc", "hpp", "hxx", "h++"],
            Language::C => &["c", "h"],
            Language::CSharp => &["cs"],
            Language::Ruby => &["rb"],
            Language::Swift => &["swift"],
            Language::Kotlin => &["kt", "kts"],
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::Cpp => "C++",
            Language::C => "C",
            Language::CSharp => "C#",
            Language::Ruby => "Ruby",
            Language::Swift => "Swift",
            Language::Kotlin => "Kotlin",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrimaryLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    Cpp,
    C,
    CSharp,
    Ruby,
    Swift,
    Kotlin,
    Mixed(Vec<Language>),
}

impl PrimaryLanguage {
    pub fn name(&self) -> String {
        match self {
            PrimaryLanguage::Rust => "Rust".to_string(),
            PrimaryLanguage::Python => "Python".to_string(),
            PrimaryLanguage::JavaScript => "JavaScript".to_string(),
            PrimaryLanguage::TypeScript => "TypeScript".to_string(),
            PrimaryLanguage::Go => "Go".to_string(),
            PrimaryLanguage::Java => "Java".to_string(),
            PrimaryLanguage::Cpp => "C++".to_string(),
            PrimaryLanguage::C => "C".to_string(),
            PrimaryLanguage::CSharp => "C#".to_string(),
            PrimaryLanguage::Ruby => "Ruby".to_string(),
            PrimaryLanguage::Swift => "Swift".to_string(),
            PrimaryLanguage::Kotlin => "Kotlin".to_string(),
            PrimaryLanguage::Mixed(langs) => {
                let names: Vec<String> = langs.iter().map(|l| l.name().to_string()).collect();
                format!("Mixed ({})", names.join(", "))
            }
        }
    }
}

impl From<Language> for PrimaryLanguage {
    fn from(lang: Language) -> Self {
        match lang {
            Language::Rust => PrimaryLanguage::Rust,
            Language::Python => PrimaryLanguage::Python,
            Language::JavaScript => PrimaryLanguage::JavaScript,
            Language::TypeScript => PrimaryLanguage::TypeScript,
            Language::Go => PrimaryLanguage::Go,
            Language::Java => PrimaryLanguage::Java,
            Language::Cpp => PrimaryLanguage::Cpp,
            Language::C => PrimaryLanguage::C,
            Language::CSharp => PrimaryLanguage::CSharp,
            Language::Ruby => PrimaryLanguage::Ruby,
            Language::Swift => PrimaryLanguage::Swift,
            Language::Kotlin => PrimaryLanguage::Kotlin,
        }
    }
}

impl std::fmt::Display for PrimaryLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimaryLanguage::Rust => write!(f, "Rust"),
            PrimaryLanguage::Python => write!(f, "Python"),
            PrimaryLanguage::JavaScript => write!(f, "JavaScript"),
            PrimaryLanguage::TypeScript => write!(f, "TypeScript"),
            PrimaryLanguage::Go => write!(f, "Go"),
            PrimaryLanguage::Java => write!(f, "Java"),
            PrimaryLanguage::Cpp => write!(f, "C++"),
            PrimaryLanguage::C => write!(f, "C"),
            PrimaryLanguage::CSharp => write!(f, "C#"),
            PrimaryLanguage::Ruby => write!(f, "Ruby"),
            PrimaryLanguage::Swift => write!(f, "Swift"),
            PrimaryLanguage::Kotlin => write!(f, "Kotlin"),
            PrimaryLanguage::Mixed(langs) => {
                let names: Vec<&str> = langs.iter().map(|l| l.name()).collect();
                write!(f, "Mixed ({})", names.join(", "))
            }
        }
    }
}
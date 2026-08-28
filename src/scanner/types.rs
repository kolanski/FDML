use serde::{Deserialize, Serialize};

/// Supported programming languages for code scanning
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    C,
    Cpp,
    Python,
    Java,
    CSharp,
    JavaScript,
    TypeScript,
    Go,
    Rust,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "c" | "h" => Some(Language::C),
            "cc" | "cpp" | "cxx" | "hpp" | "hh" => Some(Language::Cpp),
            "py" => Some(Language::Python),
            "java" => Some(Language::Java),
            "cs" => Some(Language::CSharp),
            "js" | "jsx" | "mjs" => Some(Language::JavaScript),
            "ts" | "tsx" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "rs" => Some(Language::Rust),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::Python => "python",
            Language::Java => "java",
            Language::CSharp => "csharp",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Go => "go",
            Language::Rust => "rust",
        }
    }
}

/// Type of code element extracted from source
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Module,
    Class,
    Function,
    Method,
    Interface,
    Enum,
    Field,
    Property,
    /// C/C++ preprocessor constant or function-like macro.
    Macro,
    /// A named alias for a type (`typedef`), not a type definition of its own.
    TypeAlias,
}

/// Visibility/scope of a code element
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Public,
    Private,
    Protected,
    Internal,
}

/// A parameter of a function/method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

/// A single code element (class, function, field, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeElement {
    pub element_type: ElementType,
    pub name: String,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub language: Language,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parameters: Vec<Parameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub bases: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub decorators: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<CodeElement>,
}

/// Type of relationship between code elements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    Inherits,
    Implements,
    Imports,
    Calls,
    Contains,
    Uses,
}

/// A relationship between two code elements or modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub relation_type: RelationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// An import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInfo {
    pub module: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub names: Vec<String>,
    pub is_relative: bool,
    pub file_path: String,
    pub line: usize,
}

/// Result of scanning a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub file_path: String,
    pub module_path: String,
    pub language: Language,
    pub elements: Vec<CodeElement>,
    pub imports: Vec<ImportInfo>,
    /// Navigation anchors inside oversized bodies. Not symbols — they exist so a
    /// search can answer "which 40 lines do I read next?" Scanners that do not
    /// produce them leave this empty.
    #[serde(default)]
    pub anchors: Vec<CodeAnchor>,
    /// String literals with their use site. Kept out of the symbol table so they
    /// never pollute symbol ranking.
    #[serde(default)]
    pub literals: Vec<CodeLiteral>,
}

/// A statement region worth navigating to, with every searchable feature the parse
/// can offer. Features stay separate so they can be weighted, not blended into a blob.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeAnchor {
    pub parent_symbol: String,
    pub kind: String,
    /// Best short handle, usually the first condition identifier
    pub name: String,
    pub line_start: usize,
    pub line_end: usize,
    pub depth: usize,
    /// Nearest comment above the region
    pub label: Option<String>,
    /// Identifiers appearing in the region's condition
    pub condition_ids: Vec<String>,
    /// Functions called directly in the region
    pub calls: Vec<String>,
    /// Locals declared *with an initializer* — features of the anchor, never symbols
    pub declared: Vec<String>,
    /// String literals occurring in the region
    pub literals: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeLiteral {
    pub value: String,
    pub line: usize,
    /// cli_flag | format | asset | generic
    pub kind: String,
    /// comparison | printf_format | argument | generic
    pub usage_kind: String,
    pub parent_symbol: String,
}

/// A node in the module hierarchy tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleNode {
    pub name: String,
    pub module_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<ModuleNode>,
}

/// Result of scanning an entire project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub metadata: ScanMetadata,
    pub modules: Vec<ModuleNode>,
    pub files: Vec<FileAnalysis>,
    pub relationships: Vec<Relationship>,
    pub statistics: ScanStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanMetadata {
    pub scanner_version: String,
    pub scan_timestamp: String,
    pub codebase_path: String,
    pub languages_detected: Vec<Language>,
    pub total_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatistics {
    pub classes: usize,
    pub functions: usize,
    pub methods: usize,
    pub interfaces: usize,
    pub enums: usize,
    pub fields: usize,
    pub imports_external: usize,
    pub imports_internal: usize,
    pub relationships: usize,
}

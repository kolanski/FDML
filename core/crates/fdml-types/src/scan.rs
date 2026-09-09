//! The `ScanResult` contract — ported VERBATIM from FDML's `src/scanner/types.rs`.
//!
//! One intentional divergence: `ScanMetadata` drops the old `scan_timestamp`
//! (`Utc::now()`) field, which broke reproducibility. Everything else mirrors the
//! original scanner schema field-for-field so output stays FDML-compatible.

use serde::{Deserialize, Serialize};

/// Supported programming languages for code scanning
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    Java,
    CSharp,
    JavaScript,
    TypeScript,
    Go,
    /// C and C headers. C++ is not claimed: the grammar is C, and a `.cpp` file
    /// would be scanned as something it is not.
    C,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "c" | "h" => Some(Language::C),
            "py" => Some(Language::Python),
            "java" => Some(Language::Java),
            "cs" => Some(Language::CSharp),
            "js" | "jsx" | "mjs" => Some(Language::JavaScript),
            "ts" | "tsx" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Language::Python => "python",
            Language::Java => "java",
            Language::CSharp => "csharp",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Go => "go",
            Language::C => "c",
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
    /// C preprocessor macro — file-scope wherever it is written.
    Macro,
    /// `typedef` — a name for a type, not a type of its own.
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
    /// Call sites found in this file — caller function → callee name (unresolved).
    /// The graph pass resolves these into real function→function call edges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<CallRef>,
}

/// An unresolved call site: `caller` (enclosing function/method, or "" at module level)
/// invokes `callee` at `line`. Resolved into a call edge by the graph pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRef {
    pub caller: String,
    pub callee: String,
    pub line: usize,
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

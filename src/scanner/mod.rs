pub mod types;
pub mod python;
pub mod java;
pub mod csharp;
pub mod javascript;
pub mod typescript;
pub mod go_lang;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::Result;
use types::*;

/// Default directories/patterns to exclude from scanning
const EXCLUDE_DIRS: &[&str] = &[
    "node_modules", "__pycache__", ".git", ".svn", ".hg",
    "venv", ".venv", "env", ".env",
    "target", "build", "dist", "out", "bin", "obj",
    ".idea", ".vscode", ".vs",
    "vendor", "packages", ".nuget",
    "site-packages", "lib", "libs",
    ".tox", ".pytest_cache", ".mypy_cache",
    "migrations", "static", "media", "assets",
    "testdata",
    // Frontend build outputs and caches
    ".next", ".nuxt", ".output", ".cache", ".parcel-cache", ".turbo",
    "storybook-static", "coverage", "bower_components",
    "__tests__", ".storybook",
];

/// Scan a project directory and produce a full inventory
pub fn scan_project(codebase_path: &str, exclude_patterns: &[String]) -> Result<ScanResult> {
    let path = Path::new(codebase_path);
    if !path.exists() {
        return Err(crate::error::FdmlError::project_error(
            format!("Path does not exist: {}", codebase_path)
        ));
    }

    let base_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    // Collect all source files (absolute paths)
    let mut source_files: Vec<(String, Language)> = Vec::new();
    collect_files(&base_path, &mut source_files, exclude_patterns)?;

    let mut all_files = Vec::new();
    let mut languages_detected: HashSet<String> = HashSet::new();

    // Parse each file with relative paths
    for (abs_path, language) in &source_files {
        let source = std::fs::read_to_string(abs_path).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read {}: {}", abs_path, e))
        })?;

        // Convert to relative path
        let rel_path = make_relative(&base_path, abs_path);
        let module_path = compute_module_path(&rel_path, language);

        let mut analysis = match language {
            Language::Python => python::PythonScanner::parse_file(&source, &rel_path)?,
            Language::Java => java::JavaScanner::parse_file(&source, &rel_path)?,
            Language::CSharp => csharp::CSharpScanner::parse_file(&source, &rel_path)?,
            Language::JavaScript => javascript::JavaScriptScanner::parse_file(&source, &rel_path)?,
            Language::TypeScript => typescript::TypeScriptScanner::parse_file(&source, &rel_path)?,
            Language::Go => go_lang::GoScanner::parse_file(&source, &rel_path)?,
        };

        analysis.module_path = module_path;

        languages_detected.insert(language.name().to_string());
        all_files.push(analysis);
    }

    // Build relationships from imports and inheritance
    let relationships = build_relationships(&all_files);

    // Build module hierarchy tree
    let modules = build_module_tree(&all_files);

    // Compute statistics
    let statistics = compute_statistics(&all_files, &relationships);

    let languages: Vec<Language> = {
        let mut langs = Vec::new();
        if languages_detected.contains("python") { langs.push(Language::Python); }
        if languages_detected.contains("java") { langs.push(Language::Java); }
        if languages_detected.contains("csharp") { langs.push(Language::CSharp); }
        if languages_detected.contains("javascript") { langs.push(Language::JavaScript); }
        if languages_detected.contains("typescript") { langs.push(Language::TypeScript); }
        if languages_detected.contains("go") { langs.push(Language::Go); }
        langs
    };

    Ok(ScanResult {
        metadata: ScanMetadata {
            scanner_version: env!("CARGO_PKG_VERSION").to_string(),
            scan_timestamp: chrono::Utc::now().to_rfc3339(),
            codebase_path: codebase_path.to_string(),
            languages_detected: languages,
            total_files: all_files.len(),
        },
        modules,
        files: all_files,
        relationships,
        statistics,
    })
}

/// Convert absolute path to relative from base
fn make_relative(base: &Path, abs_path: &str) -> String {
    let abs = Path::new(abs_path);
    let canonical = std::fs::canonicalize(abs).unwrap_or_else(|_| abs.to_path_buf());
    canonical.strip_prefix(base)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| abs_path.to_string())
}

/// Compute a language-appropriate module path from relative file path
fn compute_module_path(rel_path: &str, language: &Language) -> String {
    match language {
        Language::Python => {
            // foo/bar/__init__.py → foo.bar
            // foo/bar/baz.py → foo.bar.baz
            let without_ext = rel_path.trim_end_matches(".py");
            let module = without_ext.replace('/', ".").replace('\\', ".");
            if module.ends_with(".__init__") {
                module.trim_end_matches(".__init__").to_string()
            } else if module == "__init__" {
                // Root __init__.py
                String::new()
            } else {
                module
            }
        }
        Language::Java => {
            // Strip common Java source roots
            let stripped = rel_path
                .trim_start_matches("src/main/java/")
                .trim_start_matches("src/")
                .trim_end_matches(".java");
            stripped.replace('/', ".").replace('\\', ".")
        }
        Language::CSharp => {
            let stripped = rel_path.trim_end_matches(".cs");
            stripped.replace('/', ".").replace('\\', ".")
        }
        Language::Go => {
            // Strip common Go source roots
            let stripped = rel_path
                .trim_start_matches("cmd/")
                .trim_start_matches("internal/")
                .trim_start_matches("pkg/")
                .trim_end_matches(".go");
            stripped.replace('/', ".").replace('\\', ".")
        }
        Language::JavaScript | Language::TypeScript => {
            // Strip src/ prefix
            let stripped = rel_path.trim_start_matches("src/");
            // Remove extension
            let without_ext = stripped
                .trim_end_matches(".tsx")
                .trim_end_matches(".ts")
                .trim_end_matches(".jsx")
                .trim_end_matches(".mjs")
                .trim_end_matches(".js");
            let module = without_ext.replace('/', ".").replace('\\', ".");
            // index.js/index.ts → parent directory name
            if module.ends_with(".index") {
                module.trim_end_matches(".index").to_string()
            } else if module == "index" {
                String::new()
            } else {
                module
            }
        }
    }
}

/// Build a module hierarchy tree from file analyses
fn build_module_tree(files: &[FileAnalysis]) -> Vec<ModuleNode> {
    let mut module_map: HashMap<String, (Option<String>, Option<Language>)> = HashMap::new();

    for file in files {
        let mp = if file.module_path.is_empty() {
            // Root __init__.py — use the filename stem
            Path::new(&file.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            file.module_path.clone()
        };

        if mp.is_empty() {
            continue;
        }

        module_map.insert(mp.clone(), (Some(file.file_path.clone()), Some(file.language.clone())));

        // Ensure parent modules exist in the map
        let parts: Vec<&str> = mp.split('.').collect();
        for i in 1..parts.len() {
            let parent = parts[..i].join(".");
            module_map.entry(parent).or_insert((None, None));
        }
    }

    // Find root-level module names (first segment)
    let mut root_names: Vec<String> = module_map.keys()
        .filter_map(|p| {
            let first = p.split('.').next()?;
            Some(first.to_string())
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    root_names.sort();

    let mut roots = Vec::new();
    for root_name in &root_names {
        roots.push(build_tree_node(root_name, &module_map));
    }
    roots
}

fn build_tree_node(module_path: &str, module_map: &HashMap<String, (Option<String>, Option<Language>)>) -> ModuleNode {
    let name = module_path.split('.').last().unwrap_or(module_path).to_string();

    let (file_path, language) = module_map.get(module_path)
        .cloned()
        .unwrap_or((None, None));

    // Find direct children
    let prefix = format!("{}.", module_path);
    let mut child_paths: HashSet<String> = HashSet::new();

    for key in module_map.keys() {
        if let Some(rest) = key.strip_prefix(&prefix) {
            if let Some(child_seg) = rest.split('.').next() {
                child_paths.insert(format!("{}.{}", module_path, child_seg));
            }
        }
    }

    let mut children: Vec<ModuleNode> = child_paths.iter()
        .map(|cp| build_tree_node(cp, module_map))
        .collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));

    ModuleNode {
        name,
        module_path: module_path.to_string(),
        file_path,
        language,
        children,
    }
}

/// Check if a filename is a generated/bundled file that should be skipped
fn is_generated_file(filename: &str) -> bool {
    filename.ends_with(".d.ts")
        || filename.ends_with(".d.tsx")
        || filename.ends_with(".min.js")
        || filename.ends_with(".min.mjs")
        || filename.ends_with(".bundle.js")
        // Go generated/test files
        || filename.ends_with("_test.go")
        || filename.ends_with(".pb.go")
}

fn collect_files(
    dir: &Path,
    files: &mut Vec<(String, Language)>,
    exclude_patterns: &[String],
) -> Result<()> {
    if !dir.is_dir() {
        // Single file — skip generated/bundled files
        let filename = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if is_generated_file(filename) {
            return Ok(());
        }
        if let Some(ext) = dir.extension().and_then(|e| e.to_str()) {
            if let Some(lang) = Language::from_extension(ext) {
                files.push((dir.to_string_lossy().to_string(), lang));
            }
        }
        return Ok(());
    }

    let dir_name = dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Skip excluded directories
    if EXCLUDE_DIRS.contains(&dir_name) {
        return Ok(());
    }
    for pattern in exclude_patterns {
        if dir_name == pattern.as_str() {
            return Ok(());
        }
    }

    let entries = std::fs::read_dir(dir).map_err(|e| {
        crate::error::FdmlError::project_error(format!("Failed to read directory {}: {}", dir.display(), e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read entry: {}", e))
        })?;
        let path = entry.path();

        if path.is_dir() {
            collect_files(&path, files, exclude_patterns)?;
        } else {
            // Skip generated/bundled files
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if is_generated_file(filename) {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(lang) = Language::from_extension(ext) {
                    files.push((path.to_string_lossy().to_string(), lang));
                }
            }
        }
    }
    Ok(())
}

fn build_relationships(files: &[FileAnalysis]) -> Vec<Relationship> {
    let mut relationships = Vec::new();

    for file in files {
        // Use module_path for relationships, fall back to file stem
        let module = if !file.module_path.is_empty() {
            file.module_path.clone()
        } else {
            Path::new(&file.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file.file_path)
                .to_string()
        };

        // Inheritance relationships
        for element in &file.elements {
            for base in &element.bases {
                let base_name = base.split('.').last().unwrap_or(base);
                relationships.push(Relationship {
                    from: format!("{}:{}", module, element.name),
                    to: base_name.to_string(),
                    relation_type: if matches!(element.element_type, ElementType::Interface) {
                        RelationType::Implements
                    } else {
                        RelationType::Inherits
                    },
                    description: Some(format!("{} extends/implements {}", element.name, base)),
                });
            }
        }

        // Import relationships
        for import in &file.imports {
            let relation_type = if import.is_relative {
                RelationType::Imports
            } else {
                RelationType::Uses
            };

            relationships.push(Relationship {
                from: module.clone(),
                to: import.module.clone(),
                relation_type,
                description: if import.names.is_empty() {
                    None
                } else {
                    Some(format!("imports: {}", import.names.join(", ")))
                },
            });
        }
    }

    relationships
}

fn count_elements(elements: &[CodeElement], stats: &mut ScanStatistics) {
    for el in elements {
        match el.element_type {
            ElementType::Class => stats.classes += 1,
            ElementType::Function => stats.functions += 1,
            ElementType::Method => stats.methods += 1,
            ElementType::Interface => stats.interfaces += 1,
            ElementType::Enum => stats.enums += 1,
            ElementType::Field | ElementType::Property => stats.fields += 1,
            ElementType::Module => {}
        }
        count_elements(&el.children, stats);
    }
}

fn compute_statistics(files: &[FileAnalysis], relationships: &[Relationship]) -> ScanStatistics {
    let mut stats = ScanStatistics {
        classes: 0,
        functions: 0,
        methods: 0,
        interfaces: 0,
        enums: 0,
        fields: 0,
        imports_external: 0,
        imports_internal: 0,
        relationships: relationships.len(),
    };

    for file in files {
        count_elements(&file.elements, &mut stats);
        for imp in &file.imports {
            if imp.is_relative {
                stats.imports_internal += 1;
            } else {
                stats.imports_external += 1;
            }
        }
    }

    stats
}

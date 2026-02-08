pub mod types;
pub mod python;
pub mod java;
pub mod csharp;

use std::collections::HashSet;
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
];

/// Scan a project directory and produce a full inventory
pub fn scan_project(codebase_path: &str, exclude_patterns: &[String]) -> Result<ScanResult> {
    let path = Path::new(codebase_path);
    if !path.exists() {
        return Err(crate::error::FdmlError::project_error(
            format!("Path does not exist: {}", codebase_path)
        ));
    }

    // Collect all source files
    let mut source_files: Vec<(String, Language)> = Vec::new();
    collect_files(path, &mut source_files, exclude_patterns)?;

    let mut all_files = Vec::new();
    let mut languages_detected: HashSet<String> = HashSet::new();

    // Parse each file
    for (file_path, language) in &source_files {
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            crate::error::FdmlError::project_error(format!("Failed to read {}: {}", file_path, e))
        })?;

        let analysis = match language {
            Language::Python => python::PythonScanner::parse_file(&source, file_path)?,
            Language::Java => java::JavaScanner::parse_file(&source, file_path)?,
            Language::CSharp => csharp::CSharpScanner::parse_file(&source, file_path)?,
        };

        languages_detected.insert(language.name().to_string());
        all_files.push(analysis);
    }

    // Build relationships from imports and inheritance
    let relationships = build_relationships(&all_files);

    // Compute statistics
    let statistics = compute_statistics(&all_files, &relationships);

    let languages: Vec<Language> = {
        let mut langs = Vec::new();
        if languages_detected.contains("python") { langs.push(Language::Python); }
        if languages_detected.contains("java") { langs.push(Language::Java); }
        if languages_detected.contains("csharp") { langs.push(Language::CSharp); }
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
        files: all_files,
        relationships,
        statistics,
    })
}

fn collect_files(
    dir: &Path,
    files: &mut Vec<(String, Language)>,
    exclude_patterns: &[String],
) -> Result<()> {
    if !dir.is_dir() {
        // Single file
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
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(lang) = Language::from_extension(ext) {
                files.push((path.to_string_lossy().to_string(), lang));
            }
        }
    }
    Ok(())
}

fn build_relationships(files: &[FileAnalysis]) -> Vec<Relationship> {
    let mut relationships = Vec::new();

    // Build a map of class names to file paths
    let mut class_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for file in files {
        for element in &file.elements {
            if matches!(element.element_type, ElementType::Class | ElementType::Interface | ElementType::Enum) {
                class_map.insert(element.name.clone(), file.file_path.clone());
            }
        }
    }

    for file in files {
        let file_stem = Path::new(&file.file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&file.file_path);

        // Inheritance relationships
        for element in &file.elements {
            for base in &element.bases {
                let base_name = base.split('.').last().unwrap_or(base);
                relationships.push(Relationship {
                    from: format!("{}:{}", file_stem, element.name),
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
                from: file_stem.to_string(),
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

//! Pass 1 — discovery: find independent systems/projects in a folder.
//!
//! Public API is one function: [`run`]. Pure, deterministic, no LLM, instant.
//!
//! Ported FAITHFULLY from FDML's `src/linker/platform.rs::detect_systems` and the
//! `classify_*` helpers — the rework must find exactly what FDML found (behavioral
//! parity covered by tests). New on top: an optional declarative rules layer
//! (`discovery-rules.yaml`) that is tried BEFORE the built-in detection.

use std::io::BufRead;
use std::path::Path;

use fdml_types::System;
// Shared identifier helpers now live in fdml-util (deduped from the old inline copies).
// Re-export `normalize_name` so existing `fdml_discovery::normalize_name` callers/tests still work.
pub use fdml_util::normalize_name;
use fdml_util::name_similarity;

mod rules;
pub use rules::DiscoveryRules;

// ─── Public API ──────────────────────────────────────────────────

/// Read `.fdmlignore` from `root`, returning patterns to exclude (verbatim from FDML).
/// The CLI merges these with `--exclude` flags before calling [`run`].
pub fn read_fdmlignore(root: &Path) -> Vec<String> {
    let ignore_path = root.join(".fdmlignore");
    if !ignore_path.exists() {
        return Vec::new();
    }
    match std::fs::File::open(&ignore_path) {
        Ok(file) => std::io::BufReader::new(file)
            .lines()
            .filter_map(|line| {
                let line = line.ok()?;
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// ─── Boundary detection (ported from platform.rs) ─────────────────

/// Pass 1 entry point. Discover systems under `root`, excluding `exclude` dir names,
/// applying an optional ruleset before built-in detection. Faithful to FDML's
/// `detect_systems`, plus root-app detection.
pub fn run(root: &Path, exclude: &[String], rules: Option<&DiscoveryRules>) -> Vec<System> {
    let mut systems = Vec::new();
    let root_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("platform");

    // Check for docker-compose at root for service name hints.
    let compose_services = parse_docker_compose(root);

    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return systems,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if exclude.contains(&dir_name) || is_infrastructure_dir(&dir_name) {
            continue;
        }

        if let Some(system) = detect_system_in_dir(&path, &dir_name, root, rules) {
            systems.push(system);
        } else {
            // Not a system itself — recurse ONE level looking for systems nested in a
            // grouping dir. Improvement over FDML's hardcoded container whitelist, which
            // missed layouts like achiever/achiever-mvp/{backend,frontend} (depth-2 manifests).
            // Infra dirs are still skipped below; deeper nesting (3+ levels) is not followed.
            if let Ok(inner_entries) = std::fs::read_dir(&path) {
                for inner in inner_entries.flatten() {
                    let inner_path = inner.path();
                    if !inner_path.is_dir() {
                        continue;
                    }
                    let inner_name = match inner_path.file_name().and_then(|n| n.to_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    if exclude.contains(&inner_name) || is_infrastructure_dir(&inner_name) {
                        continue;
                    }
                    if let Some(system) = detect_system_in_dir(&inner_path, &inner_name, root, rules) {
                        systems.push(system);
                    }
                }
            }
        }
    }

    // Detect the root itself as a system too.
    //
    // IMPROVEMENT over FDML's detect_systems, which only checked root when NO subsystems
    // were found — that dropped the PRIMARY app in repos like a root React app with helper
    // sub-projects (e.g. studio-setup: the studio frontend was invisible). We add root
    // whenever it has its own boundary marker and isn't already represented by a subsystem.
    if let Some(root_system) = detect_system_in_dir(root, root_name, root, rules) {
        let already = systems
            .iter()
            .any(|s| s.path == root_system.path || s.id == root_system.id);
        if !already {
            systems.push(root_system);
        }
    }

    // Enrich with docker-compose hints.
    for (service_name, _service_path) in &compose_services {
        let already_detected = systems
            .iter()
            .any(|s| name_similarity(&s.id, service_name) >= 0.6);
        if !already_detected {
            let potential_path = root.join(service_name);
            if potential_path.is_dir() {
                if let Some(system) = detect_system_in_dir(&potential_path, service_name, root, rules) {
                    systems.push(system);
                }
            }
        }
    }

    systems
}

fn is_infrastructure_dir(name: &str) -> bool {
    matches!(name,
        "node_modules" | "__pycache__" | ".git" | ".svn" | ".hg"
        | "venv" | ".venv" | "env" | ".env"
        | "target" | "build" | "dist" | "out" | "bin" | "obj"
        | ".idea" | ".vscode" | ".vs"
        | "vendor" | "packages" | ".nuget"
        | "site-packages" | "lib" | "libs"
        | ".tox" | ".pytest_cache" | ".mypy_cache"
        | "migrations" | "static" | "media" | "assets"
        | ".next" | ".nuxt" | ".output" | ".cache"
        | "coverage" | "docs" | "doc" | "scripts" | "deploy"
        | "k8s" | "kubernetes" | "helm" | "terraform" | ".github"
        | "ci" | ".circleci" | "infra" | "infrastructure"
    )
}

fn detect_system_in_dir(dir: &Path, dir_name: &str, root: &Path, rules: Option<&DiscoveryRules>) -> Option<System> {
    let rel_path = dir
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dir_name.to_string());
    let rel_path = if rel_path.is_empty() { ".".to_string() } else { rel_path };

    // Experimental: declarative rules win over built-in detection when they match.
    let boundary = rules
        .and_then(|r| r.match_dir(dir))
        .or_else(|| detect_boundary(dir));

    if let Some((system_type, technology, marker)) = boundary {
        return Some(System {
            path: rel_path,
            id: normalize_name(dir_name),
            name: humanize_name(dir_name),
            system_type,
            technology,
            boundary_marker: marker,
        });
    }
    None
}

/// Detect boundary markers in a directory and classify the system (verbatim from FDML).
fn detect_boundary(dir: &Path) -> Option<(String, String, String)> {
    let pkg_json = dir.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            let (sys_type, tech) = classify_package_json(&content);
            return Some((sys_type, tech, "package.json".to_string()));
        }
    }

    let pyproject = dir.join("pyproject.toml");
    if pyproject.exists() {
        if let Ok(content) = std::fs::read_to_string(&pyproject) {
            let (sys_type, tech) = classify_python_project(&content);
            return Some((sys_type, tech, "pyproject.toml".to_string()));
        }
    }

    let requirements = dir.join("requirements.txt");
    if requirements.exists() {
        if let Ok(content) = std::fs::read_to_string(&requirements) {
            // requirements.txt uses the same framework rules as pyproject.toml.
            let (sys_type, tech) = classify_python_project(&content);
            return Some((sys_type, tech, "requirements.txt".to_string()));
        }
    }

    if dir.join("Cargo.toml").exists() {
        return Some(("service".to_string(), "Rust".to_string(), "Cargo.toml".to_string()));
    }

    if dir.join("go.mod").exists() {
        return Some(("service".to_string(), "Go".to_string(), "go.mod".to_string()));
    }

    // .NET — *.csproj / *.fsproj / *.vbproj (first one found).
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".csproj") {
                    let (sys_type, tech) = classify_csproj(&p);
                    return Some((sys_type, tech, name.to_string()));
                }
                if name.ends_with(".fsproj") {
                    return Some(("service".to_string(), "F# (.NET)".to_string(), name.to_string()));
                }
                if name.ends_with(".vbproj") {
                    return Some(("service".to_string(), "VB.NET".to_string(), name.to_string()));
                }
            }
        }
    }

    if dir.join("pom.xml").exists() {
        return Some(("service".to_string(), "Java (Maven)".to_string(), "pom.xml".to_string()));
    }
    if dir.join("build.gradle").exists() {
        return Some(("service".to_string(), "Java (Gradle)".to_string(), "build.gradle".to_string()));
    }
    if dir.join("build.gradle.kts").exists() {
        return Some(("service".to_string(), "Kotlin (Gradle)".to_string(), "build.gradle.kts".to_string()));
    }

    if dir.join("Gemfile").exists() {
        return Some(("service".to_string(), "Ruby".to_string(), "Gemfile".to_string()));
    }

    if dir.join("composer.json").exists() {
        return Some(("service".to_string(), "PHP".to_string(), "composer.json".to_string()));
    }

    if dir.join("Dockerfile").exists() {
        return Some(("service".to_string(), "Docker".to_string(), "Dockerfile".to_string()));
    }

    None
}

/// Classify a .csproj — distinguish frontend-ish (Blazor/MAUI/WinUI/WPF/WinForms) from service.
fn classify_csproj(path: &Path) -> (String, String) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let lower = content.to_lowercase();
    if lower.contains("microsoft.net.sdk.blazorwebassembly") || lower.contains("microsoft.net.sdk.razor") {
        return ("frontend".to_string(), "Blazor (.NET)".to_string());
    }
    if lower.contains("usewpf") || lower.contains("<usewpf>true</usewpf>") {
        return ("frontend".to_string(), "WPF (.NET)".to_string());
    }
    if lower.contains("usewindowsforms") {
        return ("frontend".to_string(), "WinForms (.NET)".to_string());
    }
    if lower.contains("microsoft.maui") {
        return ("frontend".to_string(), "MAUI (.NET)".to_string());
    }
    if lower.contains("microsoft.windowsappsdk") || lower.contains("winui") {
        return ("frontend".to_string(), "WinUI 3 (.NET)".to_string());
    }
    ("service".to_string(), "C# (.NET)".to_string())
}

fn classify_package_json(content: &str) -> (String, String) {
    let lower = content.to_lowercase();
    if lower.contains("\"react\"") || lower.contains("\"next\"") || lower.contains("\"@next/") {
        return ("frontend".to_string(), "React + TypeScript".to_string());
    }
    if lower.contains("\"vue\"") || lower.contains("\"nuxt\"") {
        return ("frontend".to_string(), "Vue.js".to_string());
    }
    if lower.contains("\"@angular/core\"") {
        return ("frontend".to_string(), "Angular".to_string());
    }
    if lower.contains("\"svelte\"") || lower.contains("\"@sveltejs/") {
        return ("frontend".to_string(), "Svelte".to_string());
    }
    if lower.contains("\"express\"") || lower.contains("\"fastify\"") || lower.contains("\"koa\"") {
        return ("service".to_string(), "Node.js".to_string());
    }
    if lower.contains("\"@nestjs/core\"") {
        return ("service".to_string(), "NestJS".to_string());
    }
    ("service".to_string(), "Node.js".to_string())
}

fn classify_python_project(content: &str) -> (String, String) {
    let lower = content.to_lowercase();
    if lower.contains("fastapi") {
        ("service".to_string(), "FastAPI + Python".to_string())
    } else if lower.contains("django") {
        ("service".to_string(), "Django + Python".to_string())
    } else if lower.contains("flask") {
        ("service".to_string(), "Flask + Python".to_string())
    } else if lower.contains("celery") {
        ("worker".to_string(), "Celery + Python".to_string())
    } else {
        ("service".to_string(), "Python".to_string())
    }
}

fn parse_docker_compose(root: &Path) -> Vec<(String, String)> {
    let compose_files = ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"];
    for name in &compose_files {
        let path = root.join(name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                return extract_compose_services(&content);
            }
        }
    }
    Vec::new()
}

fn extract_compose_services(content: &str) -> Vec<(String, String)> {
    // Simple YAML scan: top-level `services:` key and its 2-space-indented children.
    let mut services = Vec::new();
    let mut in_services = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "services:" {
            in_services = true;
            continue;
        }
        if in_services {
            if !line.starts_with(' ') && !line.is_empty() {
                break; // Left the services block.
            }
            let indent = line.len() - line.trim_start().len();
            if indent == 2 && trimmed.ends_with(':') {
                let service_name = trimmed.trim_end_matches(':').to_string();
                services.push((service_name.clone(), service_name));
            }
        }
    }
    services
}

fn humanize_name(dir_name: &str) -> String {
    dir_name
        .replace('-', " ")
        .replace('_', " ")
        .split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;

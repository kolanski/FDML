use std::collections::HashMap;
use std::path::Path;
use std::io::BufRead;

use crate::scanner::types::ScanResult;
use super::types::*;
use super::{normalize_name, name_similarity, link_code, generate_metaprompt};

// ─── .fdmlignore ─────────────────────────────────────────────────

/// Read .fdmlignore file from root directory, returning list of patterns to exclude
pub fn read_fdmlignore(root: &Path) -> Vec<String> {
    let ignore_path = root.join(".fdmlignore");
    if !ignore_path.exists() {
        return Vec::new();
    }
    match std::fs::File::open(&ignore_path) {
        Ok(file) => {
            std::io::BufReader::new(file)
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
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

// ─── Boundary detection ──────────────────────────────────────────

/// Detect system boundaries in a multi-system project root
pub fn detect_systems(root: &Path, exclude: &[String]) -> Vec<DetectedSystem> {
    let mut systems = Vec::new();
    let root_name = root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("platform");

    // Check for docker-compose at root for service name hints
    let compose_services = parse_docker_compose(root);

    // Walk immediate subdirectories looking for boundary markers
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

        // Skip excluded and common non-system dirs
        if exclude.contains(&dir_name) || is_infrastructure_dir(&dir_name) {
            continue;
        }

        if let Some(system) = detect_system_in_dir(&path, &dir_name, root) {
            systems.push(system);
        }
    }

    // Also check root itself if no subdirectory systems found
    if systems.is_empty() {
        if let Some(system) = detect_system_in_dir(root, root_name, root) {
            systems.push(system);
        }
    }

    // Enrich with docker-compose hints
    for (service_name, _service_path) in &compose_services {
        // If a compose service matches a detected system, skip
        let already_detected = systems.iter().any(|s| {
            name_similarity(&s.id, service_name) >= 0.6
        });
        if !already_detected {
            // Check if the service name matches a subdirectory
            let potential_path = root.join(service_name);
            if potential_path.is_dir() {
                if let Some(system) = detect_system_in_dir(&potential_path, service_name, root) {
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

fn detect_system_in_dir(dir: &Path, dir_name: &str, root: &Path) -> Option<DetectedSystem> {
    let rel_path = dir.strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dir_name.to_string());
    let rel_path = if rel_path.is_empty() { ".".to_string() } else { rel_path };

    // Check for boundary markers in priority order
    if let Some((system_type, technology, marker)) = detect_boundary(dir) {
        let id = normalize_name(dir_name);
        let name = humanize_name(dir_name);
        return Some(DetectedSystem {
            path: rel_path,
            id,
            name,
            system_type,
            technology,
            boundary_marker: marker,
        });
    }
    None
}

/// Detect boundary markers in a directory and classify the system
fn detect_boundary(dir: &Path) -> Option<(String, String, String)> {
    // package.json → frontend or service
    let pkg_json = dir.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            let (sys_type, tech) = classify_package_json(&content);
            return Some((sys_type, tech, "package.json".to_string()));
        }
    }

    // pyproject.toml
    let pyproject = dir.join("pyproject.toml");
    if pyproject.exists() {
        if let Ok(content) = std::fs::read_to_string(&pyproject) {
            let (sys_type, tech) = classify_python_project(&content);
            return Some((sys_type, tech, "pyproject.toml".to_string()));
        }
    }

    // requirements.txt
    let requirements = dir.join("requirements.txt");
    if requirements.exists() {
        if let Ok(content) = std::fs::read_to_string(&requirements) {
            let (sys_type, tech) = classify_python_requirements(&content);
            return Some((sys_type, tech, "requirements.txt".to_string()));
        }
    }

    // Cargo.toml
    let cargo_toml = dir.join("Cargo.toml");
    if cargo_toml.exists() {
        return Some(("service".to_string(), "Rust".to_string(), "Cargo.toml".to_string()));
    }

    // go.mod
    let go_mod = dir.join("go.mod");
    if go_mod.exists() {
        return Some(("service".to_string(), "Go".to_string(), "go.mod".to_string()));
    }

    // Dockerfile (fallback)
    let dockerfile = dir.join("Dockerfile");
    if dockerfile.exists() {
        return Some(("service".to_string(), "Docker".to_string(), "Dockerfile".to_string()));
    }

    None
}

fn classify_package_json(content: &str) -> (String, String) {
    let lower = content.to_lowercase();

    // Check for frontend frameworks
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

    // Check for backend frameworks
    if lower.contains("\"express\"") || lower.contains("\"fastify\"") || lower.contains("\"koa\"") {
        return ("service".to_string(), "Node.js".to_string());
    }
    if lower.contains("\"@nestjs/core\"") {
        return ("service".to_string(), "NestJS".to_string());
    }

    // Default: service (Node.js)
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

fn classify_python_requirements(content: &str) -> (String, String) {
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
    // Simple YAML parsing — look for top-level services: key and its children
    let mut services = Vec::new();
    let mut in_services = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "services:" {
            in_services = true;
            continue;
        }
        if in_services {
            // Top-level key under services (2-space indent, ends with :)
            if !line.starts_with(' ') && !line.is_empty() {
                break; // Left services block
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

// ─── Integration detection ───────────────────────────────────────

/// Integration import patterns
struct IntegrationPattern {
    keywords: &'static [&'static str],
    integration_type: &'static str,
    technology: &'static str,
}

const INTEGRATION_PATTERNS: &[IntegrationPattern] = &[
    IntegrationPattern {
        keywords: &["import requests", "import httpx", "from requests", "from httpx", "fetch(", "axios", "import axios", "require(\"axios\")", "require('axios')"],
        integration_type: "http",
        technology: "REST/JSON",
    },
    IntegrationPattern {
        keywords: &["import redis", "from redis", "aioredis", "ioredis", "require(\"redis\")", "require('redis')"],
        integration_type: "event",
        technology: "Redis pub/sub",
    },
    IntegrationPattern {
        keywords: &["import pika", "from pika", "amqplib", "amqp"],
        integration_type: "queue",
        technology: "RabbitMQ",
    },
    IntegrationPattern {
        keywords: &["from kafka", "confluent_kafka", "kafkajs", "import kafka"],
        integration_type: "event",
        technology: "Kafka",
    },
    IntegrationPattern {
        keywords: &["import psycopg", "from psycopg", "sqlalchemy", "pg.", "require(\"pg\")", "require('pg')", "prisma"],
        integration_type: "shared_db",
        technology: "PostgreSQL",
    },
    IntegrationPattern {
        keywords: &["import pymongo", "from pymongo", "mongoose", "mongodb"],
        integration_type: "shared_db",
        technology: "MongoDB",
    },
    IntegrationPattern {
        keywords: &["import grpc", "from grpc", "@grpc/", "google.golang.org/grpc"],
        integration_type: "grpc",
        technology: "gRPC",
    },
    IntegrationPattern {
        keywords: &["WebSocket", "socket.io", "ws.", "import ws", "require(\"ws\")", "require('ws')"],
        integration_type: "websocket",
        technology: "WebSocket",
    },
];

/// Detect integrations by scanning import patterns across all systems
pub fn detect_integrations(
    systems: &[(DetectedSystem, ScanResult, LinkReport)],
) -> Vec<IntegrationHint> {
    let mut hints = Vec::new();

    for (system, scan, _report) in systems {
        let mut seen_types: HashMap<String, Vec<String>> = HashMap::new();

        for file in &scan.files {
            let file_content_hints = scan_file_for_integrations(file, &scan.metadata.codebase_path);
            for (itype, tech, evidence_file) in file_content_hints {
                seen_types
                    .entry(format!("{}|{}", itype, tech))
                    .or_default()
                    .push(evidence_file);
            }
        }

        for (key, evidence) in seen_types {
            let parts: Vec<&str> = key.splitn(2, '|').collect();
            let integration_type = parts[0].to_string();
            let technology = parts.get(1).unwrap_or(&"").to_string();

            // Try to infer to_system via cross-referencing
            let to_system = infer_target_system(&integration_type, &system.id, systems);

            hints.push(IntegrationHint {
                from_system: system.id.clone(),
                to_system,
                integration_type,
                technology,
                evidence,
            });
        }
    }

    hints
}

fn scan_file_for_integrations(
    file: &crate::scanner::types::FileAnalysis,
    _codebase_path: &str,
) -> Vec<(String, String, String)> {
    let mut results = Vec::new();

    // Check imports against patterns
    for import in &file.imports {
        for pattern in INTEGRATION_PATTERNS {
            for keyword in pattern.keywords {
                // Check if the import module matches
                if import.module.contains(&keyword.replace("import ", "").replace("from ", ""))
                    || import.names.iter().any(|n| keyword.contains(n))
                {
                    results.push((
                        pattern.integration_type.to_string(),
                        pattern.technology.to_string(),
                        file.file_path.clone(),
                    ));
                    break;
                }
            }
        }
    }

    results
}

/// Try to infer which system is the target of an integration
fn infer_target_system(
    integration_type: &str,
    from_system_id: &str,
    systems: &[(DetectedSystem, ScanResult, LinkReport)],
) -> Option<String> {
    match integration_type {
        "http" => {
            // If system A makes HTTP calls, look for systems that are services
            for (sys, _scan, _report) in systems {
                if sys.id != from_system_id && sys.system_type == "service" {
                    return Some(sys.id.clone());
                }
            }
            None
        }
        _ => None,
    }
}

// ─── Shared entity detection ─────────────────────────────────────

/// Detect shared entities across systems by comparing entity names
pub fn detect_shared_entities(
    systems: &[(DetectedSystem, LinkReport)],
) -> Vec<SharedEntityHint> {
    let mut entity_map: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();

    // Collect all entities per system
    for (system, report) in systems {
        for entity in &report.entities {
            let normalized = normalize_name(&entity.entity_name);
            let fields: Vec<String> = entity.fields.iter().map(|f| f.name.clone()).collect();
            entity_map
                .entry(normalized)
                .or_default()
                .push((system.id.clone(), fields));
        }
    }

    // Also check cross-system similarity for non-exact matches
    let all_entities: Vec<(String, String, Vec<String>)> = systems
        .iter()
        .flat_map(|(sys, report)| {
            report.entities.iter().map(move |e| {
                let fields: Vec<String> = e.fields.iter().map(|f| f.name.clone()).collect();
                (sys.id.clone(), e.entity_name.clone(), fields)
            })
        })
        .collect();

    for i in 0..all_entities.len() {
        for j in (i + 1)..all_entities.len() {
            let (sys_a, name_a, fields_a) = &all_entities[i];
            let (sys_b, name_b, fields_b) = &all_entities[j];

            if sys_a == sys_b {
                continue;
            }

            let norm_a = normalize_name(name_a);
            let norm_b = normalize_name(name_b);

            // Skip if already exact match (handled above)
            if norm_a == norm_b {
                continue;
            }

            let sim = name_similarity(name_a, name_b);
            if sim >= 0.6 {
                // Use the more common name as key
                let key = if name_a.len() >= name_b.len() {
                    norm_a.clone()
                } else {
                    norm_b.clone()
                };
                let entry = entity_map.entry(key).or_default();
                // Add both if not already present
                if !entry.iter().any(|(s, _)| s == sys_a) {
                    entry.push((sys_a.clone(), fields_a.clone()));
                }
                if !entry.iter().any(|(s, _)| s == sys_b) {
                    entry.push((sys_b.clone(), fields_b.clone()));
                }
            }
        }
    }

    // Convert to SharedEntityHints (only entities appearing in 2+ systems)
    let mut hints: Vec<SharedEntityHint> = entity_map
        .into_iter()
        .filter(|(_, systems)| systems.len() >= 2)
        .map(|(name, systems)| {
            let canonical = systems
                .iter()
                .max_by_key(|(_, fields)| fields.len())
                .map(|(sys_id, _)| sys_id.clone());

            SharedEntityHint {
                entity_name: name,
                systems,
                canonical_system: canonical,
            }
        })
        .collect();

    hints.sort_by(|a, b| a.entity_name.cmp(&b.entity_name));
    hints
}

// ─── Platform metaprompt ─────────────────────────────────────────

/// Generate a structured prompt for LLM to produce an FDML 1.4 platform spec
pub fn generate_platform_metaprompt(
    report: &PlatformReport,
    per_system_specs: &[(String, String)],
    _per_system_prompts: &[(String, String)],
) -> String {
    let mut prompt = String::new();

    prompt.push_str("# FDML Platform Analysis Report\n\n");
    prompt.push_str("You are analyzing a multi-system platform to create an FDML 1.4 platform specification.\n");
    prompt.push_str("Below is the automated analysis of all detected systems, their integrations,\n");
    prompt.push_str("and shared entities. Your task is to produce a valid FDML 1.4 YAML spec.\n\n");

    // ─── Instructions
    prompt.push_str("## Instructions\n\n");
    prompt.push_str("1. **Group systems into contours** by trust/exposure level:\n");
    prompt.push_str("   - `external`: User-facing frontends, public APIs\n");
    prompt.push_str("   - `integration`: Internal API gateways, BFFs\n");
    prompt.push_str("   - `core`: Business logic services\n");
    prompt.push_str("   - `infrastructure`: Databases, caches, queues\n");
    prompt.push_str("2. **Define each system** with its entities, actions, features\n");
    prompt.push_str("3. **Map integrations** with endpoints and protocols\n");
    prompt.push_str("4. **Identify cross-flows** that span multiple systems\n");
    prompt.push_str("5. **Map shared entities** with source/projection/cache roles\n\n");

    // ─── FDML 1.4 Spec Reference
    prompt.push_str("---\n\n## FDML 1.4 Platform Specification Reference\n\n");
    prompt.push_str("Output a **single valid YAML document** with these top-level keys.\n\n");

    prompt.push_str("```yaml\n");
    prompt.push_str("metadata:\n");
    prompt.push_str("  version: \"1.4\"\n");
    prompt.push_str("  name: string           # Platform name\n\n");

    prompt.push_str("contours:                # Trust/exposure boundaries\n");
    prompt.push_str("  - id: string           # e.g. external, integration, core, infrastructure\n");
    prompt.push_str("    name: string\n");
    prompt.push_str("    description: string\n");
    prompt.push_str("    trust_level: string   # public | internal | restricted | critical\n");
    prompt.push_str("    systems:             # List of system IDs in this contour\n");
    prompt.push_str("      - string\n\n");

    prompt.push_str("systems:                 # Each system in the platform\n");
    prompt.push_str("  - id: string\n");
    prompt.push_str("    name: string\n");
    prompt.push_str("    description: string\n");
    prompt.push_str("    type: string          # frontend | service | worker | gateway\n");
    prompt.push_str("    technology: string\n");
    prompt.push_str("    spec: string          # Optional: path to per-system .fdml file\n");
    prompt.push_str("    entities:            # Inline entities for this system\n");
    prompt.push_str("      - id: string\n");
    prompt.push_str("        name: string\n");
    prompt.push_str("        fields:\n");
    prompt.push_str("          - name: string\n");
    prompt.push_str("            type: string\n");
    prompt.push_str("            required: bool\n");
    prompt.push_str("    actions:\n");
    prompt.push_str("      - id: string\n");
    prompt.push_str("        name: string\n");
    prompt.push_str("        description: string\n");
    prompt.push_str("    features:\n");
    prompt.push_str("      - id: string\n");
    prompt.push_str("        title: string\n");
    prompt.push_str("        scenarios:\n");
    prompt.push_str("          - id: string\n");
    prompt.push_str("            title: string\n");
    prompt.push_str("            given: [string]\n");
    prompt.push_str("            when: [string]\n");
    prompt.push_str("            then: [string]\n\n");

    prompt.push_str("integrations:            # Connections between systems\n");
    prompt.push_str("  - id: string\n");
    prompt.push_str("    from: string          # System ID\n");
    prompt.push_str("    to: string            # System ID\n");
    prompt.push_str("    type: string          # http | grpc | event | queue | shared_db | websocket\n");
    prompt.push_str("    technology: string\n");
    prompt.push_str("    endpoints:           # Optional: specific endpoints\n");
    prompt.push_str("      - path: string\n");
    prompt.push_str("        method: string\n");
    prompt.push_str("        description: string\n\n");

    prompt.push_str("cross_flows:             # Flows spanning multiple systems\n");
    prompt.push_str("  - id: string\n");
    prompt.push_str("    name: string\n");
    prompt.push_str("    description: string\n");
    prompt.push_str("    steps:\n");
    prompt.push_str("      - id: string\n");
    prompt.push_str("        system: string    # Which system handles this step\n");
    prompt.push_str("        action: string\n");
    prompt.push_str("        description: string\n\n");

    prompt.push_str("shared_entities:         # Entities that span systems\n");
    prompt.push_str("  - id: string\n");
    prompt.push_str("    name: string\n");
    prompt.push_str("    fields:\n");
    prompt.push_str("      - name: string\n");
    prompt.push_str("        type: string\n");
    prompt.push_str("    mappings:\n");
    prompt.push_str("      - system: string\n");
    prompt.push_str("        role: string      # source | projection | cache\n");
    prompt.push_str("        local_entity: string\n");
    prompt.push_str("```\n\n");

    // ─── Detected Systems
    prompt.push_str("---\n\n## Detected Systems\n\n");
    for (i, system) in report.detected_systems.iter().enumerate() {
        prompt.push_str(&format!("### {}. {} (`{}`)\n", i + 1, system.name, system.id));
        prompt.push_str(&format!("- **Path**: `{}`\n", system.path));
        prompt.push_str(&format!("- **Type**: {}\n", system.system_type));
        prompt.push_str(&format!("- **Technology**: {}\n", system.technology));
        prompt.push_str(&format!("- **Detected by**: `{}`\n", system.boundary_marker));

        // Find the corresponding scan/report
        if let Some((_sys_id, link_report)) = report.per_system.iter().find(|(id, _)| id == &system.id) {
            prompt.push_str(&format!("- **Entities**: {} candidates\n", link_report.entities.len()));
            prompt.push_str(&format!("- **Actions**: {} candidates\n", link_report.actions.len()));
            prompt.push_str(&format!("- **Features**: {} suggestions\n", link_report.features.len()));

            // List top entities
            if !link_report.entities.is_empty() {
                prompt.push_str("- **Entity candidates**:\n");
                for entity in &link_report.entities {
                    let field_count = entity.fields.len();
                    prompt.push_str(&format!("  - `{}` ({} fields) — `{}`\n",
                        entity.entity_name, field_count, entity.code_ref));
                }
            }

            // List top actions
            if !link_report.actions.is_empty() {
                prompt.push_str("- **Action candidates**:\n");
                for action in link_report.actions.iter().take(15) {
                    let desc = action.description.as_deref().unwrap_or("");
                    let desc_short = if desc.len() > 80 { &desc[..80] } else { desc };
                    prompt.push_str(&format!("  - `{}` — {}\n", action.action_name, desc_short));
                }
                if link_report.actions.len() > 15 {
                    prompt.push_str(&format!("  - ... and {} more\n", link_report.actions.len() - 15));
                }
            }
        }

        // Technology info from detected system
        prompt.push_str(&format!("- **Technology**: {}\n", system.technology));

        prompt.push_str("\n");
    }

    // ─── Integration Hints
    prompt.push_str("---\n\n## Integration Hints\n\n");
    if report.integration_hints.is_empty() {
        prompt.push_str("No integration patterns detected. Look for API calls, message queues, or shared databases.\n\n");
    } else {
        for (i, hint) in report.integration_hints.iter().enumerate() {
            let target = hint.to_system.as_deref().unwrap_or("(unknown)");
            prompt.push_str(&format!("{}. **{}** → **{}** via {} ({})\n",
                i + 1, hint.from_system, target, hint.integration_type, hint.technology));
            prompt.push_str("   Evidence:\n");
            for ev in &hint.evidence {
                prompt.push_str(&format!("   - `{}`\n", ev));
            }
            prompt.push_str("\n");
        }
    }

    // ─── Shared Entity Hints
    prompt.push_str("---\n\n## Shared Entity Candidates\n\n");
    if report.shared_entity_hints.is_empty() {
        prompt.push_str("No shared entities detected across systems.\n\n");
    } else {
        for (i, hint) in report.shared_entity_hints.iter().enumerate() {
            let canonical = hint.canonical_system.as_deref().unwrap_or("?");
            prompt.push_str(&format!("{}. **{}** (canonical: `{}`)\n",
                i + 1, hint.entity_name, canonical));
            for (sys_id, fields) in &hint.systems {
                prompt.push_str(&format!("   - `{}`: {} fields ({})\n",
                    sys_id, fields.len(),
                    fields.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
            }
            prompt.push_str("\n");
        }
    }

    // ─── Per-system specs (already generated)
    if !per_system_specs.is_empty() {
        prompt.push_str("---\n\n## Per-System FDML Specs (already generated)\n\n");
        for (sys_id, spec) in per_system_specs {
            prompt.push_str(&format!("### System: `{}`\n\n```yaml\n{}\n```\n\n", sys_id, spec));
        }
    }

    // ─── Per-system entity details from analysis
    prompt.push_str("---\n\n## Per-System Details\n\n");
    for (sys_id, link_report) in &report.per_system {
        prompt.push_str(&format!("### `{}`\n\n", sys_id));
        for entity in &link_report.entities {
            prompt.push_str(&format!("**Entity: {}** (`{}`)\n", entity.entity_name, entity.entity_id));
            if !entity.fields.is_empty() {
                for field in &entity.fields {
                    let ftype = field.field_type.as_deref().unwrap_or("?");
                    prompt.push_str(&format!("  - {}: {}\n", field.name, ftype));
                }
            }
            prompt.push_str("\n");
        }
    }

    // ─── Checklist
    prompt.push_str("---\n\n## Checklist\n\n");
    prompt.push_str("- [ ] Group systems into contours (external/integration/core/infrastructure)\n");
    prompt.push_str("- [ ] Define each system's entities, actions, and features\n");
    prompt.push_str("- [ ] Map integrations with protocols and endpoints\n");
    prompt.push_str("- [ ] Identify cross-flows spanning multiple systems\n");
    prompt.push_str("- [ ] Map shared entities with source/projection/cache roles\n");
    prompt.push_str("- [ ] Write BDD scenarios for each feature\n");

    prompt
}

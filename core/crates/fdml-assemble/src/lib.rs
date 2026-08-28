//! Pass 6 — deterministic FDML spec assembly from scanner + linker data.
//! No LLM needed — builds valid YAML from parsed code artifacts.
//!
//! Ported faithfully from FDML's `src/linker/assemble.rs::assemble_spec_no_llm`
//! (the `--no-llm` deterministic path). Only the mechanical rework swaps were made:
//!   * `crate::scanner::types::*`            → `fdml_types::scan::*`
//!   * `super::types::*` / `LinkReport`      → `fdml_types::graph::*`
//!   * `super::normalize_name`              → `fdml_util::normalize_name`
//!
//! Two intentional divergences, both for reproducibility / scope:
//!   * Dropped the `created: <Utc::now()>` metadata line (wall-clock, non-deterministic) —
//!     mirrors the `scan_timestamp` / `timestamp` drops in `scan.rs` / `graph.rs`. No
//!     chrono dependency is pulled in.
//!   * The original emitted a `flows:` section via `super::flows::discover_flows` /
//!     `flows_to_yaml`. Flow reconstruction is a SEPARATE pass (`fdml-flows`, pass 4)
//!     not yet ported into `core/` (and slated for a rewrite per ARCHITECTURE.md), so it
//!     is not available here. The flows section is therefore omitted. Everything else
//!     (metadata / system / entities / actions / features / traceability) is byte-faithful.
//!
//! The two LLM-classification assemblers (`assemble_from_classifications`,
//! `assemble_from_classifications_with_scenarios`) are deliberately NOT ported.

use fdml_types::flow::Flow;
use fdml_types::graph::LinkReport;
use fdml_types::integration::PlatformLinks;
use fdml_types::scan::ScanResult;
use fdml_types::System;
use fdml_util::normalize_name;

/// Heuristic filters for entity classification
fn is_likely_helper(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with("props")
        || lower.ends_with("config")
        || lower.ends_with("options")
        || lower.ends_with("params")
        || lower.ends_with("args")
        || lower.ends_with("settings")
        || lower.ends_with("context")
        || lower.ends_with("provider")
        || lower.ends_with("hook")
        || lower.ends_with("mixin")
        || lower.ends_with("util")
        || lower.ends_with("utils")
        || lower.ends_with("helper")
        || lower.ends_with("helpers")
        || lower.ends_with("constants")
        || lower.starts_with("use")
        || lower.starts_with("i18n")
        || lower == "app"
        || lower == "main"
        || lower == "index"
}

fn is_likely_utility_action(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("use")
        || lower.starts_with("render")
        || lower.starts_with("get_")
        || lower.starts_with("set_")
        || lower.starts_with("is_")
        || lower.starts_with("has_")
        || lower.starts_with("to_")
        || lower.starts_with("from_")
        || lower == "main"
        || lower == "app"
        || lower == "index"
        || lower == "setup"
        || lower == "init"
        || lower == "configure"
}

/// Build a complete FDML YAML spec deterministically from link report + scan data.
/// No LLM involved — uses heuristics for classification.
pub fn run(report: &LinkReport, scan: &ScanResult, flows: &[Flow], system_name: Option<&str>) -> String {
    let sys_name = system_name.unwrap_or_else(|| {
        scan.metadata.codebase_path.split('/').last().unwrap_or("system")
    });
    let sys_id = normalize_name(sys_name);
    let languages: Vec<String> = scan.metadata.languages_detected.iter()
        .map(|l| l.name().to_string())
        .collect();
    let tech = if languages.is_empty() { "Unknown".to_string() } else { languages.join(" + ") };

    let mut yaml = String::new();

    // ─── Metadata ───
    yaml.push_str("metadata:\n");
    yaml.push_str("  version: \"1.3\"\n");
    yaml.push_str(&format!("  name: \"{}\"\n", sys_name));
    yaml.push_str(&format!("  description: \"{} — auto-generated from code analysis\"\n", sys_name));
    // NOTE: original emitted `created: <Utc::now()>` here — dropped for determinism.
    yaml.push_str("\n");

    // ─── System ───
    let components: Vec<String> = scan.modules.iter()
        .filter(|m| !m.children.is_empty() || !m.file_path.as_ref().map(|p| p.contains('/')).unwrap_or(false))
        .map(|m| m.name.clone())
        .take(10)
        .collect();

    yaml.push_str("system:\n");
    yaml.push_str(&format!("  id: {}\n", sys_id));
    yaml.push_str(&format!("  name: \"{}\"\n", sys_name));
    yaml.push_str(&format!("  description: \"{} ({}) — {} files\"\n", sys_name, tech, scan.metadata.total_files));
    if !components.is_empty() {
        yaml.push_str("  components:\n");
        for c in &components {
            yaml.push_str(&format!("    - {}\n", c));
        }
    }
    yaml.push_str("  relationships: []\n");
    yaml.push_str("\n");

    // ─── Entities ───
    let domain_entities: Vec<&fdml_types::graph::EntityLink> = report.entities.iter()
        .filter(|e| !is_likely_helper(&e.entity_name) && e.fields.len() >= 2)
        .collect();

    if !domain_entities.is_empty() {
        yaml.push_str("entities:\n");
        for entity in &domain_entities {
            yaml.push_str(&format!("  - id: {}\n", entity.entity_id));
            yaml.push_str(&format!("    name: \"{}\"\n", entity.entity_name));
            yaml.push_str(&format!("    description: \"{} entity (from {})\"\n", entity.entity_name, entity.code_ref));

            if !entity.fields.is_empty() {
                yaml.push_str("    fields:\n");
                for field in &entity.fields {
                    let ftype = map_field_type(field.field_type.as_deref().unwrap_or("string"));
                    yaml.push_str(&format!("      - name: {}\n", field.name));
                    yaml.push_str(&format!("        type: {}\n", ftype));
                    if field.name == "id" || field.name == "key" || field.name == "uuid" {
                        yaml.push_str("        required: true\n");
                    }
                }
            }
            yaml.push_str("\n");
        }
    }

    // ─── Actions ───
    let business_actions: Vec<&fdml_types::graph::ActionLink> = report.actions.iter()
        .filter(|a| !is_likely_utility_action(&a.action_name))
        .collect();

    if !business_actions.is_empty() {
        yaml.push_str("actions:\n");
        for action in &business_actions {
            yaml.push_str(&format!("  - id: {}\n", action.action_id));
            yaml.push_str(&format!("    name: \"{}\"\n", action.action_name));
            yaml.push_str(&format!("    description: \"{} (from {})\"\n", action.action_name, action.code_ref));

            let clean_params: Vec<&str> = action.input.iter()
                .map(|p| p.name.as_str())
                .filter(|n| !n.contains('{') && !n.contains('}') && !n.is_empty() && *n != "self" && *n != "cls")
                .collect();
            if !clean_params.is_empty() {
                yaml.push_str("    input:\n");
                yaml.push_str("      fields:\n");
                for name in &clean_params {
                    yaml.push_str(&format!("        - {}\n", name));
                }
            }

            if let Some(ref ret) = action.output {
                if ret != "void" && ret != "None" && ret != "unknown" {
                    let normalized = normalize_name(ret);
                    if domain_entities.iter().any(|e| e.entity_id == normalized) {
                        yaml.push_str("    output:\n");
                        yaml.push_str(&format!("      entity: {}\n", normalized));
                    }
                }
            }
            yaml.push_str("\n");
        }
    }

    // ─── Features ───
    if !report.features.is_empty() {
        yaml.push_str("features:\n");
        for feat in &report.features {
            yaml.push_str(&format!("  - id: {}\n", feat.feature_id));
            yaml.push_str(&format!("    title: \"{}\"\n", feat.title));
            yaml.push_str(&format!("    description: \"Feature group from module {}\"\n", feat.module_path));
            yaml.push_str("    scenarios:\n");
            yaml.push_str(&format!("      - id: {}_basic\n", feat.feature_id));
            yaml.push_str(&format!("        title: \"Basic {} operation\"\n", feat.title));
            yaml.push_str("        given:\n");
            yaml.push_str("          - \"System is initialized\"\n");
            yaml.push_str("        when:\n");
            yaml.push_str(&format!("          - \"{} is invoked\"\n", feat.title));
            yaml.push_str("        then:\n");
            yaml.push_str("          - \"Expected result is produced\"\n");
            yaml.push_str("\n");
        }
    }

    // ─── Flows (reconstructed by fdml-flows, pass 4) ───
    if !flows.is_empty() {
        yaml.push_str("flows:\n");
        for flow in flows {
            yaml.push_str(&format!("  - id: {}\n", flow.id));
            yaml.push_str(&format!("    name: \"{}\"\n", flow.name));
            yaml.push_str(&format!("    description: \"Reconstructed flow through {} steps\"\n", flow.steps.len()));
            yaml.push_str("    steps:\n");
            for (i, step) in flow.steps.iter().enumerate() {
                yaml.push_str(&format!("      - id: step_{}\n", i));
                yaml.push_str(&format!("        action: {}\n", step.action_id));
                yaml.push_str(&format!("        description: \"{}\"\n", step.description));
            }
            yaml.push_str("\n");
        }
    }

    // ─── Traceability ───
    let mut has_trace = false;
    let mut trace = String::new();
    for feat in &report.features {
        for action_id in &feat.actions {
            if business_actions.iter().any(|a| &a.action_id == action_id) {
                if !has_trace { trace.push_str("traceability:\n"); has_trace = true; }
                trace.push_str(&format!("  - from: \"feature:{}\"\n", feat.feature_id));
                trace.push_str(&format!("    to: \"action:{}\"\n", action_id));
                trace.push_str("    relation: implements\n");
            }
        }
    }
    for action in &business_actions {
        for param in &action.input {
            let ptype = param.param_type.as_deref().unwrap_or("");
            let normalized = normalize_name(ptype);
            if domain_entities.iter().any(|e| e.entity_id == normalized) {
                if !has_trace { trace.push_str("traceability:\n"); has_trace = true; }
                trace.push_str(&format!("  - from: \"action:{}\"\n", action.action_id));
                trace.push_str(&format!("    to: \"entity:{}\"\n", normalized));
                trace.push_str("    relation: depends_on\n");
            }
        }
    }
    yaml.push_str(&trace);

    yaml
}

/// Map code types to FDML types
fn map_field_type(code_type: &str) -> &str {
    let lower = code_type.to_lowercase();
    if lower.contains("string") || lower.contains("str") || lower.contains("text") {
        "string"
    } else if lower.contains("int") || lower.contains("number") || lower.contains("long") {
        "integer"
    } else if lower.contains("float") || lower.contains("double") || lower.contains("decimal") {
        "float"
    } else if lower.contains("bool") {
        "boolean"
    } else if lower.contains("date") && lower.contains("time") {
        "datetime"
    } else if lower.contains("date") {
        "date"
    } else if lower.contains("uuid") || lower.contains("guid") {
        "uuid"
    } else if lower.contains("[]") || lower.contains("array") || lower.contains("list") || lower.contains("vec") {
        "array"
    } else if lower.contains("map") || lower.contains("dict") || lower.contains("hash") || lower.contains("object") {
        "object"
    } else if lower.contains("enum") {
        "enum"
    } else {
        "string"
    }
}

/// Assemble a single FDML 1.4 **platform** spec from the detected systems and their
/// cross-system links. Deterministic, no LLM (FDML did this via an LLM metaprompt that
/// failed on small models — this is the deterministic replacement).
pub fn run_platform(systems: &[System], links: &PlatformLinks, platform_name: &str) -> String {
    let mut sorted = systems.to_vec();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    let mut yaml = String::new();
    yaml.push_str("metadata:\n");
    yaml.push_str("  version: \"1.4\"\n");
    yaml.push_str(&format!("  name: \"{platform_name}\"\n"));
    yaml.push_str(&format!("  description: \"{platform_name} platform — auto-generated from code analysis\"\n\n"));

    yaml.push_str("systems:\n");
    for s in &sorted {
        yaml.push_str(&format!("  - id: {}\n", s.id));
        yaml.push_str(&format!("    name: \"{}\"\n", s.name));
        yaml.push_str(&format!("    type: {}\n", s.system_type));
        yaml.push_str(&format!("    technology: \"{}\"\n", s.technology));
        yaml.push_str(&format!("    path: {}\n", s.path));
    }
    yaml.push('\n');

    // Contours: deterministic architectural layers by system type (FDML 1.4 / CodeBoarding).
    let contours: [(&str, &str, &[&str]); 4] = [
        ("external", "External / Presentation", &["frontend"]),
        ("integration", "Integration", &["gateway"]),
        ("core", "Core", &["service", "worker"]),
        ("infrastructure", "Infrastructure", &["library", "infra"]),
    ];
    let placed: std::collections::BTreeSet<&str> = contours.iter().flat_map(|(_, _, t)| t.iter().copied()).collect();
    let mut contour_yaml = String::from("contours:\n");
    let mut any = false;
    for (cid, cname, types) in contours {
        let members: Vec<&str> = sorted.iter().filter(|s| types.contains(&s.system_type.as_str())).map(|s| s.id.as_str()).collect();
        if members.is_empty() {
            continue;
        }
        any = true;
        contour_yaml.push_str(&format!("  - id: {cid}\n    name: \"{cname}\"\n    systems:\n"));
        for m in members {
            contour_yaml.push_str(&format!("      - {m}\n"));
        }
    }
    let other: Vec<&str> = sorted.iter().filter(|s| !placed.contains(s.system_type.as_str())).map(|s| s.id.as_str()).collect();
    if !other.is_empty() {
        any = true;
        contour_yaml.push_str("  - id: other\n    name: \"Other\"\n    systems:\n");
        for m in other {
            contour_yaml.push_str(&format!("      - {m}\n"));
        }
    }
    if any {
        yaml.push_str(&contour_yaml);
        yaml.push('\n');
    }

    if !links.integrations.is_empty() {
        yaml.push_str("integrations:\n");
        for (i, ig) in links.integrations.iter().enumerate() {
            yaml.push_str(&format!("  - id: integration_{i}\n"));
            yaml.push_str(&format!("    from: {}\n", ig.from_system));
            yaml.push_str(&format!("    to: {}\n", ig.to_system.as_deref().unwrap_or("external")));
            yaml.push_str(&format!("    type: {}\n", ig.integration_type));
            yaml.push_str(&format!("    technology: \"{}\"\n", ig.technology));
        }
        yaml.push('\n');
    }

    if !links.shared_entities.is_empty() {
        yaml.push_str("shared_entities:\n");
        for se in &links.shared_entities {
            yaml.push_str(&format!("  - name: {}\n    systems:\n", se.entity_name));
            for (sid, _) in &se.systems {
                yaml.push_str(&format!("      - {sid}\n"));
            }
            if let Some(c) = &se.canonical_system {
                yaml.push_str(&format!("    canonical: {c}\n"));
            }
        }
        yaml.push('\n');
    }

    // cross_flows: one per cross-system integration edge (a known target system).
    // ponytail: edge-level cross-flows — chain per-system flows across the edge when richer
    // paths are needed.
    let cross: Vec<&_> = links.integrations.iter().filter(|i| i.to_system.is_some()).collect();
    if !cross.is_empty() {
        yaml.push_str("cross_flows:\n");
        for (i, ig) in cross.iter().enumerate() {
            let to = ig.to_system.as_deref().unwrap();
            yaml.push_str(&format!("  - id: cross_flow_{i}\n"));
            yaml.push_str(&format!("    name: \"{} -> {} ({})\"\n", ig.from_system, to, ig.integration_type));
            yaml.push_str(&format!("    from: {}\n", ig.from_system));
            yaml.push_str(&format!("    to: {to}\n"));
            yaml.push_str(&format!("    via: {}\n", ig.integration_type));
        }
    }

    yaml
}

#[cfg(test)]
mod tests;

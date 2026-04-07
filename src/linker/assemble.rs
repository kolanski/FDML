//! Deterministic FDML spec assembly from scanner + linker data.
//! No LLM needed — builds valid YAML from parsed code artifacts.

use crate::scanner::types::ScanResult;
use super::types::LinkReport;
use super::normalize_name;

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
pub fn assemble_spec_no_llm(
    report: &LinkReport,
    scan: &ScanResult,
    system_name: Option<&str>,
) -> String {
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
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    yaml.push_str(&format!("  created: \"{}\"\n", today));
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
    let domain_entities: Vec<&super::types::EntityLink> = report.entities.iter()
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
    let business_actions: Vec<&super::types::ActionLink> = report.actions.iter()
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

    // ─── Flows (auto-discovered via BFS) ───
    let flows = super::flows::discover_flows(report, scan);
    if !flows.is_empty() {
        yaml.push_str(&super::flows::flows_to_yaml(&flows));
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

/// Build FDML spec using LLM classification results + scanner data.
/// Classifications come from Ollama JSON Schema output.
pub fn assemble_from_classifications(
    report: &LinkReport,
    scan: &ScanResult,
    entity_classifications: &[(String, String, String)],  // (id, role, description)
    action_classifications: &[(String, String, String)],   // (id, role, description)
) -> String {
    let sys_name = scan.metadata.codebase_path.split('/').last().unwrap_or("system");
    let sys_id = normalize_name(sys_name);
    let languages: Vec<String> = scan.metadata.languages_detected.iter()
        .map(|l| l.name().to_string())
        .collect();
    let tech = if languages.is_empty() { "Unknown".to_string() } else { languages.join(" + ") };

    let mut yaml = String::new();

    // Metadata
    yaml.push_str("metadata:\n");
    yaml.push_str("  version: \"1.3\"\n");
    yaml.push_str(&format!("  name: \"{}\"\n", sys_name));
    yaml.push_str(&format!("  description: \"{} — generated via hybrid LLM pipeline\"\n", sys_name));
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    yaml.push_str(&format!("  created: \"{}\"\n", today));
    yaml.push_str("\n");

    // System
    let components: Vec<String> = scan.modules.iter()
        .filter(|m| !m.children.is_empty())
        .map(|m| m.name.clone())
        .take(10)
        .collect();
    yaml.push_str("system:\n");
    yaml.push_str(&format!("  id: {}\n", sys_id));
    yaml.push_str(&format!("  name: \"{}\"\n", sys_name));
    yaml.push_str(&format!("  description: \"{} ({}) — {} files\"\n", sys_name, tech, scan.metadata.total_files));
    if !components.is_empty() {
        yaml.push_str("  components:\n");
        for c in &components { yaml.push_str(&format!("    - {}\n", c)); }
    }
    yaml.push_str("  relationships: []\n\n");

    // Entities — use LLM classifications
    let domain_ids: Vec<&str> = entity_classifications.iter()
        .filter(|(_, role, _)| role == "domain_entity")
        .map(|(id, _, _)| id.as_str())
        .collect();

    let domain_entities: Vec<&super::types::EntityLink> = report.entities.iter()
        .filter(|e| domain_ids.contains(&e.entity_id.as_str()))
        .collect();

    if !domain_entities.is_empty() {
        yaml.push_str("entities:\n");
        for entity in &domain_entities {
            let llm_desc = entity_classifications.iter()
                .find(|(id, _, _)| id == &entity.entity_id)
                .map(|(_, _, desc)| desc.as_str())
                .unwrap_or("");
            let description = if llm_desc.is_empty() {
                format!("{} entity (from {})", entity.entity_name, entity.code_ref)
            } else {
                llm_desc.to_string()
            };

            yaml.push_str(&format!("  - id: {}\n", entity.entity_id));
            yaml.push_str(&format!("    name: \"{}\"\n", entity.entity_name));
            yaml.push_str(&format!("    description: \"{}\"\n", description.replace('"', "'")));
            if !entity.fields.is_empty() {
                yaml.push_str("    fields:\n");
                for field in &entity.fields {
                    let ftype = map_field_type(field.field_type.as_deref().unwrap_or("string"));
                    yaml.push_str(&format!("      - name: {}\n", field.name));
                    yaml.push_str(&format!("        type: {}\n", ftype));
                    if field.name == "id" || field.name == "key" {
                        yaml.push_str("        required: true\n");
                    }
                }
            }
            yaml.push_str("\n");
        }
    }

    // Actions — use LLM classifications
    let business_ids: Vec<&str> = action_classifications.iter()
        .filter(|(_, role, _)| role == "business_action")
        .map(|(id, _, _)| id.as_str())
        .collect();

    let business_actions: Vec<&super::types::ActionLink> = report.actions.iter()
        .filter(|a| business_ids.contains(&a.action_id.as_str()))
        .collect();

    if !business_actions.is_empty() {
        yaml.push_str("actions:\n");
        for action in &business_actions {
            let llm_desc = action_classifications.iter()
                .find(|(id, _, _)| id == &action.action_id)
                .map(|(_, _, desc)| desc.as_str())
                .unwrap_or("");
            let description = if llm_desc.is_empty() {
                format!("{} (from {})", action.action_name, action.code_ref)
            } else {
                llm_desc.to_string()
            };

            yaml.push_str(&format!("  - id: {}\n", action.action_id));
            yaml.push_str(&format!("    name: \"{}\"\n", action.action_name));
            yaml.push_str(&format!("    description: \"{}\"\n", description.replace('"', "'")));

            let clean_params: Vec<&str> = action.input.iter()
                .map(|p| p.name.as_str())
                .filter(|n| !n.contains('{') && !n.is_empty() && *n != "self")
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

    // Features with placeholder scenarios
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
            yaml.push_str("          - \"Expected result is produced\"\n\n");
        }
    }

    // Flows (auto-discovered)
    let flows = super::flows::discover_flows(report, scan);
    if !flows.is_empty() {
        yaml.push_str(&super::flows::flows_to_yaml(&flows));
    }

    // Traceability
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

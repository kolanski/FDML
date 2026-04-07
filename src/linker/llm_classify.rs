//! LLM-based classification of code artifacts using Ollama structured output.
//! Sends small batched prompts with JSON Schema constraints.

use super::types::{LinkReport, EntityLink, ActionLink};
use super::cluster::CodeCluster;

/// Classification result from LLM
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ClassificationResult {
    #[serde(default)]
    pub entities: Vec<EntityClassification>,
    #[serde(default)]
    pub actions: Vec<ActionClassification>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EntityClassification {
    pub id: String,
    pub role: String,          // "domain_entity", "helper_class", "dto", "skip"
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ActionClassification {
    pub id: String,
    pub role: String,          // "business_action", "utility", "skip"
    #[serde(default)]
    pub description: String,
}

/// JSON Schema for structured output from Ollama
pub fn classification_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "role": { "enum": ["domain_entity", "helper_class", "dto", "skip"] },
                        "description": { "type": "string" }
                    },
                    "required": ["id", "role", "description"]
                }
            },
            "actions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "role": { "enum": ["business_action", "utility", "skip"] },
                        "description": { "type": "string" }
                    },
                    "required": ["id", "role", "description"]
                }
            }
        },
        "required": ["entities", "actions"]
    })
}

/// Build a classification prompt for a cluster of entities and actions.
/// Designed to fit in ~4K tokens.
pub fn build_classify_prompt(
    cluster: &CodeCluster,
    report: &LinkReport,
    system_name: &str,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "Classify code artifacts from '{}', module '{}'.\n\n",
        system_name, cluster.module_path
    ));

    prompt.push_str("Rules:\n");
    prompt.push_str("- domain_entity: core business data with meaningful fields\n");
    prompt.push_str("- helper_class: UI component, utility, config, wrapper\n");
    prompt.push_str("- dto: data transfer object, request/response shape\n");
    prompt.push_str("- business_action: meaningful operation that changes state or produces output\n");
    prompt.push_str("- utility: helper function, getter, setter, formatter\n");
    prompt.push_str("- skip: not relevant\n");
    prompt.push_str("- Write a 1-sentence description for domain_entity and business_action items.\n\n");

    // Entities
    if !cluster.entity_ids.is_empty() {
        prompt.push_str("Entities:\n");
        for eid in &cluster.entity_ids {
            if let Some(entity) = report.entities.iter().find(|e| &e.entity_id == eid) {
                let field_names: Vec<&str> = entity.fields.iter()
                    .take(8)
                    .map(|f| f.name.as_str())
                    .collect();
                let bases = if entity.bases.is_empty() {
                    String::new()
                } else {
                    format!(" extends {}", entity.bases.join(", "))
                };
                prompt.push_str(&format!(
                    "- {} | {} | fields: [{}]{}\n",
                    entity.entity_id,
                    entity.code_ref,
                    field_names.join(", "),
                    bases,
                ));
            }
        }
        prompt.push('\n');
    }

    // Actions
    if !cluster.action_ids.is_empty() {
        prompt.push_str("Actions:\n");
        for aid in &cluster.action_ids {
            if let Some(action) = report.actions.iter().find(|a| &a.action_id == aid) {
                let params: Vec<String> = action.input.iter()
                    .take(5)
                    .map(|p| {
                        if let Some(ref t) = p.param_type {
                            format!("{}: {}", p.name, t)
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect();
                let ret = action.output.as_deref().unwrap_or("void");
                prompt.push_str(&format!(
                    "- {} | {} | params: ({}) → {}\n",
                    action.action_id,
                    action.code_ref,
                    params.join(", "),
                    ret,
                ));
            }
        }
    }

    prompt
}

/// Build Ollama request body with JSON Schema for structured output
pub fn build_ollama_request(
    prompt: &str,
    model: &str,
    num_ctx: usize,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "format": classification_schema(),
        "options": {
            "num_ctx": num_ctx,
            "temperature": 0.1
        }
    })
}

/// Parse classification response from Ollama JSON output
pub fn parse_classification(response: &str) -> Result<ClassificationResult, String> {
    // Try direct JSON parse
    if let Ok(result) = serde_json::from_str::<ClassificationResult>(response) {
        return Ok(result);
    }

    // Try to extract JSON from markdown fences
    let cleaned = response.trim();
    let json_str = if cleaned.starts_with("```json") {
        let start = cleaned.find('\n').unwrap_or(0) + 1;
        let end = cleaned.rfind("```").unwrap_or(cleaned.len());
        &cleaned[start..end]
    } else if cleaned.starts_with("```") {
        let start = cleaned.find('\n').unwrap_or(0) + 1;
        let end = cleaned.rfind("```").unwrap_or(cleaned.len());
        &cleaned[start..end]
    } else {
        cleaned
    };

    serde_json::from_str::<ClassificationResult>(json_str.trim())
        .map_err(|e| format!("Failed to parse classification JSON: {}. Response: {}...", e, &response[..response.len().min(200)]))
}

/// JSON Schema for BDD scenario generation
pub fn scenario_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "features": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "feature_id": { "type": "string" },
                        "title": { "type": "string" },
                        "scenarios": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "title": { "type": "string" },
                                    "given": { "type": "array", "items": { "type": "string" } },
                                    "when": { "type": "array", "items": { "type": "string" } },
                                    "then": { "type": "array", "items": { "type": "string" } }
                                },
                                "required": ["id", "title", "given", "when", "then"]
                            }
                        }
                    },
                    "required": ["feature_id", "title", "scenarios"]
                }
            }
        },
        "required": ["features"]
    })
}

/// Build a prompt for BDD scenario generation
pub fn build_scenario_prompt(
    domain_entities: &[(String, String)],  // (id, description)
    business_actions: &[(String, String)],  // (id, description)
    system_name: &str,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "Generate BDD scenarios for '{}'. 2 scenarios per action.\n\n",
        system_name
    ));

    prompt.push_str("Domain entities:\n");
    for (id, desc) in domain_entities.iter().take(15) {
        prompt.push_str(&format!("- {}: {}\n", id, desc));
    }

    prompt.push_str("\nBusiness actions:\n");
    for (id, desc) in business_actions.iter().take(15) {
        prompt.push_str(&format!("- {}: {}\n", id, desc));
    }

    prompt.push_str("\nGenerate features grouping related actions, with Given/When/Then scenarios.\n");

    prompt
}

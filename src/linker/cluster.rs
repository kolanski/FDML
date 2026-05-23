//! Clustering of code artifacts by module/directory structure.
//! Groups related entities and actions for batched LLM classification.

use super::types::{LinkReport, EntityLink, ActionLink};

/// A cluster of related code artifacts from the same module area
#[derive(Debug, Clone)]
pub struct CodeCluster {
    pub id: String,
    pub name: String,
    pub module_path: String,
    pub entity_ids: Vec<String>,
    pub action_ids: Vec<String>,
}

impl CodeCluster {
    pub fn total_items(&self) -> usize {
        self.entity_ids.len() + self.action_ids.len()
    }
}

/// Group entities and actions into clusters by their module/directory.
/// Each cluster has max_items items. Large modules get split.
pub fn cluster_by_module(report: &LinkReport, max_items: usize) -> Vec<CodeCluster> {
    use std::collections::HashMap;

    // Group entities by module (extracted from code_ref: "path/file.ts:ClassName")
    let mut module_entities: HashMap<String, Vec<String>> = HashMap::new();
    let mut module_actions: HashMap<String, Vec<String>> = HashMap::new();

    for entity in &report.entities {
        let module = extract_module(&entity.code_ref);
        module_entities.entry(module).or_default().push(entity.entity_id.clone());
    }

    for action in &report.actions {
        let module = extract_module(&action.code_ref);
        module_actions.entry(module).or_default().push(action.action_id.clone());
    }

    // Collect all module names
    let mut all_modules: Vec<String> = module_entities.keys()
        .chain(module_actions.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    all_modules.sort();

    let mut clusters = Vec::new();

    for module in &all_modules {
        let entities = module_entities.get(module).cloned().unwrap_or_default();
        let actions = module_actions.get(module).cloned().unwrap_or_default();
        let total = entities.len() + actions.len();

        if total == 0 { continue; }

        if total <= max_items {
            // Fits in one cluster
            clusters.push(CodeCluster {
                id: format!("cluster_{}", clusters.len()),
                name: module_to_name(module),
                module_path: module.clone(),
                entity_ids: entities,
                action_ids: actions,
            });
        } else {
            // Split into sub-clusters
            let mut remaining_entities = entities;
            let mut remaining_actions = actions;

            let mut sub_idx = 0;
            while !remaining_entities.is_empty() || !remaining_actions.is_empty() {
                let take_entities = remaining_entities.len().min(max_items / 2);
                let take_actions = remaining_actions.len().min(max_items - take_entities);

                let batch_entities: Vec<String> = remaining_entities.drain(..take_entities.min(remaining_entities.len())).collect();
                let batch_actions: Vec<String> = remaining_actions.drain(..take_actions.min(remaining_actions.len())).collect();

                if batch_entities.is_empty() && batch_actions.is_empty() { break; }

                clusters.push(CodeCluster {
                    id: format!("cluster_{}", clusters.len()),
                    name: format!("{} (part {})", module_to_name(module), sub_idx + 1),
                    module_path: module.clone(),
                    entity_ids: batch_entities,
                    action_ids: batch_actions,
                });
                sub_idx += 1;
            }
        }
    }

    // Merge tiny clusters (< 3 items) into nearest
    merge_tiny_clusters(&mut clusters, 3);

    clusters
}

/// Extract module path from code_ref like "api/types.ts:FdmlDocument" → "api"
fn extract_module(code_ref: &str) -> String {
    let file_part = code_ref.split(':').next().unwrap_or(code_ref);
    // Get directory part
    if let Some(slash_pos) = file_part.rfind('/') {
        file_part[..slash_pos].to_string()
    } else {
        "root".to_string()
    }
}

/// Convert module path to human-readable name
fn module_to_name(module: &str) -> String {
    let parts: Vec<&str> = module.split('/').collect();
    if parts.is_empty() || (parts.len() == 1 && parts[0] == "root") {
        "Root Module".to_string()
    } else {
        parts.last().unwrap_or(&"module")
            .replace('_', " ")
            .replace('-', " ")
            .split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Merge clusters with fewer than min_items into the nearest cluster
fn merge_tiny_clusters(clusters: &mut Vec<CodeCluster>, min_items: usize) {
    if clusters.len() <= 1 { return; }

    let mut i = 0;
    while i < clusters.len() {
        if clusters[i].total_items() < min_items && clusters.len() > 1 {
            let tiny = clusters.remove(i);
            // Merge into the previous or next cluster
            let target = if i > 0 { i - 1 } else { 0 };
            if target < clusters.len() {
                clusters[target].entity_ids.extend(tiny.entity_ids);
                clusters[target].action_ids.extend(tiny.action_ids);
            }
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_module() {
        assert_eq!(extract_module("api/types.ts:FdmlDocument"), "api");
        assert_eq!(extract_module("components/SpecView.tsx:SpecView"), "components");
        assert_eq!(extract_module("App.tsx:App"), "root");
        assert_eq!(extract_module("layout/inferFlows.ts:inferFlows"), "layout");
    }

    #[test]
    fn test_module_to_name() {
        assert_eq!(module_to_name("api"), "Api");
        assert_eq!(module_to_name("components"), "Components");
        assert_eq!(module_to_name("layout/build_hierarchy"), "Build Hierarchy");
        assert_eq!(module_to_name("root"), "Root Module");
    }
}

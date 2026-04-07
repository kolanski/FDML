//! Automatic flow reconstruction from code analysis.
//! Uses BFS/DFS on the unified call + import graph to discover
//! data paths from ingress points (API handlers) to sink points (DB writes, responses).

use std::collections::{HashMap, HashSet, VecDeque};
use crate::scanner::types::ScanResult;
use super::types::LinkReport;
use super::normalize_name;

/// A discovered flow skeleton — sequence of actions forming a data path
#[derive(Debug, Clone)]
pub struct FlowSkeleton {
    pub id: String,
    pub name: String,
    pub ingress: String,        // action_id of entry point
    pub steps: Vec<FlowStep>,
    pub modules_involved: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FlowStep {
    pub action_id: String,
    pub action_name: String,
    pub module: String,
    pub code_ref: String,
}

/// Build a call graph from scan data and discover flows
pub fn discover_flows(report: &LinkReport, scan: &ScanResult) -> Vec<FlowSkeleton> {
    // Build dependency graph: action → [actions it calls/depends on]
    let call_graph = build_call_graph(report, scan);

    // Find ingress points (likely API handlers, event listeners, entry points)
    let ingress_points = find_ingress_points(report);

    // Find sink points (DB operations, external calls, response builders)
    let sink_points = find_sink_points(report);

    // BFS from each ingress through the call graph
    let mut flows = Vec::new();
    let mut flow_idx = 0;

    for ingress_id in &ingress_points {
        let paths = bfs_paths(&call_graph, ingress_id, &sink_points, 8);

        for path in paths {
            if path.len() < 2 { continue; } // Skip trivial single-step flows

            let steps: Vec<FlowStep> = path.iter().filter_map(|action_id| {
                report.actions.iter().find(|a| &a.action_id == action_id).map(|a| {
                    FlowStep {
                        action_id: a.action_id.clone(),
                        action_name: a.action_name.clone(),
                        module: extract_module(&a.code_ref),
                        code_ref: a.code_ref.clone(),
                    }
                })
            }).collect();

            if steps.len() < 2 { continue; }

            let modules: Vec<String> = steps.iter()
                .map(|s| s.module.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let ingress_name = steps.first().map(|s| &s.action_name).cloned().unwrap_or_default();
            let flow_name = format!("{} Flow", title_case(&ingress_name));

            flows.push(FlowSkeleton {
                id: format!("flow_{}", flow_idx),
                name: flow_name,
                ingress: ingress_id.clone(),
                steps,
                modules_involved: modules,
            });
            flow_idx += 1;
        }
    }

    // Deduplicate flows that share >80% of steps
    deduplicate_flows(&mut flows);

    flows
}

/// Build call graph from import relationships and parameter type references
fn build_call_graph(report: &LinkReport, scan: &ScanResult) -> HashMap<String, Vec<String>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    // All action IDs for quick lookup
    let action_ids: HashSet<String> = report.actions.iter()
        .map(|a| a.action_id.clone())
        .collect();

    // Entity ID → actions that reference it
    let mut entity_to_actions: HashMap<String, Vec<String>> = HashMap::new();
    for action in &report.actions {
        for param in &action.input {
            let ptype = param.param_type.as_deref().unwrap_or("");
            let normalized = normalize_name(ptype);
            entity_to_actions.entry(normalized).or_default().push(action.action_id.clone());
        }
        if let Some(ref ret) = action.output {
            let normalized = normalize_name(ret);
            entity_to_actions.entry(normalized).or_default().push(action.action_id.clone());
        }
    }

    // Build edges from traceability/import relationships
    build_import_edges(&mut graph, report);

    // Build edges from shared entity references
    // If action A outputs entity X and action B inputs entity X → A → B
    for action_a in &report.actions {
        if let Some(ref ret_type) = action_a.output {
            let ret_normalized = normalize_name(ret_type);
            if let Some(consumers) = entity_to_actions.get(&ret_normalized) {
                for consumer_id in consumers {
                    if consumer_id != &action_a.action_id {
                        graph.entry(action_a.action_id.clone())
                            .or_default()
                            .push(consumer_id.clone());
                    }
                }
            }
        }
    }

    // Build edges from module co-location (actions in same file likely call each other)
    let mut file_actions: HashMap<String, Vec<String>> = HashMap::new();
    for action in &report.actions {
        let file = action.code_ref.split(':').next().unwrap_or("").to_string();
        file_actions.entry(file).or_default().push(action.action_id.clone());
    }
    for (_file, actions) in &file_actions {
        if actions.len() >= 2 && actions.len() <= 6 {
            // Create sequential edges within same file (heuristic: order of declaration)
            for window in actions.windows(2) {
                graph.entry(window[0].clone()).or_default().push(window[1].clone());
            }
        }
    }

    graph
}

fn build_import_edges(
    graph: &mut HashMap<String, Vec<String>>,
    report: &LinkReport,
) {
    // Use traceability links as edges
    for trace in &report.traceability {
        // Extract IDs from "action:xxx" or "entity:xxx" format
        let from_id = trace.from.split(':').last().unwrap_or(&trace.from);
        let to_id = trace.to.split(':').last().unwrap_or(&trace.to);

        let from_norm = normalize_name(from_id);
        let to_norm = normalize_name(to_id);

        // Feature → Action links suggest action ordering
        if trace.relation == "implements" || trace.relation == "calls" || trace.relation == "depends_on" {
            graph.entry(from_norm).or_default().push(to_norm);
        }
    }
}

/// Identify likely ingress points (API handlers, event listeners, exported functions)
fn find_ingress_points(report: &LinkReport) -> Vec<String> {
    let mut ingress = Vec::new();

    for action in &report.actions {
        let name_lower = action.action_name.to_lowercase();
        let code_lower = action.code_ref.to_lowercase();

        // Route handlers
        if name_lower.contains("handle")
            || name_lower.contains("endpoint")
            || name_lower.contains("route")
            || name_lower.contains("controller")
            || name_lower.contains("view")
            || name_lower.starts_with("on_")
            || name_lower.starts_with("post_")
            || name_lower.starts_with("get_")
            || name_lower.starts_with("put_")
            || name_lower.starts_with("delete_")
            || name_lower.starts_with("patch_")
        {
            ingress.push(action.action_id.clone());
            continue;
        }

        // Event listeners / subscribers
        if name_lower.contains("subscribe")
            || name_lower.contains("listener")
            || name_lower.contains("consumer")
            || name_lower.contains("receive")
            || name_lower.contains("ingest")
            || name_lower.contains("webhook")
        {
            ingress.push(action.action_id.clone());
            continue;
        }

        // Functions in router/api files
        if code_lower.contains("router")
            || code_lower.contains("api/")
            || code_lower.contains("routes/")
            || code_lower.contains("endpoints/")
            || code_lower.contains("views/")
            || code_lower.contains("controllers/")
        {
            ingress.push(action.action_id.clone());
            continue;
        }

        // Main entry points
        if name_lower == "main" || name_lower == "app" || name_lower == "run" {
            ingress.push(action.action_id.clone());
        }
    }

    ingress
}

/// Identify likely sink points (DB writes, cache updates, response builders)
fn find_sink_points(report: &LinkReport) -> HashSet<String> {
    let mut sinks = HashSet::new();

    for action in &report.actions {
        let name_lower = action.action_name.to_lowercase();

        if name_lower.contains("save")
            || name_lower.contains("write")
            || name_lower.contains("insert")
            || name_lower.contains("update")
            || name_lower.contains("delete")
            || name_lower.contains("create")
            || name_lower.contains("publish")
            || name_lower.contains("send")
            || name_lower.contains("emit")
            || name_lower.contains("notify")
            || name_lower.contains("cache")
            || name_lower.contains("store")
            || name_lower.contains("persist")
            || name_lower.contains("render")
            || name_lower.contains("respond")
        {
            sinks.insert(action.action_id.clone());
        }
    }

    // If no sinks found, use leaf nodes (actions with no outgoing edges)
    if sinks.is_empty() {
        for action in &report.actions {
            sinks.insert(action.action_id.clone());
        }
    }

    sinks
}

/// BFS to find all paths from source to any sink, up to max_depth
fn bfs_paths(
    graph: &HashMap<String, Vec<String>>,
    start: &str,
    sinks: &HashSet<String>,
    max_depth: usize,
) -> Vec<Vec<String>> {
    let mut results = Vec::new();
    let mut queue: VecDeque<(Vec<String>, HashSet<String>)> = VecDeque::new();

    let mut initial_visited = HashSet::new();
    initial_visited.insert(start.to_string());
    queue.push_back((vec![start.to_string()], initial_visited));

    while let Some((path, visited)) = queue.pop_front() {
        if path.len() > max_depth { continue; }

        let current = path.last().unwrap();

        // If we reached a sink (and it's not the start), record the path
        if path.len() > 1 && sinks.contains(current) {
            results.push(path.clone());
            // Don't continue from sinks — they're terminal
            continue;
        }

        // Explore neighbors
        if let Some(neighbors) = graph.get(current) {
            for next in neighbors {
                if !visited.contains(next) {
                    let mut new_path = path.clone();
                    new_path.push(next.clone());
                    let mut new_visited = visited.clone();
                    new_visited.insert(next.clone());
                    queue.push_back((new_path, new_visited));
                }
            }
        }
    }

    // Keep only the longest non-overlapping paths
    results.sort_by(|a, b| b.len().cmp(&a.len()));
    results.truncate(10); // Max 10 flows per ingress
    results
}

/// Remove flows that share >80% of their steps
fn deduplicate_flows(flows: &mut Vec<FlowSkeleton>) {
    let mut i = 0;
    while i < flows.len() {
        let mut j = i + 1;
        while j < flows.len() {
            let overlap = flows[i].steps.iter()
                .filter(|s| flows[j].steps.iter().any(|t| t.action_id == s.action_id))
                .count();
            let max_len = flows[i].steps.len().max(flows[j].steps.len());
            if max_len > 0 && overlap as f64 / max_len as f64 > 0.8 {
                // Keep the longer flow
                if flows[i].steps.len() >= flows[j].steps.len() {
                    flows.remove(j);
                } else {
                    flows.remove(i);
                    j = i + 1;
                    continue;
                }
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

/// Convert to FDML YAML fragment
pub fn flows_to_yaml(flows: &[FlowSkeleton]) -> String {
    if flows.is_empty() { return String::new(); }

    let mut yaml = String::from("flows:\n");
    for flow in flows {
        yaml.push_str(&format!("  - id: {}\n", flow.id));
        yaml.push_str(&format!("    name: \"{}\"\n", flow.name));
        yaml.push_str(&format!("    description: \"Auto-discovered flow from {} through {} steps\"\n",
            flow.ingress, flow.steps.len()));
        yaml.push_str("    steps:\n");
        for (i, step) in flow.steps.iter().enumerate() {
            yaml.push_str(&format!("      - id: step_{}\n", i));
            yaml.push_str(&format!("        action: {}\n", step.action_id));
            yaml.push_str(&format!("        description: \"{} ({})\"\n", step.action_name, step.code_ref));
        }
        yaml.push_str("\n");
    }
    yaml
}

fn extract_module(code_ref: &str) -> String {
    let file_part = code_ref.split(':').next().unwrap_or(code_ref);
    if let Some(slash_pos) = file_part.rfind('/') {
        file_part[..slash_pos].to_string()
    } else {
        "root".to_string()
    }
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

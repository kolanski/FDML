//! Pass 3 — the deterministic linker. Ported FAITHFULLY from FDML's
//! `src/linker/mod.rs::link_code`, restricted to the GENERATION path (the behavior
//! when `spec = None`, i.e. the old `--no-llm` flow): ScanResult → entities/actions/
//! features/traceability/coverage.
//!
//! Mechanical changes vs the original:
//!   * Removed the `spec: Option<&FdmlDocument>` parameter and every `if let Some(spec)`
//!     / spec-matching branch — no FDML parser/AST dependency is pulled in.
//!   * `crate::scanner::types::*` → `fdml_types::scan::*`; `super::types::*` →
//!     `fdml_types::graph::*`; `normalize_name` → `fdml_util::normalize_name`.
//!   * Final `entities`/`actions`/`features` vectors are sorted by id so output is
//!     reproducible regardless of HashMap iteration order in feature clustering.
//!
//! Extraction logic (entity/action/feature derivation, confidence, naming, clustering)
//! is otherwise unchanged.

use std::collections::HashMap;
use std::path::Path;

use fdml_types::graph::*;
use fdml_types::scan::{CodeElement, ElementType, ScanResult, Scope};
use fdml_util::normalize_name;

/// Phase 3B.1 — deterministic multi-language import resolver (raw imports → edges).
pub mod resolve;
pub use resolve::{build_petgraph, resolve_calls, resolve_imports};

/// Phase 3B.2 — cluster the dependency graph into a readable map of named boxes.
pub mod cluster;
pub use cluster::cluster;

/// Phase 3B.3 — tag clusters/files with architectural role tags (deterministic).
pub mod roles;
pub use roles::tag_roles;

/// Run the deterministic linker over a scan. Equivalent to the original
/// `link_code(scan, None, inventory_file, None)` (generation path).
pub fn run(scan: &ScanResult) -> LinkReport {
    let mut entities = Vec::new();
    let mut actions = Vec::new();
    let mut features = Vec::new();
    let mut traceability = Vec::new();
    let mut unlinked_code = Vec::new();

    // Step 1: Build system from module tree
    let system = build_system(scan);

    // Step 2: Suggest entities ← classes
    link_entities(scan, &mut entities, &mut unlinked_code);

    // Step 3: Suggest actions ← functions/methods
    link_actions(scan, &mut actions, &mut unlinked_code);

    // Determinism: id-sort before downstream passes consume these (feature grouping
    // and traceability follow this order). Generation order is already deterministic,
    // but feature clustering reads HashMaps — sorting keeps the whole report stable.
    entities.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    actions.sort_by(|a, b| a.action_id.cmp(&b.action_id));

    // Step 4: Suggest features from module groupings
    suggest_features(scan, &entities, &actions, &mut features);
    features.sort_by(|a, b| a.feature_id.cmp(&b.feature_id));

    // Step 5: Generate traceability links
    build_traceability(&entities, &actions, &features, &mut traceability);

    // Step 6: Coverage
    let coverage = compute_coverage(&entities, &actions);

    LinkReport {
        metadata: LinkMetadata {
            linker_version: env!("CARGO_PKG_VERSION").to_string(),
            // No external inventory path in the `run(scan)` API; the codebase path
            // identifies the source deterministically.
            inventory_file: scan.metadata.codebase_path.clone(),
            spec_file: None,
            spec_loaded: false,
        },
        system,
        entities,
        actions,
        features,
        traceability,
        coverage,
        unlinked_code,
    }
}

// ─── Test-file / test-name detection ──────────────────────────────

/// Check if a file path looks like a test file
fn is_test_path(file_path: &str) -> bool {
    let path = Path::new(file_path);
    // Check directory components
    for component in path.components() {
        let s = component.as_os_str().to_str().unwrap_or("");
        if s == "tests" || s == "test" || s == "__tests__" || s == "spec" || s == "specs" {
            return true;
        }
    }
    // Check filename
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if stem.starts_with("test_") || stem.ends_with("_test") || stem.ends_with("_spec") {
            return true;
        }
    }
    false
}

/// Check if a class/function name looks like a test
fn is_test_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("test") || lower.ends_with("test") || lower.ends_with("tests")
        || lower.starts_with("test_") || lower.ends_with("_test")
}

// ─── Step 1: System from modules ──────────────────────────────────

fn build_system(scan: &ScanResult) -> SuggestedSystem {
    let mut components = Vec::new();
    let mut relationships = Vec::new();

    // Each top-level module = component
    for module in &scan.modules {
        components.push(module.module_path.clone());
    }

    // Internal imports = dependency relationships
    for file in &scan.files {
        let from_module = if !file.module_path.is_empty() {
            file.module_path.clone()
        } else {
            Path::new(&file.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        };

        for imp in &file.imports {
            if imp.is_relative {
                let to_module = imp.module.trim_start_matches('.').to_string();
                if !to_module.is_empty() && to_module != from_module {
                    relationships.push(SystemRelationship {
                        from: from_module.clone(),
                        to: to_module,
                        rel_type: "dependency".to_string(),
                        description: if imp.names.is_empty() {
                            None
                        } else {
                            Some(format!("imports: {}", imp.names.join(", ")))
                        },
                    });
                }
            }
        }
    }

    // Deduplicate relationships
    relationships.dedup_by(|a, b| a.from == b.from && a.to == b.to);

    // Derive a system name from the codebase path
    let system_name = Path::new(&scan.metadata.codebase_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("system")
        .to_string();

    SuggestedSystem {
        id: normalize_name(&system_name),
        name: system_name,
        components,
        relationships,
    }
}

// ─── Step 2: Entities ← Classes ───────────────────────────────────

fn link_entities(
    scan: &ScanResult,
    entities: &mut Vec<EntityLink>,
    unlinked_code: &mut Vec<UnlinkedCode>,
) {
    // Collect all classes from scan (skip test files)
    let mut code_classes: Vec<(&CodeElement, &str, &str)> = Vec::new(); // (element, file_path, module_path)
    for file in &scan.files {
        if is_test_path(&file.file_path) {
            continue;
        }
        collect_classes(&file.elements, &file.file_path, &file.module_path, &mut code_classes);
    }
    // Also filter out test classes by name
    code_classes.retain(|(el, _, _)| !is_test_name(&el.name));

    for (element, file_path, module_path) in &code_classes {
        let code_ref = format!("{}:{}", file_path, element.name);

        // Build field links
        let fields = build_field_links(element);

        // Generation path: no spec to match against → always a fresh suggestion.
        let entity_id = normalize_name(&element.name);
        entities.push(EntityLink {
            entity_id: entity_id.clone(),
            entity_name: element.name.clone(),
            code_ref: code_ref.clone(),
            confidence: 0.0,
            source: LinkSource::Suggested,
            fields,
            bases: element.bases.clone(),
        });

        unlinked_code.push(UnlinkedCode {
            element_type: "class".to_string(),
            name: element.name.clone(),
            code_ref,
            module_path: module_path.to_string(),
            suggestion: Some(format!("Candidate for entity '{}'", entity_id)),
        });
    }
}

fn collect_classes<'a>(
    elements: &'a [CodeElement],
    file_path: &'a str,
    module_path: &'a str,
    out: &mut Vec<(&'a CodeElement, &'a str, &'a str)>,
) {
    for el in elements {
        if matches!(el.element_type, ElementType::Class | ElementType::Interface) {
            out.push((el, file_path, module_path));
        }
        // Check nested classes
        collect_classes(&el.children, file_path, module_path, out);
    }
}

fn build_field_links(class: &CodeElement) -> Vec<FieldLink> {
    let mut fields = Vec::new();

    // Code fields
    for child in &class.children {
        if matches!(child.element_type, ElementType::Field | ElementType::Property) {
            let field_type = child.return_type.clone()
                .or_else(|| infer_fdml_type(child.default_value.as_deref()));

            fields.push(FieldLink {
                name: child.name.clone(),
                field_type,
                default_value: child.default_value.clone(),
                in_spec: false,
                in_code: true,
            });
        }
    }

    fields
}

/// Map Python/Java/C# types to FDML types
fn infer_fdml_type(value: Option<&str>) -> Option<String> {
    match value {
        Some(v) => {
            let v = v.trim();
            if v.starts_with('"') || v.starts_with('\'') {
                Some("string".to_string())
            } else if v.parse::<i64>().is_ok() {
                Some("integer".to_string())
            } else if v.parse::<f64>().is_ok() {
                Some("float".to_string())
            } else if v == "True" || v == "False" || v == "true" || v == "false" {
                Some("boolean".to_string())
            } else if v == "None" || v == "null" || v == "nil" {
                None
            } else if v.starts_with('[') {
                Some("array".to_string())
            } else if v.starts_with('{') {
                Some("object".to_string())
            } else {
                None
            }
        }
        None => None,
    }
}

// ─── Step 3: Actions ← Functions/Methods ──────────────────────────

fn link_actions(
    scan: &ScanResult,
    actions: &mut Vec<ActionLink>,
    unlinked_code: &mut Vec<UnlinkedCode>,
) {
    for file in &scan.files {
        // Skip test files
        if is_test_path(&file.file_path) {
            continue;
        }

        let module = if !file.module_path.is_empty() {
            &file.module_path
        } else {
            &file.file_path
        };

        // Top-level functions
        for el in &file.elements {
            if matches!(el.element_type, ElementType::Function) {
                if is_test_name(&el.name) { continue; }
                process_action_candidate(el, &file.file_path, module, None, actions, unlinked_code);
            }

            // Public methods inside classes
            if matches!(el.element_type, ElementType::Class) {
                if is_test_name(&el.name) { continue; }
                for child in &el.children {
                    if matches!(child.element_type, ElementType::Method) {
                        if is_test_name(&child.name) { continue; }
                        let scope = child.scope.as_ref();
                        let is_public = matches!(scope, Some(Scope::Public) | None);
                        let is_dunder = child.name.starts_with("__") && child.name.ends_with("__");

                        // Skip private, protected, and dunder methods (except meaningful ones)
                        if !is_public || is_dunder {
                            continue;
                        }

                        process_action_candidate(
                            child, &file.file_path, module,
                            Some(&el.name), actions, unlinked_code,
                        );
                    }
                }
            }
        }
    }
}

fn process_action_candidate(
    el: &CodeElement,
    file_path: &str,
    module_path: &str,
    class_name: Option<&str>,
    actions: &mut Vec<ActionLink>,
    unlinked_code: &mut Vec<UnlinkedCode>,
) {
    let code_ref = match class_name {
        Some(cls) => format!("{}:{}.{}", file_path, cls, el.name),
        None => format!("{}:{}", file_path, el.name),
    };

    let action_id = match class_name {
        Some(cls) => format!("{}_{}", normalize_name(cls), normalize_name(&el.name)),
        None => normalize_name(&el.name),
    };

    let input: Vec<ActionParam> = el.parameters.iter().map(|p| ActionParam {
        name: p.name.clone(),
        param_type: p.type_hint.clone(),
        required: if p.default_value.is_some() { Some(false) } else { Some(true) },
    }).collect();

    // Generation path: no spec to match against → always a fresh suggestion.
    actions.push(ActionLink {
        action_id: action_id.clone(),
        action_name: el.name.clone(),
        code_ref: code_ref.clone(),
        confidence: 0.0,
        source: LinkSource::Suggested,
        input,
        output: el.return_type.clone(),
        description: el.docstring.clone(),
    });

    unlinked_code.push(UnlinkedCode {
        element_type: if el.element_type == ElementType::Function { "function" } else { "method" }.to_string(),
        name: el.name.clone(),
        code_ref,
        module_path: module_path.to_string(),
        suggestion: Some(format!("Candidate for action '{}'", action_id)),
    });
}

// ─── Step 4: Features from modules ────────────────────────────────

fn suggest_features(
    scan: &ScanResult,
    entities: &[EntityLink],
    actions: &[ActionLink],
    features: &mut Vec<FeatureSuggestion>,
) {
    // Group entities and actions by module
    let mut module_entities: HashMap<String, Vec<String>> = HashMap::new();
    let mut module_actions: HashMap<String, Vec<String>> = HashMap::new();
    // Also track action names per module for semantic grouping
    let mut module_action_names: HashMap<String, Vec<(String, String)>> = HashMap::new(); // module -> [(action_id, action_name)]

    for file in &scan.files {
        // Skip test files from feature suggestions
        if is_test_path(&file.file_path) {
            continue;
        }

        let module = if !file.module_path.is_empty() {
            file.module_path.clone()
        } else {
            continue; // skip root __init__.py etc
        };

        // Collect entity IDs for this module
        for entity in entities {
            if entity.code_ref.starts_with(&file.file_path) {
                module_entities.entry(module.clone()).or_default().push(entity.entity_id.clone());
            }
        }

        // Collect action IDs for this module
        for action in actions {
            if action.code_ref.starts_with(&file.file_path) {
                module_actions.entry(module.clone()).or_default().push(action.action_id.clone());
                module_action_names.entry(module.clone()).or_default()
                    .push((action.action_id.clone(), action.action_name.clone()));
            }
        }
    }

    // Create feature suggestion per module that has entities or actions
    let mut all_modules: Vec<String> = module_entities.keys()
        .chain(module_actions.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    all_modules.sort();

    for module in &all_modules {
        let ents = module_entities.get(module).cloned().unwrap_or_default();
        let acts = module_actions.get(module).cloned().unwrap_or_default();
        let act_names = module_action_names.get(module).cloned().unwrap_or_default();

        if ents.is_empty() && acts.is_empty() {
            continue;
        }

        // For large modules (>8 actions), try semantic sub-grouping by name prefix
        if acts.len() > 8 {
            let sub_features = split_module_into_semantic_features(
                module, &ents, &act_names, entities,
            );
            if sub_features.len() > 1 {
                features.extend(sub_features);
                continue;
            }
        }

        // Generate human-readable title from module name
        let last_segment = module.split('.').last().unwrap_or(module);
        let title = titlecase_name(last_segment);

        features.push(FeatureSuggestion {
            feature_id: normalize_name(last_segment),
            title,
            module_path: module.clone(),
            confidence: 0.0, // always suggested, LLM decides
            source: LinkSource::Suggested,
            entities: ents,
            actions: acts,
        });
    }
}

/// Split a large module into semantic sub-features using IDF-weighted token
/// similarity and agglomerative clustering.
fn split_module_into_semantic_features(
    module: &str,
    all_entity_ids: &[String],
    action_names: &[(String, String)], // (action_id, action_name)
    entities: &[EntityLink],
) -> Vec<FeatureSuggestion> {
    use std::collections::HashSet;

    if action_names.len() < 4 {
        return Vec::new();
    }

    // ── Phase 1: Tokenize all action names ──
    let tokenized: Vec<(String, Vec<String>)> = action_names.iter()
        .map(|(id, name)| (id.clone(), tokenize_action_name(name)))
        .collect();

    // ── Phase 2: Compute IDF weights ──
    let _n = tokenized.len() as f64;
    let mut doc_freq: HashMap<String, usize> = HashMap::new();
    for (_, tokens) in &tokenized {
        let unique: HashSet<&String> = tokens.iter().collect();
        for tok in unique {
            *doc_freq.entry(tok.clone()).or_default() += 1;
        }
    }

    // No explicit weighting — plain Jaccard similarity handles everything.
    // Short tokens (<=2 chars) are still excluded as they're grammatical noise.
    let weights: HashMap<String, f64> = doc_freq.keys()
        .map(|tok| {
            let w = if tok.len() <= 2 { 0.0 } else { 1.0 };
            (tok.clone(), w)
        })
        .collect();

    // All functions go through clustering — no hardcoded pattern groups
    let remaining: Vec<(usize, String, Vec<String>)> = action_names.iter().enumerate()
        .map(|(i, (id, _))| (i, id.clone(), tokenized[i].1.clone()))
        .collect();

    if remaining.len() < 4 {
        return Vec::new(); // not enough to cluster
    }

    // ── Phase 4: Pairwise weighted Jaccard similarity ──
    let rm = remaining.len();
    let mut sim = vec![vec![0.0f64; rm]; rm];

    for i in 0..rm {
        for j in (i + 1)..rm {
            let toks_a: HashSet<&String> = remaining[i].2.iter()
                .filter(|t| weights.get(*t).copied().unwrap_or(0.0) > 0.0)
                .collect();
            let toks_b: HashSet<&String> = remaining[j].2.iter()
                .filter(|t| weights.get(*t).copied().unwrap_or(0.0) > 0.0)
                .collect();

            let inter: f64 = toks_a.intersection(&toks_b)
                .map(|t| weights.get(*t).copied().unwrap_or(0.0))
                .sum();
            let union: f64 = toks_a.union(&toks_b)
                .map(|t| weights.get(*t).copied().unwrap_or(0.0))
                .sum();

            let s = if union > 0.0 { inter / union } else { 0.0 };
            sim[i][j] = s;
            sim[j][i] = s;
        }
    }

    // ── Phase 5: Agglomerative clustering (average-link) ──
    let target = ((remaining.len() as f64).sqrt().ceil() as usize).clamp(3, 8);
    let mut clusters: Vec<Vec<usize>> = (0..rm).map(|i| vec![i]).collect();

    loop {
        if clusters.len() <= target {
            break;
        }

        // Find most similar pair (average-link)
        let mut best_sim = -1.0f64;
        let mut best_i = 0;
        let mut best_j = 0;

        for ci in 0..clusters.len() {
            for cj in (ci + 1)..clusters.len() {
                let mut total = 0.0;
                let mut count = 0;
                for &a in &clusters[ci] {
                    for &b in &clusters[cj] {
                        total += sim[a][b];
                        count += 1;
                    }
                }
                let avg = if count > 0 { total / count as f64 } else { 0.0 };
                if avg > best_sim {
                    best_sim = avg;
                    best_i = ci;
                    best_j = cj;
                }
            }
        }

        if best_sim < 0.05 {
            break; // remaining clusters are too dissimilar
        }

        // Merge cluster j into cluster i
        let merged_cluster = clusters[best_j].clone();
        clusters[best_i].extend(merged_cluster);
        clusters.remove(best_j);
    }

    // ── Phase 6: Name each cluster ──
    let last_segment = module.split('.').last().unwrap_or(module);
    let mut result: Vec<FeatureSuggestion> = Vec::new();

    // Sweep singletons into utility
    let mut utility_actions: Vec<String> = Vec::new();

    for cluster in &clusters {
        let action_ids: Vec<String> = cluster.iter().map(|&i| remaining[i].1.clone()).collect();

        if action_ids.len() == 1 {
            utility_actions.extend(action_ids);
            continue;
        }

        // Find best name: token with highest coverage * weight
        let mut token_scores: HashMap<String, (usize, f64)> = HashMap::new(); // token -> (member_count, weight)
        for &idx in cluster {
            let unique: HashSet<&String> = remaining[idx].2.iter().collect();
            for tok in unique {
                let w = weights.get(tok).copied().unwrap_or(0.0);
                if w > 0.0 {
                    let entry = token_scores.entry(tok.clone()).or_insert((0, w));
                    entry.0 += 1;
                }
            }
        }

        let cluster_size = cluster.len() as f64;
        let mut candidates: Vec<(String, f64)> = token_scores.iter()
            .map(|(tok, (count, weight))| {
                let coverage = *count as f64 / cluster_size;
                (tok.clone(), coverage * weight)
            })
            .collect();
        // Primary: highest score. Secondary: token name (alphabetical) — the original
        // left equal-score ties to HashMap iteration order, which made the chosen group
        // name (and thus feature_id) nondeterministic across runs. Tie-breaking by name
        // only affects the previously-arbitrary tie case; clear winners are unchanged.
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        let group_name = if let Some((name, score)) = candidates.first() {
            if *score < 0.1 { "utility".to_string() } else { name.clone() }
        } else {
            "utility".to_string()
        };

        if group_name == "utility" {
            utility_actions.extend(action_ids);
            continue;
        }

        // Assign entities to this cluster
        let group_entities = assign_entities_to_group(&action_ids, all_entity_ids, entities);

        let feature_id = format!("{}_{}", normalize_name(last_segment), normalize_name(&group_name));
        let title = titlecase_name(&group_name);

        result.push(FeatureSuggestion {
            feature_id,
            title,
            module_path: module.to_string(),
            confidence: 0.0,
            source: LinkSource::Suggested,
            entities: group_entities,
            actions: action_ids,
        });
    }

    // Add utility group
    if !utility_actions.is_empty() {
        let group_entities = assign_entities_to_group(&utility_actions, all_entity_ids, entities);
        result.push(FeatureSuggestion {
            feature_id: format!("{}_utility", normalize_name(last_segment)),
            title: format!("{} Utility", titlecase_name(last_segment)),
            module_path: module.to_string(),
            confidence: 0.0,
            source: LinkSource::Suggested,
            entities: group_entities,
            actions: utility_actions,
        });
    }

    result
}

/// Tokenize an action/function name into stemmed lowercase words
fn tokenize_action_name(name: &str) -> Vec<String> {
    let name = name.trim_start_matches('_');

    // Split on snake_case
    let parts: Vec<&str> = name.split('_').collect();

    // Further split camelCase within each part, then stem
    let mut tokens = Vec::new();
    for part in parts {
        for word in split_camel_case(part) {
            let lower = word.to_lowercase();
            if lower.len() > 1 {
                tokens.push(cheap_stem(&lower));
            }
        }
    }
    tokens
}

/// Split camelCase into words: "getGameEnd" → ["get", "Game", "End"]
fn split_camel_case(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in s.chars() {
        if ch.is_uppercase() && !current.is_empty() {
            words.push(current);
            current = String::new();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// No stemming — return word as-is. Plural handling is done at the clustering
/// level via Jaccard similarity (functions sharing other tokens cluster together
/// regardless of game/games difference).
fn cheap_stem(word: &str) -> String {
    word.to_string()
}

/// Assign entities to a group of actions by name matching
fn assign_entities_to_group(
    action_ids: &[String],
    all_entity_ids: &[String],
    entities: &[EntityLink],
) -> Vec<String> {
    let mut group_entities: Vec<String> = Vec::new();
    for eid in all_entity_ids {
        let eid_lower = eid.to_lowercase();
        let matches = action_ids.iter().any(|aid| {
            let aid_lower = aid.to_lowercase();
            aid_lower.contains(&eid_lower) || eid_lower.contains(&aid_lower.split('_').next().unwrap_or(""))
        });
        let entity_match = entities.iter().any(|e| {
            e.entity_id == *eid && action_ids.iter().any(|aid| {
                aid.contains(&e.entity_name.to_lowercase().replace(' ', "_"))
            })
        });
        if matches || entity_match {
            group_entities.push(eid.clone());
        }
    }
    group_entities
}

fn titlecase_name(s: &str) -> String {
    s.replace('_', " ")
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

// ─── Step 5: Traceability ─────────────────────────────────────────

fn build_traceability(
    entities: &[EntityLink],
    actions: &[ActionLink],
    features: &[FeatureSuggestion],
    traceability: &mut Vec<TraceLink>,
) {
    for entity in entities {
        traceability.push(TraceLink {
            from: entity.entity_id.clone(),
            to: entity.code_ref.clone(),
            relation: "represented_by".to_string(),
            confidence: if matches!(entity.source, LinkSource::Matched) {
                entity.confidence
            } else {
                0.3
            },
            description: Some(format!("Entity '{}' ↔ class '{}'", entity.entity_id, entity.entity_name)),
        });
    }

    for action in actions {
        traceability.push(TraceLink {
            from: action.action_id.clone(),
            to: action.code_ref.clone(),
            relation: "implements".to_string(),
            confidence: if matches!(action.source, LinkSource::Matched) {
                action.confidence
            } else {
                0.3
            },
            description: Some(format!("Action '{}' ↔ {}", action.action_id, action.action_name)),
        });
    }

    for feature in features {
        if !feature.entities.is_empty() || !feature.actions.is_empty() {
            traceability.push(TraceLink {
                from: feature.feature_id.clone(),
                to: format!("module:{}", feature.module_path),
                relation: "implements".to_string(),
                confidence: 0.3,
                description: Some(format!("Feature '{}' ↔ module '{}'", feature.title, feature.module_path)),
            });
        }
    }
}

// ─── Step 6: Coverage ─────────────────────────────────────────────

fn compute_coverage(
    entities: &[EntityLink],
    actions: &[ActionLink],
) -> CoverageReport {
    // Generation path: no spec loaded → spec total is 0.
    let spec_total = 0;

    let spec_linked = entities.iter().filter(|e| matches!(e.source, LinkSource::Matched)).count()
        + actions.iter().filter(|a| matches!(a.source, LinkSource::Matched)).count();

    let code_total = entities.len() + actions.len();
    let code_linked = entities.iter().filter(|e| matches!(e.source, LinkSource::Matched)).count()
        + actions.iter().filter(|a| matches!(a.source, LinkSource::Matched)).count();

    CoverageReport {
        spec_coverage: CoverageMetric {
            total: spec_total,
            linked: spec_linked,
            percentage: if spec_total > 0 { spec_linked as f64 / spec_total as f64 * 100.0 } else { 0.0 },
        },
        code_coverage: CoverageMetric {
            total: code_total,
            linked: code_linked,
            percentage: if code_total > 0 { code_linked as f64 / code_total as f64 * 100.0 } else { 0.0 },
        },
    }
}

#[cfg(test)]
mod tests;

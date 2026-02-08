pub mod types;

use std::collections::HashMap;
use std::path::Path;

use crate::parser::ast::FdmlDocument;
use crate::scanner::types::{ScanResult, ElementType, Scope, CodeElement};
use types::*;

/// Run the full linking algorithm
pub fn link_code(
    scan: &ScanResult,
    spec: Option<&FdmlDocument>,
    inventory_file: &str,
    spec_file: Option<&str>,
) -> LinkReport {
    let mut entities = Vec::new();
    let mut actions = Vec::new();
    let mut features = Vec::new();
    let mut traceability = Vec::new();
    let mut unlinked_code = Vec::new();
    let mut unlinked_spec = Vec::new();

    // Step 1: Build system from module tree
    let system = build_system(scan);

    // Step 2: Match entities ← classes
    link_entities(scan, spec, &mut entities, &mut unlinked_code, &mut unlinked_spec);

    // Step 3: Match actions ← functions/methods
    link_actions(scan, spec, &mut actions, &mut unlinked_code);

    // Step 4: Suggest features from module groupings
    suggest_features(scan, &entities, &actions, &mut features);

    // Step 5: Generate traceability links
    build_traceability(&entities, &actions, &features, &mut traceability);

    // Add unlinked spec elements (actions, features not matched)
    if let Some(doc) = spec {
        find_unlinked_spec_actions(doc, &actions, &mut unlinked_spec);
        find_unlinked_spec_features(doc, &features, &mut unlinked_spec);
    }

    // Step 6: Coverage
    let coverage = compute_coverage(spec, &entities, &actions, &features, &unlinked_code, &unlinked_spec);

    LinkReport {
        metadata: LinkMetadata {
            linker_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            inventory_file: inventory_file.to_string(),
            spec_file: spec_file.map(|s| s.to_string()),
            spec_loaded: spec.is_some(),
        },
        system,
        entities,
        actions,
        features,
        traceability,
        coverage,
        unlinked_code,
        unlinked_spec,
    }
}

// ─── Name normalization ───────────────────────────────────────────

/// Normalize a name for comparison: CamelCase → snake_case, strip underscores
fn normalize_name(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            let prev = name.chars().nth(i - 1);
            if let Some(p) = prev {
                if p.is_lowercase() || p.is_ascii_digit() {
                    result.push('_');
                }
            }
        }
        result.push(ch.to_ascii_lowercase());
    }
    result.replace('-', "_").trim_matches('_').to_string()
}

/// Compute similarity score between two normalized names (0.0 - 1.0)
fn name_similarity(a: &str, b: &str) -> f64 {
    let na = normalize_name(a);
    let nb = normalize_name(b);

    if na == nb {
        return 1.0;
    }

    // Check if one contains the other
    if na.contains(&nb) || nb.contains(&na) {
        let longer = na.len().max(nb.len()) as f64;
        let shorter = na.len().min(nb.len()) as f64;
        return shorter / longer;
    }

    // Token-based overlap
    let tokens_a: Vec<&str> = na.split('_').filter(|s| !s.is_empty()).collect();
    let tokens_b: Vec<&str> = nb.split('_').filter(|s| !s.is_empty()).collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let common = tokens_a.iter().filter(|t| tokens_b.contains(t)).count();
    let total = tokens_a.len().max(tokens_b.len());

    common as f64 / total as f64
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
    spec: Option<&FdmlDocument>,
    entities: &mut Vec<EntityLink>,
    unlinked_code: &mut Vec<UnlinkedCode>,
    unlinked_spec: &mut Vec<UnlinkedSpec>,
) {
    // Collect all classes from scan
    let mut code_classes: Vec<(&CodeElement, &str, &str)> = Vec::new(); // (element, file_path, module_path)
    for file in &scan.files {
        collect_classes(&file.elements, &file.file_path, &file.module_path, &mut code_classes);
    }

    // Spec entities for matching
    let spec_entities: Vec<(&str, &str)> = match spec {
        Some(doc) => doc.entities.iter()
            .map(|e| (e.id.as_str(), e.name.as_deref().unwrap_or(&e.id)))
            .collect(),
        None => Vec::new(),
    };

    let mut matched_spec_ids: Vec<String> = Vec::new();

    for (element, file_path, module_path) in &code_classes {
        let code_ref = format!("{}:{}", file_path, element.name);

        // Try to match with spec entity
        let mut best_match: Option<(usize, f64)> = None;
        for (idx, (spec_id, spec_name)) in spec_entities.iter().enumerate() {
            let sim_id = name_similarity(&element.name, spec_id);
            let sim_name = name_similarity(&element.name, spec_name);
            let sim = sim_id.max(sim_name);
            if sim >= 0.5 {
                if best_match.is_none() || sim > best_match.unwrap().1 {
                    best_match = Some((idx, sim));
                }
            }
        }

        // Build field links
        let fields = build_field_links(element, spec, best_match.map(|(idx, _)| spec_entities[idx].0));

        if let Some((idx, confidence)) = best_match {
            let spec_id = spec_entities[idx].0.to_string();
            matched_spec_ids.push(spec_id.clone());

            entities.push(EntityLink {
                entity_id: spec_id,
                entity_name: element.name.clone(),
                code_ref,
                confidence,
                source: LinkSource::Matched,
                fields,
                bases: element.bases.clone(),
            });
        } else {
            // Suggest as new entity
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

    // Find spec entities not matched to any code
    if let Some(doc) = spec {
        for entity in &doc.entities {
            if !matched_spec_ids.contains(&entity.id) {
                unlinked_spec.push(UnlinkedSpec {
                    element_type: "entity".to_string(),
                    id: entity.id.clone(),
                    name: entity.name.clone(),
                });
            }
        }
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

fn build_field_links(
    class: &CodeElement,
    spec: Option<&FdmlDocument>,
    matched_entity_id: Option<&str>,
) -> Vec<FieldLink> {
    let mut fields = Vec::new();

    // Get spec fields for matched entity
    let spec_fields: HashMap<String, &crate::parser::ast::Field> = match (spec, matched_entity_id) {
        (Some(doc), Some(eid)) => {
            doc.entities.iter()
                .find(|e| e.id == eid)
                .map(|e| e.fields.iter().map(|f| (f.name.clone(), f)).collect())
                .unwrap_or_default()
        }
        _ => HashMap::new(),
    };

    // Code fields
    let mut seen_code_fields: Vec<String> = Vec::new();
    for child in &class.children {
        if matches!(child.element_type, ElementType::Field | ElementType::Property) {
            let in_spec = spec_fields.contains_key(&child.name);
            let field_type = child.return_type.clone()
                .or_else(|| infer_fdml_type(child.default_value.as_deref()));

            fields.push(FieldLink {
                name: child.name.clone(),
                field_type,
                default_value: child.default_value.clone(),
                in_spec,
                in_code: true,
            });
            seen_code_fields.push(child.name.clone());
        }
    }

    // Spec fields not in code
    for (name, spec_field) in &spec_fields {
        if !seen_code_fields.contains(name) {
            fields.push(FieldLink {
                name: name.clone(),
                field_type: Some(spec_field.field_type.clone()),
                default_value: None,
                in_spec: true,
                in_code: false,
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
    spec: Option<&FdmlDocument>,
    actions: &mut Vec<ActionLink>,
    unlinked_code: &mut Vec<UnlinkedCode>,
) {
    let spec_actions: Vec<(&str, &str)> = match spec {
        Some(doc) => doc.actions.iter()
            .map(|a| (a.id.as_str(), a.name.as_deref().unwrap_or(&a.id)))
            .collect(),
        None => Vec::new(),
    };

    for file in &scan.files {
        let module = if !file.module_path.is_empty() {
            &file.module_path
        } else {
            &file.file_path
        };

        // Top-level functions
        for el in &file.elements {
            if matches!(el.element_type, ElementType::Function) {
                process_action_candidate(el, &file.file_path, module, None, &spec_actions, actions, unlinked_code);
            }

            // Public methods inside classes
            if matches!(el.element_type, ElementType::Class) {
                for child in &el.children {
                    if matches!(child.element_type, ElementType::Method) {
                        let scope = child.scope.as_ref();
                        let is_public = matches!(scope, Some(Scope::Public) | None);
                        let is_dunder = child.name.starts_with("__") && child.name.ends_with("__");

                        // Skip private, protected, and dunder methods (except meaningful ones)
                        if !is_public || is_dunder {
                            continue;
                        }

                        process_action_candidate(
                            child, &file.file_path, module,
                            Some(&el.name), &spec_actions, actions, unlinked_code,
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
    spec_actions: &[(&str, &str)],
    actions: &mut Vec<ActionLink>,
    unlinked_code: &mut Vec<UnlinkedCode>,
) {
    let code_ref = match class_name {
        Some(cls) => format!("{}:{}.{}", file_path, cls, el.name),
        None => format!("{}:{}", file_path, el.name),
    };

    let action_id_candidate = match class_name {
        Some(cls) => format!("{}_{}", normalize_name(cls), normalize_name(&el.name)),
        None => normalize_name(&el.name),
    };

    // Try to match with spec actions
    let mut best_match: Option<(usize, f64)> = None;
    for (idx, (spec_id, spec_name)) in spec_actions.iter().enumerate() {
        let sim_id = name_similarity(&el.name, spec_id);
        let sim_name = name_similarity(&el.name, spec_name);
        let sim_class = class_name.map(|cls| {
            let full = format!("{}_{}", cls, el.name);
            name_similarity(&full, spec_id).max(name_similarity(&full, spec_name))
        }).unwrap_or(0.0);

        let sim = sim_id.max(sim_name).max(sim_class);
        if sim >= 0.5 {
            if best_match.is_none() || sim > best_match.unwrap().1 {
                best_match = Some((idx, sim));
            }
        }
    }

    let input: Vec<ActionParam> = el.parameters.iter().map(|p| ActionParam {
        name: p.name.clone(),
        param_type: p.type_hint.clone(),
        required: if p.default_value.is_some() { Some(false) } else { Some(true) },
    }).collect();

    let (action_id, confidence, source) = if let Some((idx, conf)) = best_match {
        (spec_actions[idx].0.to_string(), conf, LinkSource::Matched)
    } else {
        (action_id_candidate.clone(), 0.0, LinkSource::Suggested)
    };

    actions.push(ActionLink {
        action_id: action_id.clone(),
        action_name: el.name.clone(),
        code_ref: code_ref.clone(),
        confidence,
        source: source.clone(),
        input,
        output: el.return_type.clone(),
        description: el.docstring.clone(),
    });

    if matches!(source, LinkSource::Suggested) {
        unlinked_code.push(UnlinkedCode {
            element_type: if el.element_type == ElementType::Function { "function" } else { "method" }.to_string(),
            name: el.name.clone(),
            code_ref,
            module_path: module_path.to_string(),
            suggestion: Some(format!("Candidate for action '{}'", action_id)),
        });
    }
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

    for file in &scan.files {
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

        if ents.is_empty() && acts.is_empty() {
            continue;
        }

        // Generate human-readable title from module name
        let last_segment = module.split('.').last().unwrap_or(module);
        let title = last_segment
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
            .join(" ");

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

fn find_unlinked_spec_actions(
    doc: &FdmlDocument,
    actions: &[ActionLink],
    unlinked_spec: &mut Vec<UnlinkedSpec>,
) {
    let matched_ids: Vec<&str> = actions.iter()
        .filter(|a| matches!(a.source, LinkSource::Matched))
        .map(|a| a.action_id.as_str())
        .collect();

    for action in &doc.actions {
        if !matched_ids.contains(&action.id.as_str()) {
            unlinked_spec.push(UnlinkedSpec {
                element_type: "action".to_string(),
                id: action.id.clone(),
                name: action.name.clone(),
            });
        }
    }
}

fn find_unlinked_spec_features(
    doc: &FdmlDocument,
    features: &[FeatureSuggestion],
    unlinked_spec: &mut Vec<UnlinkedSpec>,
) {
    let matched_ids: Vec<&str> = features.iter()
        .filter(|f| matches!(f.source, LinkSource::Matched))
        .map(|f| f.feature_id.as_str())
        .collect();

    for feature in &doc.features {
        if !matched_ids.contains(&feature.id.as_str()) {
            unlinked_spec.push(UnlinkedSpec {
                element_type: "feature".to_string(),
                id: feature.id.clone(),
                name: Some(feature.title.clone()),
            });
        }
    }
}

fn compute_coverage(
    spec: Option<&FdmlDocument>,
    entities: &[EntityLink],
    actions: &[ActionLink],
    _features: &[FeatureSuggestion],
    _unlinked_code: &[UnlinkedCode],
    _unlinked_spec: &[UnlinkedSpec],
) -> CoverageReport {
    let spec_total = spec.map(|doc| {
        doc.entities.len() + doc.actions.len() + doc.features.len()
    }).unwrap_or(0);

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

// ─── Metaprompt generation ────────────────────────────────────────

/// Generate a structured prompt for LLM to process the linking report
pub fn generate_metaprompt(report: &LinkReport, scan: &ScanResult) -> String {
    let mut prompt = String::new();

    prompt.push_str("# FDML Link-Code Analysis Report\n\n");
    prompt.push_str("You are analyzing a codebase to create an FDML specification.\n");
    prompt.push_str("Below is the automated linking report. Your task is to review each section\n");
    prompt.push_str("and make semantic decisions that the CLI cannot make automatically.\n\n");

    // ─── Instructions ─────────────────────────────────────────────
    prompt.push_str("## Instructions\n\n");
    prompt.push_str("For each section, decide:\n");
    prompt.push_str("1. **Entities**: Which classes are real domain entities vs helper/utility classes?\n");
    prompt.push_str("2. **Actions**: Which functions represent business actions vs internal implementation?\n");
    prompt.push_str("3. **Features**: How should modules be grouped into user-facing features?\n");
    prompt.push_str("4. **Scenarios**: Write BDD scenarios (Given/When/Then) for each feature\n");
    prompt.push_str("5. **Constraints**: Identify business rules from the code structure\n\n");

    prompt.push_str("Output a valid FDML YAML specification with entities, actions, features,\n");
    prompt.push_str("constraints, and traceability sections.\n\n");

    // ─── System overview ──────────────────────────────────────────
    prompt.push_str("---\n\n## System Overview\n\n");
    prompt.push_str(&format!("**Name**: {}\n", report.system.name));
    prompt.push_str(&format!("**Components**: {}\n", report.system.components.join(", ")));
    prompt.push_str(&format!("**Languages**: {:?}\n",
        scan.metadata.languages_detected.iter().map(|l| l.name()).collect::<Vec<_>>()));
    prompt.push_str(&format!("**Files**: {}\n\n", scan.metadata.total_files));

    if !report.system.relationships.is_empty() {
        prompt.push_str("**Internal dependencies:**\n");
        for rel in &report.system.relationships {
            let desc = rel.description.as_deref().unwrap_or("");
            prompt.push_str(&format!("- {} → {} ({})\n", rel.from, rel.to, desc));
        }
        prompt.push_str("\n");
    }

    // ─── Entity candidates ────────────────────────────────────────
    prompt.push_str("---\n\n## Entity Candidates (classes → FDML entities)\n\n");
    prompt.push_str("Decide for each: is this a **domain entity** (keep) or a **helper class** (skip)?\n\n");

    for (i, entity) in report.entities.iter().enumerate() {
        let status = match entity.source {
            LinkSource::Matched => "✅ MATCHED",
            LinkSource::Suggested => "🔍 SUGGESTED",
        };
        prompt.push_str(&format!("### {}. {} [{}] (confidence: {:.0}%)\n",
            i + 1, entity.entity_name, status, entity.confidence * 100.0));
        prompt.push_str(&format!("- **Suggested ID**: `{}`\n", entity.entity_id));
        prompt.push_str(&format!("- **Code**: `{}`\n", entity.code_ref));

        if !entity.bases.is_empty() {
            prompt.push_str(&format!("- **Inherits**: {}\n", entity.bases.join(", ")));
        }

        if !entity.fields.is_empty() {
            prompt.push_str("- **Fields**:\n");
            for field in &entity.fields {
                let type_str = field.field_type.as_deref().unwrap_or("?");
                let status = match (field.in_spec, field.in_code) {
                    (true, true) => "✅",
                    (false, true) => "➕ code only",
                    (true, false) => "⚠️ spec only",
                    (false, false) => "?",
                };
                prompt.push_str(&format!("  - `{}`: {} [{}]\n", field.name, type_str, status));
            }
        }
        prompt.push_str("\n");
    }

    // ─── Action candidates ────────────────────────────────────────
    prompt.push_str("---\n\n## Action Candidates (functions/methods → FDML actions)\n\n");
    prompt.push_str("Decide for each: is this a **business action** (keep) or **internal implementation** (skip)?\n\n");

    for (i, action) in report.actions.iter().enumerate() {
        let status = match action.source {
            LinkSource::Matched => "✅ MATCHED",
            LinkSource::Suggested => "🔍 SUGGESTED",
        };
        prompt.push_str(&format!("### {}. {} [{}] (confidence: {:.0}%)\n",
            i + 1, action.action_name, status, action.confidence * 100.0));
        prompt.push_str(&format!("- **Suggested ID**: `{}`\n", action.action_id));
        prompt.push_str(&format!("- **Code**: `{}`\n", action.code_ref));

        if let Some(ref desc) = action.description {
            prompt.push_str(&format!("- **Docstring**: {}\n", desc));
        }

        if !action.input.is_empty() {
            let params: Vec<String> = action.input.iter().map(|p| {
                let t = p.param_type.as_deref().unwrap_or("?");
                format!("{}: {}", p.name, t)
            }).collect();
            prompt.push_str(&format!("- **Input**: ({})\n", params.join(", ")));
        }

        if let Some(ref out) = action.output {
            prompt.push_str(&format!("- **Output**: {}\n", out));
        }
        prompt.push_str("\n");
    }

    // ─── Feature suggestions ──────────────────────────────────────
    prompt.push_str("---\n\n## Feature Suggestions (modules → FDML features)\n\n");
    prompt.push_str("Decide: how to group and name features, write BDD scenarios.\n\n");

    for (i, feature) in report.features.iter().enumerate() {
        prompt.push_str(&format!("### {}. {} (module: `{}`)\n", i + 1, feature.title, feature.module_path));
        prompt.push_str(&format!("- **Suggested ID**: `{}`\n", feature.feature_id));

        if !feature.entities.is_empty() {
            prompt.push_str(&format!("- **Entities**: {}\n", feature.entities.join(", ")));
        }
        if !feature.actions.is_empty() {
            prompt.push_str(&format!("- **Actions**: {}\n", feature.actions.join(", ")));
        }
        prompt.push_str("- **TODO**: Write Given/When/Then scenarios\n\n");
    }

    // ─── Coverage ─────────────────────────────────────────────────
    prompt.push_str("---\n\n## Coverage Summary\n\n");
    prompt.push_str(&format!("- **Spec coverage**: {}/{} ({:.0}%)\n",
        report.coverage.spec_coverage.linked,
        report.coverage.spec_coverage.total,
        report.coverage.spec_coverage.percentage));
    prompt.push_str(&format!("- **Code coverage**: {}/{} ({:.0}%)\n",
        report.coverage.code_coverage.linked,
        report.coverage.code_coverage.total,
        report.coverage.code_coverage.percentage));

    if !report.unlinked_spec.is_empty() {
        prompt.push_str("\n**Spec elements without code:**\n");
        for us in &report.unlinked_spec {
            prompt.push_str(&format!("- ⚠️ {} `{}` ({})\n",
                us.element_type, us.id, us.name.as_deref().unwrap_or("-")));
        }
    }

    // ─── Checklist ────────────────────────────────────────────────
    prompt.push_str("\n---\n\n## Checklist\n\n");
    prompt.push_str("- [ ] Review entity candidates — keep domain entities, remove helpers\n");
    prompt.push_str("- [ ] Review action candidates — keep business actions, remove internal methods\n");
    prompt.push_str("- [ ] Name and group features from modules\n");
    prompt.push_str("- [ ] Write BDD scenarios (Given/When/Then) for each feature\n");
    prompt.push_str("- [ ] Add field types and constraints to entities\n");
    prompt.push_str("- [ ] Identify business constraints from code\n");
    prompt.push_str("- [ ] Verify traceability links\n");
    prompt.push_str("- [ ] Fill action.logic with pseudocode descriptions\n");

    prompt
}

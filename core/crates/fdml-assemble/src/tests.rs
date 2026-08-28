//! Tests for the deterministic FDML spec assembler.

use fdml_types::graph::{
    ActionLink, ActionParam, CoverageMetric, CoverageReport, EntityLink, FeatureSuggestion,
    FieldLink, LinkMetadata, LinkReport, LinkSource, SuggestedSystem,
};
use fdml_types::scan::{Language, ModuleNode, ScanMetadata, ScanResult, ScanStatistics};

fn sample_scan() -> ScanResult {
    ScanResult {
        metadata: ScanMetadata {
            scanner_version: "test".into(),
            codebase_path: "/tmp/projects/shop".into(),
            languages_detected: vec![Language::Python],
            total_files: 3,
        },
        modules: vec![ModuleNode {
            name: "orders".into(),
            module_path: "orders".into(),
            file_path: None,
            language: None,
            children: vec![ModuleNode {
                name: "models".into(),
                module_path: "orders.models".into(),
                file_path: Some("orders/models.py".into()),
                language: Some(Language::Python),
                children: vec![],
            }],
        }],
        files: vec![],
        relationships: vec![],
        statistics: ScanStatistics {
            classes: 1,
            functions: 1,
            methods: 0,
            interfaces: 0,
            enums: 0,
            fields: 2,
            imports_external: 0,
            imports_internal: 0,
            relationships: 0,
        },
    }
}

fn sample_report() -> LinkReport {
    LinkReport {
        metadata: LinkMetadata {
            linker_version: "test".into(),
            inventory_file: "scan.json".into(),
            spec_file: None,
            spec_loaded: false,
        },
        system: SuggestedSystem {
            id: "shop".into(),
            name: "shop".into(),
            components: vec![],
            relationships: vec![],
        },
        entities: vec![EntityLink {
            entity_id: "order".into(),
            entity_name: "Order".into(),
            code_ref: "orders/models.py:Order".into(),
            confidence: 0.9,
            source: LinkSource::Suggested,
            fields: vec![
                FieldLink {
                    name: "id".into(),
                    field_type: Some("int".into()),
                    default_value: None,
                    in_spec: false,
                    in_code: true,
                },
                FieldLink {
                    name: "total".into(),
                    field_type: Some("float".into()),
                    default_value: None,
                    in_spec: false,
                    in_code: true,
                },
            ],
            bases: vec![],
        }],
        actions: vec![ActionLink {
            action_id: "place_order".into(),
            action_name: "place_order".into(),
            code_ref: "orders/service.py:place_order".into(),
            confidence: 0.8,
            source: LinkSource::Suggested,
            input: vec![ActionParam {
                name: "order".into(),
                param_type: Some("Order".into()),
                required: Some(true),
            }],
            output: Some("Order".into()),
            description: None,
        }],
        features: vec![FeatureSuggestion {
            feature_id: "orders".into(),
            title: "Orders".into(),
            module_path: "orders".into(),
            confidence: 0.7,
            source: LinkSource::Suggested,
            entities: vec!["order".into()],
            actions: vec!["place_order".into()],
        }],
        traceability: vec![],
        coverage: CoverageReport {
            spec_coverage: CoverageMetric { total: 0, linked: 0, percentage: 0.0 },
            code_coverage: CoverageMetric { total: 2, linked: 2, percentage: 100.0 },
        },
        unlinked_code: vec![],
    }
}

#[test]
fn emits_top_level_sections() {
    let yaml = crate::run(&sample_report(), &sample_scan(), &[], Some("shop"));

    assert!(yaml.contains("metadata:\n"), "missing metadata section");
    assert!(yaml.contains("  version: \"1.3\"\n"), "missing FDML version string");
    assert!(yaml.contains("system:\n"), "missing system section");
    assert!(yaml.contains("entities:\n"), "missing entities section");
    assert!(yaml.contains("actions:\n"), "missing actions section");
    assert!(yaml.contains("features:\n"), "missing features section");
    assert!(yaml.contains("traceability:\n"), "missing traceability section");
}

#[test]
fn emits_known_entity_and_action_ids() {
    let yaml = crate::run(&sample_report(), &sample_scan(), &[], Some("shop"));

    assert!(yaml.contains("  - id: order\n"), "missing entity id `order`");
    assert!(yaml.contains("  - id: place_order\n"), "missing action id `place_order`");
    // Entity output wiring: place_order returns Order, which is a domain entity.
    assert!(yaml.contains("      entity: order\n"), "missing action output entity wiring");
    // Traceability: action depends_on entity via its typed param.
    assert!(yaml.contains("    to: \"entity:order\"\n"), "missing depends_on traceability");
}

#[test]
fn no_created_or_wall_clock() {
    let yaml = crate::run(&sample_report(), &sample_scan(), &[], Some("shop"));
    assert!(!yaml.contains("created:"), "wall-clock `created` field must be dropped");
}

#[test]
fn deterministic_two_runs_identical() {
    let report = sample_report();
    let scan = sample_scan();
    let a = crate::run(&report, &scan, &[], Some("shop"));
    let b = crate::run(&report, &scan, &[], Some("shop"));
    assert_eq!(a, b, "two assembly runs must be byte-identical");
}

#[test]
fn end_to_end_via_scan_and_graph() {
    // Build a tiny Python project, run the real scan + graph passes, then assemble.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("models.py"),
        "class Invoice:\n    def __init__(self):\n        self.id = 0\n        self.amount = 0.0\n",
    )
    .unwrap();
    std::fs::write(
        root.join("service.py"),
        "from models import Invoice\n\ndef submit_invoice(invoice):\n    return invoice\n",
    )
    .unwrap();

    let scan = fdml_scan::run(root, &[]).unwrap();
    let report = fdml_graph::run(&scan);
    let yaml = crate::run(&report, &scan, &[], Some("billing"));

    assert!(yaml.contains("metadata:\n"));
    assert!(yaml.contains("  name: \"billing\"\n"));
    assert!(yaml.contains("system:\n"));
    // The Invoice class (2 fields, not a helper) should surface as a domain entity.
    assert!(yaml.contains("entities:\n"), "expected entities from the scanned Invoice class\n{yaml}");
    assert!(yaml.contains("name: \"Invoice\""), "expected Invoice entity\n{yaml}");

    // And the whole thing stays deterministic end-to-end.
    let again = crate::run(&report, &scan, &[], Some("billing"));
    assert_eq!(yaml, again);
}

#[test]
fn platform_spec_has_sections() {
    let systems = vec![
        fdml_types::System { path: "web".into(), id: "web".into(), name: "Web".into(), system_type: "frontend".into(), technology: "React".into(), boundary_marker: "package.json".into() },
        fdml_types::System { path: "api".into(), id: "api".into(), name: "Api".into(), system_type: "service".into(), technology: "Go".into(), boundary_marker: "go.mod".into() },
    ];
    let links = fdml_types::integration::PlatformLinks {
        integrations: vec![fdml_types::integration::IntegrationHint {
            from_system: "web".into(),
            to_system: Some("api".into()),
            integration_type: "http".into(),
            technology: "REST/JSON".into(),
            evidence: vec!["web/client.ts".into()],
        }],
        shared_entities: vec![],
    };
    let yaml = crate::run_platform(&systems, &links, "demo");
    for needle in ["version: \"1.4\"", "systems:", "contours:", "id: external", "id: core", "integrations:", "cross_flows:", "from: web", "to: api"] {
        assert!(yaml.contains(needle), "platform spec missing `{needle}`\n{yaml}");
    }
    assert_eq!(yaml, crate::run_platform(&systems, &links, "demo")); // deterministic
}

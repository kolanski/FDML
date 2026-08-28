//! Role-tagging tests — drive `detect_roles` directly for the per-file signals, then a
//! synthetic `ScanResult` + `Clusters` for aggregation and determinism.

use super::{compute_file_roles, detect_roles, tag_roles};
use fdml_types::cluster::{Cluster, ClusterKind, Clusters};
use fdml_types::scan::{
    FileAnalysis, ImportInfo, Language, ScanMetadata, ScanResult, ScanStatistics,
};

// ─────────────────────────────── per-file signal tests ──

/// A `repositories/` directory tags the file `data-access` (directory signal).
#[test]
fn repository_dir_is_data_access() {
    let roles = detect_roles("repositories/userRepo.ts", &[], &[]);
    assert!(roles.contains(&"data-access".to_string()), "got {roles:?}");
}

/// A file importing `axios` is tagged `integration` (import signal), even when its
/// directory carries no role hint.
#[test]
fn axios_import_is_integration() {
    let roles = detect_roles("src/widgetry/fetcher.ts", &["axios"], &[]);
    assert_eq!(roles, vec!["integration".to_string()], "got {roles:?}");
}

/// `components/ui/button.tsx` is tagged `ui` (directory signal); `components` and `ui`
/// both map to `ui`, so the union stays a single tag.
#[test]
fn components_ui_is_ui() {
    let roles = detect_roles("components/ui/button.tsx", &[], &[]);
    assert_eq!(roles, vec!["ui".to_string()], "got {roles:?}");
}

/// Signals UNION across all three families and emit a sorted, deduped set. A file in
/// `services/` (domain) that imports `sqlalchemy` (data-access) and declares a
/// `UserController` (api) carries all three.
#[test]
fn signals_union_sorted() {
    let roles = detect_roles("app/services/user.py", &["sqlalchemy.orm"], &["UserController"]);
    assert_eq!(
        roles,
        vec!["api".to_string(), "data-access".to_string(), "domain".to_string()],
        "got {roles:?}"
    );
}

/// A file with no matching signal gets NO tag (we never force one).
#[test]
fn no_signal_no_tag() {
    assert!(detect_roles("src/foo/bar.ts", &["react", "lodash"], &["doThing"]).is_empty());
}

/// PascalCase suffix matching must not false-positive: `Restore` is not `*Store`.
#[test]
fn name_suffix_is_case_sensitive() {
    assert!(detect_roles("src/x/y.ts", &[], &["Restore"]).is_empty());
    assert_eq!(detect_roles("src/x/y.ts", &[], &["SessionStore"]), vec!["data-access".to_string()]);
}

// ─────────────────────────────── aggregation + determinism ──

fn file(path: &str, modules: &[&str]) -> FileAnalysis {
    FileAnalysis {
        file_path: path.to_string(),
        module_path: String::new(),
        language: Language::TypeScript,
        elements: Vec::new(),
        imports: modules
            .iter()
            .map(|m| ImportInfo {
                module: m.to_string(),
                names: Vec::new(),
                is_relative: false,
                file_path: path.to_string(),
                line: 1,
            })
            .collect(),
        calls: Vec::new(),
    }
}

fn scan(files: Vec<FileAnalysis>) -> ScanResult {
    ScanResult {
        metadata: ScanMetadata {
            scanner_version: "test".into(),
            codebase_path: ".".into(),
            languages_detected: vec![Language::TypeScript],
            total_files: files.len(),
        },
        modules: Vec::new(),
        files,
        relationships: Vec::new(),
        statistics: ScanStatistics {
            classes: 0,
            functions: 0,
            methods: 0,
            interfaces: 0,
            enums: 0,
            fields: 0,
            imports_external: 0,
            imports_internal: 0,
            relationships: 0,
        },
    }
}

fn cluster(id: &str, name: &str, members: &[&str]) -> Cluster {
    Cluster {
        id: id.into(),
        name: name.into(),
        members: members.iter().map(|s| s.to_string()).collect(),
        size: members.len(),
        kind: ClusterKind::Component,
        roles: Vec::new(),
    }
}

/// A cluster's `roles` is the DOMINANT (≥20%) sorted union of its members' roles; a
/// one-off role under the threshold is dropped.
#[test]
fn cluster_roles_are_dominant_union() {
    let s = scan(vec![
        file("repositories/a.ts", &[]),
        file("repositories/b.ts", &[]),
        file("services/c.ts", &[]),
        file("services/d.ts", &[]),
        file("src/lonely.ts", &["axios"]), // integration — only 1 of 5 → 20% kept
    ]);
    let mut clusters = Clusters {
        clusters: vec![cluster(
            "c1",
            "X",
            &["repositories/a.ts", "repositories/b.ts", "services/c.ts", "services/d.ts", "src/lonely.ts"],
        )],
    };
    tag_roles(&s, &mut clusters);
    // data-access (2/5), domain (2/5), integration (1/5 == 20%) all clear 20%.
    assert_eq!(
        clusters.clusters[0].roles,
        vec!["data-access".to_string(), "domain".to_string(), "integration".to_string()]
    );
}

/// A role held by < 20% of members is pruned from the dominant set.
#[test]
fn rare_role_is_pruned() {
    // 9 ui files + 1 integration file → integration is 10% < 20% → dropped.
    let mut files: Vec<FileAnalysis> = (0..9).map(|i| file(&format!("components/ui/c{i}.ts"), &[])).collect();
    files.push(file("src/edge/net.ts", &["axios"]));
    let members: Vec<String> = (0..9).map(|i| format!("components/ui/c{i}.ts")).collect::<Vec<_>>();
    let mut all: Vec<&str> = members.iter().map(String::as_str).collect();
    all.push("src/edge/net.ts");

    let s = scan(files);
    let mut clusters = Clusters { clusters: vec![cluster("c1", "Ui", &all)] };
    tag_roles(&s, &mut clusters);
    assert_eq!(clusters.clusters[0].roles, vec!["ui".to_string()], "integration should be pruned");
}

/// Tagging the same scan/clusters twice serializes byte-identically (roles included).
#[test]
fn deterministic_role_tagging() {
    let s = scan(vec![
        file("repositories/a.ts", &["prisma"]),
        file("controllers/b.ts", &["express"]),
        file("components/ui/c.tsx", &[]),
    ]);
    let make = || Clusters {
        clusters: vec![cluster(
            "c1",
            "X",
            &["components/ui/c.tsx", "controllers/b.ts", "repositories/a.ts"],
        )],
    };
    let mut a = make();
    let mut b = make();
    tag_roles(&s, &mut a);
    tag_roles(&s, &mut b);
    assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());

    // compute_file_roles keys are POSIX file paths.
    let fr = compute_file_roles(&s);
    assert!(fr.contains_key("repositories/a.ts"));
}

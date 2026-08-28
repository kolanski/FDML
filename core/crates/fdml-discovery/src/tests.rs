//! Behavioral parity tests: lock in that discovery finds what FDML found.
//! Each classify branch + structure quirk + the rules layer gets a case.

use super::{normalize_name, run, DiscoveryRules};
use fdml_types::System;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

fn ids(systems: &[System]) -> Vec<&str> {
    systems.iter().map(|s| s.id.as_str()).collect()
}

#[test]
fn detects_react_frontend() {
    let t = TempDir::new().unwrap();
    write(t.path(), "web/package.json", r#"{ "dependencies": { "react": "^18" } }"#);
    let sys = run(t.path(), &[], None);
    assert_eq!(sys.len(), 1);
    assert_eq!(sys[0].id, "web");
    assert_eq!(sys[0].system_type, "frontend");
    assert_eq!(sys[0].technology, "React + TypeScript");
    assert_eq!(sys[0].boundary_marker, "package.json");
}

#[test]
fn detects_multiple_systems_with_types() {
    let t = TempDir::new().unwrap();
    write(t.path(), "api/requirements.txt", "fastapi==0.1\nuvicorn");
    write(t.path(), "ui/package.json", r#"{ "dependencies": { "next": "14" } }"#);
    write(t.path(), "svc/go.mod", "module svc\n");
    let mut sys = run(t.path(), &[], None);
    sys.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(ids(&sys), vec!["api", "svc", "ui"]);

    let api = sys.iter().find(|s| s.id == "api").unwrap();
    assert_eq!((api.system_type.as_str(), api.technology.as_str()), ("service", "FastAPI + Python"));
    let ui = sys.iter().find(|s| s.id == "ui").unwrap();
    assert_eq!((ui.system_type.as_str(), ui.technology.as_str()), ("frontend", "React + TypeScript"));
    let svc = sys.iter().find(|s| s.id == "svc").unwrap();
    assert_eq!((svc.system_type.as_str(), svc.technology.as_str()), ("service", "Go"));
}

#[test]
fn classifies_dotnet_blazor_vs_service() {
    let t = TempDir::new().unwrap();
    write(t.path(), "plain/Api.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>");
    write(t.path(), "front/Ui.csproj", "<Project Sdk=\"Microsoft.NET.Sdk.BlazorWebAssembly\"></Project>");
    let sys = run(t.path(), &[], None);

    let plain = sys.iter().find(|s| s.id == "plain").unwrap();
    assert_eq!((plain.system_type.as_str(), plain.technology.as_str()), ("service", "C# (.NET)"));
    assert_eq!(plain.boundary_marker, "Api.csproj");
    let front = sys.iter().find(|s| s.id == "front").unwrap();
    assert_eq!((front.system_type.as_str(), front.technology.as_str()), ("frontend", "Blazor (.NET)"));
}

#[test]
fn skips_infra_and_recurses_container_dirs() {
    let t = TempDir::new().unwrap();
    write(t.path(), "node_modules/foo/package.json", r#"{"dependencies":{"react":"1"}}"#);
    write(t.path(), "services/billing/go.mod", "module billing\n");
    let sys = run(t.path(), &[], None);
    // node_modules skipped as infra; services/ is a container → recurse into billing.
    assert_eq!(ids(&sys), vec!["billing"]);
    assert_eq!(sys[0].path, "services/billing");
}

#[test]
fn detects_systems_in_nonstandard_grouping_dir() {
    // achiever-style: manifests nested under a non-whitelist grouping dir (depth 2).
    let t = TempDir::new().unwrap();
    write(t.path(), "myapp/backend/go.mod", "module backend\n");
    write(t.path(), "myapp/frontend/package.json", r#"{"dependencies":{"react":"^18"}}"#);
    let mut sys = run(t.path(), &[], None);
    sys.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(ids(&sys), vec!["backend", "frontend"]);
    assert_eq!(sys.iter().find(|s| s.id == "backend").unwrap().path, "myapp/backend");
}

#[test]
fn humanizes_and_normalizes_names() {
    let t = TempDir::new().unwrap();
    write(t.path(), "user-api/go.mod", "module x\n");
    let sys = run(t.path(), &[], None);
    assert_eq!(sys[0].id, "user_api");
    assert_eq!(sys[0].name, "User Api");
}

#[test]
fn respects_exclude() {
    let t = TempDir::new().unwrap();
    write(t.path(), "keep/go.mod", "module k\n");
    write(t.path(), "drop/go.mod", "module d\n");
    let sys = run(t.path(), &["drop".to_string()], None);
    assert_eq!(ids(&sys), vec!["keep"]);
}

#[test]
fn rules_override_builtin() {
    let t = TempDir::new().unwrap();
    write(t.path(), "job/Worker.csproj", "<Project></Project>");
    // Built-in would classify as service / C# (.NET); rule remaps *.csproj.
    let rules = DiscoveryRules::from_yaml(
        "rules:\n  - marker: \"*.csproj\"\n    kind: worker\n    tech: \"Custom .NET Worker\"\n",
    )
    .unwrap();
    let sys = run(t.path(), &[], Some(&rules));
    assert_eq!(sys[0].system_type, "worker");
    assert_eq!(sys[0].technology, "Custom .NET Worker");
    assert_eq!(sys[0].boundary_marker, "Worker.csproj");
}

#[test]
fn rules_contains_filter() {
    let t = TempDir::new().unwrap();
    write(t.path(), "app/package.json", r#"{"dependencies":{"electron":"^30"}}"#);
    // Same marker, content-gated: only matches when the file contains "electron".
    let rules = DiscoveryRules::from_yaml(
        "rules:\n  - marker: package.json\n    contains: electron\n    kind: desktop\n    tech: \"Electron\"\n",
    )
    .unwrap();
    let sys = run(t.path(), &[], Some(&rules));
    assert_eq!(sys[0].system_type, "desktop");
    assert_eq!(sys[0].technology, "Electron");
}

#[test]
fn detects_root_app_alongside_subsystems() {
    // The improvement: a root app + a helper sub-project → BOTH detected.
    // FDML's original detect_systems dropped the root once any subsystem was found.
    let t = TempDir::new().unwrap();
    write(t.path(), "package.json", r#"{ "dependencies": { "react": "^18" } }"#);
    write(t.path(), "runner/package.json", r#"{ "dependencies": {} }"#);
    let mut sys = run(t.path(), &[], None);
    sys.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(sys.len(), 2);
    assert!(ids(&sys).contains(&"runner"), "sub-project missing: {:?}", ids(&sys));
    // Root app: identified by path ".", classified as the React frontend.
    let root = sys.iter().find(|s| s.path == ".").expect("root app should be detected");
    assert_eq!(root.system_type, "frontend");
    assert_eq!(root.technology, "React + TypeScript");
    assert_eq!(root.boundary_marker, "package.json");
}

#[test]
fn root_not_added_when_root_has_no_marker() {
    // Regression guard: existing subsystem-only repos must NOT gain a spurious root system.
    let t = TempDir::new().unwrap();
    write(t.path(), "web/package.json", r#"{ "dependencies": { "react": "^18" } }"#);
    let sys = run(t.path(), &[], None);
    assert_eq!(ids(&sys), vec!["web"]);
}

#[test]
fn normalize_handles_camel_and_acronyms() {
    assert_eq!(normalize_name("user-api"), "user_api");
    assert_eq!(normalize_name("WrapperApi"), "wrapper_api");
}

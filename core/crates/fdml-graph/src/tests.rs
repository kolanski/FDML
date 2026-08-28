//! Parity + determinism tests for the deterministic linker (generation path).
//!
//! We scan a tiny real Python fixture with `fdml-scan` (dev-dependency) so the test
//! exercises the actual ScanResult contract, then run the linker over it.

use super::run;
use fdml_types::graph::LinkSource;
use std::fs;

/// One Python class (2 class-level fields + 1 method) plus one top-level function.
fn write_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("shop.py"),
        "class User:\n\
         \x20\x20\x20\x20name = \"Alice\"\n\
         \x20\x20\x20\x20email = \"a@b.com\"\n\
         \n\
         \x20\x20\x20\x20def activate(self, token):\n\
         \x20\x20\x20\x20\x20\x20\x20\x20return token\n\
         \n\
         \n\
         def compute_total(items, tax):\n\
         \x20\x20\x20\x20return items + tax\n",
    )
    .unwrap();
    dir
}

#[test]
fn class_becomes_entity_with_fields() {
    let dir = write_fixture();
    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let report = run(&scan);

    // The class `User` became a suggested entity carrying its two fields.
    let user = report
        .entities
        .iter()
        .find(|e| e.entity_name == "User")
        .expect("User class should become an entity");
    assert_eq!(user.entity_id, "user");
    assert!(matches!(user.source, LinkSource::Suggested));
    assert_eq!(user.confidence, 0.0);

    let field_names: Vec<&str> = user.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"name"), "fields={field_names:?}");
    assert!(field_names.contains(&"email"), "fields={field_names:?}");
    // Generation path: every field is code-only.
    assert!(user.fields.iter().all(|f| f.in_code && !f.in_spec));
}

#[test]
fn function_becomes_action_with_params() {
    let dir = write_fixture();
    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let report = run(&scan);

    // The top-level function became a suggested action with its parameters.
    let total = report
        .actions
        .iter()
        .find(|a| a.action_name == "compute_total")
        .expect("compute_total should become an action");
    assert_eq!(total.action_id, "compute_total");
    assert!(matches!(total.source, LinkSource::Suggested));
    let params: Vec<&str> = total.input.iter().map(|p| p.name.as_str()).collect();
    assert!(params.contains(&"items"), "params={params:?}");
    assert!(params.contains(&"tax"), "params={params:?}");
    // No defaults → required.
    assert!(total.input.iter().all(|p| p.required == Some(true)));

    // The public method became an action namespaced under its class.
    let activate = report
        .actions
        .iter()
        .find(|a| a.action_name == "activate")
        .expect("User.activate should become an action");
    assert_eq!(activate.action_id, "user_activate");
    assert!(activate.code_ref.contains("User.activate"));
}

#[test]
fn deterministic_serialized_output() {
    let dir = write_fixture();
    let scan = fdml_scan::run(dir.path(), &[]).unwrap();

    let a = run(&scan);
    let b = run(&scan);

    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb, "two linker runs over the same scan must serialize identically");
}

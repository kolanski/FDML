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
    // C alongside: a prototype before its definition (one function, not two) and a call
    fs::write(
        dir.path().join("phys.c"),
        "#include \"phys.h\"\n\
         static int clamp_speed(int v);\n\
         struct Body { int x; int vx; };\n\
         static int clamp_speed(int v) { return v > 100 ? 100 : v; }\n\
         void step(struct Body *b) { b->vx = clamp_speed(b->vx); b->x += b->vx; }\n",
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
    // identity is the location: `entity:<file>:<name>`, so a second `User` elsewhere is a different entity
    assert_eq!(user.entity_id, "entity:shop.py:User");
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
    assert_eq!(total.action_id, "action:shop.py:compute_total");
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
    assert_eq!(activate.action_id, "action:shop.py:User.activate");
    assert!(activate.code_ref.contains("User.activate"));
}

/// The invariant human-authored links stand on: same source + same scanner semantics
/// → same ids. Three full scan+graph runs, not two graph runs over one scan — the
/// scanner is part of the promise.
#[test]
fn three_scans_yield_identical_ids() {
    let dir = write_fixture();
    let runs: Vec<String> = (0..3)
        .map(|_| serde_json::to_string(&run(&fdml_scan::run(dir.path(), &[]).unwrap())).unwrap())
        .collect();
    assert_eq!(runs[0], runs[1], "run 1 and 2 must serialize identically");
    assert_eq!(runs[1], runs[2], "run 2 and 3 must serialize identically");

    let report = run(&fdml_scan::run(dir.path(), &[]).unwrap());
    // C is seen, and identity is the location
    assert!(report.entities.iter().any(|e| e.entity_id == "entity:phys.c:Body"), "C struct becomes an entity: {:?}",
        report.entities.iter().map(|e| &e.entity_id).collect::<Vec<_>>());
    assert!(report.actions.iter().any(|a| a.action_id == "action:phys.c:step"));
    // a prototype and its definition are one function, so one id
    assert_eq!(report.actions.iter().filter(|a| a.action_id == "action:phys.c:clamp_speed").count(), 1);
    // every id is unique — the whole point
    let mut ids: Vec<&str> = report.entities.iter().map(|e| e.entity_id.as_str())
        .chain(report.actions.iter().map(|a| a.action_id.as_str()))
        .chain(report.features.iter().map(|f| f.feature_id.as_str())).collect();
    let n = ids.len(); ids.sort(); ids.dedup();
    assert_eq!(n, ids.len(), "duplicate ids in the graph");
}

//! Cross-system detection tests. Fixtures come from real scans of temp dirs (the
//! contracts have no Default), so we exercise the actual scanner + linker too.

use super::{run, SystemBundle};
use fdml_types::System;
use std::fs;

/// A system bundle from a throwaway source tree: scan it, link it, label it.
fn bundle(id: &str, sys_type: &str, files: &[(&str, &str)]) -> SystemBundle {
    let tmp = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let p = tmp.path().join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }
    let scan = fdml_scan::run(tmp.path(), &[]).unwrap();
    let report = fdml_graph::run(&scan);
    let system = System {
        path: ".".into(),
        id: id.into(),
        name: id.into(),
        system_type: sys_type.into(),
        technology: String::new(),
        boundary_marker: String::new(),
    };
    (system, scan, report)
}

#[test]
fn detects_http_integration_and_infers_service_target() {
    let caller = bundle("web", "frontend", &[("client.ts", "import axios from 'axios';\nexport function go() { axios.get('/x'); }")]);
    let api = bundle("api", "service", &[("main.py", "def handler():\n    return 1\n")]);
    let links = run(&[caller, api]);

    let http: Vec<_> = links.integrations.iter().filter(|i| i.integration_type == "http").collect();
    assert!(!http.is_empty(), "expected an http integration, got {:?}", links.integrations);
    assert_eq!(http[0].from_system, "web");
    assert_eq!(http[0].to_system.as_deref(), Some("api"), "http caller should target the first service");
}

#[test]
fn detects_shared_entity_across_systems() {
    let a = bundle("svc_a", "service", &[("models.py", "class User:\n    def __init__(self):\n        self.id = 0\n        self.email = \"\"\n")]);
    let b = bundle("svc_b", "service", &[("models.py", "class User:\n    def __init__(self):\n        self.id = 0\n")]);
    let links = run(&[a, b]);

    let user: Vec<_> = links.shared_entities.iter().filter(|s| s.entity_name == "user").collect();
    assert_eq!(user.len(), 1, "expected one shared 'user', got {:?}", links.shared_entities);
    assert_eq!(user[0].systems.len(), 2, "user should appear in both systems");
}

#[test]
fn intra_system_duplicate_is_not_shared() {
    // Same entity name in two files of ONE system must NOT count as cross-system shared.
    let single = bundle(
        "svc",
        "service",
        &[("a.py", "class User:\n    pass\n"), ("b.py", "class User:\n    pass\n")],
    );
    let links = run(&[single]);
    assert!(
        !links.shared_entities.iter().any(|s| s.entity_name == "user"),
        "intra-system duplicate wrongly reported as shared: {:?}",
        links.shared_entities
    );
}

#[test]
fn deterministic_across_runs() {
    let s = vec![
        bundle("svc_a", "service", &[("m.py", "import redis\nclass Order:\n    pass\n")]),
        bundle("svc_b", "service", &[("m.py", "class Order:\n    pass\n")]),
    ];
    assert_eq!(run(&s), run(&s));
}

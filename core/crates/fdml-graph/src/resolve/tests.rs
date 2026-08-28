//! Resolver tests — drive real fixtures through `fdml-scan` so the ScanResult
//! contract is exercised end-to-end, then resolve imports into a DepGraph.

use super::{build_petgraph, resolve_imports};
use fdml_types::depgraph::DepEdgeKind;
use std::fs;

fn has_edge(dep: &fdml_types::depgraph::DepGraph, from: &str, to: &str) -> bool {
    dep.edges
        .iter()
        .any(|e| e.from == from && e.to == to && e.kind == DepEdgeKind::Import)
}

/// Python: `from .b import X` in a package `__init__.py` resolves to the sibling
/// module `pkg/b.py`; a stdlib `import os` resolves to nothing (counted external).
#[test]
fn python_relative_import_resolves_stdlib_does_not() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("pkg")).unwrap();
    fs::write(
        dir.path().join("pkg/__init__.py"),
        "from .b import X\nimport os\n",
    )
    .unwrap();
    fs::write(dir.path().join("pkg/b.py"), "class X:\n    pass\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let dep = resolve_imports(&scan, dir.path());

    assert!(
        has_edge(&dep, "pkg/__init__.py", "pkg/b.py"),
        "expected internal edge pkg/__init__.py -> pkg/b.py; edges={:?}",
        dep.edges
    );
    // `import os` is stdlib → counted external, never an edge.
    assert!(
        dep.edges.iter().all(|e| !e.to.contains("os")),
        "stdlib import os must not produce an edge; edges={:?}",
        dep.edges
    );
    assert!(dep.stats.external_imports >= 1, "import os should be counted external");
    assert_eq!(dep.stats.internal_imports, 1, "exactly one internal import (.b)");
    // Every scanned file is a node.
    assert!(dep.nodes.contains(&"pkg/__init__.py".to_string()));
    assert!(dep.nodes.contains(&"pkg/b.py".to_string()));
}

/// TS: `import { X } from './b'` resolves to `web/b.ts` via the extension probe.
#[test]
fn typescript_relative_import_resolves() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("web")).unwrap();
    fs::write(dir.path().join("web/a.ts"), "import { X } from './b';\n").unwrap();
    fs::write(dir.path().join("web/b.ts"), "export class X {}\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let dep = resolve_imports(&scan, dir.path());

    assert!(
        has_edge(&dep, "web/a.ts", "web/b.ts"),
        "expected internal edge web/a.ts -> web/b.ts; edges={:?}",
        dep.edges
    );
    assert_eq!(dep.stats.internal_imports, 1);
}

/// TS NodeNext: an explicit `./b.js` specifier resolves to the `./b.ts` source.
#[test]
fn typescript_nodenext_js_specifier_resolves_to_ts() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("web")).unwrap();
    fs::write(dir.path().join("web/a.ts"), "import { X } from './b.js';\n").unwrap();
    fs::write(dir.path().join("web/b.ts"), "export class X {}\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let dep = resolve_imports(&scan, dir.path());

    assert!(
        has_edge(&dep, "web/a.ts", "web/b.ts"),
        "NodeNext: ./b.js should resolve to web/b.ts; edges={:?}",
        dep.edges
    );
}

/// Determinism: resolving the same scan twice serializes byte-identically.
#[test]
fn deterministic_serialized_depgraph() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("pkg")).unwrap();
    fs::write(
        dir.path().join("pkg/__init__.py"),
        "from .b import X\nfrom .c import Y\nimport os\n",
    )
    .unwrap();
    fs::write(dir.path().join("pkg/b.py"), "class X:\n    pass\n").unwrap();
    fs::write(dir.path().join("pkg/c.py"), "class Y:\n    pass\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let a = resolve_imports(&scan, dir.path());
    let b = resolve_imports(&scan, dir.path());

    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb, "two resolver runs over the same scan must serialize identically");

    // Edges must be sorted by (from, to).
    let mut sorted = a.edges.clone();
    sorted.sort();
    assert_eq!(a.edges, sorted, "edges must be emitted in sorted order");
}

/// Go: `import "example.com/app/util"` resolves (via the nearest go.mod, module
/// prefix strip) to every `.go` file in the package dir.
#[test]
fn go_module_prefix_import_resolves_to_package_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("go.mod"), "module example.com/app\n\ngo 1.21\n").unwrap();
    fs::write(
        dir.path().join("main.go"),
        "package main\n\nimport \"example.com/app/util\"\n\nfunc main() { _ = util.Name }\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("util")).unwrap();
    fs::write(dir.path().join("util/helper.go"), "package util\n\nvar Name = \"x\"\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let dep = resolve_imports(&scan, dir.path());

    assert!(
        has_edge(&dep, "main.go", "util/helper.go"),
        "Go module-prefix import should resolve to util/helper.go; edges={:?}",
        dep.edges
    );
}

/// Java: `import com.example.Helper;` resolves via the dir-bounded suffix index,
/// even when sources are nested under `src/main/java/`.
#[test]
fn java_fqn_resolves_via_suffix_index() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path().join("src/main/java/com/example");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("App.java"),
        "package com.example;\n\nimport com.example.Helper;\n\npublic class App {}\n",
    )
    .unwrap();
    fs::write(pkg.join("Helper.java"), "package com.example;\n\npublic class Helper {}\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let dep = resolve_imports(&scan, dir.path());

    assert!(
        has_edge(
            &dep,
            "src/main/java/com/example/App.java",
            "src/main/java/com/example/Helper.java",
        ),
        "Java FQN should resolve through the suffix index; edges={:?}",
        dep.edges
    );
}

/// The petgraph build mirrors the DepGraph: every node present, every edge wired.
#[test]
fn petgraph_build_matches_depgraph() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("pkg")).unwrap();
    fs::write(dir.path().join("pkg/__init__.py"), "from .b import X\n").unwrap();
    fs::write(dir.path().join("pkg/b.py"), "class X:\n    pass\n").unwrap();

    let scan = fdml_scan::run(dir.path(), &[]).unwrap();
    let dep = resolve_imports(&scan, dir.path());
    let (graph, index) = build_petgraph(&dep);

    assert_eq!(graph.node_count(), dep.nodes.len());
    assert_eq!(graph.edge_count(), dep.edges.len());
    let a = index["pkg/__init__.py"];
    let b = index["pkg/b.py"];
    assert!(graph.find_edge(a, b).is_some(), "petgraph should carry __init__ -> b edge");
}

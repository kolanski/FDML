//! Flow-reconstruction tests. The perf test is the point of the rewrite: a dense
//! (maximally cyclic) graph must stay bounded where the old all-paths BFS exploded.

use super::run;
use fdml_types::depgraph::{DepEdge, DepEdgeKind, DepGraph};
use fdml_types::graph::{ActionLink, LinkReport, LinkSource};
use fdml_types::scan::ScanResult;

/// Empty LinkReport + ScanResult from scanning an empty dir — avoids hand-building the
/// whole (Default-less) contract tree; tests then inject just the actions they need.
fn empty() -> (LinkReport, ScanResult) {
    let tmp = tempfile::tempdir().unwrap();
    let scan = fdml_scan::run(tmp.path(), &[]).unwrap();
    let report = fdml_graph::run(&scan);
    (report, scan)
}

fn dep(nodes: &[&str], edges: &[(&str, &str)]) -> DepGraph {
    DepGraph {
        nodes: nodes.iter().map(|s| s.to_string()).collect(),
        edges: edges
            .iter()
            .map(|(f, t)| DepEdge { from: f.to_string(), to: t.to_string(), kind: DepEdgeKind::Import })
            .collect(),
        ..Default::default()
    }
}

fn action(id: &str, name: &str, code_ref: &str) -> ActionLink {
    ActionLink {
        action_id: id.into(),
        action_name: name.into(),
        code_ref: code_ref.into(),
        confidence: 0.0,
        source: LinkSource::Suggested,
        input: vec![],
        output: None,
        description: None,
    }
}

#[test]
fn linear_ingress_to_sink() {
    // a (in-deg 0) → b → c (out-deg 0): topological ingress=a, sink=c, one path.
    let (report, scan) = empty();
    let d = dep(&["a.ts", "b.ts", "c.ts"], &[("a.ts", "b.ts"), ("b.ts", "c.ts")]);
    let flows = run(&report, &d, &scan);
    assert_eq!(flows.len(), 1, "expected one a->c flow, got {flows:?}");
    let files: Vec<&str> = flows[0].steps.iter().map(|s| s.file.as_str()).collect();
    assert_eq!(files, vec!["a.ts", "b.ts", "c.ts"]);
}

#[test]
fn deterministic_across_runs() {
    let (report, scan) = empty();
    let d = dep(&["a.ts", "b.ts", "c.ts"], &[("a.ts", "b.ts"), ("b.ts", "c.ts")]);
    assert_eq!(run(&report, &d, &scan), run(&report, &d, &scan));
}

#[test]
fn perf_dense_clique_is_bounded() {
    // 200-node clique: maximally cyclic, factorially many simple paths. The old
    // `bfs_paths` (all simple paths to depth 8) would explode here; the rewrite takes
    // ONE shortest path per ingress → must finish well under 50 ms.
    let n = 200usize;
    let nodes: Vec<String> = (0..n).map(|i| format!("f{i:03}.ts")).collect();
    let mut edges = Vec::with_capacity(n * (n - 1));
    for i in 0..n {
        for j in 0..n {
            if i != j {
                edges.push(DepEdge { from: nodes[i].clone(), to: nodes[j].clone(), kind: DepEdgeKind::Import });
            }
        }
    }
    let d = DepGraph { nodes: nodes.clone(), edges, ..Default::default() };

    // A clique has no degree-0 nodes, so name an ingress/sink action to force a flow.
    let (mut report, scan) = empty();
    report.actions = vec![
        action("ing", "handleRequest", &format!("{}:handleRequest", nodes[0])),
        action("snk", "saveRecord", &format!("{}:saveRecord", nodes[n - 1])),
    ];

    let t = std::time::Instant::now();
    let flows = run(&report, &d, &scan);
    let ms = t.elapsed().as_millis();
    assert!(ms < 50, "200-clique flows took {ms}ms — must be <50ms (exponential regression?)");
    assert!(!flows.is_empty(), "expected at least one flow on the clique");
}

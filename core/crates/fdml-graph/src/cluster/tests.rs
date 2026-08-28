//! Clustering tests — drive synthetic `DepGraph`s with known topology so the
//! community detection + grouping is checked precisely (no scanner in the loop).

use super::cluster;
use fdml_types::depgraph::{DepEdge, DepEdgeKind, DepGraph};
use std::collections::BTreeSet;

/// Build a `DepGraph` from node names + directed `(from, to)` pairs (deduped/sorted
/// like the resolver emits).
fn graph(nodes: &[&str], edges: &[(&str, &str)]) -> DepGraph {
    let mut set: BTreeSet<DepEdge> = BTreeSet::new();
    for &(from, to) in edges {
        set.insert(DepEdge { from: from.into(), to: to.into(), kind: DepEdgeKind::Import });
    }
    let mut ns: Vec<String> = nodes.iter().map(|s| s.to_string()).collect();
    ns.sort();
    DepGraph { nodes: ns, edges: set.into_iter().collect(), ..Default::default() }
}

/// A bidirectional (weight-2) intra edge.
fn pair<'a>(a: &'a str, b: &'a str) -> [(&'a str, &'a str); 2] {
    [(a, b), (b, a)]
}

/// Two dense triangles a1↔a2↔a3 and b1↔b2↔b3 joined by a single bridge a3→b1 must
/// split into exactly two communities with the right members.
#[test]
fn two_communities_split_correctly() {
    let nodes = ["a/a1.py", "a/a2.py", "a/a3.py", "b/b1.py", "b/b2.py", "b/b3.py"];
    let mut edges = Vec::new();
    for e in pair("a/a1.py", "a/a2.py") { edges.push(e); }
    for e in pair("a/a2.py", "a/a3.py") { edges.push(e); }
    for e in pair("a/a1.py", "a/a3.py") { edges.push(e); }
    for e in pair("b/b1.py", "b/b2.py") { edges.push(e); }
    for e in pair("b/b2.py", "b/b3.py") { edges.push(e); }
    for e in pair("b/b1.py", "b/b3.py") { edges.push(e); }
    edges.push(("a/a3.py", "b/b1.py")); // single weak bridge

    let dep = graph(&nodes, &edges);
    let cl = cluster(&dep);

    assert_eq!(cl.clusters.len(), 2, "expected two communities, got {:?}", cl.clusters);

    let mut got: Vec<Vec<String>> = cl.clusters.iter().map(|c| c.members.clone()).collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            vec!["a/a1.py".to_string(), "a/a2.py".into(), "a/a3.py".into()],
            vec!["b/b1.py".to_string(), "b/b2.py".into(), "b/b3.py".into()],
        ],
        "members did not split along the triangle boundary"
    );

    // Naming derives from the dominant directory: "A" and "B".
    let names: BTreeSet<&str> = cl.clusters.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, BTreeSet::from(["A", "B"]), "names from dominant dir; got {names:?}");
}

/// Every node appears in exactly one cluster (100% coverage, no duplicates).
#[test]
fn full_coverage_partition() {
    // A larger, multi-directory graph that forces super-clustering down to the band.
    let mut nodes = Vec::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    // 6 dense components of 5 files each = 30 files; plus 5 isolated orphans = 35.
    for c in 0..6 {
        let files: Vec<String> = (0..5).map(|i| format!("mod{c}/f{i}.ts")).collect();
        for f in &files { nodes.push(f.clone()); }
        for i in 0..files.len() {
            for j in (i + 1)..files.len() {
                edges.push((files[i].clone(), files[j].clone()));
                edges.push((files[j].clone(), files[i].clone()));
            }
        }
    }
    for o in 0..5 { nodes.push(format!("orphans/o{o}.ts")); } // isolated, no edges

    let node_refs: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let edge_refs: Vec<(&str, &str)> =
        edges.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let dep = graph(&node_refs, &edge_refs);

    let cl = cluster(&dep);

    // Bounded count.
    assert!(cl.clusters.len() >= 5 && cl.clusters.len() <= 8, "count={}", cl.clusters.len());

    // Exactly-once coverage.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut total = 0usize;
    for c in &cl.clusters {
        for m in &c.members {
            assert!(seen.insert(m.clone()), "file {m} appeared in two clusters");
            total += 1;
        }
        assert_eq!(c.size, c.members.len(), "size field must match members");
    }
    assert_eq!(total, dep.nodes.len(), "every file covered exactly once");
    assert_eq!(seen, dep.nodes.iter().cloned().collect::<BTreeSet<_>>());
}

/// Clustering the same graph twice is byte-identical when serialized.
#[test]
fn deterministic_serialized_clusters() {
    let nodes = ["a/a1.py", "a/a2.py", "a/a3.py", "b/b1.py", "b/b2.py", "b/b3.py"];
    let mut edges = Vec::new();
    for e in pair("a/a1.py", "a/a2.py") { edges.push(e); }
    for e in pair("a/a2.py", "a/a3.py") { edges.push(e); }
    for e in pair("b/b1.py", "b/b2.py") { edges.push(e); }
    for e in pair("b/b2.py", "b/b3.py") { edges.push(e); }
    edges.push(("a/a3.py", "b/b1.py"));

    let dep = graph(&nodes, &edges);
    let a = cluster(&dep);
    let b = cluster(&dep);

    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb, "two clustering runs must serialize identically");
}

/// A tiny graph (one or two files) yields a single cluster and never panics.
#[test]
fn tiny_graph_one_cluster() {
    // One file, no edges.
    let dep1 = graph(&["solo.py"], &[]);
    let c1 = cluster(&dep1);
    assert_eq!(c1.clusters.len(), 1);
    assert_eq!(c1.clusters[0].members, vec!["solo.py".to_string()]);

    // Two files, no edge → singletons absorbed into one cluster.
    let dep2 = graph(&["x.py", "y.py"], &[]);
    let c2 = cluster(&dep2);
    assert_eq!(c2.clusters.len(), 1, "two edgeless files collapse to one cluster");
    assert_eq!(c2.clusters[0].size, 2);

    // Two files, one edge → one community.
    let dep3 = graph(&["x.py", "y.py"], &[("x.py", "y.py")]);
    let c3 = cluster(&dep3);
    assert_eq!(c3.clusters.len(), 1);
}

/// An empty graph produces no clusters and does not panic.
#[test]
fn empty_graph_no_panic() {
    let dep = DepGraph::default();
    let cl = cluster(&dep);
    assert!(cl.clusters.is_empty());
}

/// Two clusters whose DOMINANT directory is the same populous `components/ui` must NOT
/// collide into "Ui" / "Ui (2)": each is named by its own DISTINCTIVE subdir (the dir
/// unique to it), so they come out "Layout" and "Nav". Kills the old numeric-suffix bug.
#[test]
fn same_dominant_dir_gets_distinct_distinctive_names() {
    let nodes = [
        "components/ui/a1.tsx", "components/ui/a2.tsx", "components/ui/a3.tsx", "components/layout/a4.tsx",
        "components/ui/b1.tsx", "components/ui/b2.tsx", "components/ui/b3.tsx", "components/nav/b4.tsx",
    ];
    let mut edges = Vec::new();
    // Cluster A: dense ui triangle + a layout file pulled in.
    for e in pair("components/ui/a1.tsx", "components/ui/a2.tsx") { edges.push(e); }
    for e in pair("components/ui/a2.tsx", "components/ui/a3.tsx") { edges.push(e); }
    for e in pair("components/ui/a1.tsx", "components/ui/a3.tsx") { edges.push(e); }
    for e in pair("components/layout/a4.tsx", "components/ui/a1.tsx") { edges.push(e); }
    for e in pair("components/layout/a4.tsx", "components/ui/a3.tsx") { edges.push(e); }
    // Cluster B: dense ui triangle + a nav file pulled in.
    for e in pair("components/ui/b1.tsx", "components/ui/b2.tsx") { edges.push(e); }
    for e in pair("components/ui/b2.tsx", "components/ui/b3.tsx") { edges.push(e); }
    for e in pair("components/ui/b1.tsx", "components/ui/b3.tsx") { edges.push(e); }
    for e in pair("components/nav/b4.tsx", "components/ui/b1.tsx") { edges.push(e); }
    for e in pair("components/nav/b4.tsx", "components/ui/b3.tsx") { edges.push(e); }
    edges.push(("components/ui/a3.tsx", "components/ui/b1.tsx")); // single weak bridge

    let dep = graph(&nodes, &edges);
    let cl = cluster(&dep);

    assert_eq!(cl.clusters.len(), 2, "expected two clusters, got {:?}", cl.clusters);
    let names: BTreeSet<&str> = cl.clusters.iter().map(|c| c.name.as_str()).collect();
    // Shared `components/ui` (in every cluster → idf 0) is rejected; the unique subdir wins.
    assert_eq!(names, BTreeSet::from(["Layout", "Nav"]), "distinctive names; got {names:?}");
    // Belt-and-suspenders: no numeric-suffixed duplicate label.
    assert!(cl.clusters.iter().all(|c| !c.name.contains('(')), "no '(2)' suffixes: {names:?}");
}

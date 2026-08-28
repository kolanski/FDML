//! Pass 4 — deterministic data-flow reconstruction.
//!
//! Reconstructs ingress → sink data-flow paths over the REAL resolved cross-file
//! dependency graph (`DepGraph`, phase 3B.1), replacing FDML's old exponential flow
//! engine (`src/linker/flows.rs`).
//!
//! ## What is REUSED from the old engine (FDML semantics, unchanged)
//!   * **ingress heuristics**: handler / route / endpoint / controller / view /
//!     subscribe / listener / consumer / webhook / `on_*` / `get_*` / `post_*` … names,
//!     plus `router` / `api/` / `routes/` / `endpoints/` / `views/` / `controllers/`
//!     file paths, plus `main` / `app` / `run` entry names;
//!   * **sink heuristics**: save / write / insert / update / delete / create / publish /
//!     send / emit / notify / cache / store / persist / render / respond names;
//!   * the FDML `flows:` output SHAPE (`id`, `name`, `steps{action, description}`),
//!     emitted by `fdml-assemble` (the old `flows_to_yaml` format).
//!
//! ## What is REPLACED (the engine)
//!   * the param-type co-occurrence "call graph" → the real resolved `DepGraph`;
//!   * the all-simple-paths BFS (every path to depth 8, clone-visited per branch,
//!     truncate to 10 *after* — exponential) → exactly ONE shortest ingress→sink path
//!     per ingress file via a bounded BFS (depth ≤ 10, stop at the first sink). The
//!     whole pass is O(V + E) per ingress file, never enumerating alternative paths.
//!
//! ## Algorithm (deterministic, bounded)
//!   1. Map each action to its file (`code_ref` before the first `:`).
//!   2. **Ingress files** = files with an ingress-heuristic action, OR in-degree-0 entry
//!      files. **Sink files** = files with a sink-heuristic action, OR out-degree-0 leaf
//!      files.
//!   3. For each ingress file (in sorted node order) run a bounded BFS to the nearest
//!      reachable sink file; take that single shortest path.
//!   4. The flow's steps are the representative ACTIONS along the path (ingress action
//!      first, a representative action per intermediate file, the sink action last; a
//!      path-file with no action becomes a file/module description step).
//!   5. Dedup flows sharing the same (ingress file, sink file) — keep one. Bounded.
//!
//! ## Determinism
//!   * `DepGraph` arrives with `nodes` sorted and `edges` sorted+deduped; the petgraph
//!     node index equals the position in the sorted `nodes`, so "ascending index" is
//!     "sorted path". BFS visits successors in ascending-index order, making the nearest
//!     sink and the chosen path reproducible.
//!   * flows are sorted by (ingress file, sink file) and assigned `flow_0..` after a
//!     bounded (ingress, sink)-key dedup. No RNG, no wall-clock, no HashMap iteration
//!     order leaks. Two runs are byte-identical.
//!
//! ## Termination / cycles
//!   * a Kahn (BFS) topological pass detects cycles up-front; the per-ingress BFS is in
//!     any case bounded by a visited set AND the depth cap, so a cyclic graph (e.g.
//!     `a → b → a`) terminates with a finite, bounded result regardless.
//!
//! Note: `build_petgraph` here is a deliberate ~15-line DUPLICATE of
//! `fdml_graph::resolve::build_petgraph`. `fdml-flows` is a LEAF pass and must not
//! depend on another pass (the workspace's "no pass depends on another pass" rule), so
//! it carries its own copy rather than importing `fdml-graph`.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use fdml_types::depgraph::DepGraph;
use fdml_types::flow::{Flow, FlowStep};
use fdml_types::graph::{ActionLink, LinkReport};
use fdml_types::scan::ScanResult;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

/// Hard depth cap on a single ingress→sink BFS path (in edges). Bounds traversal even
/// before the visited set would; mirrors the spirit of the old depth-8 limit.
const MAX_DEPTH: usize = 10;

/// Reconstruct deterministic ingress → sink data flows from a [`LinkReport`]'s actions
/// laid over a resolved [`DepGraph`]. `scan` provides per-file module names used for the
/// description of pass-through files that carry no extracted action.
pub fn run(report: &LinkReport, dep: &DepGraph, scan: &ScanResult) -> Vec<Flow> {
    let n = dep.nodes.len();
    if n == 0 {
        return Vec::new();
    }

    // Real dependency graph. Node index == position in the sorted `dep.nodes`.
    let (graph, index) = build_petgraph(dep);

    // Sorted successor adjacency + in/out degree, derived ONCE from the petgraph so each
    // ingress BFS is O(V+E) and never re-sorts neighbors per visit.
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_deg = vec![0usize; n];
    let mut out_deg = vec![0usize; n];
    for i in 0..n {
        let mut nbrs: Vec<usize> = graph
            .neighbors_directed(NodeIndex::new(i), Direction::Outgoing)
            .map(|x| x.index())
            .collect();
        nbrs.sort_unstable();
        nbrs.dedup();
        out_deg[i] = nbrs.len();
        for &b in &nbrs {
            in_deg[b] += 1;
        }
        succ[i] = nbrs;
    }

    // Cycle detection (Kahn). The BFS below is bounded by `visited` + `MAX_DEPTH`, so a
    // cyclic graph still produces a finite result; this only documents termination (the
    // cycle test asserts no hang) and never alters the output.
    let (_topo_order, _has_cycle) = kahn(n, &succ, &in_deg);

    // Actions grouped by their (POSIX) file, sorted by id for a stable representative pick.
    let mut by_file: BTreeMap<String, Vec<&ActionLink>> = BTreeMap::new();
    for a in &report.actions {
        let f = to_posix(file_of(&a.code_ref));
        by_file.entry(f).or_default().push(a);
    }
    for v in by_file.values_mut() {
        v.sort_by(|x, y| x.action_id.cmp(&y.action_id).then_with(|| x.code_ref.cmp(&y.code_ref)));
    }

    // Per-file module name (from the scan) — the description for a no-action pass-through.
    let mut module_of: BTreeMap<String, String> = BTreeMap::new();
    for f in &scan.files {
        if !f.module_path.is_empty() {
            module_of.insert(to_posix(&f.file_path), f.module_path.clone());
        }
    }

    // ── Ingress / sink classification (file level) ──
    let mut ingress: BTreeSet<usize> = BTreeSet::new();
    let mut sink: BTreeSet<usize> = BTreeSet::new();

    // Heuristic signal: a file holding an ingress/sink action (only files that are graph nodes).
    for (file, acts) in &by_file {
        let Some(&idx) = index.get(file) else { continue };
        let i = idx.index();
        if acts.iter().any(|a| is_ingress_action(&a.action_name, &a.code_ref)) {
            ingress.insert(i);
        }
        if acts.iter().any(|a| is_sink_action(&a.action_name)) {
            sink.insert(i);
        }
    }
    // Topological signal: in-degree-0 entry → ingress; out-degree-0 leaf → sink. Isolated
    // nodes (no edges) are excluded — they cannot form a path.
    for i in 0..n {
        if in_deg[i] == 0 && out_deg[i] > 0 {
            ingress.insert(i);
        }
        if out_deg[i] == 0 && in_deg[i] > 0 {
            sink.insert(i);
        }
    }

    // ── One representative path per ingress file ──
    let mut raw: Vec<(usize, usize, Vec<usize>)> = Vec::new(); // (ingress, sink, path)
    for &start in &ingress {
        if let Some(path) = bfs_to_sink(start, &succ, &sink, MAX_DEPTH) {
            let end = *path.last().expect("bfs path is non-empty");
            raw.push((start, end, path));
        }
    }

    // Dedup: one flow per (ingress, sink) pair — a bounded BTreeSet of index pairs, NOT
    // an O(n²) fuzzy step comparison.
    let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
    raw.retain(|(s, e, _)| seen.insert((*s, *e)));

    // Deterministic order: by (ingress, sink) node index (== sorted node path).
    raw.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    // ── Assemble Flow structs ──
    let mut out = Vec::with_capacity(raw.len());
    for (fi, (start, _end, path)) in raw.iter().enumerate() {
        let last = path.len() - 1;
        let steps: Vec<FlowStep> = path
            .iter()
            .enumerate()
            .map(|(pos, &node)| {
                let role = if pos == 0 {
                    Role::Ingress
                } else if pos == last {
                    Role::Sink
                } else {
                    Role::Mid
                };
                build_step(&dep.nodes[node], &by_file, &module_of, role)
            })
            .collect();

        let ingress_name = steps.first().map(|s| s.action_name.as_str()).unwrap_or("");
        let name = if ingress_name.is_empty() {
            format!("{} Flow", title_case(file_stem(&dep.nodes[*start])))
        } else {
            format!("{} Flow", title_case(ingress_name))
        };

        out.push(Flow { id: format!("flow_{fi}"), name, steps });
    }
    out
}

// ───────────────────────────────────────────────── representative-step pick ──

/// A file's position on a path determines which action represents it.
enum Role {
    Ingress,
    Mid,
    Sink,
}

/// Build the step for one file on the path: pick its representative action (the
/// ingress/sink-heuristic action for the endpoints, else the first action by id), or
/// fall back to the file/module as a description when the file carries no action.
fn build_step(
    file: &str,
    by_file: &BTreeMap<String, Vec<&ActionLink>>,
    module_of: &BTreeMap<String, String>,
    role: Role,
) -> FlowStep {
    let chosen = by_file.get(file).and_then(|acts| {
        let matched = acts.iter().copied().find(|a| match role {
            Role::Ingress => is_ingress_action(&a.action_name, &a.code_ref),
            Role::Sink => is_sink_action(&a.action_name),
            Role::Mid => false,
        });
        matched.or_else(|| acts.first().copied())
    });

    match chosen {
        Some(a) => FlowStep {
            action_id: a.action_id.clone(),
            action_name: a.action_name.clone(),
            file: file.to_string(),
            description: format!("{} ({})", a.action_name, a.code_ref),
        },
        None => FlowStep {
            action_id: String::new(),
            action_name: file_stem(file).to_string(),
            file: file.to_string(),
            description: module_of.get(file).cloned().unwrap_or_else(|| file.to_string()),
        },
    }
}

// ─────────────────────────────────────────────────────── ingress/sink rules ──

/// Ingress-point heuristic (ported from FDML `find_ingress_points`): an action whose
/// name or code-ref file marks it as an entry point (route/handler/listener/entry).
fn is_ingress_action(name: &str, code_ref: &str) -> bool {
    let n = name.to_lowercase();
    let c = code_ref.to_lowercase();
    n.contains("handle")
        || n.contains("endpoint")
        || n.contains("route")
        || n.contains("controller")
        || n.contains("view")
        || n.starts_with("on_")
        || n.starts_with("post_")
        || n.starts_with("get_")
        || n.starts_with("put_")
        || n.starts_with("delete_")
        || n.starts_with("patch_")
        || n.contains("subscribe")
        || n.contains("listener")
        || n.contains("consumer")
        || n.contains("receive")
        || n.contains("ingest")
        || n.contains("webhook")
        || c.contains("router")
        || c.contains("api/")
        || c.contains("routes/")
        || c.contains("endpoints/")
        || c.contains("views/")
        || c.contains("controllers/")
        || n == "main"
        || n == "app"
        || n == "run"
}

/// Sink-point heuristic (ported from FDML `find_sink_points`): an action whose name
/// marks a data write / response / publish.
fn is_sink_action(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("save")
        || n.contains("write")
        || n.contains("insert")
        || n.contains("update")
        || n.contains("delete")
        || n.contains("create")
        || n.contains("publish")
        || n.contains("send")
        || n.contains("emit")
        || n.contains("notify")
        || n.contains("cache")
        || n.contains("store")
        || n.contains("persist")
        || n.contains("render")
        || n.contains("respond")
}

// ───────────────────────────────────────────────────────────── graph helpers ──

/// DUPLICATED (deliberately) from `fdml_graph::resolve::build_petgraph` to keep
/// `fdml-flows` a LEAF pass. Nodes are added in `dep.nodes` order, so a node's
/// `NodeIndex` equals its position in the sorted `nodes`; the map gives each path's index.
fn build_petgraph(dep: &DepGraph) -> (DiGraph<String, ()>, HashMap<String, NodeIndex>) {
    let mut graph = DiGraph::<String, ()>::new();
    let mut index: HashMap<String, NodeIndex> = HashMap::with_capacity(dep.nodes.len());
    for node in &dep.nodes {
        let idx = graph.add_node(node.clone());
        index.insert(node.clone(), idx);
    }
    for edge in &dep.edges {
        if let (Some(&a), Some(&b)) = (index.get(&edge.from), index.get(&edge.to)) {
            graph.add_edge(a, b, ());
        }
    }
    (graph, index)
}

/// Kahn topological order over the successor adjacency. Returns `(order, has_cycle)`
/// where `has_cycle == (order.len() < n)`. Pure helper; deterministic (sources and the
/// queue are processed in ascending index order).
fn kahn(n: usize, succ: &[Vec<usize>], in_deg0: &[usize]) -> (Vec<usize>, bool) {
    let mut in_deg = in_deg0.to_vec();
    let mut q: VecDeque<usize> = (0..n).filter(|&i| in_deg[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(u) = q.pop_front() {
        order.push(u);
        for &v in &succ[u] {
            in_deg[v] -= 1;
            if in_deg[v] == 0 {
                q.push_back(v);
            }
        }
    }
    let has_cycle = order.len() < n;
    (order, has_cycle)
}

/// Bounded BFS shortest path from `start` to the nearest sink at distance ≥ 1, within
/// `max_depth` edges. Successors are visited in ascending (sorted-path) order, so the
/// chosen sink and path are deterministic. Returns the node-index path `[start, …, sink]`
/// or `None` if no sink is reachable within the cap. O(V + E), never enumerates paths.
fn bfs_to_sink(
    start: usize,
    succ: &[Vec<usize>],
    sinks: &BTreeSet<usize>,
    max_depth: usize,
) -> Option<Vec<usize>> {
    let n = succ.len();
    let mut visited = vec![false; n];
    let mut parent = vec![usize::MAX; n];
    let mut depth = vec![0usize; n];
    let mut q: VecDeque<usize> = VecDeque::new();
    visited[start] = true;
    q.push_back(start);

    while let Some(u) = q.pop_front() {
        // First sink popped at distance ≥ 1 is the nearest (BFS) — reconstruct + return.
        if u != start && sinks.contains(&u) {
            let mut path = vec![u];
            let mut cur = u;
            while cur != start {
                cur = parent[cur];
                path.push(cur);
            }
            path.reverse();
            return Some(path);
        }
        if depth[u] >= max_depth {
            continue; // depth cap — do not expand further
        }
        for &v in &succ[u] {
            if !visited[v] {
                visited[v] = true;
                parent[v] = u;
                depth[v] = depth[u] + 1;
                q.push_back(v);
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────── string helpers ──

/// File portion of a `code_ref` (`"file:Class.method"` → `"file"`).
fn file_of(code_ref: &str) -> &str {
    code_ref.split(':').next().unwrap_or(code_ref)
}

/// Normalize a path to forward-slash POSIX, dropping empty segments (matches the
/// resolver's `to_posix` so action files line up with `DepGraph` node paths).
fn to_posix(p: &str) -> String {
    p.split(['\\', '/']).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/")
}

/// File stem of a path: basename minus a single trailing extension.
fn file_stem(path: &str) -> &str {
    let base = path.rsplit('/').next().unwrap_or(path);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    }
}

/// Capitalize the first character (ASCII title-case), leaving the rest unchanged.
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests;

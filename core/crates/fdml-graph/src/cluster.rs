//! Phase 3B.2 — cluster the [`DepGraph`] into a readable map of named "boxes".
//!
//! Two ported ideas combine here:
//!
//!   1. **Louvain community detection** (ported from Understand-Anything's
//!      `compute-batches.mjs` `runLouvain`, which itself calls
//!      `graphology-communities-louvain`). We port the *modularity-optimizing logic*
//!      — local-moving + aggregation — and make it DETERMINISTIC (see below).
//!   2. **CodeBoarding's readable-map grouping** (from `research/repo-studies/
//!      02-codeboarding.md`): super-cluster a too-granular partition down to ~5–8
//!      top boxes via a cluster meta-graph + small-community absorption, and a 100%
//!      node-coverage orphan cascade.
//!
//! ## How determinism was achieved (the source's nondeterminism, replaced)
//! `graphology-communities-louvain` (and Louvain/Leiden in general) is
//! NONDETERMINISTIC for two reasons, both of which we remove:
//!   * **Random node-visit order** — graphology defaults `randomWalk: true`, shuffling
//!     the order nodes are considered for moves. We replace this with a FIXED order:
//!     ascending node index, where index = position in the already-sorted `dep.nodes`.
//!   * **Hash/insertion-order iteration** of candidate neighbor communities (which
//!     community a node is tested against first changes tie outcomes). We collect the
//!     neighbor community ids, **sort** them, and only adopt a strictly-better move
//!     (epsilon-guarded); equal-gain ties keep the current community. So the argmax is
//!     reproducible regardless of map iteration order.
//! No RNG, no wall-clock, no `HashMap` iteration leaks into the result. Aggregated
//! graphs renumber communities by sorted old-id. Two runs are byte-identical.
//!
//! We run a **weighted** modularity (resolution 1): the undirected edge weight between
//! two files = the count of directed `DepGraph` edges between them in EITHER direction
//! (the task's specified weighting; graphology defaults to unweighted, we improve on it
//! deterministically).

use std::collections::{BTreeMap, BTreeSet};

use fdml_types::cluster::{Cluster, ClusterKind, Clusters};
use fdml_types::depgraph::DepGraph;

// ───────────────────────────────── tunables (documented constants) ──

/// A system with at most this many files is "small": we do NOT force it up into the
/// 5–8 band — we keep whatever communities Louvain finds (only absorbing singletons).
const SMALL_GRAPH_FILES: usize = 8;
/// Lower bound of the readable top-box band for large systems.
const TOP_MIN: usize = 5;
/// Upper bound of the readable top-box band for large systems.
const TOP_MAX: usize = 8;
/// A cluster smaller than this is "tiny" and gets absorbed into a neighbor even when
/// the cluster count is already within budget (cleans up Louvain singletons / isolated
/// nodes). Mirrors CodeBoarding's `DEFAULT_MIN_CLUSTER_SIZE = 2`.
const MIN_CLUSTER_SIZE: usize = 2;
/// Float tie-break epsilon for modularity-gain comparisons.
const EPS: f64 = 1e-12;

// ───────────────────────────────────────────────── public surface ──

/// Cluster a [`DepGraph`] into a bounded, fully-covering readable map.
///
/// Pipeline: undirected weighted graph → deterministic Louvain → CodeBoarding
/// super-cluster/absorb down to a readable count → deterministic naming + ordering.
/// Guarantees: every `dep.nodes` entry appears in exactly one cluster (100% coverage);
/// the cluster count is bounded (~5–8 for large systems, fewer for small); the output
/// is byte-identical across runs.
pub fn cluster(dep: &DepGraph) -> Clusters {
    let n = dep.nodes.len();
    if n == 0 {
        return Clusters::default();
    }

    // 1. Undirected weighted adjacency over node *indices* (= position in the sorted
    //    `dep.nodes`; this is exactly `build_petgraph`'s node indexing). We aggregate
    //    counts ourselves because petgraph's `DiGraph<String,()>` carries no weights.
    let adj = build_undirected(dep);

    // 2. Deterministic Louvain → community label per node.
    let community_of = louvain(n, &adj);

    // 3. Group node indices by community (deterministic: sorted community ids).
    let mut by_comm: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (node, &c) in community_of.iter().enumerate() {
        by_comm.entry(c).or_default().push(node);
    }
    let mut clusters: Vec<Vec<usize>> = by_comm.into_values().collect();

    // 4. CodeBoarding grouping: super-cluster / absorb down to a readable count, with
    //    the orphan cascade (graph-distance → file co-location → fallback bucket) baked
    //    into the choice of merge target. 100% coverage is preserved (merging only
    //    moves members between clusters, never drops them).
    let target = target_clusters(n);
    absorb(&mut clusters, target, dep, &adj);

    // 5. Deterministic order (size desc, then smallest member path asc), name, assemble.
    finalize(clusters, dep)
}

// ─────────────────────────────────────────── graph construction ──

/// Per-node weighted adjacency (including self-loops once aggregation introduces them).
type Adj = Vec<Vec<(usize, f64)>>;

/// Build the undirected weighted adjacency from the directed [`DepGraph`]: the weight
/// between files `a` and `b` is the number of directed edges between them in either
/// direction. Built via a `BTreeMap` so construction order is fully deterministic.
fn build_undirected(dep: &DepGraph) -> Adj {
    let idx: BTreeMap<&str, usize> =
        dep.nodes.iter().enumerate().map(|(i, p)| (p.as_str(), i)).collect();

    // Unordered pair (min,max) → accumulated weight.
    let mut pair_w: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for e in &dep.edges {
        let (Some(&a), Some(&b)) = (idx.get(e.from.as_str()), idx.get(e.to.as_str())) else {
            continue;
        };
        if a == b {
            continue; // DepGraph has no self-edges, but be defensive.
        }
        let key = (a.min(b), a.max(b));
        *pair_w.entry(key).or_insert(0.0) += 1.0;
    }

    let mut adj: Adj = vec![Vec::new(); dep.nodes.len()];
    for ((a, b), w) in pair_w {
        adj[a].push((b, w));
        adj[b].push((a, w));
    }
    adj
}

// ──────────────────────────────────── deterministic Louvain ──

/// A weighted graph in the form Louvain consumes. `adj[i]` may include a self-loop
/// `(i, w)` after aggregation. `deg[i]` is the weighted degree (self-loops counted
/// twice), and `m` is the total edge weight (so `2m == sum(deg)`).
struct WGraph {
    n: usize,
    adj: Adj,
    deg: Vec<f64>,
    m: f64,
}

impl WGraph {
    fn from_adj(n: usize, adj: Adj) -> Self {
        let mut deg = vec![0.0; n];
        for (i, nbrs) in adj.iter().enumerate() {
            for &(j, w) in nbrs {
                deg[i] += w;
                if j == i {
                    deg[i] += w; // self-loop counted twice
                }
            }
        }
        let m = deg.iter().sum::<f64>() / 2.0;
        WGraph { n, adj, deg, m }
    }
}

/// Run deterministic Louvain over the undirected weighted adjacency and return a
/// compact community label (`0..k`) per original node.
fn louvain(n: usize, adj0: &Adj) -> Vec<usize> {
    // Maps each ORIGINAL node to its node in the CURRENT (possibly aggregated) graph.
    let mut node_to_super: Vec<usize> = (0..n).collect();
    let mut g = WGraph::from_adj(n, adj0.clone());

    loop {
        let comm = local_moving(&g);
        // Compact community ids by sorted old-id → deterministic relabel.
        let uniq: BTreeSet<usize> = comm.iter().copied().collect();
        let relabel: BTreeMap<usize, usize> =
            uniq.iter().enumerate().map(|(new, &old)| (old, new)).collect();
        let k = uniq.len();

        // Map originals through this level.
        for s in node_to_super.iter_mut() {
            *s = relabel[&comm[*s]];
        }

        // Converged: local moving could not merge anything further.
        if k == g.n {
            break;
        }

        // Aggregate: each community becomes one super-node; edges sum (intra-community
        // edges become self-loops). Built via BTreeMap for deterministic adjacency.
        let mut agg: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (i, nbrs) in g.adj.iter().enumerate() {
            let ci = relabel[&comm[i]];
            for &(j, w) in nbrs {
                let cj = relabel[&comm[j]];
                // Count each undirected edge once: i<j for cross edges; an existing
                // self-loop (i==j) contributes to ci's self-loop. Intra-community
                // (ci==cj) cross edges naturally become the new node's self-loop.
                match i.cmp(&j) {
                    std::cmp::Ordering::Less => {
                        *agg.entry((ci.min(cj), ci.max(cj))).or_insert(0.0) += w;
                    }
                    std::cmp::Ordering::Equal => {
                        *agg.entry((ci, ci)).or_insert(0.0) += w;
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
        }
        let mut new_adj: Adj = vec![Vec::new(); k];
        for ((a, b), w) in agg {
            if a == b {
                new_adj[a].push((a, w));
            } else {
                new_adj[a].push((b, w));
                new_adj[b].push((a, w));
            }
        }
        g = WGraph::from_adj(k, new_adj);
    }

    node_to_super
}

/// One local-moving phase: repeatedly sweep nodes in FIXED ascending index order,
/// moving each to the neighboring community of maximal modularity gain, until a full
/// sweep makes no move (bounded for safety). Returns the per-node community label.
fn local_moving(g: &WGraph) -> Vec<usize> {
    let mut comm: Vec<usize> = (0..g.n).collect();
    let mut sigma_tot: Vec<f64> = g.deg.clone(); // each node starts alone

    if g.m == 0.0 {
        return comm; // edgeless: every node its own community
    }
    let two_m = 2.0 * g.m;

    let max_sweeps = 100;
    for _ in 0..max_sweeps {
        let mut moved = false;
        for i in 0..g.n {
            let ci = comm[i];
            let ki = g.deg[i];

            // Weight from i into each neighboring community (skip self-loops).
            let mut w_to: BTreeMap<usize, f64> = BTreeMap::new();
            for &(j, w) in &g.adj[i] {
                if j == i {
                    continue;
                }
                *w_to.entry(comm[j]).or_insert(0.0) += w;
            }

            // Remove i from its community.
            sigma_tot[ci] -= ki;
            let factor = ki / two_m;

            // Baseline: re-inserting into ci (gain measured in argmax-equivalent units,
            // common /m factor dropped). Candidates iterated in SORTED community order.
            let mut best_comm = ci;
            let mut best_gain = w_to.get(&ci).copied().unwrap_or(0.0) - factor * sigma_tot[ci];
            for (&c, &w_ic) in &w_to {
                if c == ci {
                    continue;
                }
                let gain = w_ic - factor * sigma_tot[c];
                if gain > best_gain + EPS {
                    best_gain = gain;
                    best_comm = c;
                }
            }

            // Re-insert into the chosen community.
            sigma_tot[best_comm] += ki;
            if best_comm != ci {
                comm[i] = best_comm;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    comm
}

// ─────────────────────────── CodeBoarding super-cluster / absorb ──

/// Readable target cluster count for a system of `n` files. Small systems get `n`
/// (no forced merge-down — only singleton absorption applies); large systems get the
/// readable-band upper bound `n/5`, clamped into `[TOP_MIN, TOP_MAX]`.
fn target_clusters(n: usize) -> usize {
    if n <= SMALL_GRAPH_FILES {
        n.max(1)
    } else {
        (n / 5).clamp(TOP_MIN, TOP_MAX)
    }
}

/// Collapse `clusters` down until the count is within `target` AND no tiny (< MIN_CLUSTER_SIZE)
/// cluster remains: repeatedly take the SMALLEST cluster and absorb it into the best
/// neighbor. The merge-target choice IS the orphan cascade: (1) nearest by graph
/// distance — the cluster with the most cross-edge weight to it; else (2) file
/// co-location — the cluster with the longest shared directory prefix; else (3) the
/// fallback bucket — the lexicographically-first other cluster. Members only move
/// between clusters, so 100% coverage is preserved throughout.
fn absorb(clusters: &mut Vec<Vec<usize>>, target: usize, dep: &DepGraph, adj: &Adj) {
    loop {
        if clusters.len() <= 1 {
            break;
        }
        let smallest = smallest_idx(clusters, dep);
        let over_budget = clusters.len() > target;
        let tiny = clusters[smallest].len() < MIN_CLUSTER_SIZE;
        if !over_budget && !tiny {
            break;
        }
        let dst = best_merge_target(clusters, smallest, dep, adj);
        let mut moved = std::mem::take(&mut clusters[smallest]);
        clusters[dst].append(&mut moved);
        clusters.remove(smallest);
    }
}

/// Index of the smallest cluster: size asc, ties broken by smallest member path asc.
fn smallest_idx(clusters: &[Vec<usize>], dep: &DepGraph) -> usize {
    (0..clusters.len())
        .min_by(|&a, &b| {
            clusters[a]
                .len()
                .cmp(&clusters[b].len())
                .then_with(|| min_path(&clusters[a], dep).cmp(min_path(&clusters[b], dep)))
        })
        .unwrap()
}

/// Choose where to absorb `src`. Tier 1: max cross-edge weight (graph proximity).
/// Tier 2: longest shared directory prefix (file co-location). Tier 3: fallback to the
/// lexicographically-first other cluster. All tie-breaks are by smallest member path.
fn best_merge_target(clusters: &[Vec<usize>], src: usize, dep: &DepGraph, adj: &Adj) -> usize {
    // Membership lookup so we can weigh cross-cluster edges.
    let mut cluster_of: Vec<usize> = vec![usize::MAX; dep.nodes.len()];
    for (ci, members) in clusters.iter().enumerate() {
        for &m in members {
            cluster_of[m] = ci;
        }
    }

    // Tier 1: cross-edge weight from src's members to every other cluster.
    let mut weight_to: BTreeMap<usize, f64> = BTreeMap::new();
    for &node in &clusters[src] {
        for &(j, w) in &adj[node] {
            let cj = cluster_of[j];
            if cj != src && cj != usize::MAX {
                *weight_to.entry(cj).or_insert(0.0) += w;
            }
        }
    }
    if let Some(best) = weight_to
        .iter()
        .max_by(|a, b| {
            a.1.partial_cmp(b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                // tie-break: prefer the cluster with the smallest member path
                .then_with(|| min_path(&clusters[*b.0], dep).cmp(min_path(&clusters[*a.0], dep)))
        })
        .map(|(&c, _)| c)
    {
        return best;
    }

    // Tier 2: file co-location — longest shared directory prefix with src.
    let src_dir = common_dir_prefix(&clusters[src], dep);
    let mut best_dst: Option<usize> = None;
    let mut best_overlap = 0usize;
    for (ci, members) in clusters.iter().enumerate() {
        if ci == src {
            continue;
        }
        let overlap = shared_prefix_len(&src_dir, &common_dir_prefix(members, dep));
        let better = match best_dst {
            None => true,
            Some(b) => {
                overlap > best_overlap
                    || (overlap == best_overlap
                        && min_path(members, dep) < min_path(&clusters[b], dep))
            }
        };
        if better {
            best_overlap = overlap;
            best_dst = Some(ci);
        }
    }
    if let Some(d) = best_dst {
        if best_overlap > 0 {
            return d;
        }
    }

    // Tier 3: fallback bucket — lexicographically-first OTHER cluster.
    (0..clusters.len())
        .filter(|&c| c != src)
        .min_by(|&a, &b| min_path(&clusters[a], dep).cmp(min_path(&clusters[b], dep)))
        .unwrap()
}

// ───────────────────────────────────────────── naming / finalize ──

/// Smallest member path of a cluster (its deterministic "anchor").
fn min_path<'a>(members: &[usize], dep: &'a DepGraph) -> &'a String {
    members.iter().map(|&i| &dep.nodes[i]).min().unwrap()
}

/// Directory portion of a project-relative path (`""` for a top-level file).
fn dir_of(p: &str) -> &str {
    match p.rfind('/') {
        Some(i) => &p[..i],
        None => "",
    }
}

/// The longest common directory prefix (whole path segments) of a cluster's members.
fn common_dir_prefix(members: &[usize], dep: &DepGraph) -> String {
    let mut iter = members.iter().map(|&i| dir_of(&dep.nodes[i]));
    let Some(first) = iter.next() else {
        return String::new();
    };
    let mut prefix: Vec<&str> = first.split('/').filter(|s| !s.is_empty()).collect();
    for d in iter {
        let segs: Vec<&str> = d.split('/').filter(|s| !s.is_empty()).collect();
        let keep = prefix.iter().zip(&segs).take_while(|(a, b)| a == b).count();
        prefix.truncate(keep);
        if prefix.is_empty() {
            break;
        }
    }
    prefix.join("/")
}

/// Number of leading directory segments two prefixes share.
fn shared_prefix_len(a: &str, b: &str) -> usize {
    a.split('/')
        .filter(|s| !s.is_empty())
        .zip(b.split('/').filter(|s| !s.is_empty()))
        .take_while(|(x, y)| x == y)
        .count()
}

/// Name every cluster by its **most distinctive directory** relative to the sibling
/// clusters — killing the old "Ui / Ui (2) / Ui (3)" collisions (phase 3B.3).
///
/// Rule (documented):
///  1. Score each member directory `d` of a cluster by `count(d) * ln(N / df(d))`,
///     where `count(d)` is how many of the cluster's files sit directly in `d`, `N`
///     is the cluster count, and `df(d)` is how many clusters contain `d`. This is a
///     TF-ICF (inverse *cluster* frequency): a directory shared by every cluster
///     (e.g. `src/components/ui` pulled in everywhere) has `ln(N/N)=0` and never wins,
///     while a directory unique to this cluster (`.../layout`, `.../setup`) does.
///  2. Pick the max-scoring directory (ties: higher file count, then shallower dir,
///     then lexicographically-smallest path) and label the cluster by that dir's LAST
///     segment, titlecased (`components/layout` → "Layout", `components/datasources`
///     → "Datasources"). A root-level directory → "Root".
///  3. If no directory is distinctive (every candidate scores 0 — i.e. two clusters
///     genuinely share the same directory, or N==1), fall back to the **central
///     member's basename** (its file stem). On a *name* collision, disambiguate the
///     later cluster deterministically by central basename, then `parent / basename`,
///     then the full central path — NEVER a numeric `(2)/(3)` suffix.
///
/// Determinism: clusters are already in their final sorted order; df/scores use only
/// member paths; all ties have explicit lexical tie-breaks.
fn name_clusters(clusters: &[Vec<usize>], dep: &DepGraph, deg: &[f64]) -> Vec<String> {
    let n = clusters.len();

    // Per-cluster immediate-parent-dir histogram.
    let dir_counts: Vec<BTreeMap<&str, usize>> = clusters
        .iter()
        .map(|members| {
            let mut m: BTreeMap<&str, usize> = BTreeMap::new();
            for &idx in members {
                *m.entry(dir_of(&dep.nodes[idx])).or_insert(0) += 1;
            }
            m
        })
        .collect();

    // Cluster-frequency of each directory (how many clusters contain it).
    let mut df: BTreeMap<&str, usize> = BTreeMap::new();
    for dc in &dir_counts {
        for d in dc.keys() {
            *df.entry(*d).or_insert(0) += 1;
        }
    }

    // Base label per cluster (distinctive dir, else central basename).
    let bases: Vec<String> = (0..n)
        .map(|ci| match best_distinctive_dir(&dir_counts[ci], &df, n) {
            Some(d) => last_segment_name(d),
            None => central_basename(&clusters[ci], dep, deg),
        })
        .collect();

    dedup_names(bases, clusters, dep, deg)
}

/// The most distinctive directory of one cluster, or `None` when nothing scores above
/// zero (no directory separates this cluster from the rest). Score = `count * ln(N/df)`.
fn best_distinctive_dir<'a>(
    dir_count: &BTreeMap<&'a str, usize>,
    df: &BTreeMap<&str, usize>,
    n: usize,
) -> Option<&'a str> {
    let mut best: Option<(&str, f64, usize)> = None; // (dir, score, count)
    for (&d, &count) in dir_count {
        let dfd = *df.get(d).unwrap_or(&1) as f64;
        let idf = (n as f64 / dfd).ln();
        let score = count as f64 * idf;
        if score <= EPS {
            continue; // shared-by-all dir (idf==0) carries no distinguishing signal
        }
        let take = match best {
            None => true,
            Some((bd, bs, bc)) => {
                score > bs + EPS
                    || (almost_eq(score, bs)
                        && (count > bc
                            || (count == bc
                                && (seg_depth(d) < seg_depth(bd)
                                    || (seg_depth(d) == seg_depth(bd) && d < bd)))))
            }
        };
        if take {
            best = Some((d, score, count));
        }
    }
    best.map(|(d, _, _)| d)
}

fn almost_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

/// Number of non-empty path segments in a directory (`""` → 0, `a/b` → 2).
fn seg_depth(dir: &str) -> usize {
    dir.split('/').filter(|s| !s.is_empty()).count()
}

/// Label a directory by its LAST segment, titlecased. Empty (repo root) → "Root".
fn last_segment_name(dir: &str) -> String {
    match dir.split('/').filter(|s| !s.is_empty()).next_back() {
        Some(seg) => titlecase(seg),
        None => "Root".to_string(),
    }
}

/// The cluster's most-central member (highest weighted degree; tie → smallest path).
fn central_member(members: &[usize], dep: &DepGraph, deg: &[f64]) -> usize {
    members
        .iter()
        .copied()
        .max_by(|&a, &b| {
            deg[a]
                .partial_cmp(&deg[b])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| dep.nodes[b].cmp(&dep.nodes[a]))
        })
        .unwrap()
}

/// The central member's basename (file stem), titlecased — the naming fallback.
fn central_basename(members: &[usize], dep: &DepGraph, deg: &[f64]) -> String {
    let c = central_member(members, dep, deg);
    titlecase(file_stem(&dep.nodes[c]))
}

/// File stem of a path: basename minus a single trailing extension.
fn file_stem(path: &str) -> &str {
    let base = path.rsplit('/').next().unwrap_or(path);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    }
}

/// De-duplicate cluster labels deterministically (clusters in final order). A repeated
/// label is replaced — in this fixed preference order — by the central basename, then
/// `parent / basename`, then the full central path. The full path is unique per
/// cluster (members partition the graph), so a unique label is always reached without a
/// numeric `(2)/(3)` suffix.
fn dedup_names(bases: Vec<String>, clusters: &[Vec<usize>], dep: &DepGraph, deg: &[f64]) -> Vec<String> {
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::with_capacity(bases.len());
    for (ci, base) in bases.into_iter().enumerate() {
        let c = central_member(&clusters[ci], dep, deg);
        let path = &dep.nodes[c];
        let stem = titlecase(file_stem(path));
        let parent_seg = dir_of(path).rsplit('/').find(|s| !s.is_empty());
        let parented = match parent_seg {
            Some(p) => format!("{} / {}", titlecase(p), stem),
            None => stem.clone(),
        };
        let full = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(titlecase)
            .collect::<Vec<_>>()
            .join(" / ");
        let pick = [base, stem, parented, full]
            .into_iter()
            .find(|c| !used.contains(c))
            .expect("full central path is unique → a free label always exists");
        used.insert(pick.clone());
        out.push(pick);
    }
    out
}

/// Titlecase one path segment: `server` → "Server", `my-api` → "My-api".
fn titlecase(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Order clusters deterministically, name them by their most-distinctive directory
/// (see [`name_clusters`]), and assign `c1..cN` ids. `roles` is left empty here — it is
/// populated by [`crate::roles::tag_roles`], which needs the `ScanResult`.
fn finalize(mut clusters: Vec<Vec<usize>>, dep: &DepGraph) -> Clusters {
    // Weighted degree per node (for the "most-central member" naming fallback).
    let adj = build_undirected(dep);
    let deg: Vec<f64> = adj.iter().map(|nbrs| nbrs.iter().map(|&(_, w)| w).sum()).collect();

    // Sort members within each cluster, then clusters by (size desc, min path asc).
    for m in clusters.iter_mut() {
        m.sort_by(|&a, &b| dep.nodes[a].cmp(&dep.nodes[b]));
    }
    clusters.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| dep.nodes[a[0]].cmp(&dep.nodes[b[0]]))
    });

    let names = name_clusters(&clusters, dep, &deg);
    let mut out = Vec::with_capacity(clusters.len());
    for (i, members) in clusters.iter().enumerate() {
        out.push(Cluster {
            id: format!("c{}", i + 1),
            name: names[i].clone(),
            members: members.iter().map(|&m| dep.nodes[m].clone()).collect(),
            size: members.len(),
            kind: ClusterKind::Component,
            roles: Vec::new(),
        });
    }
    Clusters { clusters: out }
}

#[cfg(test)]
mod tests;

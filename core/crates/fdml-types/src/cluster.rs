//! The `Clusters` contract (phase 3B.2): a readable architecture map derived by
//! community-detecting the [`DepGraph`] and collapsing it to a handful of named
//! "boxes" (components).
//!
//! This is the artifact that replaces the 89 too-granular module "features" with a
//! small set (~5–8 for a large system, fewer for a small one) of meaningful clusters.
//! It lives ALONGSIDE the existing `features`/LinkReport output — it does not replace it.
//!
//! Determinism contract: `clusters` are sorted (size desc, then by smallest member
//! path); each cluster's `members` are sorted; ids are assigned `c1..cN` in that
//! order. No HashMap iteration order, no RNG, no wall-clock leaks into output. Two
//! runs over the same `DepGraph` serialize byte-identically.

use serde::{Deserialize, Serialize};

/// What kind of box a cluster is. Today only `Component` (a deterministic graph
/// community); later passes may add `Layer` / `Service` / `External` without
/// breaking the artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterKind {
    /// A graph community — a group of files that depend on each other more than on
    /// the rest of the system.
    Component,
}

/// One readable "box" on the map: a named group of source files.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cluster {
    /// Stable id (`c1`, `c2`, …), assigned in the deterministic cluster order.
    pub id: String,
    /// Human-readable name derived (without an LLM) from the dominant directory of
    /// the members. E.g. "Server / Api", "Sources".
    pub name: String,
    /// The cluster's files, as project-relative POSIX paths. Sorted. Every node of
    /// the source `DepGraph` appears in exactly one cluster (100% coverage).
    pub members: Vec<String>,
    /// `members.len()`, duplicated for convenient consumption / sorting.
    pub size: usize,
    /// The kind of box this is (currently always `Component`).
    pub kind: ClusterKind,
    /// Architectural ROLE tags (phase 3B.3) — the DOMINANT set of roles held by this
    /// cluster's members (e.g. `["data-access", "model"]`). Sorted, deduplicated, and
    /// pruned to roles carried by at least a fifth of the members. Heuristic and
    /// deterministic (no LLM); empty when no member matched a role signal. Populated by
    /// `fdml_graph::tag_roles`; `cluster()` alone leaves it empty.
    #[serde(default)]
    pub roles: Vec<String>,
}

/// The readable map for one scanned system: a bounded set of named clusters that
/// together cover 100% of the `DepGraph` nodes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clusters {
    /// The clusters, sorted (size desc, then smallest member path asc). Their
    /// `members` partition the `DepGraph` nodes exactly — no file dropped, none shared.
    pub clusters: Vec<Cluster>,
}

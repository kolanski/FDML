//! The `DepGraph` contract (phase 3B.1): a deterministic cross-file dependency
//! graph derived by resolving raw import strings to real project files.
//!
//! This is the substrate the clustering pass (3B.2) consumes. It is INTENTIONALLY
//! provenance-aware: only imports that resolve to a file we actually scanned become
//! `edges` (an INTERNAL dependency). Imports that resolve to stdlib / third-party
//! packages, or that look project-local but couldn't be resolved, are *counted* in
//! `stats` but never emitted as edges — a clustering pass must not invent nodes for
//! files it has no inventory for.
//!
//! Determinism contract: `nodes` are sorted; `edges` are sorted by `(from, to, kind)`
//! and de-duplicated. No HashMap iteration order, no wall-clock, leaks into output.

use serde::{Deserialize, Serialize};

/// The kind of dependency an edge represents. Today only `Import`; later passes
/// (calls / contains / tested_by) extend this enum without breaking the artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepEdgeKind {
    /// `from` imports a symbol/module that resolved to project file `to`.
    Import,
    /// `from` contains a call to a function/method defined in project file `to`.
    Call,
}

/// A single resolved dependency edge: `from` (importer file) depends on `to`
/// (resolved target file). Both are project-relative POSIX paths present in `nodes`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DepEdge {
    pub from: String,
    pub to: String,
    pub kind: DepEdgeKind,
}

/// Provenance counters for the resolution pass. Externals/unresolved are tallied
/// here (NOT emitted as edges) so a run can report resolution health.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepGraphStats {
    /// Imports that resolved to at least one project file (one per `ImportInfo`,
    /// regardless of fan-out). This is the numerator of the resolved-edge ratio.
    pub internal_imports: usize,
    /// Imports classified as stdlib / third-party (bare, non-project-local, unresolved).
    pub external_imports: usize,
    /// Imports that look project-local (e.g. a relative `./x` or leading-dot Python)
    /// but did not resolve to any scanned file — a coverage gap, not a 3rd-party dep.
    pub unresolved_imports: usize,
    /// Call sites resolved to a project file → a `Call` edge.
    #[serde(default)]
    pub resolved_calls: usize,
    /// Call sites that didn't resolve (external / ambiguous / unknown) — no edge.
    #[serde(default)]
    pub unresolved_calls: usize,
}

/// A deterministic cross-file dependency graph for one scanned system.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepGraph {
    /// Every scanned file, as a project-relative POSIX path. Sorted. These are the
    /// graph's nodes — isolated files (no edges) are still present so clustering
    /// sees the whole system.
    pub nodes: Vec<String>,
    /// Resolved INTERNAL dependency edges. Sorted by `(from, to, kind)`, de-duplicated.
    pub edges: Vec<DepEdge>,
    /// Resolution provenance counters (externals/unresolved live here, not in `edges`).
    pub stats: DepGraphStats,
}

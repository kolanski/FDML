//! Data contracts shared between passes. ZERO logic — types only.
//!
//! The dependency rule of the rework: every pass depends on this crate and nothing
//! else of ours; no pass depends on another pass. These structs ARE the pass boundaries.
//!
//! This is still FDML: shapes here mirror FDML's existing schema so output stays
//! FDML-compatible. Do not diverge field names from FDML without a reason.

use serde::{Deserialize, Serialize};

/// The `ScanResult` contract (pass 2): code inventory produced by the tree-sitter scanner.
pub mod scan;

/// The `LinkReport` contract (pass 3): deterministic linker output (generation path).
pub mod graph;

/// The `DepGraph` contract (phase 3B.1): resolved cross-file dependency edges.
pub mod depgraph;

/// The `Clusters` contract (phase 3B.2): the readable, bounded architecture map.
pub mod cluster;

/// The `Flow` contract (pass 4): reconstructed ingress → sink data-flow paths.
pub mod flow;

/// Cross-system integration + shared-entity contracts (pass 5).
pub mod integration;

/// A detected independent system/project within a platform root.
///
/// Mirrors FDML's `DetectedSystem` (the platform `systems:` entry) field-for-field —
/// the rework must produce what FDML produced at this stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct System {
    /// Relative path from project root.
    pub path: String,
    /// Snake_case identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// frontend | service | worker | gateway | library
    pub system_type: String,
    /// e.g. "React + TypeScript", "FastAPI + Python".
    pub technology: String,
    /// File that triggered detection (e.g. "package.json").
    pub boundary_marker: String,
}

// NOTE: ScanResult / CodeGraph / Flow / Integration / FdmlSpec contracts land with
// their own passes (phases 2–6) — not pre-created as empty structs here (YAGNI).
// FDML-YAML rendering lives in the CLI (presentation), not here.

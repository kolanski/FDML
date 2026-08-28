//! Phase 3B.3 — deterministic architectural ROLE tagging (no LLM).
//!
//! Each file is tagged with the sorted union of architectural roles it matches across
//! three signal families, then clusters aggregate their members' roles into a DOMINANT
//! set. Everything is heuristic but precise-by-construction: a file with no matching
//! signal gets NO tag (we never force one).
//!
//! ## Role taxonomy (flat, multi-tag)
//! `api`, `domain` (business), `data-access`, `integration`, `infra`, `ui`,
//! `model` (dto/schema), `util`, `test`. A coarser `tech` vs `business` axis is
//! derivable downstream from these (tech = data-access/infra/util/integration,
//! business = domain/api) — we keep the artifact a flat string set.
//!
//! ## Signals (union of all matches)
//!  1. **Directory patterns** on the path's directory segments (e.g. `repositories/` →
//!     `data-access`, `components|ui/` → `ui`). A segment can map to several roles
//!     (`models` → both `data-access` and `model`; `views` → both `api` and `ui`).
//!  2. **Import signals** on `imports[].module`: a DB/ORM lib → `data-access`, an
//!     HTTP-client/RPC lib → `integration`, a web-framework route marker → `api`.
//!  3. **Name patterns** on `elements[].name` suffixes (`*Repository` → `data-access`,
//!     `*Controller` → `api`, `*Service` → `domain`, `*Component` → `ui`,
//!     `*Dto`/`*Schema`/`*Model` → `model`).
//!
//! ## Determinism
//! Per-file roles are a sorted `BTreeSet`; cluster aggregation iterates members in
//! their (already sorted) order and emits a sorted role vector. No RNG, no HashMap
//! iteration order, no wall-clock. Two runs are byte-identical.

use std::collections::{BTreeMap, BTreeSet};

use fdml_types::cluster::Clusters;
use fdml_types::scan::{CodeElement, ScanResult};

/// A role must be carried by at least this fraction of a cluster's members to survive
/// into the cluster's DOMINANT role set. Rare one-off roles are dropped as noise so the
/// tag set describes what the cluster mostly *is*.
const ROLE_MIN_SHARE: f64 = 0.20;

// ─────────────────────────────────── directory-segment → role(s) ──

/// Directory-segment patterns. Each `(role, &[segments])`; a path directory segment
/// equal (case-insensitively) to any listed token contributes that role. Tokens
/// deliberately overlap across roles (the union is taken).
const DIR_PATTERNS: &[(&str, &[&str])] = &[
    ("api", &["controllers", "routes", "api", "handlers", "endpoints", "views"]),
    (
        "data-access",
        &[
            "repositories", "repository", "dao", "models", "model", "db", "database",
            "entities", "store", "stores", "persistence",
        ],
    ),
    ("domain", &["services", "service", "domain", "usecases", "use_cases", "core", "business"]),
    ("integration", &["clients", "client", "adapters", "integration", "integrations", "external"]),
    ("infra", &["config", "infra", "infrastructure", "bootstrap", "setup", "settings"]),
    ("ui", &["components", "component", "views", "pages", "ui", "widgets"]),
    ("model", &["dto", "dtos", "schema", "schemas", "types", "models"]),
    ("util", &["utils", "util", "helpers", "helper", "common", "lib", "shared"]),
    ("test", &["tests", "test", "__tests__", "spec", "specs"]),
];

/// Import-module tokens → role. A module is split on `/` and `.` and each token is
/// matched (case-insensitively) against these keyword sets.
const DB_LIBS: &[&str] =
    &["psycopg", "sqlalchemy", "pg", "mongoose", "prisma", "sequelize", "typeorm", "redis", "sqlite", "pymongo"];
const HTTP_CLIENT_LIBS: &[&str] = &["requests", "httpx", "axios", "grpc", "@grpc", "node-fetch", "got"];
const WEB_FRAMEWORK_LIBS: &[&str] = &["fastapi", "flask", "express", "@nestjs", "gin", "echo", "axum", "django"];

/// Element-name suffix → role. Matched case-sensitively against PascalCase suffixes to
/// avoid false positives (`Restore` must not match `*Store`).
const NAME_SUFFIXES: &[(&str, &[&str])] = &[
    ("data-access", &["Repository", "Dao", "Store"]),
    ("api", &["Controller", "Handler", "Resolver"]),
    ("domain", &["Service", "UseCase"]),
    ("ui", &["Component", "Page", "View"]),
    ("model", &["Dto", "Schema", "Model"]),
];

// ───────────────────────────────────────────────────── public surface ──

/// Tag every cluster in `clusters` with its DOMINANT architectural roles, derived from
/// per-file role detection over `scan`. Idempotent and deterministic.
pub fn tag_roles(scan: &ScanResult, clusters: &mut Clusters) {
    let file_roles = compute_file_roles(scan);

    for cluster in &mut clusters.clusters {
        let n = cluster.members.len();
        if n == 0 {
            cluster.roles = Vec::new();
            continue;
        }
        // Count members holding each role.
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for member in &cluster.members {
            if let Some(roles) = file_roles.get(member) {
                for r in roles {
                    *counts.entry(r.as_str()).or_insert(0) += 1;
                }
            }
        }
        // Keep the DOMINANT set: roles held by ≥ ROLE_MIN_SHARE of the members.
        let mut roles: Vec<String> = counts
            .into_iter()
            .filter(|&(_, c)| c as f64 / n as f64 >= ROLE_MIN_SHARE)
            .map(|(r, _)| r.to_string())
            .collect();
        roles.sort();
        cluster.roles = roles;
    }
}

/// Per-file roles for an entire scan, keyed by the file's POSIX path (the same key form
/// as `DepGraph` nodes / `Cluster` members). Files with no matched signal are omitted.
pub fn compute_file_roles(scan: &ScanResult) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in &scan.files {
        let modules: Vec<&str> = file.imports.iter().map(|i| i.module.as_str()).collect();
        let mut names: Vec<&str> = Vec::new();
        collect_element_names(&file.elements, &mut names);
        let roles = detect_roles(&file.file_path, &modules, &names);
        if !roles.is_empty() {
            out.insert(to_posix(&file.file_path), roles);
        }
    }
    out
}

// ───────────────────────────────────────────────── per-file detection ──

/// The sorted-union role set for one file from its path + import modules + element
/// names. Pure and deterministic — this is the unit under test.
pub fn detect_roles(path: &str, import_modules: &[&str], element_names: &[&str]) -> Vec<String> {
    let mut roles: BTreeSet<&'static str> = BTreeSet::new();

    // 1. Directory patterns — only the directory segments (exclude the basename).
    let posix = to_posix(path);
    let segs: Vec<&str> = posix.split('/').collect();
    let dir_segs = &segs[..segs.len().saturating_sub(1)];
    for seg in dir_segs {
        let lower = seg.to_lowercase();
        for (role, tokens) in DIR_PATTERNS {
            if tokens.contains(&lower.as_str()) {
                roles.insert(role);
            }
        }
    }

    // 2. Import signals — tokenize each module on `/` and `.`.
    for module in import_modules {
        for tok in module.split(['/', '.']) {
            let lower = tok.to_lowercase();
            let t = lower.as_str();
            if DB_LIBS.contains(&t) {
                roles.insert("data-access");
            }
            if HTTP_CLIENT_LIBS.contains(&t) {
                roles.insert("integration");
            }
            if WEB_FRAMEWORK_LIBS.contains(&t) {
                roles.insert("api");
            }
        }
    }

    // 3. Name patterns — PascalCase suffixes on element names.
    for name in element_names {
        for (role, suffixes) in NAME_SUFFIXES {
            if suffixes.iter().any(|s| name.ends_with(s)) {
                roles.insert(role);
            }
        }
    }

    roles.into_iter().map(String::from).collect()
}

/// Recursively gather element names (classes, functions, nested members) for the
/// name-pattern signal.
fn collect_element_names<'a>(elements: &'a [CodeElement], out: &mut Vec<&'a str>) {
    for el in elements {
        out.push(el.name.as_str());
        collect_element_names(&el.children, out);
    }
}

/// Normalize a path to forward-slash POSIX, dropping empty segments. Mirrors the
/// resolver's `to_posix` so role keys line up with `DepGraph` node paths exactly.
fn to_posix(p: &str) -> String {
    p.split(['\\', '/']).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests;

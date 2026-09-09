//! Phase 3B.1 — deterministic multi-language IMPORT RESOLVER.
//!
//! Ported FAITHFULLY from Understand-Anything's `extract-import-map.mjs` (the
//! "crown jewel" — ~1668 lines of battle-tested per-language resolution rules),
//! adapted to FDML's `ScanResult` / `ImportInfo` shapes and to std-only Rust.
//!
//! What it does: turn each file's raw import strings into real cross-file
//! dependency edges (`from → resolved target`), assembling a [`DepGraph`]. Only
//! imports that resolve to a file we actually scanned become edges (INTERNAL);
//! stdlib / third-party imports are counted but never emitted as edges.
//!
//! Structure mirrors the source: configs (tsconfig.json / go.mod) are loaded ONCE
//! into a shared [`Ctx`]; per-import dispatch is then pure and O(1).
//!
//! ## Divergences from the .mjs source (all deliberate)
//!   * The .mjs re-runs tree-sitter to extract imports; we don't — `fdml-scan`
//!     already produced `ImportInfo` (including JS `require()` and re-exports), so
//!     this pass is purely the *resolution* half.
//!   * File-existence probing is against the scanned `fileSet` (the .mjs does the
//!     same — it probes the input file list, not raw disk) so an edge can only
//!     point at a real graph node.
//!   * Configs (tsconfig.json / go.mod) are NOT in `ScanResult.files` (the scanner
//!     only collects code files), so we discover them by walking `root` on disk.
//!   * tsconfig.json is parsed with a tiny std-only JSONC reader (no serde_json
//!     dependency) — we only need `compilerOptions.baseUrl` + `.paths`.
//!   * Languages the FDML scanner never emits (Kotlin/PHP/Ruby/Rust/C/C++) are not
//!     dispatched — see the report. Our scanner emits: Python, Java, C#, JS, TS, Go.
//!
//! ## Determinism / tie-breaks
//!   * `nodes` sorted; `edges` sorted by `(from, to, kind)` and de-duplicated.
//!   * TS/JS single-target probing tie-breaks by the fixed `TS_EXT_PROBES` /
//!     NodeNext order (mirrors the source). Multi-match resolvers (Java/C#/Go/
//!     Python) emit an edge to EVERY candidate (mirrors the source's `[...matches]`),
//!     then the global sort+dedup makes order irrelevant.
//!   * Suffix-index buckets are sorted lexicographically (the source uses
//!     `localeCompare`; for project paths that is byte order — we use `str` `Ord`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use fdml_types::depgraph::{DepEdge, DepEdgeKind, DepGraph, DepGraphStats};
use fdml_types::scan::{CodeElement, ElementType, ImportInfo, Language, ScanResult};

use petgraph::graph::{DiGraph, NodeIndex};

// ───────────────────────── path helpers (port of toPosix/resolveRelative/dirOf) ──

/// Normalize a path to forward-slash POSIX, dropping empty segments.
fn to_posix(p: &str) -> String {
    p.split(['\\', '/']).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/")
}

/// Directory portion of a project-relative path (`""` for top-level files).
fn dir_of(p: &str) -> &str {
    match p.rfind('/') {
        Some(i) => &p[..i],
        None => "",
    }
}

/// Join `dir` + relative segment, normalizing `.`/`..`. Returns `""` if it walks
/// above the project root. Port of the source `resolveRelative`.
fn resolve_relative(dir: &str, rel: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for part in dir.split('/').filter(|s| !s.is_empty()).chain(rel.split('/').filter(|s| !s.is_empty())) {
        if part == "." {
            continue;
        }
        if part == ".." {
            if stack.is_empty() {
                return String::new();
            }
            stack.pop();
        } else {
            stack.push(part);
        }
    }
    stack.join("/")
}

/// Walk ancestors of `start_dir` (deepest first) and return the deepest that is a
/// key in `keys`. Port of `findNearestConfigDir` ("nearest enclosing config").
fn find_nearest_config_dir<'a>(start_dir: &str, keys: &'a HashMap<String, ConfigEntry>) -> Option<&'a str> {
    if keys.is_empty() {
        return None;
    }
    let parts: Vec<&str> = start_dir.split('/').filter(|s| !s.is_empty()).collect();
    for i in (0..=parts.len()).rev() {
        let ancestor = parts[..i].join("/");
        if let Some((k, _)) = keys.get_key_value(&ancestor) {
            return Some(k.as_str());
        }
    }
    None
}

// ───────────────────────────────────────────────── shared resolution context ──

/// A parsed config: either a tsconfig (`base_url` + `paths`) or a go.mod (`module`).
enum ConfigEntry {
    TsConfig { base_url: String, paths: Vec<(String, Vec<String>)> },
    GoModule { module: String },
}

struct Ctx {
    /// Every scanned file as a project-relative POSIX path (the probe target set).
    file_set: BTreeSet<String>,
    /// dir → tsconfig. Walk-up from importer finds the nearest.
    ts_configs: HashMap<String, ConfigEntry>,
    /// dir → go.mod. Walk-up from importer finds the nearest module.
    go_modules: HashMap<String, ConfigEntry>,
    /// dir → sorted list of `.go` files in it (Go package-level expansion).
    go_files_by_dir: HashMap<String, Vec<String>>,
    /// dir-bounded suffix → sorted matching `.java` paths.
    java_index: HashMap<String, Vec<String>>,
    /// dir-bounded suffix → sorted matching `.cs` paths.
    cs_index: HashMap<String, Vec<String>>,
}

// ───────────────────────────────────────────────────── TS/JS resolver (port) ──

/// Extensions probed when an import has no extension. Order = the source's
/// `TS_EXT_PROBES` exactly (this IS the tie-break for ambiguous TS/JS resolution).
const TS_EXT_PROBES: &[&str] = &[
    ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs",
    "/index.ts", "/index.tsx", "/index.js", "/index.jsx",
];

/// NodeNext source-extension rewrites: a compiled specifier (`./foo.js`) maps back
/// to the TS source that produced it. Port of the source's `NODENEXT_REWRITES`.
const NODENEXT_REWRITES: &[(&str, &[&str])] = &[
    (".js", &[".ts", ".tsx", ".js", ".jsx"]),
    (".jsx", &[".tsx", ".jsx"]),
    (".mjs", &[".mts", ".mjs", ".ts"]),
    (".cjs", &[".cts", ".cjs", ".ts"]),
];

/// Probe ext candidates against the file set. Port of `probeWithExtensions`.
fn probe_with_extensions(base: &str, file_set: &BTreeSet<String>) -> Option<String> {
    if base.is_empty() {
        return None;
    }
    if file_set.contains(base) {
        return Some(base.to_string());
    }
    // NodeNext rewrite BEFORE the append loop (so `./foo.js` → `foo.ts`, never `foo.js.ts`).
    for (out_ext, src_exts) in NODENEXT_REWRITES {
        if base.ends_with(out_ext) {
            let stem = &base[..base.len() - out_ext.len()];
            for src_ext in *src_exts {
                let candidate = format!("{stem}{src_ext}");
                if file_set.contains(&candidate) {
                    return Some(candidate);
                }
            }
            // Explicit compiled extension but nothing matched — do not fall through.
            return None;
        }
    }
    for ext in TS_EXT_PROBES {
        let candidate = format!("{base}{ext}");
        if file_set.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Match an import against a tsconfig `paths` alias (`*` = single wildcard).
/// Returns the wildcard content on match. Port of `matchTsAlias`.
fn match_ts_alias(alias: &str, src: &str) -> Option<String> {
    match alias.find('*') {
        None => {
            if src == alias {
                Some(String::new())
            } else {
                None
            }
        }
        Some(star) => {
            let prefix = &alias[..star];
            let suffix = &alias[star + 1..];
            if !src.starts_with(prefix) || !src.ends_with(suffix) {
                return None;
            }
            if src.len() < prefix.len() + suffix.len() {
                return None;
            }
            Some(src[prefix.len()..src.len() - suffix.len()].to_string())
        }
    }
}

/// Substitute the wildcard into a tsconfig target. Port of `applyTsAlias`.
fn apply_ts_alias(target: &str, wildcard: &str) -> String {
    match target.find('*') {
        None => target.to_string(),
        Some(star) => format!("{}{}{}", &target[..star], wildcard, &target[star + 1..]),
    }
}

/// Normalize `.`/`..` in a POSIX path (no file-system access). Used to collapse a
/// tsconfig alias candidate the way the source's `posix.normalize` does.
fn posix_normalize(p: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    let mut leading_parent = 0usize;
    for part in p.split('/').filter(|s| !s.is_empty()) {
        if part == "." {
            continue;
        }
        if part == ".." {
            if stack.is_empty() {
                leading_parent += 1;
            } else {
                stack.pop();
            }
        } else {
            stack.push(part);
        }
    }
    let mut out = String::new();
    for _ in 0..leading_parent {
        out.push_str("../");
    }
    out.push_str(&stack.join("/"));
    out
}

/// Resolve a TS/JS import to a project file (or `None`). Port of `resolveTsJsImport`.
fn resolve_ts_js_import(src: &str, importer_dir: &str, ctx: &Ctx) -> Option<String> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }

    // Relative import — tsconfig has no bearing.
    if src.starts_with("./") || src.starts_with("../") {
        let base = resolve_relative(importer_dir, src);
        return probe_with_extensions(&base, &ctx.file_set);
    }

    // tsconfig path aliases: nearest-enclosing-config walk-up.
    if let Some(cfg_dir) = find_nearest_config_dir(importer_dir, &ctx.ts_configs) {
        if let Some(ConfigEntry::TsConfig { base_url, paths }) = ctx.ts_configs.get(cfg_dir) {
            for (alias, targets) in paths {
                let Some(wildcard) = match_ts_alias(alias, src) else { continue };
                for target in targets {
                    let mapped = apply_ts_alias(target, &wildcard);
                    let normalized_base = if base_url == "." || base_url.is_empty() {
                        String::new()
                    } else {
                        to_posix(base_url)
                    };
                    let relative_to_config = if normalized_base.is_empty() {
                        mapped.clone()
                    } else {
                        format!("{normalized_base}/{mapped}")
                    };
                    let joined = if cfg_dir.is_empty() {
                        relative_to_config
                    } else {
                        format!("{cfg_dir}/{relative_to_config}")
                    };
                    let candidate = posix_normalize(&joined);
                    if candidate.starts_with("..") {
                        continue;
                    }
                    if let Some(hit) = probe_with_extensions(&candidate, &ctx.file_set) {
                        return Some(hit);
                    }
                }
            }
        }
    }

    // Bare specifier, no alias match → external.
    None
}

// ───────────────────────────────────────────────────── Python resolver (port) ──

/// Resolve a Python import. Can produce multiple targets (package `__init__.py` +
/// per-specifier submodules). Port of `resolvePythonImport`.
///
/// `module` is the scanner's `ImportInfo.module` (leading dots preserved for
/// relatives, e.g. `.b`, `..pkg`, `.`); `specifiers` are the imported `names`.
fn resolve_python_import(module: &str, specifiers: &[String], importer_dir: &str, ctx: &Ctx) -> Vec<String> {
    let dots = module.bytes().take_while(|&b| b == b'.').count();
    let tail = &module[dots..];
    let tail_segments: Vec<&str> = if tail.is_empty() {
        Vec::new()
    } else {
        tail.split('.').filter(|s| !s.is_empty()).collect()
    };

    let importer_parts: Vec<&str> = importer_dir.split('/').filter(|s| !s.is_empty()).collect();

    if dots > 0 {
        // Relative import — leading dots encode the exact anchor (no root walk).
        let drop_levels = dots - 1;
        if drop_levels > importer_parts.len() {
            return Vec::new(); // walked above project root
        }
        let base_parts = &importer_parts[..importer_parts.len() - drop_levels];

        if tail_segments.is_empty() {
            // `from .[..] import x, y` — specifiers are siblings at base_parts.
            let base = base_parts.join("/");
            let mut matches = Vec::new();
            for spec in specifiers {
                if spec.is_empty() || spec == "*" || spec.contains('.') {
                    continue;
                }
                let sub_file = if base.is_empty() { format!("{spec}.py") } else { format!("{base}/{spec}.py") };
                let sub_init = if base.is_empty() { format!("{spec}/__init__.py") } else { format!("{base}/{spec}/__init__.py") };
                if ctx.file_set.contains(&sub_file) {
                    matches.push(sub_file);
                } else if ctx.file_set.contains(&sub_init) {
                    matches.push(sub_init);
                }
            }
            return matches;
        }

        let mut module_parts: Vec<&str> = base_parts.to_vec();
        module_parts.extend(tail_segments.iter().copied());
        return resolve_python_probe(&module_parts, specifiers, ctx);
    }

    // Absolute import — walk up every ancestor dir as a candidate Python root;
    // first that resolves wins (importer-scope precedence, deepest first).
    if tail_segments.is_empty() {
        return Vec::new();
    }
    for i in (0..=importer_parts.len()).rev() {
        let mut candidate: Vec<&str> = importer_parts[..i].to_vec();
        candidate.extend(tail_segments.iter().copied());
        let matches = resolve_python_probe(&candidate, specifiers, ctx);
        if !matches.is_empty() {
            return matches;
        }
    }
    Vec::new()
}

/// Probe `a/b/c.py` then `a/b/c/__init__.py`; on package match also probe each
/// specifier as a submodule. Port of `resolvePythonProbe`.
fn resolve_python_probe(module_parts: &[&str], specifiers: &[String], ctx: &Ctx) -> Vec<String> {
    if module_parts.is_empty() {
        return Vec::new();
    }
    let base = module_parts.join("/");
    let module_file = format!("{base}.py");
    if ctx.file_set.contains(&module_file) {
        return vec![module_file]; // leaf module — no further probing
    }
    let package_init = format!("{base}/__init__.py");
    if ctx.file_set.contains(&package_init) {
        let mut matches = vec![package_init];
        for spec in specifiers {
            if spec.is_empty() || spec == "*" || spec.contains('.') {
                continue;
            }
            let sub_file = format!("{base}/{spec}.py");
            let sub_init = format!("{base}/{spec}/__init__.py");
            if ctx.file_set.contains(&sub_file) {
                matches.push(sub_file);
            } else if ctx.file_set.contains(&sub_init) {
                matches.push(sub_init);
            }
        }
        return matches;
    }
    Vec::new()
}

// ─────────────────────────────────────────────────────── Go resolver (port) ──

/// Resolve a Go import path to all `.go` files in the target package directory.
/// Port of `resolveGoImport`.
fn resolve_go_import(src: &str, importer_dir: &str, ctx: &Ctx) -> Vec<String> {
    let src = src.trim();
    if src.is_empty() {
        return Vec::new();
    }
    let Some(module_dir) = find_nearest_config_dir(importer_dir, &ctx.go_modules) else {
        return Vec::new(); // no ancestor go.mod
    };
    let Some(ConfigEntry::GoModule { module }) = ctx.go_modules.get(module_dir) else {
        return Vec::new();
    };

    // Strip module prefix, requiring a `/` boundary.
    let remainder = if src == module {
        ""
    } else if let Some(rest) = src.strip_prefix(&format!("{module}/")) {
        rest
    } else {
        return Vec::new(); // stdlib / 3rd-party / sibling module → external
    };

    let sub_dir = to_posix(remainder);
    let target_dir = if module_dir.is_empty() {
        sub_dir
    } else if sub_dir.is_empty() {
        module_dir.to_string()
    } else {
        format!("{module_dir}/{sub_dir}")
    };
    ctx.go_files_by_dir.get(&target_dir).cloned().unwrap_or_default()
}

// ───────────────────────────────────── dotted-FQN resolver (Java / C#) (port) ──

/// Build the dir-bounded suffix index for files matching `ext`. Port of
/// `buildSuffixIndex`: `src/main/java/com/x/Y.java` indexes under `com/x/Y.java`,
/// `x/Y.java`, `Y.java`. Buckets sorted for determinism.
fn build_suffix_index(file_set: &BTreeSet<String>, ext: &str) -> HashMap<String, Vec<String>> {
    let mut idx: HashMap<String, Vec<String>> = HashMap::new();
    for p in file_set {
        if !p.ends_with(ext) {
            continue;
        }
        let parts: Vec<&str> = p.split('/').collect();
        for i in 0..parts.len() {
            let suffix = parts[i..].join("/");
            idx.entry(suffix).or_default().push(p.clone());
        }
    }
    for v in idx.values_mut() {
        v.sort();
        v.dedup();
    }
    idx
}

/// Resolve a dotted FQN (`com.example.Foo`) to file(s) via the suffix index.
/// Port of `resolveDottedFqn` (strips a trailing `.*` wildcard).
fn resolve_dotted_fqn(fqn: &str, ext: &str, index: &HashMap<String, Vec<String>>) -> Vec<String> {
    let trimmed = fqn.strip_suffix(".*").unwrap_or(fqn);
    if trimmed.is_empty() {
        return Vec::new();
    }
    let file_part = format!("{}{}", trimmed.replace('.', "/"), ext);
    index.get(&file_part).cloned().unwrap_or_default()
}

/// Reconstruct a Java import FQN from the scanner's split (`module`=package,
/// `names[0]`=type). The single-segment case stores module==names[0].
fn java_fqn(imp: &ImportInfo) -> String {
    match imp.names.first() {
        Some(n) if *n != imp.module => format!("{}.{}", imp.module, n),
        _ => imp.module.clone(),
    }
}

// ───────────────────────────────────────────────────────────────── dispatcher ──

/// Dispatch a single import to its language resolver → resolved project files.
fn resolve_one_import(imp: &ImportInfo, lang: &Language, importer_dir: &str, ctx: &Ctx) -> Vec<String> {
    match lang {
        Language::TypeScript | Language::JavaScript => {
            resolve_ts_js_import(&imp.module, importer_dir, ctx).into_iter().collect()
        }
        Language::Python => resolve_python_import(&imp.module, &imp.names, importer_dir, ctx),
        Language::Go => resolve_go_import(&imp.module, importer_dir, ctx),
        Language::Java => resolve_dotted_fqn(&java_fqn(imp), ".java", &ctx.java_index),
        // C# `using Foo.Bar;` carries the full dotted namespace in `module`.
        Language::CSharp => resolve_dotted_fqn(&imp.module, ".cs", &ctx.cs_index),
        Language::C => resolve_c_include(&imp.module, imp.is_relative, importer_dir, ctx),
    }
}

/// `#include "x.h"` is importer-relative first; failing that, and for `<x.h>` which is
/// usually an `-I` directory we cannot see, any project file whose path ends in the
/// include path. A system header matches nothing and stays external, as it should.
fn resolve_c_include(module: &str, is_relative: bool, importer_dir: &str, ctx: &Ctx) -> Vec<String> {
    if is_relative {
        let direct = resolve_relative(importer_dir, module);
        if ctx.file_set.contains(&direct) {
            return vec![direct];
        }
    }
    let suffix = format!("/{module}");
    ctx.file_set.iter()
        .filter(|p| p.as_str() == module || p.ends_with(&suffix))
        .cloned()
        .collect()
}

// ─────────────────────────────────────────────────────── config discovery (disk) ──

const CONFIG_SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", ".svn", ".hg", "target", "build", "dist", "out",
    "vendor", "venv", ".venv", "env", ".env", ".idea", ".vscode", ".vs",
    ".next", ".nuxt", ".output", ".cache", "bower_components", "__pycache__",
];

/// Recursively collect `tsconfig.json` and `go.mod` under `base`, keyed by their
/// project-relative POSIX directory. `want_ts`/`want_go` gate which we read.
fn collect_configs(
    base: &Path,
    rel_dir: &str,
    want_ts: bool,
    want_go: bool,
    ts_out: &mut HashMap<String, ConfigEntry>,
    go_out: &mut HashMap<String, ConfigEntry>,
) {
    let Ok(entries) = std::fs::read_dir(base) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if CONFIG_SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            let child_rel = if rel_dir.is_empty() { name.to_string() } else { format!("{rel_dir}/{name}") };
            collect_configs(&path, &child_rel, want_ts, want_go, ts_out, go_out);
        } else if want_ts && name == "tsconfig.json" {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Some(cfg) = parse_tsconfig(&raw) {
                    ts_out.insert(rel_dir.to_string(), cfg);
                }
            }
        } else if want_go && name == "go.mod" {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Some(module) = parse_go_module(&raw) {
                    go_out.insert(rel_dir.to_string(), ConfigEntry::GoModule { module });
                }
            }
        }
    }
}

/// Extract `module <name>` from go.mod. Port of `loadGoModules`' line scan.
fn parse_go_module(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.split("//").next().unwrap_or("").trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            let m = rest.trim();
            if !m.is_empty() {
                return Some(m.to_string());
            }
        }
    }
    None
}

/// Parse a tsconfig's `compilerOptions.baseUrl` + `.paths`. Port of
/// `parseTsConfigText` (JSONC strip then a tolerant std-only JSON read).
fn parse_tsconfig(raw: &str) -> Option<ConfigEntry> {
    let stripped = strip_jsonc_comments(raw);
    let value = minijson::parse(&stripped).or_else(|| minijson::parse(raw))?;
    let compiler_options = value.get("compilerOptions");
    let base_url = compiler_options
        .and_then(|c| c.get("baseUrl"))
        .and_then(minijson::Value::as_str)
        .unwrap_or(".")
        .to_string();
    let mut paths: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(p) = compiler_options.and_then(|c| c.get("paths")).and_then(minijson::Value::as_object) {
        for (alias, targets) in p {
            if let Some(arr) = targets.as_array() {
                let ts: Vec<String> = arr.iter().filter_map(|t| t.as_str().map(str::to_string)).collect();
                paths.push((alias.clone(), ts));
            }
        }
    }
    Some(ConfigEntry::TsConfig { base_url, paths })
}

/// Strip JSONC line + block comments (naive, matches the source's stripper).
fn strip_jsonc_comments(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            out.push(b as char);
            if b == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        // Push the full UTF-8 character (not just the byte) to keep non-ASCII intact.
        let ch_len = utf8_len(b);
        out.push_str(&raw[i..(i + ch_len).min(raw.len())]);
        i += ch_len;
    }
    out
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else if first >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

// ───────────────────────────────────────────────────────────── public surface ──

/// Resolve every file's imports into a deterministic [`DepGraph`].
///
/// `scan` provides files + raw imports; `root` is the system's directory on disk
/// (used only to read tsconfig.json / go.mod — file-existence probing is against
/// the scanned file set, never raw disk).
pub fn resolve_imports(scan: &ScanResult, root: &Path) -> DepGraph {
    // Build the probe set (scanned files, POSIX-normalized).
    let file_set: BTreeSet<String> = scan.files.iter().map(|f| to_posix(&f.file_path)).collect();

    // Which config families do we need?
    let want_ts = scan.files.iter().any(|f| matches!(f.language, Language::TypeScript | Language::JavaScript));
    let want_go = scan.files.iter().any(|f| matches!(f.language, Language::Go));
    let want_java = scan.files.iter().any(|f| matches!(f.language, Language::Java));
    let want_cs = scan.files.iter().any(|f| matches!(f.language, Language::CSharp));

    // Load configs ONCE (disk walk, canonicalized base to match scanner paths).
    let mut ts_configs = HashMap::new();
    let mut go_modules = HashMap::new();
    if want_ts || want_go {
        let base = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        collect_configs(&base, "", want_ts, want_go, &mut ts_configs, &mut go_modules);
    }

    // Index .go files by dir for package-level expansion.
    let mut go_files_by_dir: HashMap<String, Vec<String>> = HashMap::new();
    if want_go {
        for p in &file_set {
            if p.ends_with(".go") {
                go_files_by_dir.entry(dir_of(p).to_string()).or_default().push(p.clone());
            }
        }
        for v in go_files_by_dir.values_mut() {
            v.sort();
        }
    }

    let java_index = if want_java { build_suffix_index(&file_set, ".java") } else { HashMap::new() };
    let cs_index = if want_cs { build_suffix_index(&file_set, ".cs") } else { HashMap::new() };

    let ctx = Ctx { file_set, ts_configs, go_modules, go_files_by_dir, java_index, cs_index };

    // Resolve each file's imports.
    let mut edge_set: BTreeSet<DepEdge> = BTreeSet::new();
    let mut stats = DepGraphStats::default();

    for file in &scan.files {
        let from = to_posix(&file.file_path);
        let importer_dir = dir_of(&from).to_string();
        for imp in &file.imports {
            let targets = resolve_one_import(imp, &file.language, &importer_dir, &ctx);
            if targets.is_empty() {
                // Project-local-looking (relative) but unresolved vs genuine external.
                if imp.is_relative {
                    stats.unresolved_imports += 1;
                } else {
                    stats.external_imports += 1;
                }
                continue;
            }
            stats.internal_imports += 1;
            for to in targets {
                if to == from {
                    continue; // no self-edges
                }
                edge_set.insert(DepEdge { from: from.clone(), to, kind: DepEdgeKind::Import });
            }
        }
    }

    let mut nodes: Vec<String> = ctx.file_set.into_iter().collect();
    nodes.sort();
    let edges: Vec<DepEdge> = edge_set.into_iter().collect(); // BTreeSet → already sorted+deduped

    DepGraph { nodes, edges, stats }
}

/// Resolve the scanner's call sites into function→function `Call` edges, merged into
/// `dep`. A matchReference-lite cascade (codegraph): build a name→definition-files index,
/// then for each call pick a target by descending confidence — same-file (resolved, no
/// cross-file edge) → a name you imported → a unique global definition → ambiguous (skip).
/// Upgrades the import-level graph to call-level for clustering + flows. Deterministic.
pub fn resolve_calls(scan: &ScanResult, dep: &mut DepGraph) {
    let mut defs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in &scan.files {
        let f = to_posix(&file.file_path);
        let mut names = Vec::new();
        collect_def_names(&file.elements, &mut names);
        for n in names {
            defs.entry(n).or_default().insert(f.clone());
        }
    }

    // Existing (import) edges into a set so call edges merge, re-sort and de-dup.
    let mut edge_set: BTreeSet<DepEdge> = dep.edges.iter().cloned().collect();
    let (mut resolved, mut unresolved) = (0usize, 0usize);

    for file in &scan.files {
        let from = to_posix(&file.file_path);
        let imported: BTreeSet<&str> = dep
            .edges
            .iter()
            .filter(|e| e.from == from && e.kind == DepEdgeKind::Import)
            .map(|e| e.to.as_str())
            .collect();
        for call in &file.calls {
            match defs.get(&call.callee) {
                None => unresolved += 1, // external / builtin / unknown name
                Some(cands) => {
                    resolved += 1;
                    if let Some(target) = pick_call_target(&from, &imported, cands) {
                        edge_set.insert(DepEdge { from: from.clone(), to: target, kind: DepEdgeKind::Call });
                    }
                }
            }
        }
    }

    dep.edges = edge_set.into_iter().collect(); // BTreeSet → sorted + deduped
    dep.stats.resolved_calls = resolved;
    dep.stats.unresolved_calls = unresolved;
}

fn collect_def_names(elements: &[CodeElement], out: &mut Vec<String>) {
    for e in elements {
        if matches!(e.element_type, ElementType::Function | ElementType::Method | ElementType::Class) {
            out.push(e.name.clone());
        }
        collect_def_names(&e.children, out);
    }
}

/// Cross-file target for a resolved callee. `None` = same-file (no edge) or ambiguous.
fn pick_call_target(from: &str, imported: &BTreeSet<&str>, cands: &BTreeSet<String>) -> Option<String> {
    if cands.contains(from) {
        return None; // same-file call: resolved, but no cross-file edge
    }
    if let Some(t) = cands.iter().find(|c| imported.contains(c.as_str())) {
        return Some(t.clone()); // precise: you called something you imported
    }
    if cands.len() == 1 {
        return cands.iter().next().cloned(); // unique global definition
    }
    None // ambiguous → skip (matchReference ambiguity ceiling)
}

/// Build an in-memory `petgraph` directed graph from a [`DepGraph`]. Nodes are
/// added in `dep.nodes` order; the returned map gives each path's `NodeIndex`.
/// This is what the clustering pass (3B.2) consumes. Deterministic.
pub fn build_petgraph(dep: &DepGraph) -> (DiGraph<String, ()>, HashMap<String, NodeIndex>) {
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

// ─────────────────────────────────────────────────────── tiny std-only JSON ──
//
// Just enough to read tsconfig's `compilerOptions.baseUrl` (string) and `.paths`
// (object of string → array-of-strings). No serde_json dependency (guardrail).

mod minijson {
    /// A parsed JSON value. Objects keep insertion order (Vec of pairs).
    pub enum Value {
        Object(Vec<(String, Value)>),
        Array(Vec<Value>),
        Str(String),
        Other, // numbers / bool / null — we never read these from tsconfig
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn as_array(&self) -> Option<&[Value]> {
            match self {
                Value::Array(a) => Some(a),
                _ => None,
            }
        }
        pub fn as_object(&self) -> Option<&[(String, Value)]> {
            match self {
                Value::Object(o) => Some(o),
                _ => None,
            }
        }
    }

    struct P<'a> {
        b: &'a [u8],
        i: usize,
    }

    /// Parse a JSON document. Returns `None` on any malformed input.
    pub fn parse(s: &str) -> Option<Value> {
        let mut p = P { b: s.as_bytes(), i: 0 };
        p.ws();
        let v = p.value()?;
        p.ws();
        Some(v)
    }

    impl<'a> P<'a> {
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r' | b',') {
                self.i += 1;
            }
        }

        fn value(&mut self) -> Option<Value> {
            self.ws();
            match self.b.get(self.i)? {
                b'{' => self.object(),
                b'[' => self.array(),
                b'"' => self.string().map(Value::Str),
                _ => self.scalar(),
            }
        }

        fn object(&mut self) -> Option<Value> {
            self.i += 1; // {
            let mut pairs = Vec::new();
            loop {
                self.ws();
                match self.b.get(self.i)? {
                    b'}' => {
                        self.i += 1;
                        return Some(Value::Object(pairs));
                    }
                    b'"' => {
                        let key = self.string()?;
                        self.ws();
                        if self.b.get(self.i)? != &b':' {
                            return None;
                        }
                        self.i += 1;
                        let val = self.value()?;
                        pairs.push((key, val));
                    }
                    _ => return None,
                }
            }
        }

        fn array(&mut self) -> Option<Value> {
            self.i += 1; // [
            let mut items = Vec::new();
            loop {
                self.ws();
                match self.b.get(self.i)? {
                    b']' => {
                        self.i += 1;
                        return Some(Value::Array(items));
                    }
                    _ => items.push(self.value()?),
                }
            }
        }

        fn string(&mut self) -> Option<String> {
            self.i += 1; // opening quote
            let mut out = String::new();
            while let Some(&c) = self.b.get(self.i) {
                match c {
                    b'"' => {
                        self.i += 1;
                        return Some(out);
                    }
                    b'\\' => {
                        self.i += 1;
                        let e = *self.b.get(self.i)?;
                        match e {
                            b'n' => out.push('\n'),
                            b't' => out.push('\t'),
                            b'r' => out.push('\r'),
                            b'/' => out.push('/'),
                            b'\\' => out.push('\\'),
                            b'"' => out.push('"'),
                            // We don't need \uXXXX for tsconfig paths; keep the escaped char.
                            other => out.push(other as char),
                        }
                        self.i += 1;
                    }
                    _ => {
                        let len = super::utf8_len(c);
                        let s = std::str::from_utf8(&self.b[self.i..(self.i + len).min(self.b.len())]).ok()?;
                        out.push_str(s);
                        self.i += len;
                    }
                }
            }
            None // unterminated string
        }

        /// Consume a scalar (number / true / false / null) we don't interpret.
        fn scalar(&mut self) -> Option<Value> {
            let start = self.i;
            while let Some(&c) = self.b.get(self.i) {
                if matches!(c, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
                    break;
                }
                self.i += 1;
            }
            if self.i == start {
                return None;
            }
            Some(Value::Other)
        }
    }
}

#[cfg(test)]
mod tests;

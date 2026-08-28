//! Tier-0 enrich — **instant, deterministic, no LLM**. The optional prose overlay on
//! top of the deterministic core (the core never calls a model).
//!
//! It mines doc-comments already in the source (C# `///`, Python docstrings, JSDoc) for
//! real descriptions, and falls back to a humanized name where none exist. Output is an
//! overlay keyed by LinkReport ids, merged onto the model in the viewer:
//!
//! ```json
//! { "tier": "instant", "entities": {"<id>": {"description": "..."}}, "actions": {...}, "features": {...} }
//! ```
//!
//! Tier ordering (for idempotent escalation in the CLI): `none` < `instant` < `llm`.

use serde_json::{json, Map, Value};
use std::path::Path;

/// Rank a tier name for "don't downgrade / escalate only" logic. Unknown = 0.
pub fn tier_rank(tier: &str) -> u8 {
    match tier {
        "instant" => 1,
        "llm" => 2,
        _ => 0,
    }
}

/// Run the instant tier over a LinkReport (`graph/<id>.json` as JSON). `source_root` is the
/// system root the `code_ref` paths are relative to (LinkReport `metadata.inventory_file`).
pub fn run(report: &Value, source_root: &Path) -> Value {
    let mut entities = Map::new();
    for e in report["entities"].as_array().into_iter().flatten() {
        if let Some(id) = e["entity_id"].as_str() {
            let name = e["entity_name"].as_str().unwrap_or(id);
            let names: Vec<String> = e["fields"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f["name"].as_str().map(String::from))
                .collect();
            entities.insert(id.into(), json!({
                "description": describe(source_root, e["code_ref"].as_str(), name),
                "fields": field_types(source_root, e["code_ref"].as_str(), &names),
            }));
        }
    }
    let mut actions = Map::new();
    for a in report["actions"].as_array().into_iter().flatten() {
        if let Some(id) = a["action_id"].as_str() {
            let name = a["action_name"].as_str().unwrap_or(id);
            actions.insert(id.into(), json!({ "description": describe(source_root, a["code_ref"].as_str(), name) }));
        }
    }
    let mut features = Map::new();
    for f in report["features"].as_array().into_iter().flatten() {
        if let Some(id) = f["feature_id"].as_str() {
            let title = f["title"].as_str().unwrap_or(id);
            features.insert(id.into(), json!({ "description": humanize(title) }));
        }
    }
    json!({ "tier": "instant", "entities": entities, "actions": actions, "features": features })
}

fn describe(root: &Path, code_ref: Option<&str>, name: &str) -> String {
    code_ref
        .and_then(|cr| doc_comment(root, cr))
        .unwrap_or_else(|| humanize(name))
}

/// Pull a doc-comment for a symbol from its source. Heuristic, line-based: find the symbol's
/// declaration line, then collect C# `///` lines above it (skipping attributes/blanks), or a
/// Python `"""docstring"""` just below. Returns the cleaned one-liner, or None.
fn doc_comment(root: &Path, code_ref: &str) -> Option<String> {
    let (rel, sym_path) = code_ref.split_once(':')?;
    let symbol = sym_path.rsplit('.').next().unwrap_or(sym_path);
    let src = std::fs::read_to_string(root.join(rel)).ok()?;
    let lines: Vec<&str> = src.lines().collect();
    let decl = lines.iter().position(|l| {
        let t = l.trim_start();
        t.contains(&format!("class {symbol}"))
            || t.contains(&format!("{symbol}("))
            || t.contains(&format!("{symbol}<"))
            || t.contains(&format!("def {symbol}"))
            || t.contains(&format!("{symbol} ="))
    })?;

    // C#/JS `///` doc-comment block immediately above the declaration.
    let mut doc: Vec<String> = Vec::new();
    let mut started = false;
    let mut i = decl;
    while i > 0 {
        i -= 1;
        let t = lines[i].trim();
        if let Some(rest) = t.strip_prefix("///") {
            doc.push(rest.trim().to_string());
            started = true;
        } else if !started && (t.starts_with('[') || t.is_empty()) {
            continue; // attribute or blank before the doc block — keep scanning up
        } else {
            break;
        }
    }
    if !doc.is_empty() {
        doc.reverse();
        return Some(clean_xml(&doc.join(" ")));
    }

    // Python docstring on the first lines of the body.
    if rel.ends_with(".py") {
        for line in lines.iter().skip(decl + 1).take(3) {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("\"\"\"") {
                let one = rest.trim_end_matches("\"\"\"").trim();
                if !one.is_empty() {
                    return Some(one.to_string());
                }
            }
        }
    }
    None
}

/// Deterministic field types straight from the source (the type is right there in the code,
/// the scanner just doesn't capture it). Returns `{ name: {type, required} }` for the fields
/// whose declaration we can parse. C#/Java/TS-shaped declarations; misses leave the cell blank.
fn field_types(root: &Path, code_ref: Option<&str>, names: &[String]) -> Value {
    let mut out = Map::new();
    if names.is_empty() {
        return Value::Object(out);
    }
    let src = code_ref
        .and_then(|cr| cr.split_once(':'))
        .and_then(|(rel, _)| std::fs::read_to_string(root.join(rel)).ok());
    if let Some(src) = src {
        for line in src.lines() {
            for name in names {
                if out.contains_key(name) {
                    continue;
                }
                if let Some(ty) = field_decl_type(line, name) {
                    out.insert(name.clone(), json!({ "type": ty.trim_end_matches('?'), "required": !ty.ends_with('?') }));
                }
            }
        }
    }
    Value::Object(out)
}

const FIELD_MODS: &[&str] = &[
    "public", "private", "protected", "internal", "static", "readonly", "const", "volatile",
    "virtual", "override", "abstract", "sealed", "async", "extern", "unsafe", "new", "required", "partial",
];

/// Parse a field/property declaration line `<mods> <Type> <name> [;={]` and return `<Type>`.
fn field_decl_type(line: &str, name: &str) -> Option<String> {
    let t = line.trim();
    let pos = decl_pos(t, name)?;
    let mut words: Vec<&str> = t[..pos].split_whitespace().collect();
    while words.first().is_some_and(|w| FIELD_MODS.contains(w) || w.starts_with('[') || w.starts_with('@')) {
        words.remove(0);
    }
    let ty = words.join(" ").trim().to_string();
    if ty.is_empty() || ty.len() > 60 || ty.contains(['=', '(', ';']) || !is_typeish(&ty) {
        return None;
    }
    Some(ty)
}

/// A type token looks like a C#/Java/TS type: a primitive, or PascalCase (incl. generics/arrays).
fn is_typeish(ty: &str) -> bool {
    const PRIM: &[&str] = &[
        "int", "uint", "long", "ulong", "short", "ushort", "byte", "sbyte", "bool", "char",
        "string", "float", "double", "decimal", "object", "void", "nint", "nuint",
    ];
    let base = ty.split(['<', '?', '[', ' ']).next().unwrap_or(ty);
    PRIM.contains(&base) || base.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// Index where `name` appears as a declared member: a whole word followed by `;`, `=`, or `{`.
fn decl_pos(line: &str, name: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut start = 0;
    while let Some(rel) = line[start..].find(name) {
        let i = start + rel;
        let before_ok = i == 0 || (!b[i - 1].is_ascii_alphanumeric() && b[i - 1] != b'_');
        let after = &line[i + name.len()..];
        let boundary = after.as_bytes().first().is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_');
        let a = after.trim_start();
        let after_ok = a.starts_with(';') || a.starts_with('=') || a.starts_with('{');
        if before_ok && boundary && after_ok {
            return Some(i);
        }
        start = i + name.len();
    }
    None
}

/// Strip XML doc tags (`<summary>`, `<param>`, …) and collapse whitespace.
fn clean_xml(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `"AggregateAsync"` → `"Aggregate async"`, `"create_project"` → `"Create project"`.
fn humanize(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c == '_' {
            out.push(' ');
        } else {
            if c.is_uppercase() && i > 0 && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(if i == 0 { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() });
        }
    }
    out.trim().to_string()
}

// ───────────────────────── Tier 1: local-LLM prose ─────────────────────────
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const SCHEMA_VER: u64 = 1;
const MAX_SNIPPET_LINES: usize = 60;

pub struct LlmCfg {
    pub model: String,
    pub host: String,
    pub batch: usize,
    pub num_ctx: u64,
}
impl Default for LlmCfg {
    fn default() -> Self {
        Self {
            model: "qwen2.5-coder:7b-instruct-q4_K_M".into(),
            host: std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into()),
            batch: 10,
            num_ctx: 8192,
        }
    }
}

/// Tier-1 enrich: batched, schema-constrained local-LLM prose over the elements in `report`,
/// built on top of `prev` (the existing instant/llm overlay — its field types are preserved and
/// it doubles as the per-element cache). Cached by hash(snippet+model+schema); only changed
/// elements hit the model. Returns the merged overlay, or an error if no local model is reachable.
pub fn run_llm(report: &Value, source_root: &Path, prev: &Value, cfg: &LlmCfg) -> Result<Value, String> {
    preflight(cfg)?;
    let mut out = prev.clone();
    if !out.is_object() {
        out = json!({});
    }
    out["tier"] = json!("llm");
    for kind in ["entities", "actions"] {
        if !out[kind].is_object() {
            out[kind] = json!({});
        }
    }
    let id_key = |k: &str| if k == "entities" { "entity_id" } else { "action_id" };
    let name_key = |k: &str| if k == "entities" { "entity_name" } else { "action_name" };

    // Collect elements needing a call (snippet hash differs from the cached one).
    struct El { kind: &'static str, id: String, name: String, snip: String, hash: String }
    let mut todo: Vec<El> = Vec::new();
    for kind in ["entities", "actions"] {
        for e in report[kind].as_array().into_iter().flatten() {
            let (Some(id), Some(cr)) = (e[id_key(kind)].as_str(), e["code_ref"].as_str()) else { continue };
            let name = e[name_key(kind)].as_str().unwrap_or(id).to_string();
            let snip = snippet(source_root, cr);
            let hash = hash_str(&format!("{snip}|{}|{SCHEMA_VER}", cfg.model));
            if out[kind][id]["_h"].as_str() == Some(&hash) {
                continue; // cached
            }
            todo.push(El { kind, id: id.into(), name, snip, hash });
        }
    }
    if todo.is_empty() {
        return Ok(out);
    }

    let mut filled = 0usize;
    for chunk in todo.chunks(cfg.batch.max(1)) {
        let got = call_ollama(cfg, chunk.iter().map(|e| (e.kind, e.id.as_str(), e.name.as_str(), e.snip.as_str())).collect());
        for e in chunk {
            if let Some(mut v) = got.get(&e.id).cloned() {
                v["_h"] = json!(e.hash);
                // preserve tier-0 field types under the entity
                if let Some(fields) = out[e.kind][&e.id]["fields"].as_object() {
                    v["fields"] = json!(fields);
                }
                merge_into(&mut out[e.kind][&e.id], v);
                filled += 1;
            }
        }
    }
    if filled == 0 {
        return Err("model returned no usable output (check `ollama serve` / model)".into());
    }
    Ok(out)
}

/// Verify a local model is reachable before any batch — clean error instead of a hang.
fn preflight(cfg: &LlmCfg) -> Result<(), String> {
    let tags: Value = ureq::get(&format!("{}/api/tags", cfg.host))
        .timeout(std::time::Duration::from_secs(3))
        .call()
        .map_err(|_| format!("no local model at {} — run `ollama serve`", cfg.host))?
        .into_json()
        .map_err(|e| format!("bad /api/tags response: {e}"))?;
    // If a full tag (`name:tag`) was given, require an exact match; a bare name may prefix-match.
    let has = tags["models"].as_array().into_iter().flatten().any(|m| {
        m["name"].as_str().is_some_and(|n| {
            n == cfg.model || (!cfg.model.contains(':') && n.starts_with(&format!("{}:", cfg.model)))
        })
    });
    if !has {
        return Err(format!("model `{}` not found — run `ollama pull {}`", cfg.model, cfg.model));
    }
    Ok(())
}

/// One schema-constrained /api/chat call for a batch; returns id → enrichment object.
fn call_ollama(cfg: &LlmCfg, els: Vec<(&str, &str, &str, &str)>) -> std::collections::HashMap<String, Value> {
    let mut user = String::from("Document each element. Return a JSON array with one object per element, IN THE SAME ORDER, echoing its exact id.\n\n");
    for (i, (kind, id, name, snip)) in els.iter().enumerate() {
        let k = if *kind == "entities" { "entity" } else { "action" };
        user.push_str(&format!("{}. id={id} ({k}) {name}\n```\n{snip}\n```\n\n", i + 1));
    }
    let body = json!({
        "model": cfg.model, "stream": false, "keep_alive": "30m",
        "options": { "temperature": 0, "num_ctx": cfg.num_ctx, "num_predict": 2048 },
        "messages": [
            { "role": "system", "content":
              "You write FDML spec prose from code. For an action give: description, preconditions[], \
               postconditions[], side_effects[], output{entity,fields[]}. For an entity give: \
               description, relationships[{target,kind}]. Terse; infer only from the snippet. \
               Return a JSON array, one object per element in the given order, each echoing its exact `id`." },
            { "role": "user", "content": user }
        ],
        "format": array_schema(),
    });
    let parse = || -> Option<Vec<Value>> {
        let resp: Value = ureq::post(&format!("{}/api/chat", cfg.host))
            .timeout(std::time::Duration::from_secs(180))
            .send_json(body.clone()).ok()?.into_json().ok()?;
        let content = resp["message"]["content"].as_str()?;
        if std::env::var("FDML_ENRICH_DEBUG").is_ok() {
            eprintln!("DEBUG content: {content}");
        }
        serde_json::from_str::<Vec<Value>>(content).ok()
    };
    let arr = parse().or_else(parse).unwrap_or_default(); // one retry, then degrade
    // Match by echoed id; fall back to position (the array is in batch order).
    let mut out = std::collections::HashMap::new();
    for (i, (_, id, _, _)) in els.iter().enumerate() {
        if let Some(v) = arr.iter().find(|v| v["id"].as_str() == Some(*id)).or_else(|| arr.get(i)) {
            out.insert(id.to_string(), v.clone());
        }
    }
    out
}

fn array_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "description": { "type": "string" },
                "preconditions": { "type": "array", "items": { "type": "string" } },
                "postconditions": { "type": "array", "items": { "type": "string" } },
                "side_effects": { "type": "array", "items": { "type": "string" } },
                "output": { "type": "object", "properties": {
                    "entity": { "type": "string" }, "fields": { "type": "array", "items": { "type": "string" } } } },
                "relationships": { "type": "array", "items": { "type": "object", "properties": {
                    "target": { "type": "string" }, "kind": { "type": "string" } }, "required": ["target", "kind"] } }
            },
            "required": ["id", "description"]
        }
    })
}

/// Extract a symbol's body from source for the prompt — brace-balanced, capped.
fn snippet(root: &Path, code_ref: &str) -> String {
    let Some((rel, sym_path)) = code_ref.split_once(':') else { return String::new() };
    let symbol = sym_path.rsplit('.').next().unwrap_or(sym_path);
    let Ok(src) = std::fs::read_to_string(root.join(rel)) else { return String::new() };
    let lines: Vec<&str> = src.lines().collect();
    let Some(start) = lines.iter().position(|l| {
        let t = l.trim_start();
        ["class ", "struct ", "interface ", "record ", "enum ", "def "].iter().any(|kw| t.contains(&format!("{kw}{symbol}")))
            || t.contains(&format!("{symbol}(")) || t.contains(&format!("{symbol}<"))
    }) else { return String::new() };
    let mut out = Vec::new();
    let (mut depth, mut seen) = (0i32, false);
    for (k, line) in lines.iter().enumerate().skip(start) {
        if k - start >= MAX_SNIPPET_LINES { out.push("    // … (elided)"); break; }
        out.push(line);
        for c in line.chars() {
            if c == '{' { depth += 1; seen = true; } else if c == '}' { depth -= 1; }
        }
        if seen && depth <= 0 { break; }
        if !seen && k - start >= 3 { break; } // field / expression-bodied — a few lines
    }
    out.join("\n")
}

fn hash_str(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Shallow-merge object `src` into `dst` (src wins), leaving dst's other keys intact.
fn merge_into(dst: &mut Value, src: Value) {
    if let (Some(d), Value::Object(s)) = (dst.as_object_mut(), src) {
        for (k, v) in s {
            d.insert(k, v);
        }
    } else {
        *dst = json!({});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn humanize_and_clean() {
        assert_eq!(humanize("AggregateAsync"), "Aggregate async");
        assert_eq!(humanize("create_project"), "Create project");
        assert_eq!(clean_xml("<summary>Drops new value.</summary>"), "Drops new value.");
        assert!(tier_rank("llm") > tier_rank("instant"));
        assert!(tier_rank("instant") > tier_rank("none"));
    }
    #[test]
    fn field_decl() {
        let ty = |l, n| field_decl_type(l, n);
        assert_eq!(ty("private CancellationTokenSource cancellationTokenSource;", "cancellationTokenSource").as_deref(), Some("CancellationTokenSource"));
        assert_eq!(ty("public int Count { get; set; }", "Count").as_deref(), Some("int"));
        assert_eq!(ty("private readonly Dictionary<string, int> map;", "map").as_deref(), Some("Dictionary<string, int>"));
        assert_eq!(ty("string? name;", "name").as_deref(), Some("string?"));
        assert_eq!(ty("x = value;", "x"), None); // assignment, not a typed declaration
        assert_eq!(ty("void DoThing();", "DoThing"), None); // method, not a field
    }
}

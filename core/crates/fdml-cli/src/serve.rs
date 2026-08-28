//! `fdml serve` — feed the deterministic pipeline output into the bundled React viewer.
//!
//! Two jobs:
//!   1. *Adapter* — map our `.fdml/` artifacts (LinkReport / systems.json /
//!      integrations.json) onto the viewer's `FdmlDocument` shape (`web/src/api/types.ts`).
//!   2. *Server* — a tiny blocking HTTP/1.1 server (std::net, zero async deps) that serves
//!      the embedded `web/dist` plus `/api/spec` (platform) and `/api/spec/{id}` (system).
//!
//! Structure only: we map ids/names/fields/relationships, never invent descriptions,
//! preconditions, or other prose — that's a future LLM layer.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rust_embed::Embed;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use fdml_types::flow::Flow as CoreFlow;
use fdml_types::graph::{
    ActionLink, EntityLink, FeatureSuggestion, FieldLink, LinkReport, SystemRelationship, TraceLink,
};
use fdml_types::integration::PlatformLinks;
use fdml_types::System;

/// The prebuilt React viewer, embedded into the binary (path is relative to this crate).
#[derive(Embed)]
#[folder = "../../../web/dist/"]
struct WebAssets;

// ---------------------------------------------------------------------------
// Adapter: our artifacts -> the viewer's FdmlDocument shape (web/src/api/types.ts)
// ---------------------------------------------------------------------------

/// `FieldLink` -> `Field { name, type, default? }`. `type` is required in types.ts; we
/// emit the scanned type or "" when the scanner couldn't infer one (no invented value).
fn map_field(f: &FieldLink) -> Value {
    let mut o = json!({ "name": f.name, "type": f.field_type.clone().unwrap_or_default() });
    if let Some(d) = &f.default_value {
        o["default"] = json!(d);
    }
    o
}

/// `EntityLink` -> `Entity { id, name, fields }`. (`entity_id`->`id`, `entity_name`->`name`.)
fn map_entity(e: &EntityLink) -> Value {
    json!({
        "id": e.entity_id,
        "name": e.entity_name,
        "fields": e.fields.iter().map(map_field).collect::<Vec<_>>(),
    })
}

/// `ActionLink` -> `Action { id, name, description?, input?, output? }`. Our params become
/// `input.fields` (string list, matching `ActionData`); the return type becomes
/// `output.description`.
fn map_action(a: &ActionLink) -> Value {
    let mut o = json!({ "id": a.action_id, "name": a.action_name });
    if let Some(d) = &a.description {
        o["description"] = json!(d);
    }
    if !a.input.is_empty() {
        let fields: Vec<&str> = a.input.iter().map(|p| p.name.as_str()).collect();
        o["input"] = json!({ "fields": fields });
    }
    if let Some(out) = &a.output {
        o["output"] = json!({ "description": out });
    }
    o
}

/// `FeatureSuggestion` -> `Feature { id, title, scenarios }`. No scenarios are mined
/// deterministically yet, so `scenarios` is the required-but-empty array.
fn map_feature(f: &FeatureSuggestion) -> Value {
    json!({ "id": f.feature_id, "title": f.title, "scenarios": [] })
}

/// `TraceLink` -> `Traceability { from, to, relation, description? }` (drop `confidence`).
fn map_trace(t: &TraceLink) -> Value {
    let mut o = json!({ "from": t.from, "to": t.to, "relation": t.relation });
    if let Some(d) = &t.description {
        o["description"] = json!(d);
    }
    o
}

/// `SystemRelationship` -> `Relationship { from, to, type, description? }`.
fn map_rel(r: &SystemRelationship) -> Value {
    let mut o = json!({ "from": r.from, "to": r.to, "type": r.rel_type });
    if let Some(d) = &r.description {
        o["description"] = json!(d);
    }
    o
}

/// `Flow` -> `Flow { id, name, steps }`; each `FlowStep` -> `{ id, action, description }`.
fn map_flow(f: &CoreFlow) -> Value {
    json!({
        "id": f.id,
        "name": f.name,
        "steps": f.steps.iter().enumerate().map(|(i, s)| json!({
            "id": format!("{}_step_{}", f.id, i),
            "action": s.action_id,
            "description": s.description,
        })).collect::<Vec<_>>(),
    })
}

/// Per-system `LinkReport` (+ its flows) -> a system-scoped `FdmlDocument`.
fn system_doc(report: &LinkReport, flows: &[CoreFlow]) -> Value {
    json!({
        "metadata": { "version": report.metadata.linker_version },
        "system": {
            "id": report.system.id,
            "name": report.system.name,
            "components": report.system.components,
            "relationships": report.system.relationships.iter().map(map_rel).collect::<Vec<_>>(),
        },
        "entities": report.entities.iter().map(map_entity).collect::<Vec<_>>(),
        "actions": report.actions.iter().map(map_action).collect::<Vec<_>>(),
        "features": report.features.iter().map(map_feature).collect::<Vec<_>>(),
        "flows": flows.iter().map(map_flow).collect::<Vec<_>>(),
        "constraints": [],
        "traceability": report.traceability.iter().map(map_trace).collect::<Vec<_>>(),
        "generation_rules": [],
        // platform-level sections are empty in a system-scoped doc
        "contours": [],
        "systems": [],
        "integrations": [],
        "cross_flows": [],
        "shared_entities": [],
    })
}

/// Map our system_type onto the viewer's `SystemType` union (unknown/library -> service).
fn system_type_for(t: &str) -> &'static str {
    match t {
        "frontend" => "frontend",
        "gateway" => "gateway",
        "service" => "service",
        "worker" => "worker",
        "database" => "database",
        "queue" => "queue",
        "storage" => "storage",
        "external" => "external",
        _ => "service",
    }
}

/// Derive a (contour_id, contour_name) from a system_type. Presentation for UIs, Core for
/// services, etc. — gives the architecture view its trust-boundary lanes.
fn contour_for(t: &str) -> (&'static str, &'static str) {
    match t {
        "frontend" => ("presentation", "Presentation"),
        "gateway" => ("gateway", "Gateway"),
        "worker" => ("processing", "Processing"),
        "database" | "storage" => ("data", "Data"),
        "queue" => ("messaging", "Messaging"),
        "external" => ("external", "External"),
        _ => ("core", "Core"), // service, library, unknown
    }
}

/// Platform-level `FdmlDocument`: systems + derived contours + integrations + shared entities.
fn platform_doc(
    systems: &[System],
    links: &PlatformLinks,
    components: &HashMap<String, Vec<String>>,
) -> Value {
    // Distinct contours actually used (sorted by id for deterministic output).
    let mut contour_map: BTreeMap<&str, &str> = BTreeMap::new();
    for s in systems {
        let (id, name) = contour_for(&s.system_type);
        contour_map.insert(id, name);
    }
    let contours: Vec<Value> = contour_map
        .iter()
        .map(|(id, name)| json!({ "id": id, "name": name }))
        .collect();

    let system_entries: Vec<Value> = systems
        .iter()
        .map(|s| {
            let (contour, _) = contour_for(&s.system_type);
            json!({
                "id": s.id,
                "name": s.name,
                "type": system_type_for(&s.system_type),
                "technology": s.technology,
                "contour": contour,
                "components": components.get(&s.id).cloned().unwrap_or_default(),
                "relationships": [],
            })
        })
        .collect();

    let integrations: Vec<Value> = links
        .integrations
        .iter()
        .enumerate()
        .map(|(i, ig)| {
            json!({
                "id": format!("int_{i}"),
                "from": ig.from_system,
                "to": ig.to_system.clone().unwrap_or_else(|| "external".to_string()),
                "type": ig.integration_type,
                "protocol": ig.technology,
                "endpoints": [],
                "channels": [],
                "data_entities": [],
            })
        })
        .collect();

    let shared_entities: Vec<Value> = links
        .shared_entities
        .iter()
        .map(|se| {
            let contexts: Vec<Value> = se
                .systems
                .iter()
                .map(|(sid, fields)| {
                    let role = if Some(sid) == se.canonical_system.as_ref() {
                        "source"
                    } else {
                        "replica"
                    };
                    json!({
                        "system": sid,
                        "entity_id": se.entity_name,
                        "role": role,
                        "fields": fields,
                    })
                })
                .collect();
            let mut o = json!({ "entity": se.entity_name, "contexts": contexts });
            if let Some(c) = &se.canonical_system {
                o["canonical_system"] = json!(c);
            }
            o
        })
        .collect();

    json!({
        "metadata": { "version": "1.5" },
        "entities": [],
        "actions": [],
        "features": [],
        "flows": [],
        "constraints": [],
        "traceability": [],
        "generation_rules": [],
        "contours": contours,
        "systems": system_entries,
        "integrations": integrations,
        "cross_flows": [],
        "shared_entities": shared_entities,
    })
}

// ---------------------------------------------------------------------------
// Server: a tiny blocking HTTP/1.1 server over the embedded viewer + adapted JSON
// ---------------------------------------------------------------------------

/// Everything the request handlers need, computed once at startup and shared read-only.
struct ServeState {
    platform: String,
    system_docs: HashMap<String, String>,
}

fn read_json<T: DeserializeOwned>(path: PathBuf) -> anyhow::Result<T> {
    let data = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&data).map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))
}

/// Load `.fdml/` artifacts, build the JSON the viewer expects, and serve it.
pub fn run_serve(dir: &Path, port: u16, no_open: bool) -> anyhow::Result<()> {
    let systems: Vec<System> = read_json(dir.join("systems.json")).map_err(|e| {
        anyhow::anyhow!("{e}\n(run `fdml all <root>` first to generate .fdml/ artifacts)")
    })?;
    // integrations.json is optional — a platform with no cross-links still serves.
    let links: PlatformLinks = read_json(dir.join("integrations.json")).unwrap_or_default();

    let graph_dir = dir.join("graph");
    let mut components: HashMap<String, Vec<String>> = HashMap::new();
    let mut system_docs: HashMap<String, String> = HashMap::new();
    for s in &systems {
        let Ok(report) = read_json::<LinkReport>(graph_dir.join(format!("{}.json", s.id))) else {
            continue; // no graph for this system → it still appears in the platform list
        };
        components.insert(s.id.clone(), report.system.components.clone());
        let flows: Vec<CoreFlow> =
            read_json(graph_dir.join(format!("{}.flows.json", s.id))).unwrap_or_default();
        system_docs.insert(s.id.clone(), serde_json::to_string(&system_doc(&report, &flows))?);
    }
    let platform = serde_json::to_string(&platform_doc(&systems, &links, &components))?;

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .map_err(|e| anyhow::anyhow!("failed to bind {addr}: {e}"))?;
    let url = format!("http://localhost:{port}");
    println!("fdml serve: {} system(s), {} integration(s) -> {url}", systems.len(), links.integrations.len());
    if !no_open {
        let _ = std::process::Command::new("open").arg(&url).status();
    }

    let state = Arc::new(ServeState { platform, system_docs });
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = state.clone();
                std::thread::spawn(move || {
                    let _ = handle(stream, &state);
                });
            }
            Err(e) => eprintln!("connection error: {e}"),
        }
    }
    Ok(())
}

fn handle(mut stream: TcpStream, state: &ServeState) -> std::io::Result<()> {
    // Read just the request line ("GET /path HTTP/1.1"); we don't need headers/body for GET.
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/");

    let (status, ctype, body) = route(path, state);
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()
}

/// Route a request path to (status, content-type, body).
fn route(path: &str, state: &ServeState) -> (&'static str, &'static str, Vec<u8>) {
    let p = path.split('?').next().unwrap_or(path); // strip query string

    if p == "/api/spec" {
        return ("200 OK", "application/json", state.platform.clone().into_bytes());
    }
    if let Some(id) = p.strip_prefix("/api/spec/") {
        return match state.system_docs.get(id) {
            Some(doc) => ("200 OK", "application/json", doc.clone().into_bytes()),
            None => ("404 Not Found", "application/json", b"{}".to_vec()),
        };
    }
    // Stubs for the live-viewer endpoints (no live reload / generation in this mode).
    if p == "/api/generation-status" {
        return ("200 OK", "application/json", br#"{"phase":"idle","progress":0,"message":""}"#.to_vec());
    }
    if p == "/api/events" {
        return ("404 Not Found", "text/plain", Vec::new());
    }

    // Static assets from the embedded web/dist, with an SPA fallback to index.html.
    let rel = if p == "/" { "index.html" } else { p.trim_start_matches('/') };
    if let Some(f) = WebAssets::get(rel) {
        return ("200 OK", mime_for(rel), f.data.to_vec());
    }
    if let Some(f) = WebAssets::get("index.html") {
        return ("200 OK", "text/html; charset=utf-8", f.data.to_vec());
    }
    ("404 Not Found", "text/plain", b"Not found".to_vec())
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

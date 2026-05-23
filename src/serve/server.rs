use axum::{
    Router,
    extract::{Path, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Json,
    http::{header, StatusCode, Uri},
};
use rust_embed::Embed;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::parser::ast::FdmlDocument;
use crate::validator::rules::Validator;

#[derive(Embed)]
#[folder = "web/dist/"]
struct WebAssets;

#[derive(Clone, Debug, serde::Serialize)]
pub struct GenerationStatus {
    pub phase: String,       // "idle" | "scanning" | "generating" | "done" | "error"
    pub progress: f32,       // 0.0 - 1.0
    pub message: String,
}

impl Default for GenerationStatus {
    fn default() -> Self {
        Self {
            phase: "idle".to_string(),
            progress: 0.0,
            message: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub document: Arc<RwLock<FdmlDocument>>,
    pub file_path: PathBuf,
    pub tx: broadcast::Sender<()>,
    pub log_tx: broadcast::Sender<String>,
    pub log_buffer: Arc<RwLock<Vec<String>>>,
    pub generation_status: Arc<RwLock<GenerationStatus>>,
}

pub async fn run_server(
    state: AppState,
    port: u16,
    no_open: bool,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/spec", get(get_spec))
        .route("/api/spec/{system_id}", get(get_system_spec))
        .route("/api/validate", get(get_validate))
        .route("/api/events", get(sse_handler))
        .route("/api/generation-logs", get(generation_logs_handler))
        .route("/api/generation-status", get(generation_status_handler))
        .fallback(get(static_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let url = format!("http://localhost:{}", port);
    eprintln!("  Serving at {}", url);

    if !no_open {
        let _ = open::that(&url);
    }

    axum::serve(listener, app).await?;
    Ok(())
}

async fn get_spec(State(state): State<AppState>) -> Json<FdmlDocument> {
    let doc = state.document.read().await;
    Json(doc.clone())
}

async fn get_system_spec(
    State(state): State<AppState>,
    Path(system_id): Path<String>,
) -> Response {
    let doc = state.document.read().await;

    // Find the system entry
    let system_entry = doc.systems.iter().find(|s| s.id == system_id);
    let Some(entry) = system_entry else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": format!("System '{}' not found", system_id)
        }))).into_response();
    };

    // If spec path is provided, try to load the per-system file
    if let Some(ref spec_path) = entry.spec {
        let base_dir = state.file_path.parent().unwrap_or(std::path::Path::new("."));

        // Try multiple naming conventions:
        // 1. Exact path from spec field
        // 2. Platform prefix (e.g. "platform.system_id.fdml")
        let candidates = vec![
            base_dir.join(spec_path),
            {
                // Derive prefix from platform file name
                let platform_stem = state.file_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                base_dir.join(format!("{}.{}", platform_stem, spec_path))
            },
            // Also try with system_id directly
            base_dir.join(format!("{}.fdml", system_id)),
            {
                let platform_stem = state.file_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                base_dir.join(format!("{}.{}.fdml", platform_stem, system_id))
            },
        ];

        for candidate in candidates {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if let Ok(sub_doc) = crate::parser::parse_fdml_yaml(&content) {
                    return Json(sub_doc).into_response();
                }
            }
        }
    }

    // Return empty doc scoped to this system
    let empty = FdmlDocument::default();
    Json(empty).into_response()
}

async fn get_validate(State(state): State<AppState>) -> Json<serde_json::Value> {
    let doc = state.document.read().await;
    let validator = Validator::new();
    let errors = validator.validate(&doc).unwrap_or_default();
    Json(serde_json::json!({ "errors": errors, "count": errors.len() }))
}

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|_| {
        Ok(Event::default().event("spec-changed").data("reload"))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn generation_status_handler(
    State(state): State<AppState>,
) -> Json<GenerationStatus> {
    let status = state.generation_status.read().await;
    Json(status.clone())
}

async fn generation_logs_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    // First: replay buffered logs so the client sees history
    let buffered = state.log_buffer.read().await.clone();
    let history = tokio_stream::iter(
        buffered.into_iter().map(|line| Ok(Event::default().event("log").data(line)))
    );

    // Then: stream new logs via broadcast
    let rx = state.log_tx.subscribe();
    let live = BroadcastStream::new(rx).filter_map(|msg| {
        match msg {
            Ok(line) => Some(Ok(Event::default().event("log").data(line))),
            Err(_) => None,
        }
    });

    let stream = history.chain(live);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try exact path first
    if let Some(file) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data.to_vec(),
        )
            .into_response();
    }

    // SPA fallback: serve index.html for any non-file path
    if let Some(file) = WebAssets::get("index.html") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            file.data.to_vec(),
        )
            .into_response();
    }

    (StatusCode::NOT_FOUND, "Not found").into_response()
}

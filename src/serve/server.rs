use axum::{
    Router,
    extract::State,
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

#[derive(Clone)]
pub struct AppState {
    pub document: Arc<RwLock<FdmlDocument>>,
    pub file_path: PathBuf,
    pub tx: broadcast::Sender<()>,
}

pub async fn run_server(
    state: AppState,
    port: u16,
    no_open: bool,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/spec", get(get_spec))
        .route("/api/validate", get(get_validate))
        .route("/api/events", get(sse_handler))
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

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

use crate::parser::ast::FdmlDocument;
use crate::parser::parse_fdml_yaml;

pub fn start_watcher(
    file_path: PathBuf,
    document: Arc<RwLock<FdmlDocument>>,
    tx: broadcast::Sender<()>,
    extra_watch_paths: Vec<PathBuf>,
) -> anyhow::Result<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>> {
    let watched_path = file_path.clone();

    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            if let Ok(events) = events {
                let relevant = events.iter().any(|e| {
                    e.kind == DebouncedEventKind::Any
                });
                if !relevant {
                    return;
                }

                // Re-read and re-parse
                let content = match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("  Watch: failed to read file: {}", e);
                        return;
                    }
                };

                let new_doc = match parse_fdml_yaml(&content) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("  Watch: parse error: {}", e);
                        return;
                    }
                };

                // Update state and notify
                let doc = document.clone();
                let tx = tx.clone();
                // Use a blocking approach since we're in a sync callback
                let rt = tokio::runtime::Handle::try_current();
                if let Ok(handle) = rt {
                    handle.spawn(async move {
                        let mut w = doc.write().await;
                        *w = new_doc;
                        let _ = tx.send(());
                        eprintln!("  Watch: spec reloaded");
                    });
                }
            }
        },
    )?;

    // Watch the main file
    debouncer
        .watcher()
        .watch(&watched_path, notify::RecursiveMode::NonRecursive)?;

    // FDML 1.4: Also watch per-system spec files
    for extra_path in &extra_watch_paths {
        if extra_path.exists() {
            if let Err(e) = debouncer.watcher().watch(extra_path, notify::RecursiveMode::NonRecursive) {
                eprintln!("  Watch: could not watch spec file {:?}: {}", extra_path, e);
            } else {
                eprintln!("  Watch: also watching {:?}", extra_path);
            }
        }
    }

    Ok(debouncer)
}

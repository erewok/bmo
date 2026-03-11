use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    response::Redirect,
    routing::{get, post},
};

pub mod handlers;
pub mod templates;

/// Capacity of the SSE broadcast channel.  Large enough to absorb bursts
/// without dropping slow subscribers in normal usage.
const SSE_BROADCAST_CAPACITY: usize = 16;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub env: Arc<minijinja::Environment<'static>>,
    pub shutdown: tokio::sync::watch::Receiver<bool>,
    /// Shared broadcaster for SSE events.  Handlers subscribe to this instead
    /// of polling the DB individually.
    pub events_tx: tokio::sync::broadcast::Sender<String>,
}

pub async fn start_server(host: &str, port: u16, db_path: PathBuf) -> anyhow::Result<()> {
    let env = Arc::new(templates::make_env());
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (events_tx, _events_rx) = tokio::sync::broadcast::channel(SSE_BROADCAST_CAPACITY);

    // Spawn a single background task that polls the DB every 3 seconds and
    // broadcasts a serialized SSE payload to all connected subscribers.
    {
        let db_path_bg = db_path.clone();
        let events_tx_bg = events_tx.clone();
        let mut shutdown_bg = shutdown_rx.clone();
        tokio::spawn(async move {
            // Seed initial snapshot so we only emit on real changes.
            let mut last_snapshot: String = tokio::task::spawn_blocking({
                let p = db_path_bg.clone();
                move || handlers::board_snapshot(&p)
            })
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default();

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(3)) => {}
                    _ = shutdown_bg.changed() => { break; }
                }

                let db_path_poll = db_path_bg.clone();
                let current_snapshot =
                    tokio::task::spawn_blocking(move || handlers::board_snapshot(&db_path_poll))
                        .await
                        .ok()
                        .and_then(|r| r.ok())
                        .unwrap_or_default();

                if current_snapshot != last_snapshot {
                    last_snapshot = current_snapshot;
                    let timestamp = chrono::Utc::now().to_rfc3339();
                    let payload = serde_json::json!({
                        "type": "board_updated",
                        "timestamp": timestamp,
                    })
                    .to_string();
                    // send() only fails when there are zero receivers — that's fine.
                    let _ = events_tx_bg.send(payload);
                }
            }
        });
    }

    let state = AppState {
        db_path,
        env,
        shutdown: shutdown_rx,
        events_tx,
    };

    let app = Router::new()
        .route("/favicon.ico", get(handlers::favicon))
        .route("/logo.png", get(handlers::logo))
        .route("/", get(|| async { Redirect::permanent("/board") }))
        .route("/board", get(handlers::board_page))
        .route("/issues", get(handlers::issue_list_page))
        .route("/issues/:id", get(handlers::issue_detail_page))
        .route("/graph", get(handlers::graph_page))
        .route("/api/issues", get(handlers::api_issue_list))
        .route("/api/issues/:id", get(handlers::api_issue_detail))
        .route("/api/issues/:id/comments", post(handlers::api_post_comment))
        .route("/api/graph", get(handlers::api_graph))
        .route("/api/board", get(handlers::api_board))
        .route("/api/stats", get(handlers::api_stats))
        .route("/api/events", get(handlers::api_events))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("bmo web running at http://{addr}");
    println!("Press Ctrl+C to stop.");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.ok();
            println!("\nbmo web shutting down.");
            let _ = shutdown_tx.send(true);
        })
        .await?;
    Ok(())
}

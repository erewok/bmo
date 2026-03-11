use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    response::Redirect,
    routing::{get, post},
};

pub mod handlers;
pub mod templates;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub env: Arc<minijinja::Environment<'static>>,
}

pub async fn start_server(host: &str, port: u16, db_path: PathBuf) -> anyhow::Result<()> {
    let env = Arc::new(templates::make_env());
    let state = AppState { db_path, env };

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
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            println!("\nbmo web shutting down.");
        })
        .await?;
    Ok(())
}

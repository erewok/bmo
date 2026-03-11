use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::sse::{Event, Sse},
    response::{Html, IntoResponse, Json},
};
use futures_util::stream;
use minijinja::context;
use serde::Deserialize;

use crate::db::{AddCommentInput, Repository, open_db};
use crate::model::{IssueFilter, Kind, Priority, Status};

use super::AppState;

// ── Query parameter structs ───────────────────────────────────────────────────

/// Query parameters accepted by GET /api/issues.
#[derive(Debug, Default, Deserialize)]
pub struct IssueQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub q: Option<String>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub priority: Option<String>,
    /// defaults to false; pass ?include_done=true to include done issues
    pub include_done: Option<bool>,
}

/// Query parameters accepted by GET /api/board.
#[derive(Debug, Default, Deserialize)]
pub struct BoardQuery {
    pub limit: Option<usize>,
}

const DEFAULT_LIMIT: usize = 50;

// TODO: replace with per-session identity once authentication exists
const WEB_COMMENT_AUTHOR: &str = "web";

// ── Static asset handlers ─────────────────────────────────────────────────────

pub async fn favicon() -> impl IntoResponse {
    let bytes = include_bytes!("../../assets/bmo-clear-bg.ico");
    ([(header::CONTENT_TYPE, "image/x-icon")], bytes.as_ref()).into_response()
}

pub async fn logo() -> impl IntoResponse {
    let bytes = include_bytes!("../../assets/bmo-full.png");
    ([(header::CONTENT_TYPE, "image/png")], bytes.as_ref()).into_response()
}

// ── HTML page handlers ────────────────────────────────────────────────────────

pub async fn board_page(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;

        // Group issues by status into column objects, applying a per-column limit
        let statuses = [
            (Status::Backlog, "Backlog"),
            (Status::Todo, "Todo"),
            (Status::InProgress, "In Progress"),
            (Status::Review, "Review"),
            (Status::Done, "Done"),
        ];

        let columns: Vec<serde_json::Value> = statuses
            .iter()
            .map(|(status, label)| -> anyhow::Result<serde_json::Value> {
                // The Done column intentionally truncates at DEFAULT_LIMIT to keep the board
                // view focused and performant. Full history is available on the /issues page.
                // include_done: false defers to the status filter above; Done issues are still
                // shown because `status` is explicitly set to Status::Done for that column.
                let col_issues_raw = repo.list_issues(&IssueFilter {
                    status: Some(vec![*status]),
                    include_done: false,
                    limit: Some(DEFAULT_LIMIT),
                    ..Default::default()
                })?;
                let col_issues: Vec<serde_json::Value> = col_issues_raw
                    .iter()
                    .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(serde_json::json!({
                    "label": label,
                    "issues": col_issues,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let tmpl = state.env.get_template("board.html")?;
        let html = tmpl.render(context!(columns => columns))?;
        anyhow::Ok(html)
    })
    .await;

    match result {
        Ok(Ok(html)) => Html(html).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
    }
}

pub async fn issue_list_page(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        let issues = repo.list_issues(&IssueFilter {
            include_done: true,
            limit: Some(DEFAULT_LIMIT),
            offset: Some(0),
            ..Default::default()
        })?;
        let issues_json: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>, _>>()?;

        let tmpl = state.env.get_template("issue_list.html")?;
        let html = tmpl.render(context!(issues => issues_json))?;
        anyhow::Ok(html)
    })
    .await;

    match result {
        Ok(Ok(html)) => Html(html).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
    }
}

pub async fn issue_detail_page(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        let issue = repo.get_issue(id)?;
        if let Some(issue) = issue {
            let comments = repo.list_comments(id)?;
            let issue_json = serde_json::to_value(&issue)?;
            let comments_json: Vec<serde_json::Value> = comments
                .iter()
                .map(|c| serde_json::to_value(c).map_err(|e| anyhow::anyhow!(e)))
                .collect::<Result<Vec<_>, _>>()?;
            let tmpl = state.env.get_template("issue.html")?;
            let html = tmpl.render(context!(issue => issue_json, comments => comments_json))?;
            anyhow::Ok(Some(html))
        } else {
            anyhow::Ok(None)
        }
    })
    .await;

    match result {
        Ok(Ok(Some(html))) => Html(html).into_response(),
        Ok(Ok(None)) => {
            (StatusCode::NOT_FOUND, Html("Issue not found".to_string())).into_response()
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
    }
}

// ── JSON API handlers ─────────────────────────────────────────────────────────

pub async fn api_issue_list(
    State(state): State<AppState>,
    Query(params): Query<IssueQuery>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;

        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = params.offset.unwrap_or(0);

        // Parse optional single-value filters into the Vec<T> that IssueFilter expects.
        let status_filter: Option<Vec<Status>> = params
            .status
            .as_deref()
            .map(|s| s.parse::<Status>().map(|v| vec![v]))
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid status: {e}"))?;

        let kind_filter: Option<Vec<Kind>> = params
            .kind
            .as_deref()
            .map(|s| s.parse::<Kind>().map(|v| vec![v]))
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid kind: {e}"))?;

        let priority_filter: Option<Vec<Priority>> = params
            .priority
            .as_deref()
            .map(|s| s.parse::<Priority>().map(|v| vec![v]))
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid priority: {e}"))?;

        let include_done = params.include_done.unwrap_or(false);

        let filter = IssueFilter {
            include_done,
            status: status_filter.clone(),
            kind: kind_filter.clone(),
            priority: priority_filter.clone(),
            search: params.q.clone(),
            limit: Some(limit),
            offset: Some(offset),
            ..Default::default()
        };

        // Count total matching records (without limit/offset) for pagination metadata.
        let count_filter = IssueFilter {
            include_done,
            status: status_filter,
            kind: kind_filter,
            priority: priority_filter,
            search: params.q,
            limit: None,
            offset: None,
            ..Default::default()
        };

        let issues = repo.list_issues(&filter)?;
        let total = repo.count_issues(&count_filter)? as usize;

        let issues_json: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>, _>>()?;

        anyhow::Ok((issues_json, total, limit, offset))
    })
    .await;

    match result {
        Ok(Ok((data, total, limit, offset))) => Json(serde_json::json!({
            "ok": true,
            "data": data,
            "total": total,
            "limit": limit,
            "offset": offset,
        }))
        .into_response(),
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn api_issue_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        repo.get_issue(id)
    })
    .await;

    match result {
        Ok(Ok(Some(issue))) => Json(serde_json::json!({"ok": true, "data": issue})).into_response(),
        Ok(Ok(None)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"ok": false, "error": "issue not found"})),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn api_board(
    State(state): State<AppState>,
    Query(params): Query<BoardQuery>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        let per_column_limit = params.limit.unwrap_or(DEFAULT_LIMIT);

        let col_statuses: &[(&str, &str, Status)] = &[
            ("backlog", "Backlog", Status::Backlog),
            ("todo", "Todo", Status::Todo),
            ("in_progress", "In Progress", Status::InProgress),
            ("review", "Review", Status::Review),
            ("done", "Done", Status::Done),
        ];

        let columns: Vec<serde_json::Value> = col_statuses
            .iter()
            .map(|(col_key, label, status)| -> anyhow::Result<serde_json::Value> {
                let col_issues = repo.list_issues(&IssueFilter {
                    status: Some(vec![*status]),
                    include_done: false,
                    limit: Some(per_column_limit),
                    ..Default::default()
                })?;
                let col_json: Vec<serde_json::Value> = col_issues
                    .iter()
                    .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(serde_json::json!({
                    "status": col_key,
                    "label": label,
                    "issues": col_json,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;

        anyhow::Ok(serde_json::json!({ "columns": columns }))
    })
    .await;

    match result {
        Ok(Ok(data)) => Json(serde_json::json!({"ok": true, "data": data})).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn api_stats(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        repo.get_stats()
    })
    .await;

    match result {
        Ok(Ok(stats)) => Json(serde_json::json!({"ok": true, "data": stats})).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── Comment body ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PostCommentBody {
    pub body: String,
}

/// Typed sentinel for domain-level failures in [`api_post_comment`].
#[derive(Debug)]
pub enum CommentError {
    /// The referenced issue does not exist.
    NotFound,
    /// The issue is currently in-progress; agent has priority.
    InProgress,
}

// ── POST /api/issues/:id/comments ─────────────────────────────────────────────

/// Post a comment on an issue.
///
/// Returns 409 if the issue is currently in-progress (agent has priority).
/// Returns 400 if the comment body is empty.
/// Returns 404 if the issue does not exist.
/// Returns 201 with the created comment on success.
pub async fn api_post_comment(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<PostCommentBody>,
) -> impl IntoResponse {
    // Validate body is non-empty before hitting the DB.
    let body_text = payload.body.trim().to_string();
    if body_text.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "error": "body must not be empty"})),
        )
            .into_response();
    }

    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;

        // Fetch the issue to confirm it exists and check its status.
        let issue = repo.get_issue(id)?;
        let issue = match issue {
            Some(i) => i,
            None => return anyhow::Ok(Err(CommentError::NotFound)),
        };

        // Conflict: agent has priority when issue is in-progress.
        if issue.status == Status::InProgress {
            return anyhow::Ok(Err(CommentError::InProgress));
        }

        let comment = repo.add_comment(&AddCommentInput {
            issue_id: id,
            body: body_text,
            author: Some(WEB_COMMENT_AUTHOR.to_string()),
        })?;

        anyhow::Ok(Ok(comment))
    })
    .await;

    match result {
        Ok(Ok(Ok(comment))) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"ok": true, "data": comment})),
        )
            .into_response(),
        Ok(Ok(Err(CommentError::NotFound))) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"ok": false, "error": "issue not found"})),
        )
            .into_response(),
        Ok(Ok(Err(CommentError::InProgress))) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"ok": false, "error": "Issue is in progress — agent has priority"})),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── GET /api/events (SSE) ─────────────────────────────────────────────────────

/// SSE stream that emits a `board_updated` event whenever the board changes.
///
/// Polls the database every 3 seconds, comparing the maximum `updated_at`
/// timestamp across all issues. Emits an event when a change is detected.
/// Clients reconnect automatically via the native EventSource protocol.
pub async fn api_events(
    State(state): State<AppState>,
) -> Sse<impl stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    // Seed the initial snapshot so we only emit events on actual changes.
    let initial_snapshot = {
        let db_path = state.db_path.clone();
        tokio::task::spawn_blocking(move || board_snapshot(&db_path))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default()
    };

    let sse_stream = async_stream::stream! {
        let mut last_snapshot = initial_snapshot;
        loop {
            // Wait 3 seconds between polls.
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            let db_path = state.db_path.clone();
            let current_snapshot =
                tokio::task::spawn_blocking(move || board_snapshot(&db_path))
                    .await
                    .ok()
                    .and_then(|r| r.ok())
                    .unwrap_or_default();

            if current_snapshot != last_snapshot {
                let timestamp = chrono::Utc::now().to_rfc3339();
                let data = serde_json::json!({
                    "type": "board_updated",
                    "timestamp": timestamp,
                })
                .to_string();
                last_snapshot = current_snapshot;
                yield Ok::<Event, std::convert::Infallible>(Event::default().event("board_updated").data(data));
            }
            // No change — emit nothing; axum KeepAlive handles TCP keepalive.
        }
    };

    Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(tokio::time::Duration::from_secs(15))
            .text("ping"),
    )
}

// ── GET /graph ────────────────────────────────────────────────────────────────

pub async fn graph_page(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let tmpl = state.env.get_template("graph.html")?;
        let html = tmpl.render(context!())?;
        anyhow::Ok(html)
    })
    .await;

    match result {
        Ok(Ok(html)) => Html(html).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<pre>Error: {e}</pre>")),
        )
            .into_response(),
    }
}

// ── GET /api/graph ────────────────────────────────────────────────────────────

pub async fn api_graph(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;

        // Fetch all issues (include done so the graph is complete)
        let issues = repo.list_issues(&IssueFilter {
            include_done: true,
            limit: None,
            offset: None,
            ..Default::default()
        })?;

        let nodes: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| {
                serde_json::json!({
                    "id":       i.id,
                    "title":    i.title,
                    "status":   i.status.label(),
                    "priority": i.priority.label(),
                    "kind":     i.kind.label(),
                })
            })
            .collect();

        // Fetch all relations
        let relations = repo.list_all_relations()?;
        let edges: Vec<serde_json::Value> = relations
            .iter()
            .map(|r| {
                serde_json::json!({
                    "from": r.from_id,
                    "to":   r.to_id,
                    "kind": r.kind.label(),
                })
            })
            .collect();

        anyhow::Ok(serde_json::json!({ "nodes": nodes, "edges": edges }))
    })
    .await;

    match result {
        Ok(Ok(data)) => Json(serde_json::json!({"ok": true, "data": data})).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Returns a snapshot string representing the current board state.
///
/// Uses the maximum `updated_at` timestamp combined with the total issue count
/// so that both edits and additions/deletions are detected.
fn board_snapshot(db_path: &std::path::Path) -> anyhow::Result<String> {
    let repo = open_db(db_path)?;
    let (count, max_updated) = repo.board_snapshot_stats()?;
    let max_updated_str = max_updated.map(|t| t.to_rfc3339()).unwrap_or_default();
    Ok(format!("{count}:{max_updated_str}"))
}

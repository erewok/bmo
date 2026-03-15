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
    /// defaults to false; pass ?all=true to include done issues
    pub all: Option<bool>,
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

        // Fetch all board columns — one query per status (5 total) — then
        // build the ordered column list for the template.
        let by_status = repo.list_issues_by_status(DEFAULT_LIMIT)?;

        let col_defs: &[(&str, &str, Status)] = &[
            ("backlog", "Backlog", Status::Backlog),
            ("todo", "Todo", Status::Todo),
            ("in_progress", "In Progress", Status::InProgress),
            ("review", "Review", Status::Review),
            ("done", "Done", Status::Done),
        ];

        let columns: Vec<serde_json::Value> = col_defs
            .iter()
            .map(
                |(col_key, label, status)| -> anyhow::Result<serde_json::Value> {
                    let col_issues_raw = by_status.get(status).map(Vec::as_slice).unwrap_or(&[]);
                    let col_issues: Vec<serde_json::Value> = col_issues_raw
                        .iter()
                        .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(serde_json::json!({
                        "status": col_key,
                        "label": label,
                        "issues": col_issues,
                    }))
                },
            )
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
        let tmpl = state.env.get_template("issue_list.html")?;
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

        let findall = params.all.unwrap_or(false);

        let mut filter = IssueFilter {
            findall,
            status: status_filter.clone(),
            kind: kind_filter.clone(),
            priority: priority_filter.clone(),
            search: params.q.clone(),
            limit: Some(limit),
            offset: Some(offset),
            ..Default::default()
        };

        // Count total matching records (without limit/offset) for pagination metadata.
        let mut count_filter = IssueFilter {
            findall,
            status: status_filter,
            kind: kind_filter,
            priority: priority_filter,
            search: params.q,
            limit: None,
            offset: None,
            ..Default::default()
        };

        let issues = repo.list_issues(&mut filter)?;
        let total = repo.count_issues(&mut count_filter)? as usize;

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

        // Fetch all board columns — one query per status (5 total).
        let by_status = repo.list_issues_by_status(per_column_limit)?;

        let col_defs: &[(&str, &str, Status)] = &[
            ("backlog", "Backlog", Status::Backlog),
            ("todo", "Todo", Status::Todo),
            ("in_progress", "In Progress", Status::InProgress),
            ("review", "Review", Status::Review),
            ("done", "Done", Status::Done),
        ];

        let columns: Vec<serde_json::Value> = col_defs
            .iter()
            .map(
                |(col_key, label, status)| -> anyhow::Result<serde_json::Value> {
                    let col_issues_raw = by_status.get(status).map(Vec::as_slice).unwrap_or(&[]);
                    let col_json: Vec<serde_json::Value> = col_issues_raw
                        .iter()
                        .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(serde_json::json!({
                        "status": col_key,
                        "label": label,
                        "issues": col_json,
                    }))
                },
            )
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
/// Subscribes to the shared [`AppState::events_tx`] broadcast channel, which is
/// fed by a single background poller in [`super`].  This avoids opening the DB
/// once per connected client per tick — all clients share the same 3-second
/// poll.
///
/// If the subscriber's channel buffer overflows (i.e. a client is very slow),
/// `RecvError::Lagged` is returned; we log a warning and continue rather than
/// dropping the connection.
pub async fn api_events(
    State(state): State<AppState>,
) -> Sse<impl stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut rx = state.events_tx.subscribe();
    let mut shutdown = state.shutdown.clone();

    let sse_stream = async_stream::stream! {
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(data) => {
                            yield Ok::<Event, std::convert::Infallible>(
                                Event::default().event("board_updated").data(data)
                            );
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("bmo SSE: subscriber lagged, skipped {n} message(s)");
                            // Continue — don't drop the connection.
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            // Broadcaster shut down; end the stream.
                            break;
                        }
                    }
                }
                _ = shutdown.changed() => { break; }
            }
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

        // Fetch all issues so we can identify active ones and their parents.
        let all_issues = repo.list_issues(&mut IssueFilter {
            findall: true,
            ..Default::default()
        })?
;
        use std::collections::{HashMap, HashSet};

        // Index all issues by id for parent lookups.
        let by_id: HashMap<i64, _> = all_issues.iter().map(|i| (i.id, i)).collect();

        // Active issues: status != done.
        let active_ids: HashSet<i64> = all_issues
            .iter()
            .filter(|i| i.status != Status::Done)
            .map(|i| i.id)
            .collect();

        // Collect done immediate parents of any active issue.
        //
        // Design decision: only one level up (direct parent_id) is included here.
        // Pulling in the full ancestor chain would drag in large swaths of the
        // hierarchy that are irrelevant to day-to-day work, making the graph
        // noisy and hard to read.  One level is enough to provide context for
        // an active issue without overwhelming the view.
        //
        // Future work: if multi-level traversal is ever wanted, consider adding
        // a `?depth=N` query parameter to this endpoint and walking the ancestor
        // chain up to N steps.
        let completed_parent_ids: HashSet<i64> = all_issues
            .iter()
            .filter(|i| active_ids.contains(&i.id))
            .filter_map(|i| i.parent_id)
            .filter(|pid| {
                // Only include if the parent is done and not already active.
                by_id
                    .get(pid)
                    .map(|p| p.status == Status::Done)
                    .unwrap_or(false)
                    && !active_ids.contains(pid)
            })
            .collect();

        // Union of node ids that will appear in the graph.
        let visible_ids: HashSet<i64> = active_ids.union(&completed_parent_ids).copied().collect();

        // Build node list: active nodes first, then completed parents.
        let mut nodes: Vec<serde_json::Value> = Vec::new();
        for i in &all_issues {
            if active_ids.contains(&i.id) {
                nodes.push(serde_json::json!({
                    "id":        i.id,
                    "title":     i.title,
                    "status":    i.status.label(),
                    "priority":  i.priority.label(),
                    "kind":      i.kind.label(),
                    "completed": false,
                }));
            } else if completed_parent_ids.contains(&i.id) {
                nodes.push(serde_json::json!({
                    "id":        i.id,
                    "title":     i.title,
                    "status":    i.status.label(),
                    "priority":  i.priority.label(),
                    "kind":      i.kind.label(),
                    "completed": true,
                }));
            }
        }

        // Fetch all relations; only keep edges where both endpoints are visible.
        let relations = repo.list_all_relations()?;
        let edges: Vec<serde_json::Value> = relations
            .iter()
            .filter(|r| visible_ids.contains(&r.from_id) && visible_ids.contains(&r.to_id))
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
///
/// Public so that the background broadcaster in [`super`] can call it.
pub fn board_snapshot(db_path: &std::path::Path) -> anyhow::Result<String> {
    let repo = open_db(db_path)?;
    let (count, max_updated) = repo.board_snapshot_stats()?;
    let max_updated_str = max_updated.map(|t| t.to_rfc3339()).unwrap_or_default();
    Ok(format!("{count}:{max_updated_str}"))
}

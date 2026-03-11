use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Json},
};
use minijinja::context;

use crate::db::{Repository, open_db};
use crate::model::{IssueFilter, Status};

use super::AppState;

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
        let issues = repo.list_issues(&IssueFilter {
            include_done: true,
            ..Default::default()
        })?;

        // Group issues by status into column objects
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
                let col_issues: Vec<serde_json::Value> = issues
                    .iter()
                    .filter(|i| i.status == *status)
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

pub async fn api_issue_list(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        let issues = repo.list_issues(&IssueFilter {
            include_done: true,
            ..Default::default()
        })?;
        let issues_json: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| serde_json::to_value(i).map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>, _>>()?;
        anyhow::Ok(issues_json)
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

pub async fn api_board(State(state): State<AppState>) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || {
        let repo = open_db(&state.db_path)?;
        let issues = repo.list_issues(&IssueFilter {
            include_done: true,
            ..Default::default()
        })?;

        let mut board: std::collections::HashMap<&str, Vec<serde_json::Value>> = [
            ("backlog", vec![]),
            ("todo", vec![]),
            ("in_progress", vec![]),
            ("review", vec![]),
            ("done", vec![]),
        ]
        .into_iter()
        .collect();

        for issue in issues {
            let col = match issue.status {
                Status::Backlog => "backlog",
                Status::Todo => "todo",
                Status::InProgress => "in_progress",
                Status::Review => "review",
                Status::Done => "done",
            };
            let val = serde_json::to_value(&issue).map_err(|e| anyhow::anyhow!(e))?;
            board.get_mut(col).unwrap().push(val); // safe: col is always a known key from the match
        }

        anyhow::Ok(serde_json::to_value(board)?)
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

use axum::body::Body;
use http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use bmo::db::{CreateIssueInput, Repository, open_db};
use bmo::model::{Kind, Priority, RelationKind, Status};
use bmo::web::{build_router, test_state};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create a temp directory, initialize a SQLite DB inside it, build the Axum
/// router, and return all three guards. The caller must hold `TempDir` so the
/// DB file is not deleted, and must hold the `watch::Sender` so the shutdown
/// channel stays open (dropping it would terminate SSE streams immediately).
fn setup_app() -> (axum::Router, TempDir, tokio::sync::watch::Sender<bool>) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("issues.db");
    // open_db runs schema initialization
    open_db(&db_path).unwrap();
    let (state, shutdown_guard) = test_state(db_path);
    let router = build_router(state);
    (router, dir, shutdown_guard)
}

/// Seed one issue in the DB and return its id.
fn create_test_issue(dir: &TempDir) -> i64 {
    let db_path = dir.path().join("issues.db");
    let repo = open_db(&db_path).unwrap();
    let issue = repo
        .create_issue(&CreateIssueInput {
            parent_id: None,
            title: "Test issue".to_string(),
            description: "A test issue".to_string(),
            status: Status::Todo,
            priority: Priority::Medium,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            actor: None,
        })
        .unwrap();
    issue.id
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn api_issues_empty() {
    let (app, _dir, _shutdown) = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/issues")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert!(json["data"].is_array());
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn api_issue_detail_found() {
    let (app, dir, _shutdown) = setup_app();
    let id = create_test_issue(&dir);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/issues/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["id"], id);
}

#[tokio::test]
async fn api_issue_detail_not_found() {
    let (app, _dir, _shutdown) = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/issues/9999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn api_post_comment_success() {
    let (app, dir, _shutdown) = setup_app();
    let id = create_test_issue(&dir);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/issues/{id}/comments"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"body":"hello"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
}

#[tokio::test]
async fn api_post_comment_empty_body() {
    let (app, dir, _shutdown) = setup_app();
    let id = create_test_issue(&dir);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/issues/{id}/comments"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"body":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn api_stats_ok() {
    let (app, _dir, _shutdown) = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
}

#[tokio::test]
async fn issue_detail_page_renders_markdown() {
    let (app, dir, _shutdown) = setup_app();

    // Create an issue with markdown in the description
    let db_path = dir.path().join("issues.db");
    let repo = open_db(&db_path).unwrap();
    let issue = repo
        .create_issue(&CreateIssueInput {
            parent_id: None,
            title: "Markdown Test".to_string(),
            description: "**Bold text** and _italic_".to_string(),
            status: Status::Todo,
            priority: Priority::Medium,
            kind: Kind::Task,
            assignee: None,
            labels: vec![],
            files: vec![],
            actor: None,
        })
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/issues/{}", issue.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = std::str::from_utf8(&body).unwrap();

    // This test guards against the markdown filter being de-registered or the
    // `| markdown` pipe being removed from the template.
    // Markdown should be rendered to HTML, not shown as raw text.
    assert!(
        html.contains("<strong>Bold text</strong>"),
        "Expected <strong>Bold text</strong> in rendered HTML, but raw markdown may have been escaped"
    );
    assert!(
        !html.contains("**Bold text**"),
        "Raw markdown syntax should not appear in rendered HTML output"
    );
    assert!(
        html.contains("<em>italic</em>"),
        "Expected <em>italic</em> in rendered HTML"
    );
    assert!(
        !html.contains("_italic_"),
        "Raw markdown syntax should not appear in rendered HTML output"
    );
}

/// Fetch `/issues/{id}` and return the markup of its relation list.
async fn relation_list_html(app: axum::Router, id: i64) -> String {
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/issues/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    let (_, after_open) = html
        .split_once(r#"<ul class="relation-list">"#)
        .expect("issue page has a relation list");
    let (list, _) = after_open.split_once("</ul>").unwrap();
    list.to_string()
}

#[tokio::test]
async fn issue_detail_page_on_the_to_side_links_the_other_issue_with_the_inverted_verb() {
    let cases = [
        (RelationKind::Blocks, "← blocked by"),
        (RelationKind::BlockedBy, "→ blocks"),
        (RelationKind::DependsOn, "← dependency of"),
        (RelationKind::DependencyOf, "→ depends on"),
        (RelationKind::RelatesTo, "↔ relates to"),
        (RelationKind::Duplicates, "← duplicate of"),
        (RelationKind::DuplicateOf, "→ duplicates"),
    ];
    for (stored_kind, expected_verb) in cases {
        let (app, dir, _shutdown) = setup_app();
        let from_id = create_test_issue(&dir);
        let shown_id = create_test_issue(&dir);
        open_db(&dir.path().join("issues.db"))
            .unwrap()
            .add_relation(from_id, stored_kind, shown_id)
            .unwrap();

        let list = relation_list_html(app, shown_id).await;

        let expected = format!(r#"{expected_verb} <a href="/issues/{from_id}">BMO-{from_id}</a>"#);
        assert!(
            list.contains(&expected),
            "stored kind {stored_kind}: expected `{expected}` in `{list}`"
        );
        assert_eq!(
            list.matches("<li>").count(),
            1,
            "stored kind {stored_kind}: `{list}`"
        );
        assert!(
            !list.contains(&format!(r#"href="/issues/{shown_id}""#)),
            "stored kind {stored_kind}: relation links to the shown issue itself in `{list}`"
        );
    }
}

#[tokio::test]
async fn issue_detail_page_renders_each_relation_from_the_shown_issue() {
    let (app, dir, _shutdown) = setup_app();
    let first = create_test_issue(&dir);
    let second = create_test_issue(&dir);
    let third = create_test_issue(&dir);
    let repo = open_db(&dir.path().join("issues.db")).unwrap();
    repo.add_relation(first, RelationKind::Blocks, second)
        .unwrap();
    repo.add_relation(third, RelationKind::BlockedBy, second)
        .unwrap();

    let blocked_by_first = format!(r#"← blocked by <a href="/issues/{first}">BMO-{first}</a>"#);
    let blocks_third = format!(r#"→ blocks <a href="/issues/{third}">BMO-{third}</a>"#);
    let blocks_second = format!(r#"→ blocks <a href="/issues/{second}">BMO-{second}</a>"#);
    let blocked_by_second = format!(r#"← blocked by <a href="/issues/{second}">BMO-{second}</a>"#);

    let list = relation_list_html(app.clone(), second).await;
    assert!(list.contains(&blocked_by_first), "`{list}`");
    assert!(list.contains(&blocks_third), "`{list}`");
    assert!(
        !list.contains(&format!(r#"href="/issues/{second}""#)),
        "relation links to the shown issue itself in `{list}`"
    );

    let list = relation_list_html(app.clone(), first).await;
    assert!(list.contains(&blocks_second), "`{list}`");

    let list = relation_list_html(app, third).await;
    assert!(list.contains(&blocked_by_second), "`{list}`");
}

#[tokio::test]
async fn board_page_renders() {
    let (app, _dir, _shutdown) = setup_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/board")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/html"),
        "expected text/html content-type, got: {content_type}"
    );
}

#[tokio::test]
async fn issue_detail_page_flags_a_self_relation() {
    let (app, dir, _shutdown) = setup_app();
    let id = create_test_issue(&dir);
    // `add_relation` rejects self-links, so write the row directly.
    rusqlite::Connection::open(dir.path().join("issues.db"))
        .unwrap()
        .execute(
            "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (?1, ?1, 'blocks')",
            rusqlite::params![id],
        )
        .unwrap();

    let list = relation_list_html(app, id).await;
    assert!(list.contains("(invalid self-link)"), "`{list}`");
}

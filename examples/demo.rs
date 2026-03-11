//! BMO Demo — the keyboard demo song for your issue tracker.
//!
//! Run with: `cargo run --example demo`
//! Run fast:  `cargo run --example demo -- --fast`
//!
//! Spawns a local web server, opens a fresh temp database, then walks through
//! every major BMO capability in a scripted narrative while you watch the
//! board update in your browser.

use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

use bmo::db::{AddCommentInput, CreateIssueInput, Repository, UpdateIssueInput, open_db};
use bmo::model::{IssueFilter, Kind, Priority, Status};
use bmo::model::relation::RelationKind;
use bmo::web::start_server;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pause(fast: bool) {
    let ms = if fast { 200 } else { 2000 };
    std::thread::sleep(Duration::from_millis(ms));
}

fn pause_long(fast: bool) {
    let ms = if fast { 400 } else { 4000 };
    std::thread::sleep(Duration::from_millis(ms));
}

fn section(title: &str) {
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  {title}");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

fn narrate(msg: &str) {
    println!("  >> {msg}");
}

fn created(kind: &str, id: i64, title: &str) {
    println!("  [+] {kind} BMO-{id}: {title}");
}

fn moved(id: i64, from: Status, to: Status) {
    println!("  [~] BMO-{id}: {} -> {}", from.label(), to.label());
}

fn find_free_port() -> u16 {
    // Bind port 0 to get an OS-assigned free port, record it, then release.
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral port");
    listener.local_addr().unwrap().port()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse --fast flag
    let fast = std::env::args().any(|a| a == "--fast");

    // ── Temp database ──────────────────────────────────────────────────────
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let db_path: PathBuf = tmp.path().join("demo.db");

    // Initialize the schema by opening once and dropping.
    {
        let _repo = open_db(&db_path)?;
    }

    // ── Web server ─────────────────────────────────────────────────────────
    let port = find_free_port();
    let url = format!("http://127.0.0.1:{port}");

    {
        let db_path_clone = db_path.clone();
        tokio::spawn(async move {
            // Ignore the result — the server may be shut down when the demo
            // process exits.
            let _ = start_server("127.0.0.1", port, db_path_clone).await;
        });
    }

    // Give the server a moment to bind.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // ── Intro ──────────────────────────────────────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║           BMO  —  The Demo Song                  ║");
    println!("║   A cinematic walkthrough of your issue tracker  ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("  Web view is live at: {url}");
    println!("  Open that URL in your browser, then watch the board");
    println!("  update as the story unfolds.");
    println!();
    if !fast {
        println!("  (Running in normal mode — ~2s between steps.)");
        println!("  (Pass --fast for a 0.2s pace.)");
    } else {
        println!("  (Running in --fast mode.)");
    }

    // Wait for the user to open the browser
    pause_long(fast);

    // ── Open a repo handle for the rest of the demo ────────────────────────
    // We re-open after each logical group to avoid holding the connection
    // across the tokio yield points (SqliteRepository is not Send).
    // Instead we open/close synchronously in a blocking context.

    // =========================================================================
    // ACT 1: PLANNING
    // =========================================================================
    section("Act 1: Planning — the epic begins");
    narrate("A small team sits down to plan the next big thing:");
    narrate("building a next-generation AI-powered task tracker.");
    narrate("First, they create the epic that will hold it all together.");
    pause(fast);

    let (epic_id, feat_db_id, feat_web_id, feat_cli_id, task_docs_id, bug_perf_id) = {
        let repo = open_db(&db_path)?;

        let epic = repo.create_issue(&CreateIssueInput {
            parent_id: None,
            title: "Launch NOVA — AI-Powered Task Tracker v1.0".into(),
            description: "The overarching epic for the initial public release of NOVA. \
                          Covers all features, docs, and bug fixes required to ship v1.0."
                .into(),
            status: Status::Backlog,
            priority: Priority::High,
            kind: Kind::Epic,
            assignee: None,
            labels: vec!["v1.0".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Epic", epic.id, &epic.title);
        pause(fast);

        narrate("Now the child issues — features, a task, and a bug, all lurking in the backlog.");

        let feat_db = repo.create_issue(&CreateIssueInput {
            parent_id: Some(epic.id),
            title: "Design and implement SQLite schema".into(),
            description: "Define the core relational schema for issues, comments, labels, \
                          and relations. Include migration strategy."
                .into(),
            status: Status::Backlog,
            priority: Priority::Critical,
            kind: Kind::Feature,
            assignee: Some("alice".into()),
            labels: vec!["backend".into(), "v1.0".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Feature", feat_db.id, &feat_db.title);
        pause(fast);

        let feat_web = repo.create_issue(&CreateIssueInput {
            parent_id: Some(epic.id),
            title: "Build real-time web board with SSE updates".into(),
            description: "Axum-based web server with a Kanban board page that pushes \
                          live updates to the browser via Server-Sent Events."
                .into(),
            status: Status::Backlog,
            priority: Priority::High,
            kind: Kind::Feature,
            assignee: Some("bob".into()),
            labels: vec!["frontend".into(), "v1.0".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Feature", feat_web.id, &feat_web.title);
        pause(fast);

        let feat_cli = repo.create_issue(&CreateIssueInput {
            parent_id: Some(epic.id),
            title: "Implement CLI issue CRUD commands".into(),
            description: "All the standard create/read/update/delete commands for issues, \
                          plus list, show, move, close, and reopen."
                .into(),
            status: Status::Backlog,
            priority: Priority::High,
            kind: Kind::Feature,
            assignee: Some("alice".into()),
            labels: vec!["cli".into(), "v1.0".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Feature", feat_cli.id, &feat_cli.title);
        pause(fast);

        let task_docs = repo.create_issue(&CreateIssueInput {
            parent_id: Some(epic.id),
            title: "Write README and quick-start guide".into(),
            description: "A clear README covering installation, configuration, and \
                          the top 10 most common workflows."
                .into(),
            status: Status::Backlog,
            priority: Priority::Medium,
            kind: Kind::Task,
            assignee: Some("carol".into()),
            labels: vec!["docs".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Task", task_docs.id, &task_docs.title);
        pause(fast);

        let bug_perf = repo.create_issue(&CreateIssueInput {
            parent_id: Some(epic.id),
            title: "Issue list is slow with >1000 rows".into(),
            description: "Profiling shows a missing index on the status column. \
                          The full table scan makes the board unusable at scale."
                .into(),
            status: Status::Backlog,
            priority: Priority::High,
            kind: Kind::Bug,
            assignee: None,
            labels: vec!["performance".into(), "backend".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Bug", bug_perf.id, &bug_perf.title);

        (
            epic.id,
            feat_db.id,
            feat_web.id,
            feat_cli.id,
            task_docs.id,
            bug_perf.id,
        )
    };

    pause_long(fast);

    // =========================================================================
    // ACT 2: KICKOFF
    // =========================================================================
    section("Act 2: Kickoff — the board starts moving");
    narrate("Sprint planning is done. Work begins. Alice and Bob pick up their tickets.");

    {
        let repo = open_db(&db_path)?;

        // Move DB feature: backlog -> todo -> in-progress
        repo.update_issue(feat_db_id, &UpdateIssueInput {
            status: Some(Status::Todo),
            ..Default::default()
        })?;
        moved(feat_db_id, Status::Backlog, Status::Todo);
        pause(fast);

        repo.update_issue(feat_db_id, &UpdateIssueInput {
            status: Some(Status::InProgress),
            ..Default::default()
        })?;
        moved(feat_db_id, Status::Todo, Status::InProgress);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_db_id,
            body: "Working on the initial schema. Starting with the issues table and \
                   foreign key constraints. Will open PR when migrations are ready."
                .into(),
            author: Some("alice".into()),
        })?;
        narrate(&format!(
            "Alice left a comment on BMO-{feat_db_id}: 'Working on the initial schema...'"
        ));
        pause(fast);

        // Move CLI feature: backlog -> todo -> in-progress
        repo.update_issue(feat_cli_id, &UpdateIssueInput {
            status: Some(Status::Todo),
            ..Default::default()
        })?;
        moved(feat_cli_id, Status::Backlog, Status::Todo);
        pause(fast);

        repo.update_issue(feat_cli_id, &UpdateIssueInput {
            status: Some(Status::InProgress),
            ..Default::default()
        })?;
        moved(feat_cli_id, Status::Todo, Status::InProgress);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_cli_id,
            body: "Scaffolding the clap CLI structure. Using the derive API for \
                   subcommands. Will wire up DB calls once the schema lands."
                .into(),
            author: Some("alice".into()),
        })?;
        narrate(&format!(
            "Alice also picked up BMO-{feat_cli_id}: CLI commands are in-flight."
        ));
        pause(fast);

        // Move web feature to todo
        repo.update_issue(feat_web_id, &UpdateIssueInput {
            status: Some(Status::Todo),
            ..Default::default()
        })?;
        moved(feat_web_id, Status::Backlog, Status::Todo);
        narrate(&format!(
            "Bob moved the web board feature (BMO-{feat_web_id}) to todo — waiting on the schema first."
        ));
    }

    pause_long(fast);

    // =========================================================================
    // ACT 3: DEPENDENCIES
    // =========================================================================
    section("Act 3: Dependencies — the graph tells a story");
    narrate("Bob realizes the web board can't start until the DB schema is done.");
    narrate("He adds a 'blocked-by' relation. Check the Graph view in your browser.");

    {
        let repo = open_db(&db_path)?;

        let rel = repo.add_relation(feat_web_id, RelationKind::BlockedBy, feat_db_id)?;
        println!(
            "  [~] Added relation: BMO-{feat_web_id} blocked-by BMO-{feat_db_id} (relation #{})",
            rel.id
        );
        narrate("The graph now shows the dependency chain. Nothing ships until the foundation is laid.");
    }

    pause_long(fast);

    // =========================================================================
    // ACT 4: PROGRESS
    // =========================================================================
    section("Act 4: Progress — issues march toward Done");
    narrate("Alice finishes the schema. It's beautiful. It's reviewed. It ships.");

    {
        let repo = open_db(&db_path)?;

        repo.update_issue(feat_db_id, &UpdateIssueInput {
            status: Some(Status::Review),
            ..Default::default()
        })?;
        moved(feat_db_id, Status::InProgress, Status::Review);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_db_id,
            body: "PR is up. Schema includes indexes on status, priority, and parent_id. \
                   WAL mode enabled. Migrations run automatically on startup."
                .into(),
            author: Some("alice".into()),
        })?;
        pause(fast);

        repo.update_issue(feat_db_id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(feat_db_id, Status::Review, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_db_id,
            body: "Merged. Schema is live. All tests pass. Closing."
                .into(),
            author: Some("carol".into()),
        })?;
        narrate(&format!("BMO-{feat_db_id} is done! The foundation is in place."));
        pause(fast);

        // Now unblock the web feature
        narrate(&format!(
            "The blocker is resolved. Bob moves the web board (BMO-{feat_web_id}) into in-progress."
        ));
        repo.update_issue(feat_web_id, &UpdateIssueInput {
            status: Some(Status::InProgress),
            ..Default::default()
        })?;
        moved(feat_web_id, Status::Todo, Status::InProgress);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_web_id,
            body: "Schema is merged. Starting the Axum handlers and Jinja2 templates. \
                   SSE endpoint will stream board updates every 10s."
                .into(),
            author: Some("bob".into()),
        })?;
        pause(fast);

        // Alice finishes CLI
        repo.update_issue(feat_cli_id, &UpdateIssueInput {
            status: Some(Status::Review),
            ..Default::default()
        })?;
        moved(feat_cli_id, Status::InProgress, Status::Review);
        pause(fast);

        repo.update_issue(feat_cli_id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(feat_cli_id, Status::Review, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_cli_id,
            body: "All CRUD commands implemented and tested. \
                   `bmo issue create/list/show/edit/move/close` all working."
                .into(),
            author: Some("alice".into()),
        })?;
        narrate(&format!("BMO-{feat_cli_id}: CLI commands shipped!"));
        pause(fast);

        // Bob's web feature completes
        repo.update_issue(feat_web_id, &UpdateIssueInput {
            status: Some(Status::Review),
            ..Default::default()
        })?;
        moved(feat_web_id, Status::InProgress, Status::Review);
        pause(fast);

        repo.update_issue(feat_web_id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(feat_web_id, Status::Review, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: feat_web_id,
            body: "Web board is live! SSE updates working. Board, issues list, detail page, \
                   and graph view all rendering correctly. Merged."
                .into(),
            author: Some("bob".into()),
        })?;
        narrate(&format!("BMO-{feat_web_id}: The web board is live. You are literally looking at it right now."));
    }

    pause_long(fast);

    // =========================================================================
    // ACT 5: A BUG APPEARS
    // =========================================================================
    section("Act 5: A bug appears — all hands on deck");
    narrate("The performance bug surfaces in staging. 1200 issues. Screaming.");
    narrate("It moves fast — this is a fire drill.");

    let _bug_sprint_id = {
        let repo = open_db(&db_path)?;

        // Move the pre-existing performance bug to in-progress (it was in backlog)
        repo.update_issue(bug_perf_id, &UpdateIssueInput {
            status: Some(Status::InProgress),
            assignee: Some("alice".into()),
            ..Default::default()
        })?;
        moved(bug_perf_id, Status::Backlog, Status::InProgress);

        repo.add_comment(&AddCommentInput {
            issue_id: bug_perf_id,
            body: "Confirmed. `EXPLAIN QUERY PLAN` shows a full table scan. \
                   Adding index on (status, priority, id). ETA: 20 minutes."
                .into(),
            author: Some("alice".into()),
        })?;
        narrate(&format!("BMO-{bug_perf_id}: Alice is on it. 20-minute ETA."));
        pause(fast);

        // Also create a brand-new bug that was just discovered
        let new_bug = repo.create_issue(&CreateIssueInput {
            parent_id: Some(epic_id),
            title: "SSE connection drops after 30s in Safari".into(),
            description: "Safari disconnects EventSource connections after 30 seconds \
                          if no data is received. Need to send keepalive pings."
                .into(),
            status: Status::InProgress,
            priority: Priority::Critical,
            kind: Kind::Bug,
            assignee: Some("bob".into()),
            labels: vec!["frontend".into(), "safari".into()],
            files: vec![],
            actor: Some("demo".into()),
        })?;
        created("Bug", new_bug.id, &new_bug.title);
        narrate(&format!("BMO-{}: Bob spotted a Safari SSE bug in testing. Assigning to himself.", new_bug.id));
        pause(fast);

        // Resolve the performance bug
        repo.update_issue(bug_perf_id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(bug_perf_id, Status::InProgress, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: bug_perf_id,
            body: "Index added. List query time dropped from 890ms to 3ms. Merged and deployed."
                .into(),
            author: Some("alice".into()),
        })?;
        narrate(&format!("BMO-{bug_perf_id}: Fixed in under 20 minutes. Alice is a wizard."));
        pause(fast);

        // Resolve Safari bug
        repo.update_issue(new_bug.id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(new_bug.id, Status::InProgress, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: new_bug.id,
            body: "Added 15s keepalive ping to the SSE stream. Safari now maintains the \
                   connection indefinitely. Tested in Safari 17."
                .into(),
            author: Some("bob".into()),
        })?;
        narrate(&format!("BMO-{}: Safari SSE bug squashed. Keepalive ping added.", new_bug.id));

        new_bug.id
    };

    pause_long(fast);

    // =========================================================================
    // ACT 6: WRAP-UP
    // =========================================================================
    section("Act 6: Wrap-up — the epic closes");
    narrate("Docs are written. The board is nearly full of green checkmarks.");
    narrate("Time to close the epic.");

    {
        let repo = open_db(&db_path)?;

        // Move docs task through to done
        repo.update_issue(task_docs_id, &UpdateIssueInput {
            status: Some(Status::InProgress),
            ..Default::default()
        })?;
        moved(task_docs_id, Status::Backlog, Status::InProgress);
        pause(fast);

        repo.update_issue(task_docs_id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(task_docs_id, Status::InProgress, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: task_docs_id,
            body: "README written. Quick-start guide covers install, init, and the top 10 \
                   workflows. Published to docs site."
                .into(),
            author: Some("carol".into()),
        })?;
        narrate(&format!("BMO-{task_docs_id}: Docs are live. The README is excellent."));
        pause(fast);

        // Close the epic
        repo.update_issue(epic_id, &UpdateIssueInput {
            status: Some(Status::Done),
            ..Default::default()
        })?;
        moved(epic_id, Status::Backlog, Status::Done);

        repo.add_comment(&AddCommentInput {
            issue_id: epic_id,
            body: "NOVA v1.0 is shipped! All child issues resolved. \
                   The team did incredible work. See you at v1.1."
                .into(),
            author: Some("demo".into()),
        })?;
        narrate(&format!("BMO-{epic_id}: Epic closed. NOVA v1.0 is out the door."));
        pause(fast);

        // Print summary stats
        let stats = repo.get_stats()?;
        let all_issues = repo.list_issues(&IssueFilter {
            include_done: true,
            ..Default::default()
        })?;

        let total_comments: usize = all_issues
            .iter()
            .map(|i| repo.list_comments(i.id).unwrap_or_default().len())
            .sum();

        let all_relations = repo.list_all_relations()?;

        println!();
        println!("  Summary:");
        println!("  --------");
        println!("  Total issues created : {}", stats.total);
        println!("  Comments added       : {total_comments}");
        println!("  Relations created    : {}", all_relations.len());
        println!("  Issues done          : {}", stats.by_status.get("done").copied().unwrap_or(0));
        println!();
        narrate("Take a final look at the board — every column tells the story of a sprint.");
    }

    pause_long(fast);

    // =========================================================================
    // EXIT
    // =========================================================================
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║              Demo complete.                      ║");
    println!("║                                                  ║");
    println!("║  That's BMO — local-first, agent-friendly,       ║");
    println!("║  and fast enough to track how it was built.      ║");
    println!("║                                                  ║");
    println!("║  The temp database has been cleaned up.          ║");
    println!("║  Run `bmo init` to start your own project.       ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    // `tmp` is dropped here, removing the temp directory.
    drop(tmp);

    Ok(())
}

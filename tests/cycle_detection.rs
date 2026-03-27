// Cycle detection integration tests.
//
// Covers every surface that participates in DAG integrity:
//   1. `bmo issue link add`  — cycle rejected at insertion (exit 3)
//   2. `bmo plan`            — fails loud on a cyclic graph
//   3. `bmo next`            — fails loud on a cyclic graph
//   4. `bmo agent-init`      — fails loud on a cyclic graph
//
// The downstream tests (2–4) need a cyclic graph that bypassed the insertion
// guard, so they inject the cycle directly into the SQLite DB.

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::str::contains;
use rusqlite::Connection;
use std::process::Command;
use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> TempDir {
    let dir = TempDir::new().unwrap();
    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    dir
}

fn bmo(dir: &TempDir) -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("bmo"));
    cmd.current_dir(dir.path());
    cmd
}

fn create_issues(dir: &TempDir, n: usize) -> Vec<String> {
    (1..=n)
        .map(|i| {
            bmo(dir)
                .args(["issue", "create", "--title", &format!("Issue {i}")])
                .assert()
                .success();
            format!("BMO-{i}")
        })
        .collect()
}

fn link(dir: &TempDir, ids: &[String], from_idx: usize, rel: &str, to_idx: usize) {
    bmo(dir)
        .args([
            "issue",
            "link",
            "add",
            &ids[from_idx - 1],
            rel,
            &ids[to_idx - 1],
        ])
        .assert()
        .success();
}

/// Bypass the insertion guard and write a `blocks` edge directly to the DB.
/// Used only to set up the downstream loud-failure tests.
fn inject_blocks_edge(dir: &TempDir, from_id: i64, to_id: i64) {
    let conn = Connection::open(dir.path().join(".bmo/issues.db")).unwrap();
    conn.execute(
        "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (?1, ?2, 'blocks')",
        rusqlite::params![from_id, to_id],
    )
    .unwrap();
}

/// Build a dir with issues 1 and 2 linked 1→2 via the CLI, then inject 2→1
/// directly to create a cycle that bypassed the insertion guard.
fn setup_with_injected_cycle() -> TempDir {
    let dir = setup();
    create_issues(&dir, 2);
    link(&dir, &["BMO-1".into(), "BMO-2".into()], 1, "blocks", 2);
    inject_blocks_edge(&dir, 2, 1);
    dir
}

// ── 1. Cycle prevention at link insertion ─────────────────────────────────────

struct CycleCase {
    #[allow(dead_code)]
    name: &'static str,
    issue_count: usize,
    /// (from_idx, relation, to_idx) — 1-based, matches BMO-N numbering
    setup_links: &'static [(usize, &'static str, usize)],
    rejected: (usize, &'static str, usize),
}

/// DAG-edge relations that must be rejected when they would form a cycle.
///
/// Each case exercises a distinct code path in `can_reach_impl`:
/// - `blocks` uses the forward direction (A→B)
/// - `depends-on` uses the reversed direction (DependsOn(A,B) = DAG edge B→A)
/// - The transitive case exercises the BFS beyond depth 1
/// - The self-loop case hits the `start == target` early return
/// - The cross-kind case exercises mixing `blocks` and `depends-on` edges
static CYCLE_CASES: &[CycleCase] = &[
    CycleCase {
        name: "blocks direct: A blocks B, then B blocks A",
        issue_count: 2,
        setup_links: &[(1, "blocks", 2)],
        rejected: (2, "blocks", 1),
    },
    CycleCase {
        name: "blocks transitive: A→B→C, then C→A rejected",
        issue_count: 3,
        setup_links: &[(1, "blocks", 2), (2, "blocks", 3)],
        rejected: (3, "blocks", 1),
    },
    CycleCase {
        name: "blocks self-loop: A blocks A",
        issue_count: 1,
        setup_links: &[],
        rejected: (1, "blocks", 1),
    },
    CycleCase {
        name: "depends-on direct: A depends-on B (DAG: B→A), then B depends-on A (DAG: A→B)",
        issue_count: 2,
        setup_links: &[(1, "depends-on", 2)],
        rejected: (2, "depends-on", 1),
    },
    CycleCase {
        name: "cross-kind: A blocks B (A→B), then A depends-on B (B→A) closes cycle",
        issue_count: 2,
        setup_links: &[(1, "blocks", 2)],
        rejected: (1, "depends-on", 2),
    },
];

#[test]
fn link_add_rejects_dag_cycles() {
    for case in CYCLE_CASES {
        let dir = setup();
        let ids = create_issues(&dir, case.issue_count);

        for &(from_idx, rel, to_idx) in case.setup_links {
            link(&dir, &ids, from_idx, rel, to_idx);
        }

        let (from_idx, rel, to_idx) = case.rejected;
        bmo(&dir)
            .args([
                "issue",
                "link",
                "add",
                &ids[from_idx - 1],
                rel,
                &ids[to_idx - 1],
            ])
            .assert()
            .code(3); // ErrorCode::Validation
    }
}

// Non-DAG edge kinds bypass the cycle check entirely.
// Even when they are the semantic inverse of an existing DAG edge, they must succeed.
struct AllowedCase {
    #[allow(dead_code)]
    name: &'static str,
    dag_link: (usize, &'static str, usize),
    allowed_link: (usize, &'static str, usize),
}

static ALLOWED_CASES: &[AllowedCase] = &[
    AllowedCase {
        name: "blocked-by is not a DAG edge — allowed even when inverse blocks exists",
        dag_link: (1, "blocks", 2),
        allowed_link: (2, "blocked-by", 1),
    },
    AllowedCase {
        name: "dependency-of is not a DAG edge — allowed even when inverse depends-on exists",
        dag_link: (1, "depends-on", 2),
        allowed_link: (2, "dependency-of", 1),
    },
    AllowedCase {
        name: "relates-to is never a DAG edge — always allowed",
        dag_link: (1, "blocks", 2),
        allowed_link: (2, "relates-to", 1),
    },
];

#[test]
fn link_add_allows_non_dag_edges() {
    for case in ALLOWED_CASES {
        let dir = setup();
        let ids = create_issues(&dir, 2);

        let (fi, rel, ti) = case.dag_link;
        link(&dir, &ids, fi, rel, ti);

        let (fi, rel, ti) = case.allowed_link;
        bmo(&dir)
            .args([
                "issue",
                "link",
                "add",
                &ids[fi - 1],
                rel,
                &ids[ti - 1],
            ])
            .assert()
            .success();
    }
}

// ── 2. Downstream commands fail loud on cyclic graph ─────────────────────────
//
// These tests inject a cycle past the insertion guard to verify that plan,
// next, and agent-init all error loudly rather than producing silent wrong
// results. An acyclic companion test confirms normal behavior is unchanged.

#[test]
fn plan_fails_loud_on_cycle() {
    let dir = setup_with_injected_cycle();
    bmo(&dir)
        .args(["plan"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}

#[test]
fn next_fails_loud_on_cycle() {
    let dir = setup_with_injected_cycle();
    bmo(&dir)
        .args(["next"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}

#[test]
fn agent_init_fails_loud_on_cycle() {
    let dir = setup_with_injected_cycle();
    bmo(&dir)
        .args(["agent-init"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}

// Confirm acyclic graphs still work correctly after all the above changes.
#[test]
fn plan_next_agent_init_work_with_acyclic_graph() {
    let dir = setup();
    let ids = create_issues(&dir, 3);
    link(&dir, &ids, 1, "blocks", 2);
    link(&dir, &ids, 2, "blocks", 3);

    bmo(&dir).args(["plan"]).assert().success();
    bmo(&dir).args(["next"]).assert().success();
    bmo(&dir).args(["agent-init"]).assert().success();
}

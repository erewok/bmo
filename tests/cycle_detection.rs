// Cycle detection integration tests.
//
// Covers every surface that participates in DAG integrity:
//   1. `bmo issue link add`  — cycle rejected at insertion (exit 3), for all
//                              four directional relation kinds (`blocks`,
//                              `blocked-by`, `depends-on`, `dependency-of`)
//   2. `bmo plan`            — fails loud on a cyclic graph
//   3. `bmo next`            — fails loud on a cyclic graph
//   4. `bmo agent-init`      — fails loud on a cyclic graph
//   5. Equivalence           — a relation and its semantic inverse (e.g.
//                              `blocked-by` vs. `blocks`) produce identical
//                              `bmo plan` phase ordering
//   6. `--no-links`          — confirms the flag actually changes output when
//                              real link relations are present
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

/// Bypass the insertion guard and write a `blocked-by` edge directly to the DB.
/// Used only to set up the downstream loud-failure tests for a cycle closed
/// entirely via `blocked-by` (not `blocks`) rows.
fn inject_blocked_by_edge(dir: &TempDir, from_id: i64, to_id: i64) {
    let conn = Connection::open(dir.path().join(".bmo/issues.db")).unwrap();
    conn.execute(
        "INSERT INTO issue_relations (from_id, to_id, relation) VALUES (?1, ?2, 'blocked-by')",
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

/// Build a dir with issues 1 and 2 linked 1 blocked-by 2 via the CLI, then
/// inject the opposing `blocked-by` row directly to create a cycle that
/// bypassed the insertion guard — a 2-cycle closed entirely via `blocked-by`
/// rows, which must be rejected rather than silently ignored.
fn setup_with_injected_blocked_by_cycle() -> TempDir {
    let dir = setup();
    create_issues(&dir, 2);
    link(&dir, &["BMO-1".into(), "BMO-2".into()], 1, "blocked-by", 2);
    inject_blocked_by_edge(&dir, 2, 1);
    dir
}

/// Run `bmo plan --json` (optionally with extra args, e.g. `--no-links`) and
/// parse the JSON envelope.
fn plan_json(dir: &TempDir, extra_args: &[&str]) -> serde_json::Value {
    let mut args = vec!["plan", "--json"];
    args.extend_from_slice(extra_args);
    let output = bmo(dir).args(&args).output().unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Extract the phase ordering from a `bmo plan --json` envelope as a list of
/// (per-phase, sorted) issue ids, discarding all other issue fields (title,
/// timestamps, etc.) so plans from different temp directories can be compared
/// structurally.
fn phase_id_order(plan: &serde_json::Value) -> Vec<Vec<i64>> {
    plan["data"]["phases"]
        .as_array()
        .expect("plan JSON missing phases array")
        .iter()
        .map(|phase| {
            let mut ids: Vec<i64> = phase["issues"]
                .as_array()
                .expect("phase missing issues array")
                .iter()
                .map(|issue| issue["id"].as_i64().expect("issue missing id"))
                .collect();
            ids.sort_unstable();
            ids
        })
        .collect()
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
/// - The `blocked-by`/`dependency-of` cases prove a *true* cycle (the same
///   relation kind declared in opposing directions, e.g. "A blocked-by B"
///   then "B blocked-by A") is rejected exactly like its `blocks`/`depends-on`
///   counterpart. These are distinct from the `ALLOWED_CASES` below, which
///   merely restate an existing edge using its semantic-inverse verb (not a
///   cycle at all).
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
    CycleCase {
        name: "blocked-by direct (true cycle): A blocked-by B (DAG: B→A), then B blocked-by A (DAG: A→B)",
        issue_count: 2,
        setup_links: &[(1, "blocked-by", 2)],
        rejected: (2, "blocked-by", 1),
    },
    CycleCase {
        name: "dependency-of direct (true cycle): A dependency-of B (DAG: A→B), then B dependency-of A (DAG: B→A)",
        issue_count: 2,
        setup_links: &[(1, "dependency-of", 2)],
        rejected: (2, "dependency-of", 1),
    },
    CycleCase {
        name: "cross-kind: A blocks B (A→B), then A blocked-by B (B→A) closes a true cycle",
        issue_count: 2,
        setup_links: &[(1, "blocks", 2)],
        rejected: (1, "blocked-by", 2),
    },
    CycleCase {
        name: "cross-kind: A depends-on B (B→A), then A dependency-of B (A→B) closes a true cycle",
        issue_count: 2,
        setup_links: &[(1, "depends-on", 2)],
        rejected: (1, "dependency-of", 2),
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

// These cases are allowed, but for two different reasons:
//
// - `blocked-by`/`dependency-of` ARE DAG edges (they mirror `blocks`/
//   `depends-on` in the reversed direction). The two cases below
//   are allowed not because the relation kind is exempt from cycle checking,
//   but because each one *restates the same real-world edge* using its
//   semantic-inverse verb (e.g. "1 blocks 2" and "2 blocked-by 1" both
//   produce the identical DAG edge 1→2) — that is not a cycle. A *true*
//   cycle via `blocked-by`/`dependency-of` (the same kind declared in
//   opposing directions) IS rejected; see `CYCLE_CASES` above.
// - `relates-to` is genuinely never a DAG edge, so it always bypasses the
//   cycle check regardless of direction.
struct AllowedCase {
    #[allow(dead_code)]
    name: &'static str,
    dag_link: (usize, &'static str, usize),
    allowed_link: (usize, &'static str, usize),
}

static ALLOWED_CASES: &[AllowedCase] = &[
    AllowedCase {
        name: "blocked-by restating the same edge as its inverse blocks — not a cycle, allowed",
        dag_link: (1, "blocks", 2),
        allowed_link: (2, "blocked-by", 1),
    },
    AllowedCase {
        name: "dependency-of restating the same edge as its inverse depends-on — not a cycle, allowed",
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
            .args(["issue", "link", "add", &ids[fi - 1], rel, &ids[ti - 1]])
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

// Exercises a 2-cycle closed entirely via `blocked-by` rows (not `blocks`),
// injected past the insertion guard directly into the DB, confirming that
// `plan`/`next`/`agent-init` fail loudly instead of silently ignoring it.
#[test]
fn plan_fails_loud_on_blocked_by_cycle() {
    let dir = setup_with_injected_blocked_by_cycle();
    bmo(&dir)
        .args(["plan"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}

#[test]
fn next_fails_loud_on_blocked_by_cycle() {
    let dir = setup_with_injected_blocked_by_cycle();
    bmo(&dir)
        .args(["next"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}

#[test]
fn agent_init_fails_loud_on_blocked_by_cycle() {
    let dir = setup_with_injected_blocked_by_cycle();
    bmo(&dir)
        .args(["agent-init"])
        .assert()
        .failure()
        .stderr(contains("cycle"));
}

// ── 3. Equivalence: a relation and its semantic inverse produce the same plan ─

/// `CHILD blocked-by PARENT` must place issues in the exact same plan phases
/// as `PARENT blocks CHILD` on an equivalent issue set — both declare the
/// same real-world edge (PARENT must finish first), just with the inverse
/// verb.
#[test]
fn blocked_by_and_blocks_produce_equivalent_plan_ordering() {
    let dir_blocked_by = setup();
    let ids = create_issues(&dir_blocked_by, 2);
    link(&dir_blocked_by, &ids, 2, "blocked-by", 1); // issue 2 blocked-by issue 1

    let dir_blocks = setup();
    let ids = create_issues(&dir_blocks, 2);
    link(&dir_blocks, &ids, 1, "blocks", 2); // issue 1 blocks issue 2

    let plan_blocked_by = plan_json(&dir_blocked_by, &[]);
    let plan_blocks = plan_json(&dir_blocks, &[]);

    assert_eq!(
        phase_id_order(&plan_blocked_by),
        phase_id_order(&plan_blocks),
        "blocked-by and its inverse blocks must produce identical phase ordering"
    );
    assert_eq!(
        plan_blocked_by["data"]["total_phases"],
        plan_blocks["data"]["total_phases"]
    );
}

/// `CHILD dependency-of PARENT` must place issues in the exact same plan
/// phases as `PARENT depends-on CHILD` on an equivalent issue set.
#[test]
fn dependency_of_and_depends_on_produce_equivalent_plan_ordering() {
    let dir_dependency_of = setup();
    let ids = create_issues(&dir_dependency_of, 2);
    link(&dir_dependency_of, &ids, 2, "dependency-of", 1); // issue 2 dependency-of issue 1

    let dir_depends_on = setup();
    let ids = create_issues(&dir_depends_on, 2);
    link(&dir_depends_on, &ids, 1, "depends-on", 2); // issue 1 depends-on issue 2

    let plan_dependency_of = plan_json(&dir_dependency_of, &[]);
    let plan_depends_on = plan_json(&dir_depends_on, &[]);

    assert_eq!(
        phase_id_order(&plan_dependency_of),
        phase_id_order(&plan_depends_on),
        "dependency-of and its inverse depends-on must produce identical phase ordering"
    );
    assert_eq!(
        plan_dependency_of["data"]["total_phases"],
        plan_depends_on["data"]["total_phases"]
    );
}

// ── 4. `--no-links` regression: the flag is no longer a no-op ────────────────

/// With a real `blocked-by` link present, `bmo plan --json` and
/// `bmo plan --no-links --json` must produce different phase orderings —
/// `--no-links` should demonstrably strip link-derived ordering constraints,
/// not leave output unchanged.
#[test]
fn no_links_flag_changes_plan_output_when_blocked_by_links_exist() {
    let dir = setup();
    let ids = create_issues(&dir, 2);
    link(&dir, &ids, 2, "blocked-by", 1); // issue 2 blocked-by issue 1

    let with_links = plan_json(&dir, &[]);
    let without_links = plan_json(&dir, &["--no-links"]);

    assert_ne!(
        phase_id_order(&with_links),
        phase_id_order(&without_links),
        "--no-links must produce different phase ordering than the default \
         when real blocked-by links exist"
    );
}

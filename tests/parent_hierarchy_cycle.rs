// Integration test coverage for the `parent_id` hierarchy self-reference and
// cycle guard.
//
// `parent_id` is a completely separate mechanism from the `issue_relations`
// table (`blocks`/`blocked-by`/`depends-on`/`dependency-of`) already covered
// by `tests/cycle_detection.rs` — this file exercises only the parent/child
// hierarchy edge on the `issues` table itself, at both create time (direct
// self-reference only, since a new row's id is unknown until after INSERT)
// and edit time (direct self-reference, 2-cycles, and longer chain cycles,
// since an existing issue's id is known and its ancestor chain can be
// walked).
//
// Covers:
//   1. Create-time self-reference: guessing the next AUTOINCREMENT id and
//      passing it as `--parent` on the create that will be assigned that
//      very id. Rejected with a validation error (exit code 3), and the
//      guard runs inside an atomic transaction so no row (and no
//      `sqlite_sequence` bump) survives the rejection — proven here by
//      confirming the next successful create reuses the same id.
//   2. Edit-time self-reference: `bmo issue edit <id> --parent <id>`.
//   3. A 2-cycle via edit: A.parent = B (succeeds), then B.parent = A
//      (rejected).
//   4. A 3-chain cycle via edit: A.parent = B, B.parent = C (both succeed),
//      then C.parent = A (rejected).
//   5. Valid hierarchy regression: normal parent/child chains via both
//      `create --parent` and `edit --parent` continue to work with no false
//      positives.
//   6. `bmo plan` unaffected: a valid, non-cyclic parent hierarchy alone
//      (with no `relations` table entries) never triggers the planner's DAG
//      cycle check, since `parent_id` is intentionally not consumed by
//      `Dag::build`.

use assert_cmd::cargo;
use assert_cmd::prelude::*;
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

/// Create an issue with the given title (and, optionally, a `--parent`),
/// returning the created issue's numeric id parsed from the `--json` output.
fn create_issue(dir: &TempDir, title: &str, parent: Option<i64>) -> i64 {
    let mut args = vec!["issue", "create", "--title", title, "--json"];
    let parent_str;
    if let Some(p) = parent {
        parent_str = p.to_string();
        args.push("--parent");
        args.push(&parent_str);
    }
    let output = bmo(dir).args(&args).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    json["data"]["id"]
        .as_i64()
        .expect("create --json response missing numeric data.id")
}

/// Fetch an issue's `parent_id` (as an `Option<i64>`) via `bmo issue show --json`.
fn parent_id_of(dir: &TempDir, id: i64) -> Option<i64> {
    let output = bmo(dir)
        .args(["issue", "show", &id.to_string(), "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    json["data"]["issue"]["parent_id"].as_i64()
}

/// Attempt `bmo issue edit <id> --parent <parent>` and return the exit
/// status, without asserting success or failure.
fn edit_parent(dir: &TempDir, id: i64, parent: i64) -> std::process::ExitStatus {
    bmo(dir)
        .args([
            "issue",
            "edit",
            &id.to_string(),
            "--parent",
            &parent.to_string(),
        ])
        .status()
        .unwrap()
}

// ── 1. Create-time self-reference ────────────────────────────────────────────

/// A brand-new SQLite AUTOINCREMENT sequence starts at 1, so the very first
/// `bmo issue create` in a fresh temp DB is deterministically about to be
/// assigned id 1. Passing `--parent 1` on that create is a self-reference
/// exploit attempt — it must be rejected atomically (exit code 3), leaving
/// no trace: no issue at all persists, and the next successful create still
/// gets id 1 (not 2), proving the rejected insert's autoincrement bump was
/// rolled back, not merely compensated for after the fact.
#[test]
fn create_rejects_self_reference_and_rolls_back_atomically() {
    let dir = setup();

    bmo(&dir)
        .args([
            "issue",
            "create",
            "--title",
            "self-parent exploit",
            "--parent",
            "1",
        ])
        .assert()
        .failure()
        .code(3); // ErrorCode::Validation

    // No row persisted at all — the exploit issue never became visible.
    let list_output = bmo(&dir)
        .args(["issue", "list", "--json"])
        .output()
        .unwrap();
    let list_json: serde_json::Value = serde_json::from_slice(&list_output.stdout).unwrap();
    assert_eq!(
        list_json["data"].as_array().unwrap().len(),
        0,
        "rejected self-parent create must not persist any issue"
    );

    // The next successful create must reuse id 1, proving the autoincrement
    // sequence bump from the rejected insert was rolled back atomically
    // (not just compensated for via a separate delete after the fact).
    let id = create_issue(&dir, "legitimate first issue", None);
    assert_eq!(
        id, 1,
        "next create after a rejected self-parent attempt must reuse id 1, \
         proving the create-time guard is transactionally atomic"
    );
}

// ── 2. Edit-time self-reference ──────────────────────────────────────────────

#[test]
fn edit_rejects_direct_self_reference() {
    let dir = setup();
    let a = create_issue(&dir, "A", None);

    let status = edit_parent(&dir, a, a);
    assert!(
        !status.success(),
        "editing an issue to be its own parent must fail"
    );
    assert_eq!(status.code(), Some(3));

    assert_eq!(
        parent_id_of(&dir, a),
        None,
        "rejected self-parent edit must leave parent_id unchanged"
    );
}

// ── 3. 2-cycle via edit ───────────────────────────────────────────────────────

#[test]
fn edit_rejects_two_cycle() {
    let dir = setup();
    let a = create_issue(&dir, "A", None);
    let b = create_issue(&dir, "B", None);

    // A.parent = B is a legitimate, acyclic edit.
    bmo(&dir)
        .args(["issue", "edit", &a.to_string(), "--parent", &b.to_string()])
        .assert()
        .success();
    assert_eq!(parent_id_of(&dir, a), Some(b));

    // B.parent = A would close a 2-cycle (A -> B -> A) and must be rejected.
    let status = edit_parent(&dir, b, a);
    assert!(!status.success());
    assert_eq!(status.code(), Some(3));

    assert_eq!(
        parent_id_of(&dir, b),
        None,
        "rejected 2-cycle edit must leave B's parent_id unchanged"
    );
}

// ── 4. Longer chain cycle via edit ───────────────────────────────────────────

#[test]
fn edit_rejects_longer_chain_cycle() {
    let dir = setup();
    let a = create_issue(&dir, "A", None);
    let b = create_issue(&dir, "B", None);
    let c = create_issue(&dir, "C", None);

    // Build the chain A -> B -> C (each edit legitimate on its own).
    bmo(&dir)
        .args(["issue", "edit", &a.to_string(), "--parent", &b.to_string()])
        .assert()
        .success();
    bmo(&dir)
        .args(["issue", "edit", &b.to_string(), "--parent", &c.to_string()])
        .assert()
        .success();
    assert_eq!(parent_id_of(&dir, a), Some(b));
    assert_eq!(parent_id_of(&dir, b), Some(c));

    // Closing the loop at C (C.parent = A) must be rejected: the ancestor
    // walk from A is A -> B -> C, which contains C, so this is a 3-cycle.
    let status = edit_parent(&dir, c, a);
    assert!(!status.success());
    assert_eq!(status.code(), Some(3));

    assert_eq!(
        parent_id_of(&dir, c),
        None,
        "rejected longer-chain cycle edit must leave C's parent_id unchanged"
    );
}

// ── 5. Valid hierarchy regression (no false positives) ───────────────────────

#[test]
fn valid_hierarchies_via_create_and_edit_still_work() {
    let dir = setup();

    // Legitimate parent/child hierarchy via `create --parent`.
    let epic = create_issue(&dir, "Epic", None);
    let child1 = create_issue(&dir, "Child 1", Some(epic));
    let child2 = create_issue(&dir, "Child 2", Some(epic));
    assert_eq!(parent_id_of(&dir, child1), Some(epic));
    assert_eq!(parent_id_of(&dir, child2), Some(epic));

    // Legitimate re-parenting via `edit --parent` to a distinct, unrelated,
    // non-cyclic ancestor.
    let standalone = create_issue(&dir, "Standalone", None);
    let new_parent = create_issue(&dir, "New parent", None);
    bmo(&dir)
        .args([
            "issue",
            "edit",
            &standalone.to_string(),
            "--parent",
            &new_parent.to_string(),
        ])
        .assert()
        .success();
    assert_eq!(parent_id_of(&dir, standalone), Some(new_parent));
}

// ── 6. `bmo plan` unaffected by a valid parent hierarchy ─────────────────────

/// A valid (non-cyclic) `parent_id` hierarchy, with no `issue_relations`
/// table entries at all, must never trigger the planner's DAG cycle check —
/// `parent_id` is intentionally not consumed by `Dag::build`, so `bmo plan`
/// must succeed normally.
#[test]
fn plan_succeeds_with_valid_parent_hierarchy_and_no_relations() {
    let dir = setup();
    let epic = create_issue(&dir, "Epic", None);
    let _child1 = create_issue(&dir, "Child 1", Some(epic));
    let _child2 = create_issue(&dir, "Child 2", Some(epic));

    let output = bmo(&dir).args(["plan", "--json"]).output().unwrap();
    assert!(
        output.status.success(),
        "bmo plan must succeed on a valid parent hierarchy with no relations: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
}

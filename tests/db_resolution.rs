//! Integration coverage for `--db` / `BMO_DB` resolution precedence across subcommands.
//!
//! Locks in the documented priority order (see `crate::db::find_db`):
//!   1. `--db` flag
//!   2. `BMO_DB` environment variable
//!   3. Walk up from CWD to find `.bmo/issues.db`
//!
//! Every test below runs commands with `current_dir` set to a "real" bmo project, but
//! points `--db`/`BMO_DB` at a separate "scratch" project, and asserts that the scratch
//! DB (not the real dir's DB) received the operation. Before BMO-5's fix, most commands
//! silently ignored `--db`/`BMO_DB` in favor of a CWD walk-up via `find_bmo_dir()`, so
//! these tests would observe the *real* dir's DB being mutated/read instead of the
//! scratch one.

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Create a new TempDir with `bmo init` already run inside it, so it has its own
/// `.bmo/issues.db`.
fn init_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    dir
}

fn issues_db_path(dir: &TempDir) -> PathBuf {
    dir.path().join(".bmo").join("issues.db")
}

/// `bmo stats --json --db <path>` -> total issue count. Used to observe which DB a
/// preceding command actually touched, independent of the mechanism (--db/BMO_DB/CWD)
/// under test.
fn stats_total(db_path: &Path) -> i64 {
    let output = Command::new(cargo::cargo_bin!("bmo"))
        .args(["stats", "--json", "--db"])
        .arg(db_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stats --db {:?} failed: {}",
        db_path,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    json["data"]["total"].as_i64().unwrap()
}

// ── --db flag precedence ──────────────────────────────────────────────────────

#[test]
fn db_flag_create_targets_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["create", "--title", "Scratch create"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success()
        .stdout(contains("BMO-1"));

    assert_eq!(
        stats_total(&scratch_db),
        1,
        "scratch db should have received the new issue"
    );
    assert_eq!(
        stats_total(&issues_db_path(&real)),
        0,
        "real dir's db must be unaffected by a --db-targeted create"
    );
}

#[test]
fn db_flag_list_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "Scratch list target"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success();

    // list --db from inside the real dir should see the scratch issue.
    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("list")
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success()
        .stdout(contains("Scratch list target"));

    // The real dir's own db (CWD walk-up, no --db) must not show the scratch issue.
    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("list")
        .assert()
        .success()
        .stdout(contains("Scratch list target").not());
}

#[test]
fn db_flag_board_shows_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "Scratch board issue"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success();

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("board")
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success()
        .stdout(contains("Scratch board issue"));

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("board")
        .assert()
        .success()
        .stdout(contains("Scratch board issue").not());
}

#[test]
fn db_flag_next_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "Scratch next issue"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success();

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["next", "--json", "--db"])
        .arg(&scratch_db)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert!(
        !json["data"].as_array().unwrap().is_empty(),
        "next --db <scratch> should surface the scratch db's ready issue"
    );

    // Real dir's own db (no --db) has zero issues, so next must report none.
    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["next", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["data"].as_array().unwrap().is_empty(),
        "real dir's db must be unaffected by a --db-targeted next"
    );
}

#[test]
fn db_flag_stats_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "Scratch stats issue"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success();

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["stats", "--json", "--db"])
        .arg(&scratch_db)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["total"], 1);

    assert_eq!(
        stats_total(&issues_db_path(&real)),
        0,
        "real dir's db must be unaffected by a --db-targeted stats"
    );
}

#[test]
fn db_flag_plan_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "Scratch plan issue"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success();

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["plan", "--json", "--db"])
        .arg(&scratch_db)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["total_issues"], 1);

    assert_eq!(
        stats_total(&issues_db_path(&real)),
        0,
        "real dir's db must be unaffected by a --db-targeted plan"
    );
}

// ── BMO_DB env var precedence ─────────────────────────────────────────────────

#[test]
fn bmo_db_env_create_targets_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["create", "--title", "BMO_DB create"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success()
        .stdout(contains("BMO-1"));

    assert_eq!(
        stats_total(&scratch_db),
        1,
        "scratch db should have received the new issue via BMO_DB"
    );
    assert_eq!(
        stats_total(&issues_db_path(&real)),
        0,
        "real dir's db must be unaffected by a BMO_DB-targeted create"
    );
}

#[test]
fn bmo_db_env_list_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "BMO_DB list target"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success();

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("list")
        .env("BMO_DB", &scratch_db)
        .assert()
        .success()
        .stdout(contains("BMO_DB list target"));

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("list")
        .assert()
        .success()
        .stdout(contains("BMO_DB list target").not());
}

#[test]
fn bmo_db_env_board_shows_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "BMO_DB board issue"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success();

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("board")
        .env("BMO_DB", &scratch_db)
        .assert()
        .success()
        .stdout(contains("BMO_DB board issue"));

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("board")
        .assert()
        .success()
        .stdout(contains("BMO_DB board issue").not());
}

#[test]
fn bmo_db_env_next_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "BMO_DB next issue"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success();

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["next", "--json"])
        .env("BMO_DB", &scratch_db)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!json["data"].as_array().unwrap().is_empty());

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["next", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["data"].as_array().unwrap().is_empty(),
        "real dir's db must be unaffected by a BMO_DB-targeted next"
    );
}

#[test]
fn bmo_db_env_stats_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "BMO_DB stats issue"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success();

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["stats", "--json"])
        .env("BMO_DB", &scratch_db)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["total"], 1);

    assert_eq!(stats_total(&issues_db_path(&real)), 0);
}

#[test]
fn bmo_db_env_plan_reads_scratch_leaves_real_untouched() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["create", "--title", "BMO_DB plan issue"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success();

    let output = Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["plan", "--json"])
        .env("BMO_DB", &scratch_db)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["total_issues"], 1);

    assert_eq!(stats_total(&issues_db_path(&real)), 0);
}

// ── --db overrides BMO_DB ─────────────────────────────────────────────────────

#[test]
fn db_flag_overrides_bmo_db_env_var() {
    let real = init_project();
    let env_scratch = init_project();
    let flag_scratch = init_project();
    let env_db = issues_db_path(&env_scratch);
    let flag_db = issues_db_path(&flag_scratch);

    // Both BMO_DB and --db are set, to two *different* paths. --db must win.
    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["create", "--title", "Precedence test"])
        .arg("--db")
        .arg(&flag_db)
        .env("BMO_DB", &env_db)
        .assert()
        .success()
        .stdout(contains("BMO-1"));

    assert_eq!(
        stats_total(&flag_db),
        1,
        "--db path should receive the issue when both --db and BMO_DB are set"
    );
    assert_eq!(
        stats_total(&env_db),
        0,
        "BMO_DB path must be untouched when --db is also set"
    );
    assert_eq!(
        stats_total(&issues_db_path(&real)),
        0,
        "real dir's db must be unaffected"
    );
}

// ── Fallback to CWD walk-up (no --db, no BMO_DB) ──────────────────────────────

#[test]
fn no_db_flag_or_env_falls_back_to_cwd_walk_up() {
    let real = init_project();

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["create", "--title", "CWD walk-up issue"])
        .assert()
        .success()
        .stdout(contains("BMO-1"));

    assert_eq!(
        stats_total(&issues_db_path(&real)),
        1,
        "with neither --db nor BMO_DB set, the CWD-discovered .bmo/issues.db should be used"
    );

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .arg("list")
        .assert()
        .success()
        .stdout(contains("CWD walk-up issue"));
}

// ── `bmo issue <cmd>` long-form dispatch ──────────────────────────────────────

#[test]
fn issue_create_long_form_respects_db_flag() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["issue", "create", "--title", "Long-form scratch issue"])
        .arg("--db")
        .arg(&scratch_db)
        .assert()
        .success()
        .stdout(contains("BMO-1"));

    assert_eq!(
        stats_total(&scratch_db),
        1,
        "bmo issue create --db <scratch> should target the scratch db"
    );
    assert_eq!(
        stats_total(&issues_db_path(&real)),
        0,
        "real dir's db must be unaffected by the long-form issue create dispatch"
    );
}

#[test]
fn issue_list_long_form_respects_bmo_db_env() {
    let real = init_project();
    let scratch = init_project();
    let scratch_db = issues_db_path(&scratch);

    Command::new(cargo::cargo_bin!("bmo"))
        .args(["issue", "create", "--title", "Long-form BMO_DB issue"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success();

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["issue", "list"])
        .env("BMO_DB", &scratch_db)
        .assert()
        .success()
        .stdout(contains("Long-form BMO_DB issue"));

    Command::new(cargo::cargo_bin!("bmo"))
        .current_dir(real.path())
        .args(["issue", "list"])
        .assert()
        .success()
        .stdout(contains("Long-form BMO_DB issue").not());
}

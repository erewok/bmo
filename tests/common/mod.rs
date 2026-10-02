use assert_cmd::cargo;
use std::process::Command;

/// A `bmo` command that does not inherit `BMO_DB` from the environment running
/// the tests. clap reads `BMO_DB` as `--db`, so an inherited value would point
/// every command, including `init`, at that database instead of the test's
/// temp directory. Tests that exercise `BMO_DB` set it with `.env(...)`.
pub fn bmo_command() -> Command {
    let mut cmd = Command::new(cargo::cargo_bin!("bmo"));
    cmd.env_remove("BMO_DB");
    cmd
}

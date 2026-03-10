use clap::Args;

use super::move_cmd::run_move;

#[derive(Args)]
pub struct ReopenArgs {
    /// Issue ID
    pub id: String,
}

pub fn run(args: &ReopenArgs, json: bool) -> anyhow::Result<()> {
    run_move(&args.id, "todo", json)
}

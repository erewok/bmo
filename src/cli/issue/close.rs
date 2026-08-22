use clap::Args;

use super::move_cmd::run_move;

#[derive(Args)]
pub struct CloseArgs {
    /// Issue ID
    pub id: String,
}

pub fn run(args: &CloseArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    run_move(&args.id, "done", json, db)
}

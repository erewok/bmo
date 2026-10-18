use clap::Args;

use crate::model::IssueId;

use super::move_cmd::run_move;

#[derive(Args)]
pub struct ReopenArgs {
    /// Issue ID
    pub id: IssueId,
}

pub fn run(args: &ReopenArgs, json: bool, db: Option<String>) -> anyhow::Result<()> {
    run_move(args.id, "todo", json, db)
}

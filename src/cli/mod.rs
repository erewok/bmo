use clap::{Parser, Subcommand};

pub mod board;
pub mod config;
pub mod export;
pub mod import;
pub mod init;
pub mod issue;
pub mod next;
pub mod plan;
pub mod stats;
pub mod truncate;
pub mod version;
pub mod web;

#[derive(Parser)]
#[command(name = "bmo", about = "Local-first issue tracker for AI agents")]
pub struct Cli {
    /// Output results as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Path to the bmo database file
    #[arg(long, global = true, env = "BMO_DB")]
    pub db: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new bmo project in the current directory
    Init(init::InitArgs),
    /// Show or modify project configuration
    Config(config::ConfigArgs),
    /// Print the bmo version
    Version,
    /// Show issue statistics
    Stats,
    /// Export all issues to JSON
    Export(export::ExportArgs),
    /// Import issues from a JSON export
    Import(import::ImportArgs),
    /// Show a Kanban board of all issues
    Board(board::BoardArgs),
    /// Show next work-ready issues
    Next(next::NextArgs),
    /// Show a phased execution plan
    Plan(plan::PlanArgs),
    /// Start the local web UI
    Web(web::WebArgs),
    /// Delete issues in bulk
    Truncate(truncate::TruncateArgs),
    /// Manage issues
    #[command(subcommand)]
    Issue(issue::IssueCommands),
}

/// Parse an issue ID that may be in "42" or "BMO-42" format.
#[allow(dead_code)]
pub fn parse_id(s: &str) -> anyhow::Result<i64> {
    let stripped = s
        .trim()
        .to_uppercase()
        .strip_prefix("BMO-")
        .map(|s| s.to_string())
        .unwrap_or_else(|| s.trim().to_string());
    stripped
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("invalid issue ID: {s}"))
}

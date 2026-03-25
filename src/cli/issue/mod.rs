use clap::Subcommand;

pub mod claim;
pub mod close;
pub mod comment;
pub mod create;
pub mod delete;
pub mod edit;
pub mod file_cmd;
pub mod graph;
pub mod label;
pub mod link;
pub mod list;
pub mod log_cmd;
pub mod move_cmd;
pub mod reopen;
pub mod show;

#[derive(Subcommand)]
pub enum IssueCommands {
    /// Atomically claim an issue (sets status=in-progress and assignee)
    Claim(claim::ClaimArgs),
    /// Create a new issue
    Create(create::CreateArgs),
    /// List issues
    #[command(alias = "ls")]
    List(list::ListArgs),
    /// Show issue details
    Show(show::ShowArgs),
    /// Edit an issue
    Edit(edit::EditArgs),
    /// Change an issue's status
    Move(move_cmd::MoveArgs),
    /// Mark an issue as done
    Close(close::CloseArgs),
    /// Reopen a closed issue
    Reopen(reopen::ReopenArgs),
    /// Delete an issue
    Delete(delete::DeleteArgs),
    /// Show issue activity log
    Log(log_cmd::LogArgs),
    /// Show issue dependency graph
    Graph(graph::GraphArgs),
    /// Manage comments
    #[command(subcommand)]
    Comment(comment::CommentCommands),
    /// Manage labels
    #[command(subcommand)]
    Label(label::LabelCommands),
    /// Manage issue relations
    #[command(subcommand)]
    Link(link::LinkCommands),
    /// Manage attached files
    #[command(subcommand)]
    File(file_cmd::FileCommands),
}

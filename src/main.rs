// Dead code is expected during incremental development; remove before 1.0
#![allow(dead_code)]

use clap::Parser;

mod cli;
mod config;
mod db;
mod errors;
mod filter;
mod model;
mod output;
mod planner;
mod web;

use cli::{
    Cli, Commands, issue::IssueCommands, issue::comment::CommentCommands,
    issue::file_cmd::FileCommands, issue::label::LabelCommands, issue::link::LinkCommands,
};

fn main() {
    let cli = Cli::parse();
    let json = cli.json;

    let result = dispatch(cli.command, json, cli.db);

    if let Err(e) = result {
        if json {
            let code = "general";
            let envelope = serde_json::json!({
                "ok": false,
                "error": e.to_string(),
                "code": code
            });
            eprintln!("{}", serde_json::to_string_pretty(&envelope).unwrap());
        } else {
            eprintln!("error: {e}");
        }
        std::process::exit(1);
    }
}

fn dispatch(command: Commands, json: bool, db: Option<String>) -> anyhow::Result<()> {
    match command {
        Commands::Init(args) => cli::init::run(&args, json),
        Commands::Config(args) => cli::config::run(&args, json),
        Commands::Version => cli::version::run(json),
        Commands::Stats => cli::stats::run(json),
        Commands::Export(args) => cli::export::run(&args, json),
        Commands::Import(args) => cli::import::run(&args, json),
        Commands::Board(args) => cli::board::run(&args, json),
        Commands::Next(args) => cli::next::run(&args, json),
        Commands::Plan(args) => cli::plan::run(&args, json),
        Commands::Web(args) => cli::web::run(&args, json, db),
        Commands::Issue(sub) => dispatch_issue(sub, json),
    }
}

fn dispatch_issue(command: IssueCommands, json: bool) -> anyhow::Result<()> {
    match command {
        IssueCommands::Create(args) => cli::issue::create::run(&args, json),
        IssueCommands::List(args) => cli::issue::list::run(&args, json),
        IssueCommands::Show(args) => cli::issue::show::run(&args, json),
        IssueCommands::Edit(args) => cli::issue::edit::run(&args, json),
        IssueCommands::Move(args) => cli::issue::move_cmd::run(&args, json),
        IssueCommands::Close(args) => cli::issue::close::run(&args, json),
        IssueCommands::Reopen(args) => cli::issue::reopen::run(&args, json),
        IssueCommands::Delete(args) => cli::issue::delete::run(&args, json),
        IssueCommands::Log(args) => cli::issue::log_cmd::run(&args, json),
        IssueCommands::Graph(args) => cli::issue::graph::run(&args, json),
        IssueCommands::Comment(sub) => match sub {
            CommentCommands::Add(args) => cli::issue::comment::run_add(&args, json),
            CommentCommands::List(args) => cli::issue::comment::run_list(&args, json),
        },
        IssueCommands::Label(sub) => match sub {
            LabelCommands::Add(args) => cli::issue::label::run_add(&args, json),
            LabelCommands::Rm(args) => cli::issue::label::run_rm(&args, json),
            LabelCommands::List(args) => cli::issue::label::run_list(&args, json),
            LabelCommands::Delete(args) => cli::issue::label::run_delete(&args, json),
        },
        IssueCommands::Link(sub) => match sub {
            LinkCommands::Add(args) => cli::issue::link::run_add(&args, json),
            LinkCommands::Remove(args) => cli::issue::link::run_remove(&args, json),
            LinkCommands::List(args) => cli::issue::link::run_list(&args, json),
        },
        IssueCommands::File(sub) => match sub {
            FileCommands::Add(args) => cli::issue::file_cmd::run_add(&args, json),
            FileCommands::Rm(args) => cli::issue::file_cmd::run_rm(&args, json),
            FileCommands::List(args) => cli::issue::file_cmd::run_list(&args, json),
        },
    }
}

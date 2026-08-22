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
        Commands::Init(args) => cli::init::run(&args, json, db),
        Commands::Config(args) => cli::config::run(&args, json, db),
        Commands::Version => cli::version::run(json),
        Commands::Stats => cli::stats::run(json, db),
        Commands::Export(args) => cli::export::run(&args, json, db),
        Commands::Import(args) => cli::import::run(&args, json, db),
        Commands::Board(args) => cli::board::run(&args, json, db),
        Commands::Next(args) => cli::next::run(&args, json, db),
        Commands::Plan(args) => cli::plan::run(&args, json, db),
        Commands::Web(args) => cli::web::run(&args, json, db),
        Commands::Truncate(args) => cli::truncate::run(&args, json, db),
        Commands::AgentInit(args) => cli::agent_init::run(&args, json, db),
        // Issue commands at top level (short form: `bmo <cmd>`)
        Commands::Claim(args) => cli::issue::claim::run(&args, json, db),
        Commands::Create(args) => cli::issue::create::run(&args, json, db),
        Commands::List(args) => cli::issue::list::run(&args, json, db),
        Commands::Show(args) => cli::issue::show::run(&args, json, db),
        Commands::Edit(args) => cli::issue::edit::run(&args, json, db),
        Commands::Move(args) => cli::issue::move_cmd::run(&args, json, db),
        Commands::Close(args) => cli::issue::close::run(&args, json, db),
        Commands::Reopen(args) => cli::issue::reopen::run(&args, json, db),
        Commands::Delete(args) => cli::issue::delete::run(&args, json, db),
        Commands::Log(args) => cli::issue::log_cmd::run(&args, json, db),
        Commands::Graph(args) => cli::issue::graph::run(&args, json, db),
        Commands::Comment(sub) => match sub {
            CommentCommands::Add(args) => cli::issue::comment::run_add(&args, json, db),
            CommentCommands::List(args) => cli::issue::comment::run_list(&args, json, db),
        },
        Commands::Label(sub) => match sub {
            LabelCommands::Add(args) => cli::issue::label::run_add(&args, json, db),
            LabelCommands::Rm(args) => cli::issue::label::run_rm(&args, json, db),
            LabelCommands::List(args) => cli::issue::label::run_list(&args, json, db),
            LabelCommands::Delete(args) => cli::issue::label::run_delete(&args, json, db),
        },
        Commands::Link(sub) => match sub {
            LinkCommands::Add(args) => cli::issue::link::run_add(&args, json, db),
            LinkCommands::Remove(args) => cli::issue::link::run_remove(&args, json, db),
            LinkCommands::List(args) => cli::issue::link::run_list(&args, json, db),
        },
        Commands::File(sub) => match sub {
            FileCommands::Add(args) => cli::issue::file_cmd::run_add(&args, json, db),
            FileCommands::Rm(args) => cli::issue::file_cmd::run_rm(&args, json, db),
            FileCommands::List(args) => cli::issue::file_cmd::run_list(&args, json, db),
            FileCommands::Conflicts(args) => cli::issue::file_cmd::run_conflicts(&args, json, db),
        },
        // Long form (backward compatible): `bmo issue <cmd>`
        Commands::Issue(sub) => dispatch_issue(sub, json, db),
    }
}

fn dispatch_issue(command: IssueCommands, json: bool, db: Option<String>) -> anyhow::Result<()> {
    match command {
        IssueCommands::Claim(args) => cli::issue::claim::run(&args, json, db),
        IssueCommands::Create(args) => cli::issue::create::run(&args, json, db),
        IssueCommands::List(args) => cli::issue::list::run(&args, json, db),
        IssueCommands::Show(args) => cli::issue::show::run(&args, json, db),
        IssueCommands::Edit(args) => cli::issue::edit::run(&args, json, db),
        IssueCommands::Move(args) => cli::issue::move_cmd::run(&args, json, db),
        IssueCommands::Close(args) => cli::issue::close::run(&args, json, db),
        IssueCommands::Reopen(args) => cli::issue::reopen::run(&args, json, db),
        IssueCommands::Delete(args) => cli::issue::delete::run(&args, json, db),
        IssueCommands::Log(args) => cli::issue::log_cmd::run(&args, json, db),
        IssueCommands::Graph(args) => cli::issue::graph::run(&args, json, db),
        IssueCommands::Comment(sub) => match sub {
            CommentCommands::Add(args) => cli::issue::comment::run_add(&args, json, db),
            CommentCommands::List(args) => cli::issue::comment::run_list(&args, json, db),
        },
        IssueCommands::Label(sub) => match sub {
            LabelCommands::Add(args) => cli::issue::label::run_add(&args, json, db),
            LabelCommands::Rm(args) => cli::issue::label::run_rm(&args, json, db),
            LabelCommands::List(args) => cli::issue::label::run_list(&args, json, db),
            LabelCommands::Delete(args) => cli::issue::label::run_delete(&args, json, db),
        },
        IssueCommands::Link(sub) => match sub {
            LinkCommands::Add(args) => cli::issue::link::run_add(&args, json, db),
            LinkCommands::Remove(args) => cli::issue::link::run_remove(&args, json, db),
            LinkCommands::List(args) => cli::issue::link::run_list(&args, json, db),
        },
        IssueCommands::File(sub) => match sub {
            FileCommands::Add(args) => cli::issue::file_cmd::run_add(&args, json, db),
            FileCommands::Rm(args) => cli::issue::file_cmd::run_rm(&args, json, db),
            FileCommands::List(args) => cli::issue::file_cmd::run_list(&args, json, db),
            FileCommands::Conflicts(args) => cli::issue::file_cmd::run_conflicts(&args, json, db),
        },
    }
}

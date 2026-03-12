pub mod human;
pub mod json;

use crate::db::Stats;
use crate::errors::ErrorCode;
use crate::model::{Comment, Issue, Label, Relation};

// ── Data structs for board/plan output ───────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct BoardColumns {
    pub backlog: Vec<Issue>,
    pub todo: Vec<Issue>,
    pub in_progress: Vec<Issue>,
    pub review: Vec<Issue>,
    pub done: Vec<Issue>,
}

#[derive(Debug, serde::Serialize)]
pub struct Phase {
    pub number: usize,
    pub issues: Vec<Issue>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutionPlan {
    pub phases: Vec<Phase>,
    pub total_issues: usize,
    pub total_phases: usize,
    pub max_parallelism: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct IssueDetail {
    pub issue: Issue,
    pub sub_issues: Vec<Issue>,
    pub relations: Vec<Relation>,
    pub comments: Vec<Comment>,
    pub labels: Vec<Label>,
}

// ── Printer trait ─────────────────────────────────────────────────────────────

pub trait Printer {
    fn print_issue(&self, issue: &Issue);
    fn print_issue_list(&self, issues: &[Issue]);
    fn print_issue_detail(&self, detail: &IssueDetail);
    fn print_board(&self, board: &BoardColumns);
    fn print_plan(&self, plan: &ExecutionPlan);
    fn print_stats(&self, stats: &Stats);
    fn print_message(&self, message: &str);
    fn print_error(&self, message: &str, code: ErrorCode);
    fn print_comments(&self, comments: &[Comment]);
    fn print_labels(&self, labels: &[Label]);
    fn print_relations(&self, relations: &[Relation]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Oneline,
}

pub fn make_printer(mode: OutputMode) -> Box<dyn Printer> {
    match mode {
        OutputMode::Human => Box::new(human::HumanPrinter::new()),
        OutputMode::Json => Box::new(json::JsonPrinter),
        OutputMode::Oneline => Box::new(human::HumanPrinter::oneline()),
    }
}

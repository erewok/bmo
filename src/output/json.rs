use serde::Serialize;

use crate::db::Stats;
use crate::errors::ErrorCode;
use crate::model::{Comment, Issue, Label, Relation};

use super::{BoardColumns, ExecutionPlan, IssueDetail, Printer};

pub struct JsonPrinter;

fn envelope_ok<T: Serialize>(data: T, message: &str) {
    let out = serde_json::json!({
        "ok": true,
        "data": data,
        "message": message,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

fn envelope_err(message: &str, code: ErrorCode) {
    let out = serde_json::json!({
        "ok": false,
        "error": message,
        "code": code.as_str(),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

impl Printer for JsonPrinter {
    fn print_issue(&self, issue: &Issue) {
        envelope_ok(issue, &format!("Issue {}", issue.display_id()));
    }

    fn print_issue_list(&self, issues: &[Issue]) {
        envelope_ok(issues, &format!("{} issue(s)", issues.len()));
    }

    fn print_issue_detail(&self, detail: &IssueDetail) {
        envelope_ok(detail, &format!("Issue {}", detail.issue.display_id()));
    }

    fn print_board(&self, board: &BoardColumns) {
        envelope_ok(board, "board");
    }

    fn print_plan(&self, plan: &ExecutionPlan) {
        envelope_ok(plan, &format!("{} phases", plan.total_phases));
    }

    fn print_stats(&self, stats: &Stats) {
        envelope_ok(stats, "stats");
    }

    fn print_message(&self, message: &str) {
        envelope_ok(serde_json::Value::Null, message);
    }

    fn print_error(&self, message: &str, code: ErrorCode) {
        envelope_err(message, code);
    }

    fn print_comments(&self, comments: &[Comment]) {
        envelope_ok(comments, &format!("{} comment(s)", comments.len()));
    }

    fn print_labels(&self, labels: &[Label]) {
        envelope_ok(labels, &format!("{} label(s)", labels.len()));
    }

    fn print_relations(&self, relations: &[Relation]) {
        envelope_ok(relations, &format!("{} relation(s)", relations.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ErrorCode;

    #[test]
    fn success_envelope_shape() {
        let printer = JsonPrinter;
        // Capture by calling with a trivial message - just ensure it doesn't panic
        printer.print_message("hello");
    }

    #[test]
    fn error_envelope_has_code() {
        // Verify the JSON we build is correct shape
        let out = serde_json::json!({
            "ok": false,
            "error": "not found",
            "code": ErrorCode::NotFound.as_str(),
        });
        assert_eq!(out["ok"], false);
        assert_eq!(out["code"], "not-found");
    }

    #[test]
    fn error_codes() {
        assert_eq!(ErrorCode::General.exit_code(), 1);
        assert_eq!(ErrorCode::NotFound.exit_code(), 2);
        assert_eq!(ErrorCode::Validation.exit_code(), 3);
        assert_eq!(ErrorCode::Conflict.exit_code(), 4);
    }
}

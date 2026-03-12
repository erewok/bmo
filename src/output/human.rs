use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use owo_colors::OwoColorize;

use crate::db::Stats;
use crate::errors::ErrorCode;
use crate::model::{Comment, Issue, Kind, Label, Priority, Relation, RelationKind, Status};

use super::{BoardColumns, ExecutionPlan, IssueDetail, Printer};

fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
}

// ── Table helper ──────────────────────────────────────────────────────────────

/// Create a consistently styled table that respects terminal width.
/// All tables in bmo go through this so layout settings only need changing here.
fn make_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|h| Cell::new(h).add_attribute(Attribute::Bold)),
        );
    table
}

// ── Cell helpers (use comfy_table native color, not ANSI strings) ─────────────
// Using Cell::fg() lets comfy_table measure the true visible width of each cell,
// avoiding the column-stagger that occurs when ANSI escape bytes are counted
// as visible characters.

fn status_cell(status: Status) -> Cell {
    let text = format!("{} {}", status.icon(), status);
    if no_color() {
        Cell::new(text)
    } else {
        let (color, dim) = match status {
            Status::Backlog => (Color::White, true),
            Status::Todo => (Color::Blue, false),
            Status::InProgress => (Color::Yellow, false),
            Status::Review => (Color::Magenta, false),
            Status::Done => (Color::Green, false),
        };
        let cell = Cell::new(text).fg(color);
        if dim {
            cell.add_attribute(Attribute::Dim)
        } else {
            cell
        }
    }
}

fn priority_cell(priority: Priority) -> Cell {
    let text = format!("{} {}", priority.icon(), priority);
    if no_color() {
        Cell::new(text)
    } else {
        let (color, dim) = match priority {
            Priority::Critical => (Color::Red, false),
            Priority::High => (Color::Yellow, false),
            Priority::Medium => (Color::Blue, false),
            Priority::Low => (Color::White, true),
            Priority::None => (Color::White, false),
        };
        let cell = Cell::new(text).fg(color);
        if dim {
            cell.add_attribute(Attribute::Dim)
        } else {
            cell
        }
    }
}

fn kind_cell(kind: Kind) -> Cell {
    let text = format!("{} {}", kind.icon(), kind);
    if no_color() {
        Cell::new(text)
    } else {
        let color = match kind {
            Kind::Bug => Color::Red,
            Kind::Feature => Color::Green,
            Kind::Task => Color::Blue,
            Kind::Epic => Color::Magenta,
            Kind::Chore => Color::Yellow,
        };
        Cell::new(text).fg(color)
    }
}

// ── Plain colored strings for non-table output (board, detail, plan) ──────────

fn status_colored(status: Status) -> String {
    if no_color() {
        format!("{} {}", status.icon(), status)
    } else {
        match status {
            Status::Backlog => format!("{} {}", status.icon(), status).dimmed().to_string(),
            Status::Todo => format!("{} {}", status.icon(), status).blue().to_string(),
            Status::InProgress => format!("{} {}", status.icon(), status).yellow().to_string(),
            Status::Review => format!("{} {}", status.icon(), status)
                .magenta()
                .to_string(),
            Status::Done => format!("{} {}", status.icon(), status).green().to_string(),
        }
    }
}

fn priority_colored(priority: Priority) -> String {
    if no_color() {
        format!("{} {}", priority.icon(), priority)
    } else {
        match priority {
            Priority::Critical => format!("{} {}", priority.icon(), priority)
                .red()
                .to_string(),
            Priority::High => format!("{} {}", priority.icon(), priority)
                .yellow()
                .to_string(),
            Priority::Medium => format!("{} {}", priority.icon(), priority)
                .blue()
                .to_string(),
            Priority::Low => format!("{} {}", priority.icon(), priority)
                .dimmed()
                .to_string(),
            Priority::None => format!("{} {}", priority.icon(), priority)
                .white()
                .to_string(),
        }
    }
}

fn kind_colored(kind: Kind) -> String {
    if no_color() {
        format!("{} {}", kind.icon(), kind)
    } else {
        match kind {
            Kind::Bug => format!("{} {}", kind.icon(), kind).red().to_string(),
            Kind::Feature => format!("{} {}", kind.icon(), kind).green().to_string(),
            Kind::Task => format!("{} {}", kind.icon(), kind).blue().to_string(),
            Kind::Epic => format!("{} {}", kind.icon(), kind).magenta().to_string(),
            Kind::Chore => format!("{} {}", kind.icon(), kind).yellow().to_string(),
        }
    }
}

pub struct HumanPrinter {
    oneline: bool,
}

impl HumanPrinter {
    pub fn new() -> Self {
        HumanPrinter { oneline: false }
    }

    pub fn oneline() -> Self {
        HumanPrinter { oneline: true }
    }
}

impl Default for HumanPrinter {
    fn default() -> Self {
        HumanPrinter::new()
    }
}

impl Printer for HumanPrinter {
    fn print_issue(&self, issue: &Issue) {
        println!("{} — {}", issue.display_id().bold(), issue.title);
        println!("  Status:   {}", status_colored(issue.status));
        println!("  Priority: {}", priority_colored(issue.priority));
        println!("  Kind:     {}", kind_colored(issue.kind));
        if let Some(a) = &issue.assignee {
            println!("  Assignee: {a}");
        }
        if !issue.labels.is_empty() {
            println!("  Labels:   {}", issue.labels.join(", "));
        }
    }

    fn print_issue_list(&self, issues: &[Issue]) {
        if issues.is_empty() {
            println!("No issues found.");
            return;
        }
        if self.oneline {
            for issue in issues {
                if no_color() {
                    println!(
                        "{}  {}  {}  {}  {}",
                        issue.display_id(),
                        status_colored(issue.status),
                        priority_colored(issue.priority),
                        kind_colored(issue.kind),
                        issue.title,
                    );
                } else {
                    println!(
                        "{}  {}  {}  {}  {}",
                        issue.display_id().bold(),
                        status_colored(issue.status),
                        priority_colored(issue.priority),
                        kind_colored(issue.kind),
                        issue.title,
                    );
                }
            }
            return;
        }
        let mut table = make_table(&["ID", "Status", "Priority", "Kind", "Title", "Assignee"]);
        for issue in issues {
            table.add_row(vec![
                Cell::new(issue.display_id()),
                status_cell(issue.status),
                priority_cell(issue.priority),
                kind_cell(issue.kind),
                Cell::new(&issue.title),
                Cell::new(issue.assignee.as_deref().unwrap_or("")),
            ]);
        }
        println!("{table}");
    }

    fn print_issue_detail(&self, detail: &IssueDetail) {
        let issue = &detail.issue;
        println!("\n{} — {}", issue.display_id().bold(), issue.title);
        println!("  Status:   {}", status_colored(issue.status));
        println!("  Priority: {}", priority_colored(issue.priority));
        println!("  Kind:     {}", kind_colored(issue.kind));
        if let Some(a) = &issue.assignee {
            println!("  Assignee: {a}");
        }
        if !issue.labels.is_empty() {
            println!("  Labels:   {}", issue.labels.join(", "));
        }
        if !issue.description.is_empty() {
            println!(
                "\n  Description:\n  {}",
                issue.description.replace('\n', "\n  ")
            );
        }
        if !detail.sub_issues.is_empty() {
            println!("\n  Sub-issues ({}):", detail.sub_issues.len());
            for sub in &detail.sub_issues {
                println!(
                    "    {} {} — {}",
                    sub.display_id(),
                    status_colored(sub.status),
                    sub.title
                );
            }
        }
        if !detail.relations.is_empty() {
            println!("\n  Relations:");
            for rel in &detail.relations {
                let direction = match rel.kind {
                    RelationKind::Blocks => format!("→ blocks BMO-{}", rel.to_id),
                    RelationKind::BlockedBy => format!("← blocked by BMO-{}", rel.from_id),
                    RelationKind::DependsOn => format!("→ depends on BMO-{}", rel.to_id),
                    RelationKind::DependencyOf => format!("← dependency of BMO-{}", rel.from_id),
                    RelationKind::RelatesTo => format!("↔ relates to BMO-{}", rel.to_id),
                    RelationKind::Duplicates => format!("→ duplicates BMO-{}", rel.to_id),
                    RelationKind::DuplicateOf => format!("← duplicate of BMO-{}", rel.from_id),
                };
                println!("    {direction}");
            }
        }
        if !detail.comments.is_empty() {
            println!("\n  Comments ({}):", detail.comments.len());
            for c in &detail.comments {
                let author = c.author.as_deref().unwrap_or("unknown");
                println!(
                    "    [{}] {}: {}",
                    c.created_at.format("%Y-%m-%d"),
                    author,
                    c.body
                );
            }
        }
    }

    fn print_board(&self, board: &BoardColumns) {
        let columns = [
            ("BACKLOG", &board.backlog),
            ("TODO", &board.todo),
            ("IN PROGRESS", &board.in_progress),
            ("REVIEW", &board.review),
            ("DONE", &board.done),
        ];
        for (name, issues) in &columns {
            if no_color() {
                println!("\n── {} ({}) ──", name, issues.len());
            } else {
                println!("\n{}", format!("── {} ({}) ──", name, issues.len()).bold());
            }
            if issues.is_empty() {
                println!("  (empty)");
            } else {
                for issue in issues.iter() {
                    println!(
                        "  {} {} {}",
                        issue.display_id(),
                        priority_colored(issue.priority),
                        issue.title
                    );
                }
            }
        }
    }

    fn print_plan(&self, plan: &ExecutionPlan) {
        println!(
            "{} phases, {} issues, max parallelism: {}",
            plan.total_phases, plan.total_issues, plan.max_parallelism
        );
        for phase in &plan.phases {
            println!("\nPhase {}:", phase.number);
            for issue in &phase.issues {
                println!(
                    "  {} {} — {}",
                    issue.display_id(),
                    priority_colored(issue.priority),
                    issue.title
                );
            }
        }
    }

    fn print_stats(&self, stats: &Stats) {
        println!("Total issues: {}", stats.total);
        println!("\nBy status:");
        for status in Status::all() {
            let count = stats.by_status.get(status.label()).copied().unwrap_or(0);
            println!("  {}: {count}", status_colored(*status));
        }
        println!("\nBy priority:");
        for p in [
            Priority::Critical,
            Priority::High,
            Priority::Medium,
            Priority::Low,
            Priority::None,
        ] {
            let count = stats.by_priority.get(p.label()).copied().unwrap_or(0);
            println!("  {}: {count}", priority_colored(p));
        }
    }

    fn print_message(&self, message: &str) {
        println!("{message}");
    }

    fn print_error(&self, message: &str, _code: ErrorCode) {
        if no_color() {
            eprintln!("error: {message}");
        } else {
            eprintln!("{} {message}", "error:".red().bold());
        }
    }

    fn print_comments(&self, comments: &[Comment]) {
        if comments.is_empty() {
            println!("No comments.");
            return;
        }
        for c in comments {
            let author = c.author.as_deref().unwrap_or("unknown");
            println!(
                "[{}] {}: {}",
                c.created_at.format("%Y-%m-%d %H:%M"),
                author,
                c.body
            );
        }
    }

    fn print_labels(&self, labels: &[Label]) {
        if labels.is_empty() {
            println!("No labels.");
            return;
        }
        let mut table = make_table(&["Name", "Color"]);
        for label in labels {
            table.add_row(vec![
                Cell::new(&label.name),
                Cell::new(label.color.as_deref().unwrap_or("—")),
            ]);
        }
        println!("{table}");
    }

    fn print_relations(&self, relations: &[Relation]) {
        if relations.is_empty() {
            println!("No relations.");
            return;
        }
        let mut table = make_table(&["ID", "From", "Relation", "To"]);
        for rel in relations {
            table.add_row(vec![
                Cell::new(rel.id),
                Cell::new(format!("BMO-{}", rel.from_id)),
                Cell::new(rel.kind.label()),
                Cell::new(format!("BMO-{}", rel.to_id)),
            ]);
        }
        println!("{table}");
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

// Make Color available (suppresses unused import warning)
const _: Option<Color> = None;

use std::collections::HashMap;

use clap::Args;
use serde::Deserialize;

use crate::config::find_bmo_dir;
use crate::db::{AddCommentInput, CreateIssueInput, Repository, open_db};
use crate::model::activity::NewActivityEntry;
use crate::model::export::ExportBundle;
use crate::model::{Kind, Priority, RelationKind, Status};

#[derive(Args)]
pub struct ImportArgs {
    /// Path to the JSON export file
    pub file: String,
    /// Import from a docket export (remaps DKT- IDs to BMO-)
    #[arg(long)]
    pub from_docket: bool,
}

// ── Docket-specific deserialization structs ───────────────────────────────────
//
// Docket exports use string IDs like "DKT-1" for all ID fields. These private
// structs mirror the docket JSON schema using serde_json::Value for ID fields so
// they tolerate both string IDs ("DKT-1") and bare integer IDs (1) that appear
// in bmo-format exports used with --from-docket.

#[derive(Debug, Deserialize)]
struct DocketIssue {
    pub id: serde_json::Value,
    pub parent_id: Option<serde_json::Value>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: String,
    pub priority: String,
    pub kind: String,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DocketComment {
    pub issue_id: serde_json::Value,
    pub body: String,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DocketRelation {
    pub source_issue_id: serde_json::Value,
    pub target_issue_id: serde_json::Value,
    pub relation_type: String,
}

#[derive(Debug, Deserialize)]
struct DocketLabel {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DocketActivity {
    pub issue_id: serde_json::Value,
    #[serde(default)]
    pub field_changed: Option<String>,
    #[serde(default)]
    pub old_value: Option<String>,
    #[serde(default)]
    pub new_value: Option<String>,
    #[serde(default)]
    pub changed_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DocketFile {
    pub issue_id: serde_json::Value,
    pub path: String,
}

#[derive(Debug, Deserialize)]
struct DocketExportBundle {
    #[serde(default)]
    pub issues: Vec<DocketIssue>,
    #[serde(default)]
    pub comments: Vec<DocketComment>,
    #[serde(default)]
    pub labels: Vec<DocketLabel>,
    #[serde(default)]
    pub relations: Vec<DocketRelation>,
    #[serde(default)]
    pub activity: Vec<DocketActivity>,
    #[serde(default)]
    pub files: Vec<DocketFile>,
}

// ── ID parsing ────────────────────────────────────────────────────────────────

/// Parse a docket ID value that may be a JSON string ("DKT-1") or integer (1).
///
/// For string values: strips any leading alphabetic-and-dash prefix and parses
/// the trailing integer, e.g. "DKT-42" → 42, "BMO-7" → 7, "100" → 100.
/// For integer values: returns the value directly.
fn parse_dkt_id_value(v: &serde_json::Value) -> anyhow::Result<i64> {
    match v {
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("docket ID is not a valid i64: {n}")),
        serde_json::Value::String(s) => parse_dkt_id(s),
        other => anyhow::bail!("unexpected docket ID type: {other}"),
    }
}

/// Strip any leading alphabetic-and-dash prefix (e.g. "DKT-", "BMO-") and
/// parse the trailing integer. Returns an error if no numeric suffix is found.
fn parse_dkt_id(s: &str) -> anyhow::Result<i64> {
    // Find where the trailing numeric run starts.
    let numeric_start = s
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);
    let digits = &s[numeric_start..];
    if digits.is_empty() {
        anyhow::bail!("no numeric suffix in docket ID: {s}");
    }
    let n: i64 = digits
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid numeric suffix in docket ID: {s}"))?;
    Ok(n)
}

/// Canonical string key for an ID value (used as HashMap key).
fn id_key(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

// ── run ───────────────────────────────────────────────────────────────────────

pub fn run(args: &ImportArgs, json: bool) -> anyhow::Result<()> {
    let bmo_dir = find_bmo_dir()?;
    let repo = open_db(&bmo_dir.join("issues.db"))?;

    let contents = std::fs::read_to_string(&args.file)?;

    let mut imported_issues = 0usize;
    let mut imported_comments = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    if args.from_docket {
        import_from_docket(
            &repo,
            &contents,
            &mut imported_issues,
            &mut imported_comments,
            &mut warnings,
        )?;
    } else {
        import_from_bmo(
            &repo,
            &contents,
            &mut imported_issues,
            &mut imported_comments,
        )?;
    }

    let suffix = if args.from_docket {
        " (from docket format)"
    } else {
        ""
    };
    let mut msg =
        format!("Imported {imported_issues} issue(s) and {imported_comments} comment(s){suffix}");
    for w in &warnings {
        msg.push_str(&format!(" [{w}]"));
    }

    if json {
        let envelope = serde_json::json!({
            "ok": true,
            "data": { "issues": imported_issues, "comments": imported_comments },
            "message": msg,
            "warnings": warnings
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("{msg}");
    }

    Ok(())
}

// ── bmo native import (unchanged path) ───────────────────────────────────────

fn import_from_bmo(
    repo: &impl Repository,
    contents: &str,
    imported_issues: &mut usize,
    imported_comments: &mut usize,
) -> anyhow::Result<()> {
    let bundle: ExportBundle = serde_json::from_str(contents)?;

    for issue in &bundle.issues {
        let input = CreateIssueInput {
            parent_id: issue.parent_id,
            title: issue.title.clone(),
            description: issue.description.clone(),
            status: issue.status,
            priority: issue.priority,
            kind: issue.kind,
            assignee: issue.assignee.clone(),
            labels: issue.labels.clone(),
            files: issue.files.clone(),
            actor: Some("import".to_string()),
        };
        repo.create_issue(&input)?;
        *imported_issues += 1;
    }

    for comment in &bundle.comments {
        let input = AddCommentInput {
            issue_id: comment.issue_id,
            body: comment.body.clone(),
            author: comment.author.clone(),
        };
        if repo.get_issue(comment.issue_id)?.is_some() {
            repo.add_comment(&input)?;
            *imported_comments += 1;
        }
    }

    Ok(())
}

// ── docket import ─────────────────────────────────────────────────────────────

fn import_from_docket(
    repo: &impl Repository,
    contents: &str,
    imported_issues: &mut usize,
    imported_comments: &mut usize,
    warnings: &mut Vec<String>,
) -> anyhow::Result<()> {
    let bundle: DocketExportBundle = serde_json::from_str(contents)?;

    // Map docket ID string/number key -> new bmo i64 ID so we can remap
    // relations, comments, activity, and files after issue creation.
    // We import issues in the order provided; docket typically exports parents
    // before children, so a single pass handles most cases.
    let mut id_map: HashMap<String, i64> = HashMap::new();

    let mut skipped_comments: usize = 0;
    let mut skipped_relations: usize = 0;
    let mut skipped_files: usize = 0;
    let mut skipped_activity: usize = 0;

    for issue in &bundle.issues {
        let parent_id = match &issue.parent_id {
            Some(pid) if !pid.is_null() => {
                let key = id_key(pid);
                // Only use IDs that have already been imported; if the parent is
                // not in id_map, set parent_id to None rather than guessing.
                id_map.get(&key).copied()
            }
            _ => None,
        };

        let status: Status = issue.status.parse().unwrap_or(Status::Backlog);
        let priority: Priority = issue.priority.parse().unwrap_or(Priority::None);
        let kind: Kind = issue.kind.parse().unwrap_or(Kind::Task);

        // Normalize the assignee: docket stores "" for unset, bmo wants None.
        let assignee = issue
            .assignee
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let input = CreateIssueInput {
            parent_id,
            title: issue.title.clone(),
            description: issue.description.clone(),
            status,
            priority,
            kind,
            assignee,
            labels: issue.labels.clone(),
            files: issue.files.clone(),
            actor: Some("import".to_string()),
        };

        let created = repo.create_issue(&input)?;
        id_map.insert(id_key(&issue.id), created.id);
        *imported_issues += 1;
    }

    // Import comments — remap issue_id via id_map only
    for comment in &bundle.comments {
        let key = id_key(&comment.issue_id);
        match id_map.get(&key).copied() {
            Some(issue_id) => {
                if repo.get_issue(issue_id)?.is_some() {
                    let input = AddCommentInput {
                        issue_id,
                        body: comment.body.clone(),
                        author: comment.author.clone(),
                    };
                    repo.add_comment(&input)?;
                    *imported_comments += 1;
                }
            }
            None => {
                skipped_comments += 1;
            }
        }
    }

    // Import labels — ensure each label exists in bmo
    for label in &bundle.labels {
        let color = label.color.as_deref().filter(|s| !s.is_empty());
        // Ignore errors; the label may already exist or the name may be invalid.
        let _ = repo.get_or_create_label(&label.name, color);
    }

    // Import relations — both endpoints must have been imported via id_map
    for relation in &bundle.relations {
        let from_key = id_key(&relation.source_issue_id);
        let to_key = id_key(&relation.target_issue_id);
        let from_id = id_map.get(&from_key).copied();
        let to_id = id_map.get(&to_key).copied();

        match (from_id, to_id) {
            (Some(from), Some(to)) => {
                if repo.get_issue(from)?.is_some() && repo.get_issue(to)?.is_some() {
                    let kind: RelationKind = relation
                        .relation_type
                        .parse()
                        .unwrap_or(RelationKind::RelatesTo);
                    // Ignore errors (e.g. duplicate relations)
                    let _ = repo.add_relation(from, kind, to);
                }
            }
            _ => {
                skipped_relations += 1;
            }
        }
    }

    // Import activity — remap issue_id via id_map only
    for entry in &bundle.activity {
        let key = id_key(&entry.issue_id);
        match id_map.get(&key).copied() {
            Some(issue_id) => {
                if repo.get_issue(issue_id)?.is_some() {
                    // Map docket activity shape (field_changed / old_value / new_value / changed_by)
                    // to bmo's NewActivityEntry (kind / detail / actor).
                    let kind = entry
                        .field_changed
                        .clone()
                        .unwrap_or_else(|| "update".to_string());
                    let detail = match (&entry.old_value, &entry.new_value) {
                        (Some(old), Some(new)) if !old.is_empty() || !new.is_empty() => {
                            Some(format!("{old} → {new}"))
                        }
                        (None, Some(new)) if !new.is_empty() => Some(new.clone()),
                        _ => None,
                    };
                    let actor = entry
                        .changed_by
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());

                    let new_entry = NewActivityEntry {
                        issue_id,
                        kind,
                        detail,
                        actor,
                    };
                    // Ignore errors; activity is best-effort
                    let _ = repo.log_activity(&new_entry);
                }
            }
            None => {
                skipped_activity += 1;
            }
        }
    }

    // Import files — remap issue_id via id_map only
    for file in &bundle.files {
        let key = id_key(&file.issue_id);
        match id_map.get(&key).copied() {
            Some(issue_id) => {
                if repo.get_issue(issue_id)?.is_some() {
                    // Ignore errors (e.g. duplicate file attachments)
                    let _ = repo.add_file(issue_id, &file.path);
                }
            }
            None => {
                skipped_files += 1;
            }
        }
    }

    // Collect non-zero skip counts as warnings
    if skipped_comments > 0 {
        warnings.push(format!(
            "{skipped_comments} comment(s) skipped: unresolvable issue ID"
        ));
    }
    if skipped_relations > 0 {
        warnings.push(format!(
            "{skipped_relations} relation(s) skipped: unresolvable issue ID"
        ));
    }
    if skipped_files > 0 {
        warnings.push(format!(
            "{skipped_files} file(s) skipped: unresolvable issue ID"
        ));
    }
    if skipped_activity > 0 {
        warnings.push(format!(
            "{skipped_activity} activity record(s) skipped: unresolvable issue ID"
        ));
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dkt_id_string_prefix() {
        assert_eq!(parse_dkt_id("DKT-1").unwrap(), 1);
        assert_eq!(parse_dkt_id("DKT-42").unwrap(), 42);
        assert_eq!(parse_dkt_id("BMO-7").unwrap(), 7);
    }

    #[test]
    fn parse_dkt_id_numeric_only() {
        assert_eq!(parse_dkt_id("100").unwrap(), 100);
    }

    #[test]
    fn parse_dkt_id_empty_suffix_errors() {
        assert!(parse_dkt_id("DKT-").is_err());
        assert!(parse_dkt_id("abc").is_err());
    }

    #[test]
    fn parse_dkt_id_value_string() {
        assert_eq!(
            parse_dkt_id_value(&serde_json::Value::String("DKT-5".into())).unwrap(),
            5
        );
    }

    #[test]
    fn parse_dkt_id_value_integer() {
        assert_eq!(parse_dkt_id_value(&serde_json::json!(3)).unwrap(), 3);
    }
}

use sea_query::enum_def;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationKind {
    Blocks,
    BlockedBy,
    DependsOn,
    DependencyOf,
    RelatesTo,
    Duplicates,
    DuplicateOf,
}

impl RelationKind {
    pub fn label(self) -> &'static str {
        match self {
            RelationKind::Blocks => "blocks",
            RelationKind::BlockedBy => "blocked-by",
            RelationKind::DependsOn => "depends-on",
            RelationKind::DependencyOf => "dependency-of",
            RelationKind::RelatesTo => "relates-to",
            RelationKind::Duplicates => "duplicates",
            RelationKind::DuplicateOf => "duplicate-of",
        }
    }

    /// Returns the inverse relation kind.
    pub fn inverse(self) -> RelationKind {
        match self {
            RelationKind::Blocks => RelationKind::BlockedBy,
            RelationKind::BlockedBy => RelationKind::Blocks,
            RelationKind::DependsOn => RelationKind::DependencyOf,
            RelationKind::DependencyOf => RelationKind::DependsOn,
            RelationKind::RelatesTo => RelationKind::RelatesTo,
            RelationKind::Duplicates => RelationKind::DuplicateOf,
            RelationKind::DuplicateOf => RelationKind::Duplicates,
        }
    }

    /// True if this relation kind contributes a blocking edge in the DAG.
    ///
    /// All four directional kinds (`Blocks`, `BlockedBy`, `DependsOn`,
    /// `DependencyOf`) express a real ordering constraint and must be treated
    /// as DAG edges regardless of which of the two equivalent verbs was used
    /// to declare them. `RelatesTo`, `Duplicates`, and `DuplicateOf` are
    /// informational only and must remain non-DAG edges.
    pub fn is_dag_edge(self) -> bool {
        matches!(
            self,
            RelationKind::Blocks
                | RelationKind::BlockedBy
                | RelationKind::DependsOn
                | RelationKind::DependencyOf
        )
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for RelationKind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "blocks" => Ok(RelationKind::Blocks),
            "blocked-by" => Ok(RelationKind::BlockedBy),
            "depends-on" => Ok(RelationKind::DependsOn),
            "dependency-of" => Ok(RelationKind::DependencyOf),
            "relates-to" => Ok(RelationKind::RelatesTo),
            "duplicates" => Ok(RelationKind::Duplicates),
            "duplicate-of" => Ok(RelationKind::DuplicateOf),
            _ => anyhow::bail!("unknown relation kind: {s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def(table_name = "issue_relations")] // Generate RelationIden for use in sea-query
pub struct Relation {
    pub id: i64,
    pub from_id: i64,
    pub to_id: i64,
    pub kind: RelationKind,
}

/// A relation as it reads from one of its two endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RelationView {
    pub kind: RelationKind,
    pub other_id: i64,
    /// True for a relation of an issue to itself. `link add` rejects these,
    /// but older databases or direct SQL writes can still hold them.
    pub self_link: bool,
}

/// A stored relation together with how it reads from the issue being shown.
#[derive(Debug, Clone, Serialize)]
pub struct IssueRelation {
    #[serde(flatten)]
    pub relation: Relation,
    pub view: RelationView,
}

impl Relation {
    /// Describes this relation from the point of view of `issue_id`: the kind
    /// that issue has towards the issue at the other endpoint.
    pub fn viewed_from(&self, issue_id: i64) -> RelationView {
        let self_link = self.from_id == self.to_id;
        if issue_id == self.to_id && !self_link {
            RelationView {
                kind: self.kind.inverse(),
                other_id: self.from_id,
                self_link,
            }
        } else {
            RelationView {
                kind: self.kind,
                other_id: self.to_id,
                self_link,
            }
        }
    }

    /// Pairs this relation with its view from `issue_id`.
    pub fn with_view_from(self, issue_id: i64) -> IssueRelation {
        let view = self.viewed_from(issue_id);
        IssueRelation {
            relation: self,
            view,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KINDS: [RelationKind; 7] = [
        RelationKind::Blocks,
        RelationKind::BlockedBy,
        RelationKind::DependsOn,
        RelationKind::DependencyOf,
        RelationKind::RelatesTo,
        RelationKind::Duplicates,
        RelationKind::DuplicateOf,
    ];

    fn relation(from_id: i64, kind: RelationKind, to_id: i64) -> Relation {
        Relation {
            id: 1,
            from_id,
            to_id,
            kind,
        }
    }

    #[test]
    fn viewed_from_the_from_side_keeps_stored_kind_and_names_to_id() {
        for kind in ALL_KINDS {
            assert_eq!(
                relation(1, kind, 2).viewed_from(1),
                RelationView {
                    kind,
                    other_id: 2,
                    self_link: false
                },
                "from side of {kind}"
            );
        }
    }

    #[test]
    fn viewed_from_the_to_side_inverts_kind_and_names_from_id() {
        let expected = [
            (RelationKind::Blocks, RelationKind::BlockedBy),
            (RelationKind::BlockedBy, RelationKind::Blocks),
            (RelationKind::DependsOn, RelationKind::DependencyOf),
            (RelationKind::DependencyOf, RelationKind::DependsOn),
            (RelationKind::RelatesTo, RelationKind::RelatesTo),
            (RelationKind::Duplicates, RelationKind::DuplicateOf),
            (RelationKind::DuplicateOf, RelationKind::Duplicates),
        ];
        for (stored, seen) in expected {
            assert_eq!(
                relation(1, stored, 2).viewed_from(2),
                RelationView {
                    kind: seen,
                    other_id: 1,
                    self_link: false
                },
                "to side of {stored}"
            );
        }
    }

    #[test]
    fn viewed_from_a_self_relation_keeps_stored_kind_and_names_the_issue_itself() {
        for kind in ALL_KINDS {
            assert_eq!(
                relation(5, kind, 5).viewed_from(5),
                RelationView {
                    kind,
                    other_id: 5,
                    self_link: true
                },
                "self-relation of {kind}"
            );
        }
    }

    #[test]
    fn viewed_from_an_issue_at_neither_endpoint_reads_as_the_from_side() {
        for kind in ALL_KINDS {
            assert_eq!(
                relation(1, kind, 2).viewed_from(3),
                RelationView {
                    kind,
                    other_id: 2,
                    self_link: false
                },
                "{kind} viewed from an unrelated issue"
            );
        }
    }

    #[test]
    fn viewed_from_serialises_kind_as_kebab_case_label() {
        let view = relation(1, RelationKind::Blocks, 2).viewed_from(2);
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            serde_json::json!({"kind": "blocked-by", "other_id": 1, "self_link": false})
        );
    }

    #[test]
    fn with_view_from_serialises_the_stored_row_and_its_view() {
        let shown = relation(1, RelationKind::Blocks, 2).with_view_from(2);
        assert_eq!(
            serde_json::to_value(shown).unwrap(),
            serde_json::json!({
                "id": 1,
                "from_id": 1,
                "to_id": 2,
                "kind": "blocks",
                "view": {"kind": "blocked-by", "other_id": 1, "self_link": false}
            })
        );
    }

    #[test]
    fn relation_kind_round_trip() {
        let kinds = [
            RelationKind::Blocks,
            RelationKind::BlockedBy,
            RelationKind::DependsOn,
            RelationKind::DependencyOf,
            RelationKind::RelatesTo,
            RelationKind::Duplicates,
            RelationKind::DuplicateOf,
        ];
        for k in kinds {
            let label = k.label();
            let parsed: RelationKind = label.parse().unwrap();
            assert_eq!(k, parsed, "round trip failed for {label}");
        }
    }

    #[test]
    fn inverse_pairs() {
        assert_eq!(RelationKind::Blocks.inverse(), RelationKind::BlockedBy);
        assert_eq!(
            RelationKind::DependsOn.inverse(),
            RelationKind::DependencyOf
        );
        assert_eq!(RelationKind::RelatesTo.inverse(), RelationKind::RelatesTo);
    }

    #[test]
    fn is_dag_edge_true_for_all_directional_kinds() {
        assert!(RelationKind::Blocks.is_dag_edge());
        assert!(RelationKind::BlockedBy.is_dag_edge());
        assert!(RelationKind::DependsOn.is_dag_edge());
        assert!(RelationKind::DependencyOf.is_dag_edge());
    }

    #[test]
    fn is_dag_edge_false_for_informational_kinds() {
        assert!(!RelationKind::RelatesTo.is_dag_edge());
        assert!(!RelationKind::Duplicates.is_dag_edge());
        assert!(!RelationKind::DuplicateOf.is_dag_edge());
    }
}

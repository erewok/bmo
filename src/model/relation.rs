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
    pub fn is_dag_edge(self) -> bool {
        matches!(self, RelationKind::Blocks | RelationKind::DependsOn)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

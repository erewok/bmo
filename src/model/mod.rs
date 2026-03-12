//! Domain model types: [`Issue`], [`Status`], [`Priority`], [`Kind`], and related types.

pub mod activity;
pub mod comment;
pub mod export;
pub mod file;
pub mod issue;
pub mod label;
pub mod relation;

pub use activity::ActivityEntry;
pub use comment::Comment;
pub use file::IssueFile;
pub use issue::{Issue, IssueFilter, Kind, Priority, Status};
pub use label::Label;
pub use relation::{Relation, RelationKind};

use sea_query::enum_def;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[enum_def] // Generate LabelIden for use in sea-query
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

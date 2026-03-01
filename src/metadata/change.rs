use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub git_sha: Option<String>,
    pub summary: String,
    pub intent: Option<String>,
    pub author_type: AuthorType,
    pub author_name: String,
    pub task_id: Option<String>,
    pub change_type: ChangeType,
    pub status: ChangeStatus,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub parent_change_id: Option<String>,
    #[serde(default)]
    pub author_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorType {
    Human,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Change,
    Checkpoint,
    Fix,
    Undo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Active,
    Undone,
    Squashed,
}

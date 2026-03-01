use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub status: TaskStatus,
    pub branch: String,
    pub worktree_path: Option<String>,
    pub base_ref: String,
    pub changes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ticket_ref: Option<String>,
    #[serde(default)]
    pub abandoned_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    InProgress,
    Completed,
    Abandoned,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

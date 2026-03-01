use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bidirectional mapping of Change UUID ↔ Git commit SHA.
/// Persisted to refs/arc/index/change-map.json.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChangeMap {
    entries: HashMap<String, String>,
}

impl ChangeMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn insert(&mut self, uuid: String, sha: String) {
        self.entries.insert(uuid, sha);
    }

    pub fn sha_for_uuid(&self, uuid: &str) -> Option<&str> {
        self.entries.get(uuid).map(|s| s.as_str())
    }
}

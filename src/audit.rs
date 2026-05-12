use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PrivacyReport {
    pub session_id: String,
    pub started_at: String,
    pub history_stored: bool,
    pub backend: String,
    pub security_mode: String,
    pub gpu_layers: String,
    pub process_exited_cleanly: bool,
    pub workspace_deleted: bool,
    pub residual_risk: String,
}

impl PrivacyReport {
    pub fn new(
        session_id: String,
        started_at: DateTime<Utc>,
        history_stored: bool,
        backend: String,
        security_mode: String,
        gpu_layers: String,
        process_exited_cleanly: bool,
        workspace_deleted: bool,
    ) -> Self {
        Self {
            session_id,
            started_at: started_at.to_rfc3339(),
            history_stored,
            backend,
            security_mode,
            gpu_layers,
            process_exited_cleanly,
            workspace_deleted,
            residual_risk:
                "OS memory, swap, shell history, and llama.cpp internal allocations are not yet sanitized."
                    .to_string(),
        }
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

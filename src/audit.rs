use crate::cleanup::CleanupReport;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PrivacyReport {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub history_stored: bool,
    pub backend: String,
    pub security_mode: String,
    pub gpu_layers: String,
    pub process_exited_cleanly: bool,
    pub cleanup: CleanupReport,
    pub session_profile: Option<SessionProfile>,
    pub residual_risk: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionProfile {
    pub session_kind: String,
    pub runtime_lifetime: String,
    pub turn_count: usize,
    pub runtime_duration_ms: i64,
    pub history_policy: String,
    pub persistence_policy: String,
    pub prompt_source: String,
    pub turn_artifacts: Vec<TurnArtifact>,
    pub active_runtime_residual_risk: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnArtifact {
    pub turn: usize,
    pub prompt_path: String,
    pub response_path: String,
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
        cleanup: CleanupReport,
    ) -> Self {
        Self {
            session_id,
            started_at,
            history_stored,
            backend,
            security_mode,
            gpu_layers,
            process_exited_cleanly,
            cleanup,
            session_profile: None,
            residual_risk:
                "OS memory, swap, shell history, and llama.cpp internal allocations are not yet sanitized."
                    .to_string(),
        }
    }

    pub fn with_session_profile(mut self, profile: SessionProfile) -> Self {
        self.session_profile = Some(profile);
        self
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

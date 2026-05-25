use crate::cleanup::CleanupReport;
use crate::registry::{
    CleanupReason, RetentionPolicy, SessionLifecycleMetadata, SessionLifecycleState,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub lifecycle: Option<LifecycleReport>,
    pub residual_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnArtifact {
    pub turn: usize,
    pub prompt_path: String,
    pub response_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleReport {
    pub state: String,
    pub retention_policy: String,
    pub retention_deadline: Option<String>,
    pub cleanup_requested_at: Option<String>,
    pub cleanup_completed_at: Option<String>,
    pub cleanup_reason: Option<String>,
    pub updated_at: Option<String>,
    pub policy_summary: String,
    pub decision_summary: String,
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
            lifecycle: None,
            residual_risk:
                "OS memory, swap, shell history, and llama.cpp internal allocations are not yet sanitized."
                    .to_string(),
        }
    }

    pub fn with_session_profile(mut self, profile: SessionProfile) -> Self {
        self.session_profile = Some(profile);
        self
    }

    pub fn with_lifecycle(mut self, lifecycle: &SessionLifecycleMetadata) -> Self {
        self.lifecycle = Some(LifecycleReport::from_metadata(lifecycle));
        self
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl LifecycleReport {
    pub fn from_metadata(metadata: &SessionLifecycleMetadata) -> Self {
        Self {
            state: metadata.state.as_str().to_string(),
            retention_policy: metadata.retention_policy.as_str().to_string(),
            retention_deadline: metadata.retention_deadline.clone(),
            cleanup_requested_at: metadata.cleanup_requested_at.clone(),
            cleanup_completed_at: metadata.cleanup_completed_at.clone(),
            cleanup_reason: metadata
                .cleanup_reason
                .as_ref()
                .map(|reason| reason.as_str().to_string()),
            updated_at: metadata.updated_at.clone(),
            policy_summary: lifecycle_policy_summary(metadata),
            decision_summary: lifecycle_decision_summary(metadata),
        }
    }
}

pub fn sync_report_lifecycle(
    report_path: &Path,
    lifecycle: &SessionLifecycleMetadata,
) -> Result<()> {
    if !report_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(report_path)?;
    let mut report: PrivacyReport = serde_json::from_str(&raw)?;
    report.lifecycle = Some(LifecycleReport::from_metadata(lifecycle));
    fs::write(report_path, report.to_pretty_json()?)?;

    Ok(())
}

fn lifecycle_policy_summary(metadata: &SessionLifecycleMetadata) -> String {
    match metadata.retention_policy {
        RetentionPolicy::EphemeralImmediate => {
            "Ephemeral policy targeted immediate cleanup at session end.".to_string()
        }
        RetentionPolicy::RetainUntilManualCleanup => {
            "Session is retained until an explicit operator cleanup action is requested."
                .to_string()
        }
        RetentionPolicy::RetainForDuration => {
            if let Some(deadline) = &metadata.retention_deadline {
                format!(
                    "Session is retained until {deadline}, after which scheduled cleanup may run."
                )
            } else {
                "Session is configured for scheduled retention expiry, but no deadline is currently recorded."
                    .to_string()
            }
        }
    }
}

fn lifecycle_decision_summary(metadata: &SessionLifecycleMetadata) -> String {
    match metadata.state {
        SessionLifecycleState::CompletedRetained => {
            "Session completed and its retained artifacts remain available under the current lifecycle policy."
                .to_string()
        }
        SessionLifecycleState::CleanupPending => {
            "Cleanup has been requested but has not yet completed.".to_string()
        }
        SessionLifecycleState::CleanupSucceeded => {
            let reason = metadata
                .cleanup_reason
                .as_ref()
                .map(cleanup_reason_summary)
                .unwrap_or("Cleanup completed successfully.");

            reason.to_string()
        }
        SessionLifecycleState::CleanupFailed => {
            let reason = metadata
                .cleanup_reason
                .as_ref()
                .map(cleanup_reason_summary)
                .unwrap_or("Cleanup attempted but did not complete successfully.");

            format!("{reason} Cleanup failed or requires operator follow-up.")
        }
        SessionLifecycleState::Orphaned => {
            "Lifecycle reconciliation detected an inconsistency between registry state and on-disk artifacts. Operator review is recommended."
                .to_string()
        }
        SessionLifecycleState::Active => {
            "Session is still marked active in lifecycle metadata.".to_string()
        }
    }
}

fn cleanup_reason_summary(reason: &CleanupReason) -> &'static str {
    match reason {
        CleanupReason::EphemeralPolicy => {
            "Cleanup ran because the session policy was ephemeral-at-end."
        }
        CleanupReason::ManualOperatorRequest => {
            "Cleanup ran because an operator explicitly requested lifecycle cleanup."
        }
        CleanupReason::ScheduledRetentionExpiry => {
            "Cleanup ran because the scheduled retention deadline expired."
        }
        CleanupReason::StartupOrphanReconciliation => {
            "Lifecycle reconciliation changed the session state during startup recovery."
        }
    }
}

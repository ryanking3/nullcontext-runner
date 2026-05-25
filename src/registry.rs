use crate::cleanup::CleanupReport;
use crate::config::SessionConfig;
use crate::session::Session;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRegistry {
    pub sessions: Vec<SessionIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexEntry {
    pub session_id: String,
    pub started_at: String,
    pub security_mode: String,
    pub prompt_source: String,
    pub history_stored: bool,
    pub backend: String,
    pub model_path: String,
    pub workspace: String,
    pub report_path: String,
    pub artifacts_detected: usize,
    pub cleanup_attempted: bool,
    pub cleanup_successful: bool,
    pub workspace_deleted: bool,
    #[serde(default)]
    pub lifecycle: SessionLifecycleMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleState {
    Active,
    #[default]
    CompletedRetained,
    CleanupPending,
    CleanupSucceeded,
    CleanupFailed,
    Orphaned,
}

impl SessionLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::CompletedRetained => "completed_retained",
            Self::CleanupPending => "cleanup_pending",
            Self::CleanupSucceeded => "cleanup_succeeded",
            Self::CleanupFailed => "cleanup_failed",
            Self::Orphaned => "orphaned",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    EphemeralImmediate,
    #[default]
    RetainUntilManualCleanup,
    RetainForDuration,
}

impl RetentionPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EphemeralImmediate => "ephemeral_immediate",
            Self::RetainUntilManualCleanup => "retain_until_manual_cleanup",
            Self::RetainForDuration => "retain_for_duration",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupReason {
    EphemeralPolicy,
    ManualOperatorRequest,
    ScheduledRetentionExpiry,
    StartupOrphanReconciliation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionLifecycleMetadata {
    #[serde(default)]
    pub state: SessionLifecycleState,
    #[serde(default)]
    pub retention_policy: RetentionPolicy,
    #[serde(default)]
    pub retention_deadline: Option<String>,
    #[serde(default)]
    pub cleanup_requested_at: Option<String>,
    #[serde(default)]
    pub cleanup_completed_at: Option<String>,
    #[serde(default)]
    pub cleanup_reason: Option<CleanupReason>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl SessionRegistry {
    pub fn load(home: &str) -> Result<Self> {
        let index_path = registry_path(home);

        if !index_path.exists() {
            return Ok(Self { sessions: vec![] });
        }

        let raw = fs::read_to_string(&index_path)
            .with_context(|| format!("Failed to read registry at {}", index_path.display()))?;

        let parsed = serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse registry at {}", index_path.display()))?;

        Ok(parsed)
    }

    pub fn save(&self, home: &str) -> Result<()> {
        let root = registry_root(home);
        fs::create_dir_all(&root)?;

        let index_path = registry_path(home);
        let temp_path = root.join("index.json.tmp");

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&temp_path, json)?;

        fs::rename(&temp_path, &index_path)
            .with_context(|| format!("Failed to write registry at {}", index_path.display()))?;

        Ok(())
    }

    pub fn register(&mut self, entry: SessionIndexEntry) {
        self.sessions.retain(|s| s.session_id != entry.session_id);
        self.sessions.push(entry);
        self.sessions
            .sort_by(|a, b| b.started_at.cmp(&a.started_at));
    }

    pub fn find(&self, session_id: &str) -> Option<&SessionIndexEntry> {
        self.sessions.iter().find(|s| s.session_id == session_id)
    }

    #[allow(dead_code)]
    pub fn find_mut(&mut self, session_id: &str) -> Option<&mut SessionIndexEntry> {
        self.sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
    }
}

impl SessionIndexEntry {
    pub fn from_session(
        session: &Session,
        config: &SessionConfig,
        cleanup: &CleanupReport,
    ) -> Self {
        let report_path = session.workspace.join("report.json");

        Self {
            session_id: session.id.clone(),
            started_at: session.started_at.to_rfc3339(),
            security_mode: config.security_mode.as_str().to_string(),
            prompt_source: config.prompt_source.as_str().to_string(),
            history_stored: !config.ephemeral,
            backend: "llama-server".to_string(),
            model_path: config.model_path.clone(),
            workspace: session.workspace.display().to_string(),
            report_path: report_path.display().to_string(),
            artifacts_detected: cleanup.artifacts_detected.len(),
            cleanup_attempted: cleanup.attempted,
            cleanup_successful: cleanup.successful,
            workspace_deleted: cleanup.workspace_deleted,
            lifecycle: SessionLifecycleMetadata::for_completed_session(config, cleanup),
        }
    }

    #[allow(dead_code)]
    pub fn mark_cleanup_pending(&mut self, reason: CleanupReason) {
        self.lifecycle.state = SessionLifecycleState::CleanupPending;
        self.lifecycle.cleanup_requested_at = Some(current_timestamp());
        self.lifecycle.cleanup_reason = Some(reason);
        self.lifecycle.updated_at = self.lifecycle.cleanup_requested_at.clone();
    }

    #[allow(dead_code)]
    pub fn mark_cleanup_result(&mut self, cleanup: &CleanupReport, reason: CleanupReason) {
        self.artifacts_detected = cleanup.artifacts_detected.len();
        self.cleanup_attempted = cleanup.attempted;
        self.cleanup_successful = cleanup.successful;
        self.workspace_deleted = cleanup.workspace_deleted;
        self.lifecycle.state = if cleanup.successful {
            SessionLifecycleState::CleanupSucceeded
        } else {
            SessionLifecycleState::CleanupFailed
        };
        self.lifecycle.cleanup_completed_at = Some(current_timestamp());
        self.lifecycle.cleanup_reason = Some(reason);
        self.lifecycle.updated_at = self.lifecycle.cleanup_completed_at.clone();
    }

    #[allow(dead_code)]
    pub fn mark_orphaned(&mut self) {
        self.lifecycle.state = SessionLifecycleState::Orphaned;
        self.lifecycle.updated_at = Some(current_timestamp());
    }
}

impl SessionLifecycleMetadata {
    pub fn for_completed_session(config: &SessionConfig, cleanup: &CleanupReport) -> Self {
        let updated_at = Some(current_timestamp());

        if config.ephemeral {
            let state = if cleanup.successful {
                SessionLifecycleState::CleanupSucceeded
            } else {
                SessionLifecycleState::CleanupFailed
            };

            return Self {
                state,
                retention_policy: RetentionPolicy::EphemeralImmediate,
                retention_deadline: None,
                cleanup_requested_at: None,
                cleanup_completed_at: updated_at.clone(),
                cleanup_reason: Some(CleanupReason::EphemeralPolicy),
                updated_at,
            };
        }

        Self {
            state: SessionLifecycleState::CompletedRetained,
            retention_policy: RetentionPolicy::RetainUntilManualCleanup,
            retention_deadline: None,
            cleanup_requested_at: None,
            cleanup_completed_at: None,
            cleanup_reason: None,
            updated_at,
        }
    }
}

pub fn register_persistent_session(
    home: &str,
    session: &Session,
    config: &SessionConfig,
    cleanup: &CleanupReport,
) -> Result<()> {
    let mut registry = SessionRegistry::load(home)?;
    let entry = SessionIndexEntry::from_session(session, config, cleanup);

    registry.register(entry);
    registry.save(home)?;

    Ok(())
}

pub fn list_sessions(home: &str) -> Result<()> {
    let registry = SessionRegistry::load(home)?;

    if registry.sessions.is_empty() {
        println!("No persistent NullContext sessions found.");
        return Ok(());
    }

    println!("Persistent NullContext sessions:\n");

    for session in registry.sessions {
        println!("Session ID: {}", session.session_id);
        println!("Started: {}", session.started_at);
        println!("Mode: {}", session.security_mode);
        println!("Lifecycle state: {}", session.lifecycle.state.as_str());
        println!(
            "Retention policy: {}",
            session.lifecycle.retention_policy.as_str()
        );
        println!("Prompt source: {}", session.prompt_source);
        println!("Workspace: {}", session.workspace);
        println!("Report: {}", session.report_path);
        println!("Artifacts detected: {}", session.artifacts_detected);
        println!("---");
    }

    Ok(())
}

pub fn show_report(home: &str, session_id: &str) -> Result<()> {
    let registry = SessionRegistry::load(home)?;

    let entry = registry
        .find(session_id)
        .with_context(|| format!("Session not found in registry: {session_id}"))?;

    let report_path = PathBuf::from(&entry.report_path);

    if !report_path.exists() {
        anyhow::bail!(
            "Report path exists in registry but file was not found: {}",
            report_path.display()
        );
    }

    let report = fs::read_to_string(&report_path)
        .with_context(|| format!("Failed to read report at {}", report_path.display()))?;

    println!("{}", report);

    Ok(())
}

fn registry_root(home: &str) -> PathBuf {
    Path::new(home).join(".nullcontext")
}

fn registry_path(home: &str) -> PathBuf {
    registry_root(home).join("index.json")
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

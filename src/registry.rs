use crate::cleanup::CleanupReport;
use crate::config::SessionConfig;
use crate::logging::stdout_line;
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

#[derive(Debug, Clone)]
pub struct StartupReconciliationSummary {
    pub scanned_sessions: usize,
    pub changed_sessions: usize,
    pub orphaned_sessions: usize,
    pub abandoned_active_sessions: usize,
    pub cleanup_succeeded_consistent: usize,
    pub unchanged_sessions: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SessionReportAvailability {
    pub current_exists: bool,
    pub available: bool,
    pub storage: &'static str,
    pub loadable_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexEntry {
    pub session_id: String,
    pub started_at: String,
    pub security_mode: String,
    pub prompt_source: String,
    pub history_stored: bool,
    pub backend: String,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub model_name: String,
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
    AbandonedActive,
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
            Self::AbandonedActive => "abandoned_active",
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

    pub fn from_str(value: &str) -> Result<Self> {
        match value {
            "ephemeral_immediate" => Ok(Self::EphemeralImmediate),
            "retain_until_manual_cleanup" => Ok(Self::RetainUntilManualCleanup),
            "retain_for_duration" => Ok(Self::RetainForDuration),
            _ => anyhow::bail!("Invalid retention policy: {value}"),
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

impl CleanupReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EphemeralPolicy => "ephemeral_policy",
            Self::ManualOperatorRequest => "manual_operator_request",
            Self::ScheduledRetentionExpiry => "scheduled_retention_expiry",
            Self::StartupOrphanReconciliation => "startup_orphan_reconciliation",
        }
    }
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
    pub state_note: Option<String>,
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

    pub fn remove(&mut self, session_id: &str) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.session_id != session_id);
        self.sessions.len() != before
    }
}

impl SessionIndexEntry {
    pub fn from_active_session(session: &Session, config: &SessionConfig) -> Self {
        let report_path = session.workspace.join("report.json");

        Self {
            session_id: session.id.clone(),
            started_at: session.started_at.to_rfc3339(),
            security_mode: config.security_mode.as_str().to_string(),
            prompt_source: config.prompt_source.as_str().to_string(),
            history_stored: !config.ephemeral,
            backend: "llama-server".to_string(),
            model_id: config.model_id.clone(),
            model_name: config.model_name.clone(),
            model_path: config.model_path.clone(),
            workspace: session.workspace.display().to_string(),
            report_path: report_path.display().to_string(),
            artifacts_detected: 0,
            cleanup_attempted: false,
            cleanup_successful: false,
            workspace_deleted: false,
            lifecycle: SessionLifecycleMetadata::for_started_session(config),
        }
    }

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
            model_id: config.model_id.clone(),
            model_name: config.model_name.clone(),
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
        self.lifecycle.state_note = Some(
            "Lifecycle cleanup was requested and NullContext is preparing to archive the report and remove the retained workspace."
                .to_string(),
        );
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
        self.lifecycle.state_note = Some(if cleanup.successful {
            "Lifecycle cleanup finished and NullContext recorded the retained workspace cleanup result."
                .to_string()
        } else {
            "Lifecycle cleanup finished with a failed or partial result. Operator follow-up may still be required."
                .to_string()
        });
        self.lifecycle.updated_at = self.lifecycle.cleanup_completed_at.clone();
    }

    #[allow(dead_code)]
    pub fn mark_orphaned(&mut self) {
        self.mark_orphaned_with_note(
            "Lifecycle reconciliation detected an inconsistency between the registry entry and on-disk session artifacts."
                .to_string(),
        );
    }

    pub fn mark_orphaned_with_note(&mut self, note: String) {
        self.lifecycle.state = SessionLifecycleState::Orphaned;
        self.lifecycle.cleanup_reason = Some(CleanupReason::StartupOrphanReconciliation);
        self.lifecycle.state_note = Some(note);
        self.lifecycle.updated_at = Some(current_timestamp());
    }

    pub fn mark_abandoned_active_with_note(&mut self, note: String) {
        self.lifecycle.state = SessionLifecycleState::AbandonedActive;
        self.lifecycle.cleanup_reason = Some(CleanupReason::StartupOrphanReconciliation);
        self.lifecycle.state_note = Some(note);
        self.lifecycle.updated_at = Some(current_timestamp());
    }

    pub fn apply_retention_policy(
        &mut self,
        retention_policy: RetentionPolicy,
        retention_deadline: Option<String>,
    ) {
        self.lifecycle.retention_policy = retention_policy;
        self.lifecycle.retention_deadline = retention_deadline;
        self.lifecycle.updated_at = Some(current_timestamp());
    }
}

impl SessionLifecycleMetadata {
    pub fn for_started_session(config: &SessionConfig) -> Self {
        let updated_at = Some(current_timestamp());

        if config.ephemeral {
            return Self {
                state: SessionLifecycleState::Active,
                retention_policy: RetentionPolicy::EphemeralImmediate,
                retention_deadline: None,
                cleanup_requested_at: None,
                cleanup_completed_at: None,
                cleanup_reason: None,
                state_note: Some(
                    "Session is currently live in memory and has not yet reached its cleanup boundary."
                        .to_string(),
                ),
                updated_at,
            };
        }

        Self {
            state: SessionLifecycleState::Active,
            retention_policy: RetentionPolicy::RetainUntilManualCleanup,
            retention_deadline: None,
            cleanup_requested_at: None,
            cleanup_completed_at: None,
            cleanup_reason: None,
            state_note: Some(
                "Persistent active chat was registered at session start so startup reconciliation can flag it if NullContext exits unexpectedly."
                    .to_string(),
            ),
            updated_at,
        }
    }

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
                state_note: Some(
                    "Session reached its ephemeral cleanup boundary and NullContext recorded the cleanup result."
                        .to_string(),
                ),
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
            state_note: Some(
                "Session ended cleanly and its retained artifacts remain available under the current lifecycle policy."
                    .to_string(),
            ),
            updated_at,
        }
    }

    pub fn for_failed_startup(config: &SessionConfig, cleanup: &CleanupReport) -> Self {
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
                state_note: Some(
                    "Session failed before llama-server became ready, and NullContext recorded the resulting ephemeral cleanup outcome."
                        .to_string(),
                ),
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
            state_note: Some(
                "Session failed before llama-server became ready. Retained prompt and report artifacts remain available for operator review under the current lifecycle policy."
                    .to_string(),
            ),
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
    let previous = registry.find(&session.id).cloned();
    let mut entry = SessionIndexEntry::from_session(session, config, cleanup);

    if let Some(previous) = previous {
        entry.lifecycle.retention_policy = previous.lifecycle.retention_policy;
        entry.lifecycle.retention_deadline = previous.lifecycle.retention_deadline;
    }

    registry.register(entry);
    registry.save(home)?;

    Ok(())
}

pub fn register_active_persistent_session(
    home: &str,
    session: &Session,
    config: &SessionConfig,
) -> Result<()> {
    if config.ephemeral {
        anyhow::bail!("Ephemeral sessions are not registered in the persistent registry at start");
    }

    let mut registry = SessionRegistry::load(home)?;
    let entry = SessionIndexEntry::from_active_session(session, config);
    registry.register(entry);
    registry.save(home)?;

    Ok(())
}

pub fn unregister_persistent_session(home: &str, session_id: &str) -> Result<bool> {
    let mut registry = SessionRegistry::load(home)?;
    let removed = registry.remove(session_id);

    if removed {
        registry.save(home)?;
    }

    Ok(removed)
}

pub fn list_sessions(home: &str) -> Result<()> {
    let registry = SessionRegistry::load(home)?;

    if registry.sessions.is_empty() {
        stdout_line("No persistent NullContext sessions found.");
        return Ok(());
    }

    stdout_line("Persistent NullContext sessions:\n");

    for session in registry.sessions {
        let report = resolve_session_report_availability(home, &session);
        stdout_line(format!("Session ID: {}", session.session_id));
        stdout_line(format!("Started: {}", session.started_at));
        stdout_line(format!("Mode: {}", session.security_mode));
        stdout_line(format!(
            "Lifecycle state: {}",
            session.lifecycle.state.as_str()
        ));
        stdout_line(format!(
            "Retention policy: {}",
            session.lifecycle.retention_policy.as_str()
        ));
        stdout_line(format!("Prompt source: {}", session.prompt_source));
        stdout_line(format!("Workspace: {}", session.workspace));
        stdout_line(format!("Report: {}", session.report_path));
        stdout_line(format!(
            "Report available: {} ({})",
            if report.available { "yes" } else { "no" },
            report.storage
        ));
        if let Some(loadable_path) = report.loadable_path {
            stdout_line(format!("Loadable report path: {}", loadable_path.display()));
        }
        stdout_line(format!(
            "Artifacts detected: {}",
            session.artifacts_detected
        ));
        stdout_line("---");
    }

    Ok(())
}

pub fn show_report(home: &str, session_id: &str) -> Result<()> {
    let registry = SessionRegistry::load(home)?;

    let entry = registry
        .find(session_id)
        .with_context(|| format!("Session not found in registry: {session_id}"))?;

    let availability = resolve_session_report_availability(home, entry);
    let Some(report_path) = availability.loadable_path else {
        anyhow::bail!(
            "No loadable report was found for session {session_id}. NullContext checked the current report path and any archived lifecycle report."
        );
    };

    let report = fs::read_to_string(&report_path)
        .with_context(|| format!("Failed to read report at {}", report_path.display()))?;

    stdout_line(&report);

    Ok(())
}

pub fn reconcile_registry_on_startup(home: &str) -> Result<StartupReconciliationSummary> {
    let mut registry = SessionRegistry::load(home)?;
    let mut summary = StartupReconciliationSummary {
        scanned_sessions: registry.sessions.len(),
        changed_sessions: 0,
        orphaned_sessions: 0,
        abandoned_active_sessions: 0,
        cleanup_succeeded_consistent: 0,
        unchanged_sessions: 0,
        notes: Vec::new(),
    };

    for entry in &mut registry.sessions {
        let message = reconcile_entry(home, entry);

        match message {
            ReconciliationOutcome::Changed(note) => {
                summary.changed_sessions += 1;

                if entry.lifecycle.state == SessionLifecycleState::Orphaned {
                    summary.orphaned_sessions += 1;
                }

                if entry.lifecycle.state == SessionLifecycleState::AbandonedActive {
                    summary.abandoned_active_sessions += 1;
                }

                summary
                    .notes
                    .push(format!("{}: {}", entry.session_id, note));
            }
            ReconciliationOutcome::CleanupConsistent(note) => {
                summary.cleanup_succeeded_consistent += 1;
                summary.unchanged_sessions += 1;
                summary
                    .notes
                    .push(format!("{}: {}", entry.session_id, note));
            }
            ReconciliationOutcome::Unchanged(note) => {
                summary.unchanged_sessions += 1;
                summary
                    .notes
                    .push(format!("{}: {}", entry.session_id, note));
            }
        }
    }

    if summary.changed_sessions > 0 {
        registry.save(home)?;
    }

    Ok(summary)
}

pub fn due_retention_cleanup_session_ids(home: &str) -> Result<Vec<String>> {
    let registry = SessionRegistry::load(home)?;
    let now = Utc::now();

    let due = registry
        .sessions
        .iter()
        .filter(|entry| {
            entry.lifecycle.retention_policy == RetentionPolicy::RetainForDuration
                && entry.lifecycle.state == SessionLifecycleState::CompletedRetained
                && entry
                    .lifecycle
                    .retention_deadline
                    .as_deref()
                    .and_then(parse_timestamp)
                    .is_some_and(|deadline| deadline <= now)
        })
        .map(|entry| entry.session_id.clone())
        .collect();

    Ok(due)
}

pub fn ensure_registry_dirs(home: &str) -> Result<()> {
    fs::create_dir_all(registry_root(home).join("reports"))?;
    Ok(())
}

pub fn archived_report_path(home: &str, session_id: &str) -> PathBuf {
    registry_root(home)
        .join("reports")
        .join(format!("{session_id}.json"))
}

pub fn resolve_session_report_availability(
    home: &str,
    entry: &SessionIndexEntry,
) -> SessionReportAvailability {
    let current_path = PathBuf::from(&entry.report_path);
    let current_exists = current_path.exists();
    let archived_path = archived_report_path(home, &entry.session_id);
    let archived_exists = archived_path.exists();
    let stored_is_archived = current_path == archived_path;

    if current_exists {
        return SessionReportAvailability {
            current_exists: true,
            available: true,
            storage: if stored_is_archived {
                "archived"
            } else {
                "current"
            },
            loadable_path: Some(current_path),
        };
    }

    if archived_exists {
        return SessionReportAvailability {
            current_exists: false,
            available: true,
            storage: "archived_fallback",
            loadable_path: Some(archived_path),
        };
    }

    SessionReportAvailability {
        current_exists: false,
        available: false,
        storage: "missing",
        loadable_path: None,
    }
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

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

enum ReconciliationOutcome {
    Changed(String),
    CleanupConsistent(String),
    Unchanged(String),
}

fn reconcile_entry(home: &str, entry: &mut SessionIndexEntry) -> ReconciliationOutcome {
    let workspace_exists = Path::new(&entry.workspace).exists();
    let mut report_exists = Path::new(&entry.report_path).exists();
    let archived_report_path = archived_report_path(home, &entry.session_id);
    let recovered_archived_report = !report_exists && archived_report_path.exists();

    if recovered_archived_report {
        entry.report_path = archived_report_path.display().to_string();
        report_exists = true;
        entry.lifecycle.updated_at = Some(current_timestamp());
    }

    if entry.lifecycle.state == SessionLifecycleState::CleanupPending {
        entry.mark_orphaned_with_note(
            "Startup found this retained session still marked cleanup_pending, so the prior cleanup attempt likely ended before completion and needs operator review."
                .to_string(),
        );
        return ReconciliationOutcome::Changed(
            "Startup found a session still marked cleanup_pending; reclassified as orphaned."
                .to_string(),
        );
    }

    if entry.lifecycle.state == SessionLifecycleState::Active {
        entry.mark_abandoned_active_with_note(
            "Startup found this retained session still marked active. The in-memory runtime could not be recovered after restart, so the session was marked abandoned_active for review."
                .to_string(),
        );
        return ReconciliationOutcome::Changed(
            "Startup found a registry entry still marked active; reclassified as abandoned_active because active in-memory state cannot be recovered after restart."
                .to_string(),
        );
    }

    if entry.cleanup_successful && !workspace_exists {
        if !report_exists {
            entry.mark_orphaned_with_note(
                "Cleanup had been recorded as successful and the workspace is gone, but the saved report path is also missing. The session was marked orphaned for investigation."
                    .to_string(),
            );
            return ReconciliationOutcome::Changed(
                "Cleanup had been recorded as successful and the workspace is gone, but the report path is missing; marked orphaned for investigation."
                    .to_string(),
            );
        }

        if recovered_archived_report {
            entry.lifecycle.state_note = Some(
                "Startup reconciliation confirmed that cleanup had already succeeded, the retained workspace is gone, and the registry was relinked to the archived session report."
                    .to_string(),
            );
            return ReconciliationOutcome::Changed(
                "Cleanup had already succeeded and the workspace is gone; startup relinked the registry to the archived report."
                    .to_string(),
            );
        }

        return ReconciliationOutcome::CleanupConsistent(
            "Cleanup had already succeeded and startup confirmed the workspace remains removed."
                .to_string(),
        );
    }

    if !workspace_exists && !entry.cleanup_successful {
        entry.mark_orphaned_with_note(
            "The retained workspace is missing even though successful lifecycle cleanup was never recorded. The session was marked orphaned for investigation."
                .to_string(),
        );
        return ReconciliationOutcome::Changed(
            "Workspace is missing even though lifecycle cleanup was not recorded as successful; marked orphaned."
                .to_string(),
        );
    }

    if workspace_exists && entry.cleanup_successful {
        entry.mark_orphaned_with_note(
            "Lifecycle cleanup had been recorded as successful, but the retained workspace still exists on disk. The session was marked orphaned for investigation."
                .to_string(),
        );
        return ReconciliationOutcome::Changed(
            "Workspace still exists even though cleanup was previously recorded as successful; marked orphaned."
                .to_string(),
        );
    }

    if !report_exists && !entry.cleanup_successful {
        entry.mark_orphaned_with_note(
            "The retained session report is missing even though lifecycle cleanup was not recorded as successful. The session was marked orphaned for investigation."
                .to_string(),
        );
        return ReconciliationOutcome::Changed(
            "Report path is missing while cleanup was not recorded as successful; marked orphaned."
                .to_string(),
        );
    }

    if recovered_archived_report {
        entry.lifecycle.state_note = Some(
            "Startup reconciliation found that the retained session report had moved to the archived report path and relinked the registry entry."
                .to_string(),
        );
        return ReconciliationOutcome::Changed(
            "Startup relinked the retained session to the archived report path.".to_string(),
        );
    }

    ReconciliationOutcome::Unchanged(
        "Registry paths are present and no startup lifecycle changes were needed.".to_string(),
    )
}

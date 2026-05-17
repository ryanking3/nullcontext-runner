use crate::cleanup::CleanupReport;
use crate::config::SessionConfig;
use crate::session::Session;
use anyhow::{Context, Result};
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

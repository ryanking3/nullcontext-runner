use crate::audit::PrivacyReport;
use crate::cleanup::{
    cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation,
};
use crate::config::SessionConfig;
use crate::registry::register_persistent_session;
use crate::runtime::ManagedRuntime;
use crate::session::Session;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ChatSessionManager {
    sessions: Arc<Mutex<HashMap<String, ActiveChatSession>>>,
}

#[derive(Debug)]
struct ActiveChatSession {
    session: Session,
    config: SessionConfig,
    runtime: ManagedRuntime,
    turns: usize,
}

#[derive(Debug, Deserialize)]
pub struct StartChatRequest {
    pub mode: Option<String>,
    pub persistent: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct StartChatResponse {
    pub session_id: String,
    pub workspace: String,
    pub security_mode: String,
    pub persistent: bool,
    pub runtime_active: bool,
    pub turns: usize,
}

#[derive(Debug, Serialize)]
pub struct ChatStatusResponse {
    pub session_id: String,
    pub workspace: String,
    pub security_mode: String,
    pub persistent: bool,
    pub runtime_active: bool,
    pub turns: usize,
}

#[derive(Debug, Serialize)]
pub struct EndChatResponse {
    pub session_id: String,
    pub runtime_stopped: bool,
    pub report: serde_json::Value,
}

impl ChatSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_session(
        &self,
        home: String,
        request: StartChatRequest,
    ) -> Result<StartChatResponse> {
        let persistent = request.persistent.unwrap_or(false);

        let config =
            SessionConfig::from_web_request(home, String::new(), request.mode, persistent)?;

        let session = Session::create()?;

        println!("Starting active chat session...");
        println!("Session ID: {}", session.id);
        println!("Workspace: {}", session.workspace.display());
        println!("Security mode: {}", config.security_mode.as_str());

        let runtime = ManagedRuntime::launch(&config)?;

        let response = StartChatResponse {
            session_id: session.id.clone(),
            workspace: session.workspace.display().to_string(),
            security_mode: config.security_mode.as_str().to_string(),
            persistent: !config.ephemeral,
            runtime_active: true,
            turns: 0,
        };

        let active = ActiveChatSession {
            session,
            config,
            runtime,
            turns: 0,
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        sessions.insert(response.session_id.clone(), active);

        Ok(response)
    }

    pub fn status(&self, session_id: &str) -> Result<ChatStatusResponse> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        let Some(active) = sessions.get(session_id) else {
            bail!("Active chat session not found: {session_id}");
        };

        Ok(ChatStatusResponse {
            session_id: active.session.id.clone(),
            workspace: active.session.workspace.display().to_string(),
            security_mode: active.config.security_mode.as_str().to_string(),
            persistent: !active.config.ephemeral,
            runtime_active: true,
            turns: active.turns,
        })
    }

    pub fn end_session(&self, session_id: &str) -> Result<EndChatResponse> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        let Some(mut active) = sessions.remove(session_id) else {
            bail!("Active chat session not found: {session_id}");
        };

        drop(sessions);

        println!("Ending active chat session...");
        println!("Session ID: {}", active.session.id);

        let runtime_stopped = active.runtime.shutdown()?;

        let (artifacts_detected, scan_operation) = scan_artifacts(&active.session.workspace)?;

        let mut sanitization_operations = vec![scan_operation];

        sanitization_operations.push(SanitizationOperation {
            operation: "managed_chat_runtime_shutdown".to_string(),
            status: if runtime_stopped {
                "successful".to_string()
            } else {
                "failed".to_string()
            },
            details: "Long-lived llama-server chat runtime was terminated at session end."
                .to_string(),
        });

        sanitization_operations.push(SanitizationOperation {
            operation: "chat_session_lifecycle_end".to_string(),
            status: "successful".to_string(),
            details: format!(
                "Chat session ended after {} turn(s). Runtime lifetime was scoped to this session.",
                active.turns
            ),
        });

        let cleanup_report = if active.config.ephemeral {
            cleanup_ephemeral_workspace(
                &active.session.workspace,
                artifacts_detected,
                sanitization_operations,
            )
        } else {
            CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
        };

        let report = PrivacyReport::new(
            active.session.id.clone(),
            active.session.started_at,
            !active.config.ephemeral,
            "llama-server".to_string(),
            active.config.security_mode.as_str().to_string(),
            active.config.gpu_layers.clone(),
            runtime_stopped,
            cleanup_report.clone(),
        );

        let report_json = report.to_pretty_json()?;
        let parsed_report: serde_json::Value = serde_json::from_str(&report_json)?;

        if !active.config.ephemeral {
            active.session.write_report(&report_json)?;
            register_persistent_session(
                &active.config.home,
                &active.session,
                &active.config,
                &cleanup_report,
            )?;
        }

        Ok(EndChatResponse {
            session_id: active.session.id,
            runtime_stopped,
            report: parsed_report,
        })
    }
}

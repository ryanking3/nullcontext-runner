use crate::audit::{PrivacyReport, SessionProfile, TurnArtifact};
use crate::cleanup::{
    cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation,
};
use crate::config::{ChatTemplate, SessionConfig};
use crate::registry::register_persistent_session;
use crate::runtime::ManagedRuntime;
use crate::sensitive::SensitiveBytes;
use crate::session::Session;
use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroize;

#[derive(Debug, Clone)]
pub struct ChatSessionManager {
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<ActiveChatSession>>>>>,
}

#[derive(Debug)]
struct ActiveChatSession {
    session: Session,
    config: SessionConfig,
    runtime: ManagedRuntime,
    turns: Vec<ChatTurn>,
    generation_active: bool,
    ending: bool,
}

#[derive(Debug)]
struct ChatTurn {
    user: SensitiveBytes,
    assistant: SensitiveBytes,
}

#[derive(Debug, Deserialize)]
pub struct StartChatRequest {
    pub mode: Option<String>,
    pub persistent: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessageRequest {
    pub prompt: String,
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
    pub runtime_duration_ms: i64,
    pub history_policy: String,
    pub residual_risk: String,
}

#[derive(Debug, Serialize)]
pub struct EndChatResponse {
    pub session_id: String,
    pub runtime_stopped: bool,
    pub report: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

#[derive(Serialize)]
struct StreamingCompletionRequest {
    prompt: String,
    n_predict: u32,
    stream: bool,
}

impl StreamingCompletionRequest {
    fn sanitize(&mut self) {
        self.prompt.zeroize();
    }
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
            turns: vec![],
            generation_active: false,
            ending: false,
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        sessions.insert(response.session_id.clone(), Arc::new(Mutex::new(active)));

        Ok(response)
    }

    pub fn status(&self, session_id: &str) -> Result<ChatStatusResponse> {
        let session_handle = self.session_handle(session_id)?;
        let active = session_handle
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        let runtime_duration_ms = UtcNow::duration_since(active.session.started_at);

        Ok(ChatStatusResponse {
            session_id: active.session.id.clone(),
            workspace: active.session.workspace.display().to_string(),
            security_mode: active.config.security_mode.as_str().to_string(),
            persistent: !active.config.ephemeral,
            runtime_active: !active.ending,
            turns: active.turns.len(),
            runtime_duration_ms,
            history_policy: active_history_policy(active.config.ephemeral),
            residual_risk: active_runtime_risk(),
        })
    }

    pub fn stream_message<F>(
        &self,
        session_id: &str,
        request: ChatMessageRequest,
        mut emit: F,
    ) -> Result<()>
    where
        F: FnMut(ChatStreamEvent) -> bool,
    {
        let session_handle = self.session_handle(session_id)?;
        let user_buffer = SensitiveBytes::new(request.prompt);

        let (turn_number, completion_url, max_tokens, mut full_prompt) = {
            let mut active = session_handle
                .lock()
                .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

            if active.ending {
                bail!("Active chat session is ending: {session_id}");
            }

            if active.generation_active {
                bail!("Active chat session is already generating: {session_id}");
            }

            let turn_number = active.turns.len() + 1;
            let completion_url = active.runtime.completion_url();
            let max_tokens = active.config.max_tokens.parse::<u32>()?;
            let full_prompt = build_chat_prompt(
                active.config.chat_template,
                &active.turns,
                user_buffer.as_str(),
            );

            write_turn_prompt(&active.session, turn_number, user_buffer.as_bytes())?;

            active.generation_active = true;

            (turn_number, completion_url, max_tokens, full_prompt)
        };

        if !emit(runtime_event(format!(
            "Running chat turn {turn_number} on active runtime..."
        ))) {
            full_prompt.zeroize();
            clear_generation_active(&session_handle)?;
            return Ok(());
        }

        if !emit(runtime_event("--- Model Output ---")) {
            full_prompt.zeroize();
            clear_generation_active(&session_handle)?;
            return Ok(());
        }

        let stream_result =
            stream_completion_from_llama(&completion_url, &full_prompt, max_tokens, &mut emit);

        full_prompt.zeroize();

        let (response_text, completed) = match stream_result {
            Ok(result) => result,
            Err(error) => {
                clear_generation_active(&session_handle)?;
                return Err(error);
            }
        };

        if !completed {
            clear_generation_active(&session_handle)?;

            let _ = emit(audit_event(SanitizationOperation {
                operation: "chat_turn_cancelled".to_string(),
                status: "warning".to_string(),
                details: format!(
                    "Cancelled chat turn {turn_number}. Partial response was not committed to chat history."
                ),
            }));

            let _ = emit(complete_event(false));

            return Ok(());
        }

        let assistant_buffer = SensitiveBytes::new(response_text);

        let commit_result = (|| -> Result<()> {
            let mut active = session_handle
                .lock()
                .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

            write_turn_response(&active.session, turn_number, assistant_buffer.as_bytes())?;

            active.turns.push(ChatTurn {
                user: user_buffer,
                assistant: assistant_buffer,
            });

            active.generation_active = false;

            Ok(())
        })();

        if let Err(error) = commit_result {
            clear_generation_active(&session_handle)?;
            return Err(error);
        }

        let _ = emit(audit_event(SanitizationOperation {
            operation: "chat_turn_completed".to_string(),
            status: "successful".to_string(),
            details: format!(
                "Completed chat turn {turn_number}. Runtime remains active until session end."
            ),
        }));

        let _ = emit(complete_event(true));

        Ok(())
    }

    pub fn end_session(&self, session_id: &str) -> Result<EndChatResponse> {
        let session_handle = self.session_handle(session_id)?;

        {
            let mut active = session_handle
                .lock()
                .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

            if active.generation_active {
                bail!(
                    "Active chat generation is still in progress for session {session_id}. Stop the current generation and retry End + Sanitize once streaming has finished."
                );
            }

            if active.ending {
                bail!("Active chat session is already ending: {session_id}");
            }

            active.ending = true;
        }

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        if sessions.remove(session_id).is_none() {
            bail!("Active chat session not found: {session_id}");
        }

        drop(sessions);

        let mut active = session_handle
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        println!("Ending active chat session...");
        println!("Session ID: {}", active.session.id);

        let runtime_duration_ms = UtcNow::duration_since(active.session.started_at);
        let turn_count = active.turns.len();
        let turn_artifacts = build_turn_artifacts(&active.session, turn_count);

        let runtime_stopped = active.runtime.shutdown()?;

        for turn in &mut active.turns {
            turn.user.sanitize();
            turn.assistant.sanitize();
        }

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
            operation: "chat_history_buffer_zeroization".to_string(),
            status: "successful".to_string(),
            details: "Zeroized Rust-owned in-memory chat turn buffers at session end.".to_string(),
        });

        sanitization_operations.push(SanitizationOperation {
            operation: "chat_session_lifecycle_end".to_string(),
            status: "successful".to_string(),
            details: format!(
                "Chat session ended after {turn_count} turn(s). Runtime lifetime was scoped to this session."
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

        let profile = SessionProfile {
            session_kind: "active_chat".to_string(),
            runtime_lifetime: "session_scoped".to_string(),
            turn_count,
            runtime_duration_ms,
            history_policy: active_history_policy(active.config.ephemeral),
            persistence_policy: if active.config.ephemeral {
                "ephemeral_workspace_deleted_at_session_end".to_string()
            } else {
                "persistent_workspace_and_report_retained".to_string()
            },
            prompt_source: active.config.prompt_source.as_str().to_string(),
            turn_artifacts,
            active_runtime_residual_risk: active_runtime_risk(),
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
        )
        .with_session_profile(profile);

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
            session_id: active.session.id.clone(),
            runtime_stopped,
            report: parsed_report,
        })
    }

    fn session_handle(&self, session_id: &str) -> Result<Arc<Mutex<ActiveChatSession>>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        let Some(active) = sessions.get(session_id) else {
            bail!("Active chat session not found: {session_id}");
        };

        Ok(active.clone())
    }
}

struct UtcNow;

impl UtcNow {
    fn duration_since(started_at: chrono::DateTime<chrono::Utc>) -> i64 {
        let now = chrono::Utc::now();
        now.signed_duration_since(started_at).num_milliseconds()
    }
}

fn build_chat_prompt(
    template: ChatTemplate,
    turns: &[ChatTurn],
    current_user_prompt: &str,
) -> String {
    match template {
        ChatTemplate::Generic => build_generic_chat_prompt(turns, current_user_prompt),
        ChatTemplate::ChatMl => build_chatml_prompt(turns, current_user_prompt),
        ChatTemplate::Llama3Instruct => build_llama3_prompt(turns, current_user_prompt),
    }
}

fn build_generic_chat_prompt(turns: &[ChatTurn], current_user_prompt: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are a helpful local assistant.\n\n");

    for turn in turns {
        prompt.push_str("User: ");
        prompt.push_str(turn.user.as_str());
        prompt.push_str("\n\nAssistant: ");
        prompt.push_str(turn.assistant.as_str());
        prompt.push_str("\n\n");
    }

    prompt.push_str("User: ");
    prompt.push_str(current_user_prompt);
    prompt.push_str("\n\nAssistant: ");

    prompt
}

fn build_chatml_prompt(turns: &[ChatTurn], current_user_prompt: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("<|im_start|>system\nYou are a helpful local assistant.<|im_end|>\n");

    for turn in turns {
        prompt.push_str("<|im_start|>user\n");
        prompt.push_str(turn.user.as_str());
        prompt.push_str("<|im_end|>\n");
        prompt.push_str("<|im_start|>assistant\n");
        prompt.push_str(turn.assistant.as_str());
        prompt.push_str("<|im_end|>\n");
    }

    prompt.push_str("<|im_start|>user\n");
    prompt.push_str(current_user_prompt);
    prompt.push_str("<|im_end|>\n");
    prompt.push_str("<|im_start|>assistant\n");

    prompt
}

fn build_llama3_prompt(turns: &[ChatTurn], current_user_prompt: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("<|begin_of_text|>");
    prompt.push_str("<|start_header_id|>system<|end_header_id|>\n\n");
    prompt.push_str("You are a helpful local assistant.");
    prompt.push_str("<|eot_id|>");

    for turn in turns {
        prompt.push_str("<|start_header_id|>user<|end_header_id|>\n\n");
        prompt.push_str(turn.user.as_str());
        prompt.push_str("<|eot_id|>");
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        prompt.push_str(turn.assistant.as_str());
        prompt.push_str("<|eot_id|>");
    }

    prompt.push_str("<|start_header_id|>user<|end_header_id|>\n\n");
    prompt.push_str(current_user_prompt);
    prompt.push_str("<|eot_id|>");
    prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");

    prompt
}

fn stream_completion_from_llama<F>(
    completion_url: &str,
    prompt: &str,
    n_predict: u32,
    emit: &mut F,
) -> Result<(String, bool)>
where
    F: FnMut(ChatStreamEvent) -> bool,
{
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    let mut request = StreamingCompletionRequest {
        prompt: prompt.to_string(),
        n_predict,
        stream: true,
    };

    let response = client.post(completion_url).json(&request).send()?;

    request.sanitize();

    let reader = BufReader::new(response);
    let mut full_response = String::new();

    for line_result in reader.lines() {
        let line = line_result?;

        if !line.starts_with("data:") {
            continue;
        }

        let data = line.trim_start_matches("data:").trim();

        if data.is_empty() || data == "[DONE]" {
            continue;
        }

        let parsed: serde_json::Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(content) = parsed.get("content").and_then(|value| value.as_str()) {
            if !content.is_empty() {
                full_response.push_str(content);

                if !emit(model_event(content.to_string())) {
                    return Ok((full_response, false));
                }
            }
        }

        let stopped = parsed
            .get("stop")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        if stopped {
            break;
        }
    }

    Ok((full_response, true))
}

fn write_turn_prompt(session: &Session, turn_number: usize, prompt: &[u8]) -> Result<()> {
    fs::write(
        session
            .workspace
            .join(format!("turn-{turn_number:04}-prompt.txt")),
        prompt,
    )?;

    Ok(())
}

fn write_turn_response(session: &Session, turn_number: usize, response: &[u8]) -> Result<()> {
    fs::write(
        session
            .workspace
            .join(format!("turn-{turn_number:04}-response.txt")),
        response,
    )?;

    Ok(())
}

fn clear_generation_active(session_handle: &Arc<Mutex<ActiveChatSession>>) -> Result<()> {
    let mut active = session_handle
        .lock()
        .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

    active.generation_active = false;

    Ok(())
}

fn build_turn_artifacts(session: &Session, turn_count: usize) -> Vec<TurnArtifact> {
    (1..=turn_count)
        .map(|turn| TurnArtifact {
            turn,
            prompt_path: session
                .workspace
                .join(format!("turn-{turn:04}-prompt.txt"))
                .display()
                .to_string(),
            response_path: session
                .workspace
                .join(format!("turn-{turn:04}-response.txt"))
                .display()
                .to_string(),
        })
        .collect()
}

fn active_history_policy(ephemeral: bool) -> String {
    if ephemeral {
        "continuous_context_in_memory_only_until_end_sanitize".to_string()
    } else {
        "continuous_context_in_memory_and_retained_workspace_until_explicit_cleanup".to_string()
    }
}

fn active_runtime_risk() -> String {
    "During an active chat session, llama.cpp remains loaded, KV/cache state may remain live, and prompts/responses remain recoverable from process memory until session end and cleanup."
        .to_string()
}

fn runtime_event(message: impl Into<String>) -> ChatStreamEvent {
    ChatStreamEvent {
        event_type: "runtime".to_string(),
        message: Some(message.into()),
        text: None,
        operation: None,
        status: None,
        details: None,
        success: None,
    }
}

fn model_event(text: impl Into<String>) -> ChatStreamEvent {
    ChatStreamEvent {
        event_type: "model".to_string(),
        message: None,
        text: Some(text.into()),
        operation: None,
        status: None,
        details: None,
        success: None,
    }
}

fn audit_event(operation: SanitizationOperation) -> ChatStreamEvent {
    ChatStreamEvent {
        event_type: "audit".to_string(),
        message: None,
        text: None,
        operation: Some(operation.operation),
        status: Some(operation.status),
        details: Some(operation.details),
        success: None,
    }
}

fn complete_event(success: bool) -> ChatStreamEvent {
    ChatStreamEvent {
        event_type: "complete".to_string(),
        message: None,
        text: None,
        operation: None,
        status: None,
        details: None,
        success: Some(success),
    }
}

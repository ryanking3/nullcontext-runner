use crate::audit::{
    build_llama_runtime_report, PrivacyReport, RetrievalReport, SessionProfile, TurnArtifact,
};
use crate::cleanup::{
    cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation,
};
use crate::config::{ChatTemplate, SessionConfig};
use crate::corpus_registry::{validate_corpus_ready, CorpusRegistry};
use crate::llama_stream::{stream_completion_from_llama, StreamTermination};
use crate::logging::stdout_line;
use crate::process_scan::{
    build_process_scan_report, build_skipped_process_scan_report, scan_live_process_phase,
    scan_post_shutdown_process_phase, ProcessScanMarker,
};
use crate::registry::{
    register_active_persistent_session, register_persistent_session, unregister_persistent_session,
    SessionLifecycleMetadata,
};
use crate::retrieval::{
    build_active_chat_retrieval_report, build_grounded_prompt, build_retrieval_report,
    query_corpus, QueryCorpusRequest,
};
use crate::runtime::{
    observe_post_shutdown_with_stage_process_scan, ManagedRuntime, RuntimeProcessScanMarker,
};
use crate::sensitive::SensitiveBytes;
use crate::session::Session;
use crate::validation_harness::run_controlled_canary_validation;
use crate::validation_history::apply_and_record_memory_validation_history;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    bound_corpus_id: Option<String>,
    bound_corpus_name: Option<String>,
    retrieval_history: Vec<RetrievalReport>,
    generation_active: bool,
    cancel_requested: Arc<AtomicBool>,
    ending: bool,
}

#[derive(Debug)]
struct ChatTurn {
    user: SensitiveBytes,
    assistant: SensitiveBytes,
}

#[derive(Debug, Clone)]
struct ChatTurnSnapshot {
    user: String,
    assistant: String,
}

struct ActiveChatProcessScanMarker {
    kind: &'static str,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct ChatContextWindow {
    prompt: String,
    total_turns: usize,
    included_turns: usize,
    dropped_turns: usize,
    approx_prompt_tokens: usize,
    token_budget: usize,
    turn_limit: usize,
    truncated_by_turn_limit: bool,
    truncated_by_token_budget: bool,
    current_prompt_over_budget: bool,
}

impl ChatContextWindow {
    fn audit_operation(&self) -> SanitizationOperation {
        if self.dropped_turns == 0 && !self.current_prompt_over_budget {
            return SanitizationOperation {
                operation: "chat_context_window_prepared".to_string(),
                status: "recorded".to_string(),
                details: format!(
                    "Prepared active chat context with all {} prior turn(s) included (approx {} / {} tokens, turn limit {}).",
                    self.total_turns,
                    self.approx_prompt_tokens,
                    self.token_budget,
                    self.turn_limit
                ),
            };
        }

        let mut reasons = Vec::new();

        if self.truncated_by_turn_limit {
            reasons.push("turn limit");
        }

        if self.truncated_by_token_budget {
            reasons.push("approximate token budget");
        }

        if self.current_prompt_over_budget && !self.truncated_by_token_budget {
            reasons.push("approximate token budget");
        }

        let mut details = format!(
            "Prepared active chat context with {} of {} prior turn(s) included (approx {} / {} tokens, turn limit {}). Dropped {} oldest turn(s) due to {}.",
            self.included_turns,
            self.total_turns,
            self.approx_prompt_tokens,
            self.token_budget,
            self.turn_limit,
            self.dropped_turns,
            reasons.join(" and ")
        );

        if self.current_prompt_over_budget {
            details.push_str(
                " The current prompt plus template framing alone exceeded the configured approximate token budget, so no prior turns were included."
            );
        }

        SanitizationOperation {
            operation: "chat_context_window_truncated".to_string(),
            status: "warning".to_string(),
            details,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StartChatRequest {
    pub mode: Option<String>,
    pub persistent: Option<bool>,
    pub model_id: Option<String>,
    pub corpus_id: Option<String>,
    pub chat_template: Option<String>,
    pub chat_context_token_budget: Option<u32>,
    pub chat_context_turn_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessageRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize)]
pub struct StartChatResponse {
    pub session_id: String,
    pub workspace: String,
    pub runtime_endpoint: String,
    pub security_mode: String,
    pub persistent: bool,
    pub model_id: String,
    pub model_name: String,
    pub corpus_id: Option<String>,
    pub corpus_name: Option<String>,
    pub runtime_active: bool,
    pub turns: usize,
    pub grounded_turns: usize,
    pub chat_template: String,
    pub chat_context_token_budget: usize,
    pub chat_context_turn_limit: usize,
    pub history_policy: String,
}

#[derive(Debug, Serialize)]
pub struct ChatStatusResponse {
    pub session_id: String,
    pub workspace: String,
    pub runtime_endpoint: String,
    pub security_mode: String,
    pub persistent: bool,
    pub model_id: String,
    pub model_name: String,
    pub corpus_id: Option<String>,
    pub corpus_name: Option<String>,
    pub runtime_active: bool,
    pub turns: usize,
    pub grounded_turns: usize,
    pub runtime_duration_ms: i64,
    pub chat_template: String,
    pub chat_context_token_budget: usize,
    pub chat_context_turn_limit: usize,
    pub history_policy: String,
    pub residual_risk: String,
}

#[derive(Debug, Serialize)]
pub struct EndChatResponse {
    pub session_id: String,
    pub runtime_stopped: bool,
    pub report: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct CancelChatResponse {
    pub session_id: String,
    pub generation_active: bool,
    pub cancel_requested: bool,
    pub message: String,
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
        let bound_corpus = resolve_bound_corpus(&home, request.corpus_id.as_deref())?;

        let config = SessionConfig::from_web_request(
            home,
            String::new(),
            request.mode,
            persistent,
            request.model_id,
            request.chat_template,
            request.chat_context_token_budget,
            request.chat_context_turn_limit,
        )?;

        stdout_line("Starting active chat session...");
        stdout_line(format!("Security mode: {}", config.security_mode.as_str()));
        stdout_line(format!(
            "Model: {} ({})",
            config.model_name, config.model_id
        ));
        stdout_line(format!("Model path: {}", config.model_path));

        let mut runtime = ManagedRuntime::launch(&config)
            .context("Active chat startup failed before a live session could be created")?;
        let session = match Session::create() {
            Ok(session) => session,
            Err(error) => {
                let cleanup_summary = cleanup_failed_active_chat_start(&mut runtime, None);
                return Err(error.context(format!(
                    "Active chat startup failed while creating the session workspace. {cleanup_summary}"
                )));
            }
        };

        stdout_line(format!("Session ID: {}", session.id));
        stdout_line(format!("Workspace: {}", session.workspace.display()));

        if !config.ephemeral {
            if let Err(error) = register_active_persistent_session(&config.home, &session, &config)
            {
                let cleanup_summary =
                    cleanup_failed_active_chat_start(&mut runtime, Some(&session));
                return Err(error.context(format!(
                    "Persistent active chat session started but could not be registered for startup reconciliation. {cleanup_summary}"
                )));
            }
        }

        let response = StartChatResponse {
            session_id: session.id.clone(),
            workspace: session.workspace.display().to_string(),
            runtime_endpoint: runtime.endpoint_url().to_string(),
            security_mode: config.security_mode.as_str().to_string(),
            persistent: !config.ephemeral,
            model_id: config.model_id.clone(),
            model_name: config.model_name.clone(),
            corpus_id: bound_corpus.as_ref().map(|(id, _)| id.clone()),
            corpus_name: bound_corpus.as_ref().map(|(_, name)| name.clone()),
            runtime_active: true,
            turns: 0,
            grounded_turns: 0,
            chat_template: config.chat_template.as_str().to_string(),
            chat_context_token_budget: config.chat_context_token_budget,
            chat_context_turn_limit: config.chat_context_turn_limit,
            history_policy: active_history_policy(&config),
        };

        let mut active = ActiveChatSession {
            session,
            config,
            runtime,
            turns: vec![],
            bound_corpus_id: bound_corpus.as_ref().map(|(id, _)| id.clone()),
            bound_corpus_name: bound_corpus.as_ref().map(|(_, name)| name.clone()),
            retrieval_history: vec![],
            generation_active: false,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            ending: false,
        };

        let mut sessions = match self.sessions.lock() {
            Ok(sessions) => sessions,
            Err(_) => {
                let cleanup_summary =
                    cleanup_failed_active_chat_start(&mut active.runtime, Some(&active.session));
                let registry_summary = if !active.config.ephemeral {
                    match unregister_persistent_session(&active.config.home, &active.session.id) {
                        Ok(true) => {
                            "Rolled back the provisional persistent registry entry.".to_string()
                        }
                        Ok(false) => {
                            "No provisional persistent registry entry needed rollback.".to_string()
                        }
                        Err(error) => format!(
                            "NullContext also failed to roll back the provisional persistent registry entry: {error}."
                        ),
                    }
                } else {
                    "No persistent registry entry had been created.".to_string()
                };
                return Err(anyhow::anyhow!(
                    "Chat session lock poisoned before the active session could be published. {cleanup_summary} {registry_summary}"
                ));
            }
        };

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
            runtime_endpoint: active.runtime.endpoint_url().to_string(),
            security_mode: active.config.security_mode.as_str().to_string(),
            persistent: !active.config.ephemeral,
            model_id: active.config.model_id.clone(),
            model_name: active.config.model_name.clone(),
            corpus_id: active.bound_corpus_id.clone(),
            corpus_name: active.bound_corpus_name.clone(),
            runtime_active: !active.ending,
            turns: active.turns.len(),
            grounded_turns: active.retrieval_history.len(),
            runtime_duration_ms,
            chat_template: active.config.chat_template.as_str().to_string(),
            chat_context_token_budget: active.config.chat_context_token_budget,
            chat_context_turn_limit: active.config.chat_context_turn_limit,
            history_policy: active_history_policy(&active.config),
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

        let (
            turn_number,
            completion_url,
            max_tokens,
            cancel_requested,
            home,
            template,
            token_budget,
            turn_limit,
            prior_turns,
            bound_corpus_id,
            bound_corpus_name,
        ) = {
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
            let prior_turns = active
                .turns
                .iter()
                .map(|turn| ChatTurnSnapshot {
                    user: turn.user.as_str().to_string(),
                    assistant: turn.assistant.as_str().to_string(),
                })
                .collect::<Vec<_>>();

            write_turn_prompt(&active.session, turn_number, user_buffer.as_bytes())?;

            active.generation_active = true;
            active.cancel_requested.store(false, Ordering::SeqCst);

            (
                turn_number,
                completion_url,
                max_tokens,
                active.cancel_requested.clone(),
                active.config.home.clone(),
                active.config.chat_template,
                active.config.chat_context_token_budget,
                active.config.chat_context_turn_limit,
                prior_turns,
                active.bound_corpus_id.clone(),
                active.bound_corpus_name.clone(),
            )
        };

        let mut retrieval_report = None;
        let prompt_for_model = if let Some(corpus_id) = bound_corpus_id.as_deref() {
            if !emit(runtime_event(format!(
                "Retrieving local corpus context for active chat turn {turn_number}..."
            ))) {
                clear_generation_active(&session_handle)?;
                return Ok(());
            }

            let retrieval = match query_corpus(
                &home,
                corpus_id,
                QueryCorpusRequest {
                    query: user_buffer.as_str().to_string(),
                    top_k: None,
                },
            ) {
                Ok(retrieval) => retrieval,
                Err(error) => {
                    clear_generation_active(&session_handle)?;
                    return Err(error);
                }
            };
            let retrieval_corpus_name = bound_corpus_name
                .clone()
                .unwrap_or_else(|| retrieval.corpus_name.clone());

            retrieval_report = Some(build_retrieval_report(&retrieval));

            let _ = emit(audit_event(SanitizationOperation {
                operation: "chat_turn_corpus_context_injected".to_string(),
                status: "recorded".to_string(),
                details: format!(
                    "Injected retrieval context from corpus '{}' ({}) for chat turn {} using {} chunk(s) across {} source file(s).",
                    retrieval_corpus_name,
                    retrieval.corpus_id,
                    turn_number,
                    retrieval.results.len(),
                    retrieval_report
                        .as_ref()
                        .map(|report| report.source_paths.len())
                        .unwrap_or(0)
                ),
            }));

            build_grounded_prompt(&retrieval)
        } else {
            user_buffer.as_str().to_string()
        };

        let mut context_window = prepare_chat_context(
            template,
            &prior_turns,
            &prompt_for_model,
            token_budget,
            turn_limit,
        );

        if !emit(runtime_event(format!(
            "Running chat turn {turn_number} on active runtime..."
        ))) {
            context_window.prompt.zeroize();
            clear_generation_active(&session_handle)?;
            return Ok(());
        }

        let _ = emit(audit_event(context_window.audit_operation()));

        if !emit(runtime_event("--- Model Output ---")) {
            context_window.prompt.zeroize();
            clear_generation_active(&session_handle)?;
            return Ok(());
        }

        let stream_result = stream_completion_from_llama(
            &completion_url,
            &context_window.prompt,
            max_tokens,
            || cancel_requested.load(Ordering::SeqCst),
            |text| emit(model_event(text.to_string())),
        );

        context_window.prompt.zeroize();

        let (response_text, termination) = match stream_result {
            Ok(result) => result,
            Err(error) => {
                clear_generation_active(&session_handle)?;
                return Err(error);
            }
        };

        if termination != StreamTermination::Completed {
            clear_generation_active(&session_handle)?;

            let (operation, details) = match termination {
                StreamTermination::CancelRequested => (
                    "chat_turn_cancelled",
                    format!(
                        "Cancelled chat turn {turn_number} after an explicit cancel request. Partial response was not committed to chat history."
                    ),
                ),
                StreamTermination::StreamClosed => (
                    "chat_turn_stream_closed",
                    format!(
                        "Stopped chat turn {turn_number} after the client stream closed. Partial response was not committed to chat history."
                    ),
                ),
                StreamTermination::Completed => unreachable!(),
            };

            let _ = emit(audit_event(SanitizationOperation {
                operation: operation.to_string(),
                status: "warning".to_string(),
                details,
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
            if let Some(report) = retrieval_report {
                active.retrieval_history.push(report);
            }

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

    pub fn cancel_generation(&self, session_id: &str) -> Result<CancelChatResponse> {
        let session_handle = self.session_handle(session_id)?;
        let active = session_handle
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat session lock poisoned"))?;

        if active.ending {
            bail!("Active chat session is ending: {session_id}");
        }

        if !active.generation_active {
            bail!("No active chat generation is currently running for session {session_id}");
        }

        let already_requested = active.cancel_requested.swap(true, Ordering::SeqCst);

        Ok(CancelChatResponse {
            session_id: active.session.id.clone(),
            generation_active: true,
            cancel_requested: true,
            message: if already_requested {
                "Cancellation was already requested for the current active chat generation."
                    .to_string()
            } else {
                "Cancellation requested for the current active chat generation.".to_string()
            },
        })
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

        stdout_line("Ending active chat session...");
        stdout_line(format!("Session ID: {}", active.session.id));

        let runtime_duration_ms = UtcNow::duration_since(active.session.started_at);
        let turn_count = active.turns.len();
        let turn_artifacts = build_turn_artifacts(&active.session, turn_count);
        let grounded_turn_count = active.retrieval_history.len();
        let runtime_pid = active.runtime.pid();
        let runtime_endpoint = active.runtime.endpoint_url().to_string();
        let process_scan_markers = active_chat_process_scan_markers(&active.turns);
        let borrowed_live_markers = borrow_active_chat_process_scan_markers(&process_scan_markers);
        let runtime_usage = active.runtime.observe_usage();
        let live_process_scan = if process_scan_markers.is_empty() {
            None
        } else {
            Some(scan_live_process_phase(runtime_pid, &borrowed_live_markers))
        };

        let runtime_shutdown = active.runtime.shutdown()?;
        let stage_process_scan_markers = process_scan_markers
            .iter()
            .map(|marker| RuntimeProcessScanMarker {
                kind: marker.kind.to_string(),
                bytes: marker.bytes.clone(),
            })
            .collect::<Vec<_>>();
        let post_shutdown_observation = observe_post_shutdown_with_stage_process_scan(
            runtime_pid,
            active.config.gpu_layers.parse::<u32>().unwrap_or(0) > 0,
            Some(&active.config),
            &stage_process_scan_markers,
        );
        let process_scan_report = if process_scan_markers.is_empty() {
            build_skipped_process_scan_report(
                Some(runtime_pid),
                "This active chat session ended without any completed turns, so NullContext did not have representative chat-content markers to search for in process memory.",
                "Without completed turn content to use as representative markers, NullContext cannot say whether chat-related content remained present in readable llama-server process memory.",
                vec![
                    "Direct process scanning is wired for active chat end, but this specific session had no completed turns to sample."
                        .to_string(),
                ],
            )
        } else {
            let borrowed_post_shutdown_markers =
                borrow_active_chat_process_scan_markers(&process_scan_markers);
            build_process_scan_report(
                Some(runtime_pid),
                vec![
                    live_process_scan.expect("active chat markers should exist when scan runs"),
                    scan_post_shutdown_process_phase(
                        runtime_pid,
                        &post_shutdown_observation,
                        &borrowed_post_shutdown_markers,
                    ),
                ],
            )
        };

        for turn in &mut active.turns {
            turn.user.sanitize();
            turn.assistant.sanitize();
        }

        let (artifacts_detected, scan_operation) = scan_artifacts(&active.session.workspace)?;

        let mut sanitization_operations = vec![scan_operation];

        sanitization_operations.push(SanitizationOperation {
            operation: "managed_chat_runtime_shutdown".to_string(),
            status: if runtime_shutdown.stopped {
                "successful".to_string()
            } else {
                "failed".to_string()
            },
            details: format!(
                "Long-lived llama-server chat runtime shutdown completed using method {}. Exit code: {}.",
                runtime_shutdown.shutdown_method,
                runtime_shutdown
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
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
            history_policy: active_history_policy(&active.config),
            persistence_policy: if active.config.ephemeral {
                "ephemeral_workspace_deleted_at_session_end".to_string()
            } else {
                "persistent_workspace_and_report_retained".to_string()
            },
            prompt_source: active.config.prompt_source.as_str().to_string(),
            turn_artifacts,
            active_runtime_residual_risk: active_runtime_risk(),
            grounding_scope: active
                .bound_corpus_id
                .as_ref()
                .map(|_| "corpus_bound_retrieval_per_turn_until_end_sanitize".to_string()),
            bound_corpus_id: active.bound_corpus_id.clone(),
            bound_corpus_name: active.bound_corpus_name.clone(),
            grounded_turn_count,
        };

        let lifecycle =
            SessionLifecycleMetadata::for_completed_session(&active.config, &cleanup_report);

        let report = PrivacyReport::new(
            active.session.id.clone(),
            active.session.started_at,
            !active.config.ephemeral,
            "llama-server".to_string(),
            active.config.security_mode.as_str().to_string(),
            active.config.gpu_layers.clone(),
            runtime_shutdown.stopped,
            cleanup_report.clone(),
        )
        .with_lifecycle(&lifecycle)
        .with_session_profile(profile)
        .with_process_scan({
            let mut report = process_scan_report;
            if turn_count > 0 {
                report.notes.push(
                    "Active chat direct scanning currently samples earliest and latest completed turn buffers rather than every turn in the full conversation history."
                        .to_string(),
                );
            }
            report
        })
        .with_llama_runtime(build_llama_runtime_report(
            &active.config,
            Some(runtime_pid),
            Some(&runtime_endpoint),
            &runtime_shutdown,
            &runtime_usage,
            &post_shutdown_observation,
        ))
        .with_controlled_canary_run(run_controlled_canary_validation(&active.config));

        let report = if let (Some(corpus_id), Some(corpus_name)) = (
            active.bound_corpus_id.as_deref(),
            active.bound_corpus_name.as_deref(),
        ) {
            if let Some(retrieval_report) = build_active_chat_retrieval_report(
                corpus_id,
                corpus_name,
                &active.retrieval_history,
            ) {
                report.with_retrieval(retrieval_report)
            } else {
                report
            }
        } else {
            report
        };
        let report = apply_and_record_memory_validation_history(&active.config.home, report);

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
            runtime_stopped: runtime_shutdown.stopped,
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

fn active_chat_process_scan_markers(turns: &[ChatTurn]) -> Vec<ActiveChatProcessScanMarker> {
    let Some(first_turn) = turns.first() else {
        return vec![];
    };

    let mut markers = vec![
        ActiveChatProcessScanMarker {
            kind: "earliest_user_turn_marker",
            bytes: first_turn.user.as_bytes().to_vec(),
        },
        ActiveChatProcessScanMarker {
            kind: "earliest_assistant_turn_marker",
            bytes: first_turn.assistant.as_bytes().to_vec(),
        },
    ];

    if let Some(last_turn) = turns.last() {
        if !std::ptr::eq(first_turn, last_turn) {
            markers.push(ActiveChatProcessScanMarker {
                kind: "latest_user_turn_marker",
                bytes: last_turn.user.as_bytes().to_vec(),
            });
            markers.push(ActiveChatProcessScanMarker {
                kind: "latest_assistant_turn_marker",
                bytes: last_turn.assistant.as_bytes().to_vec(),
            });
        }
    }

    markers
}

fn borrow_active_chat_process_scan_markers(
    markers: &[ActiveChatProcessScanMarker],
) -> Vec<ProcessScanMarker<'_>> {
    markers
        .iter()
        .map(|marker| ProcessScanMarker {
            kind: marker.kind,
            bytes: &marker.bytes,
        })
        .collect()
}

fn prepare_chat_context(
    template: ChatTemplate,
    turns: &[ChatTurnSnapshot],
    current_user_prompt: &str,
    token_budget: usize,
    turn_limit: usize,
) -> ChatContextWindow {
    let prefix = prompt_prefix(template);
    let suffix = prompt_suffix(template, current_user_prompt);
    let total_turns = turns.len();
    let turn_limit = turns.len().min(turn_limit);
    let char_budget = token_budget.saturating_mul(4);

    let mut used_chars = text_char_count(&prefix) + text_char_count(&suffix);
    let current_prompt_over_budget = used_chars > char_budget;
    let mut rendered_turns_rev = Vec::new();
    let mut truncated_by_token_budget = current_prompt_over_budget && total_turns > 0;

    for turn in turns.iter().rev().take(turn_limit) {
        let rendered_turn = render_chat_turn_snapshot(template, turn);
        let turn_chars = text_char_count(&rendered_turn);

        if used_chars + turn_chars > char_budget {
            truncated_by_token_budget = true;
            break;
        }

        rendered_turns_rev.push(rendered_turn);
        used_chars += turn_chars;
    }

    let included_turns = rendered_turns_rev.len();
    let dropped_turns = total_turns.saturating_sub(included_turns);
    let truncated_by_turn_limit = total_turns > turn_limit;

    let mut prompt = prefix;

    for rendered_turn in rendered_turns_rev.iter().rev() {
        prompt.push_str(rendered_turn);
    }

    prompt.push_str(&suffix);

    ChatContextWindow {
        approx_prompt_tokens: approximate_token_count(&prompt),
        prompt,
        total_turns,
        included_turns,
        dropped_turns,
        token_budget,
        turn_limit,
        truncated_by_turn_limit,
        truncated_by_token_budget,
        current_prompt_over_budget,
    }
}

fn prompt_prefix(template: ChatTemplate) -> String {
    match template {
        ChatTemplate::Generic => "You are a helpful local assistant.\n\n".to_string(),
        ChatTemplate::ChatMl => {
            "<|im_start|>system\nYou are a helpful local assistant.<|im_end|>\n".to_string()
        }
        ChatTemplate::Llama3Instruct => {
            let mut prompt = String::new();
            prompt.push_str("<|begin_of_text|>");
            prompt.push_str("<|start_header_id|>system<|end_header_id|>\n\n");
            prompt.push_str("You are a helpful local assistant.");
            prompt.push_str("<|eot_id|>");
            prompt
        }
    }
}

fn prompt_suffix(template: ChatTemplate, current_user_prompt: &str) -> String {
    match template {
        ChatTemplate::Generic => format!("User: {current_user_prompt}\n\nAssistant: "),
        ChatTemplate::ChatMl => {
            format!("<|im_start|>user\n{current_user_prompt}<|im_end|>\n<|im_start|>assistant\n")
        }
        ChatTemplate::Llama3Instruct => format!(
            "<|start_header_id|>user<|end_header_id|>\n\n{current_user_prompt}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n"
        ),
    }
}

fn render_chat_turn_snapshot(template: ChatTemplate, turn: &ChatTurnSnapshot) -> String {
    match template {
        ChatTemplate::Generic => {
            format!("User: {}\n\nAssistant: {}\n\n", turn.user, turn.assistant)
        }
        ChatTemplate::ChatMl => format!(
            "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{}<|im_end|>\n",
            turn.user,
            turn.assistant
        ),
        ChatTemplate::Llama3Instruct => format!(
            "<|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n{}<|eot_id|>",
            turn.user,
            turn.assistant
        ),
    }
}

fn resolve_bound_corpus(home: &str, corpus_id: Option<&str>) -> Result<Option<(String, String)>> {
    let Some(corpus_id) = corpus_id else {
        return Ok(None);
    };

    let registry = CorpusRegistry::load(home)?;
    let corpus = registry
        .find(corpus_id)
        .ok_or_else(|| anyhow::anyhow!("Corpus not found in registry: {corpus_id}"))?;
    validate_corpus_ready(corpus)?;

    Ok(Some((corpus.corpus_id.clone(), corpus.name.clone())))
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
    active.cancel_requested.store(false, Ordering::SeqCst);

    Ok(())
}

fn text_char_count(text: &str) -> usize {
    text.chars().count()
}

fn approximate_token_count(text: &str) -> usize {
    let chars = text_char_count(text);

    if chars == 0 {
        0
    } else {
        chars.div_ceil(4)
    }
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

fn cleanup_failed_active_chat_start(
    runtime: &mut ManagedRuntime,
    session: Option<&Session>,
) -> String {
    let runtime_summary = match runtime.shutdown() {
        Ok(outcome) => format!(
            "NullContext shut down the startup runtime using {} (exit code {}).",
            outcome.shutdown_method,
            outcome
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        Err(error) => format!(
            "NullContext also failed to shut down the startup runtime automatically: {error}."
        ),
    };

    let workspace_summary = match session {
        Some(session) => match scan_artifacts(&session.workspace) {
            Ok((artifacts_detected, scan_operation)) => {
                let cleanup = cleanup_ephemeral_workspace(
                    &session.workspace,
                    artifacts_detected,
                    vec![
                        scan_operation,
                        SanitizationOperation {
                            operation: "active_chat_startup_failure_cleanup".to_string(),
                            status: "successful".to_string(),
                            details:
                                "NullContext cleaned the workspace created for an active chat session that never became live."
                                    .to_string(),
                        },
                    ],
                );

                if cleanup.workspace_deleted {
                    format!(
                        "Temporary startup workspace {} was removed.",
                        session.workspace.display()
                    )
                } else if let Some(error) = cleanup.error {
                    format!(
                        "Temporary startup workspace {} could not be fully removed: {}",
                        session.workspace.display(),
                        error
                    )
                } else {
                    format!(
                        "Temporary startup workspace {} could not be confirmed deleted.",
                        session.workspace.display()
                    )
                }
            }
            Err(error) => format!(
                "NullContext could not inspect or clean the temporary startup workspace {}: {}",
                session.workspace.display(),
                error
            ),
        },
        None => "No active-chat workspace had been created yet.".to_string(),
    };

    let summary = format!("{runtime_summary} {workspace_summary}");
    stdout_line(&summary);
    summary
}

fn active_history_policy(config: &SessionConfig) -> String {
    let bounds = format!(
        "turn_limit={}, approx_token_budget={}",
        config.chat_context_turn_limit, config.chat_context_token_budget
    );

    if config.ephemeral {
        format!("bounded_recent_context_in_memory_only_until_end_sanitize ({bounds})")
    } else {
        format!(
            "bounded_recent_context_in_memory_and_retained_workspace_until_explicit_cleanup ({bounds})"
        )
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

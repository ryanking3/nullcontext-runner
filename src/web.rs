use crate::audit::{sync_report_lifecycle, PrivacyReport};
use crate::chat::{CancelChatResponse, ChatMessageRequest, ChatSessionManager, StartChatRequest};
use crate::cleanup::{
    cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation,
};
use crate::config::SessionConfig;
use crate::llama_stream::{stream_completion_from_llama, StreamTermination};
use crate::memory_scan::{buffer_contains_pattern, verify_buffer_zeroization};
use crate::registry::{
    archived_report_path, due_retention_cleanup_session_ids, ensure_registry_dirs,
    reconcile_registry_on_startup, register_persistent_session, CleanupReason, RetentionPolicy,
    SessionIndexEntry, SessionLifecycleMetadata, SessionRegistry,
};
use crate::runtime::ManagedRuntime;
use crate::sensitive::SensitiveBytes;
use crate::session::Session;
use anyhow::Result;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path as FsPath;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

const RETENTION_SWEEP_INTERVAL_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
struct WebState {
    home: Arc<String>,
    chat_manager: ChatSessionManager,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct SessionLifecycleActionResponse {
    session_id: String,
    lifecycle_state: String,
    retention_policy: String,
    retention_deadline: Option<String>,
    cleanup_reason: Option<String>,
    cleanup_attempted: bool,
    cleanup_successful: bool,
    workspace_deleted: bool,
    workspace_exists: bool,
    report_exists: bool,
    workspace: String,
    report_path: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct UpdateRetentionPolicyRequest {
    retention_policy: String,
    retain_for_minutes: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    prompt: String,
    mode: Option<String>,
    persistent: Option<bool>,
    chat_template: Option<String>,
    chat_context_token_budget: Option<u32>,
    chat_context_turn_limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    success: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, Serialize)]
struct StreamPayload {
    #[serde(rename = "type")]
    event_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    success: Option<bool>,
}

pub async fn serve() -> Result<()> {
    let home = home_dir()?;
    emit_startup_reconciliation(&home)?;

    let state = WebState {
        home: Arc::new(home),
        chat_manager: ChatSessionManager::new(),
    };

    spawn_retention_scheduler(state.home.clone());

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/run", post(run_session))
        .route("/api/run/stream", post(run_session_stream))
        .route("/api/chat/start", post(start_chat_session))
        .route("/api/chat/:session_id/status", get(chat_session_status))
        .route("/api/chat/:session_id/cancel", post(cancel_chat_generation))
        .route("/api/chat/:session_id/end", post(end_chat_session))
        .route(
            "/api/chat/:session_id/message/stream",
            post(stream_chat_message),
        )
        .route("/api/sessions", get(list_sessions))
        .route(
            "/api/sessions/:session_id/retention",
            post(update_retention_policy),
        )
        .route("/api/sessions/:session_id/cleanup", post(cleanup_session))
        .route(
            "/api/sessions/:session_id/reconcile",
            post(reconcile_session),
        )
        .route("/api/reports/:session_id", get(show_report))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3333));

    println!("NullContext web server listening on http://{addr}");
    println!("Health: http://{addr}/api/health");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

fn spawn_retention_scheduler(home: Arc<String>) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(RETENTION_SWEEP_INTERVAL_SECONDS));

        loop {
            interval.tick().await;

            let home = home.clone();

            let result = tokio::task::spawn_blocking(move || run_retention_sweep(&home)).await;

            match result {
                Ok(Ok(swept)) if !swept.is_empty() => {
                    println!(
                        "Retention sweep cleaned {} session(s): {}",
                        swept.len(),
                        swept.join(", ")
                    );
                }
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    println!("Retention sweep error: {error}");
                }
                Err(error) => {
                    println!("Retention sweep task failed: {error}");
                }
            }
        }
    });
}

fn emit_startup_reconciliation(home: &str) -> Result<()> {
    let summary = reconcile_registry_on_startup(home)?;
    sync_registry_report_lifecycle(home)?;

    println!(
        "Lifecycle reconciliation: scanned {} session(s), changed {}, orphaned {}, cleanup-consistent {}, unchanged {}.",
        summary.scanned_sessions,
        summary.changed_sessions,
        summary.orphaned_sessions,
        summary.cleanup_succeeded_consistent,
        summary.unchanged_sessions
    );

    for note in summary.notes.iter().take(8) {
        println!("  [lifecycle] {note}");
    }

    if summary.notes.len() > 8 {
        println!(
            "  [lifecycle] ... and {} more session note(s)",
            summary.notes.len() - 8
        );
    }

    Ok(())
}

fn sync_registry_report_lifecycle(home: &str) -> Result<()> {
    let registry = SessionRegistry::load(home)?;

    for entry in registry.sessions {
        sync_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;
    }

    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "nullcontext".to_string(),
    })
}

async fn start_chat_session(
    State(state): State<WebState>,
    Json(request): Json<StartChatRequest>,
) -> Response {
    let manager = state.chat_manager.clone();
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || manager.start_session(home, request)).await {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn chat_session_status(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Response {
    let manager = state.chat_manager.clone();

    match tokio::task::spawn_blocking(move || manager.status(&session_id)).await {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::NOT_FOUND, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn end_chat_session(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Response {
    let manager = state.chat_manager.clone();

    match tokio::task::spawn_blocking(move || manager.end_session(&session_id)).await {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::NOT_FOUND, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn cancel_chat_generation(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Response {
    let manager = state.chat_manager.clone();

    match tokio::task::spawn_blocking(move || manager.cancel_generation(&session_id)).await {
        Ok(Ok(response)) => Json::<CancelChatResponse>(response).into_response(),
        Ok(Err(error)) => {
            let message = error.to_string();
            let status = if message.contains("No active chat generation is currently running") {
                StatusCode::CONFLICT
            } else if message.contains("Active chat session not found") {
                StatusCode::NOT_FOUND
            } else if message.contains("session is ending") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            json_error(status, message)
        }
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn stream_chat_message(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
    Json(request): Json<ChatMessageRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<StreamPayload>(512);
    let manager = state.chat_manager.clone();

    std::thread::spawn(move || {
        let result = manager.stream_message(&session_id, request, |event| {
            let payload = StreamPayload {
                event_type: event.event_type,
                message: event.message,
                text: event.text,
                operation: event.operation,
                status: event.status,
                details: event.details,
                success: event.success,
            };

            tx.blocking_send(payload).is_ok()
        });

        if let Err(error) = result {
            let _ = tx.blocking_send(StreamPayload {
                event_type: "error".to_string(),
                message: Some(error.to_string()),
                text: None,
                operation: None,
                status: None,
                details: None,
                success: None,
            });

            let _ = tx.blocking_send(StreamPayload {
                event_type: "complete".to_string(),
                message: None,
                text: None,
                operation: None,
                status: None,
                details: None,
                success: Some(false),
            });
        }
    });

    let stream = ReceiverStream::new(rx).map(|payload| {
        let json = serde_json::to_string(&payload).unwrap_or_else(|error| {
            serde_json::json!({
                "type": "error",
                "message": format!("failed to serialize stream payload: {error}")
            })
            .to_string()
        });

        Ok(Event::default().data(json))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn run_session(Json(request): Json<RunRequest>) -> Response {
    match run_cli_session(request) {
        Ok(response) => Json(response).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn run_session_stream(
    State(state): State<WebState>,
    Json(request): Json<RunRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<StreamPayload>(512);
    let home = state.home.as_ref().clone();

    std::thread::spawn(move || {
        if let Err(error) = run_direct_streaming_session(home, request, tx.clone()) {
            let _ = send_payload(
                &tx,
                StreamPayload {
                    event_type: "error".to_string(),
                    message: Some(error.to_string()),
                    text: None,
                    operation: None,
                    status: None,
                    details: None,
                    success: None,
                },
            );

            let _ = send_complete(&tx, false);
        }
    });

    let stream = ReceiverStream::new(rx).map(|payload| {
        let json = serde_json::to_string(&payload).unwrap_or_else(|error| {
            serde_json::json!({
                "type": "error",
                "message": format!("failed to serialize stream payload: {error}")
            })
            .to_string()
        });

        Ok(Event::default().data(json))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn run_direct_streaming_session(
    home: String,
    request: RunRequest,
    tx: mpsc::Sender<StreamPayload>,
) -> Result<()> {
    let persistent = request.persistent.unwrap_or(false);

    let mut config = SessionConfig::from_web_request(
        home,
        request.prompt,
        request.mode,
        persistent,
        request.chat_template,
        request.chat_context_token_budget,
        request.chat_context_turn_limit,
    )?;

    let session = Session::create()?;

    let _ = send_runtime(&tx, "Starting NullContext session...");
    let _ = send_runtime(&tx, &format!("Session ID: {}", session.id));
    let _ = send_runtime(&tx, &format!("Workspace: {}", session.workspace.display()));
    let _ = send_runtime(
        &tx,
        &format!("Security mode: {}", config.security_mode.as_str()),
    );
    let _ = send_runtime(
        &tx,
        &format!("Prompt source: {}", config.prompt_source.as_str()),
    );

    session.write_prompt(config.prompt.as_bytes())?;

    let prompt_probe = config.prompt.as_bytes().to_vec();

    let prompt_found_before = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);

    let _ = send_runtime(&tx, "Launching llama-server...");

    let mut runtime = ManagedRuntime::launch(&config)?;

    let _ = send_runtime(&tx, "Runtime healthy.");
    let _ = send_runtime(&tx, "Running streaming inference...");
    let _ = send_runtime(&tx, "--- Model Output ---");

    let (response_text, termination) = stream_completion_from_llama(
        &runtime.completion_url(),
        config.prompt.as_str(),
        config.max_tokens.parse::<u32>()?,
        || false,
        |text| send_model_text(&tx, text),
    )?;

    let generation_completed = termination == StreamTermination::Completed;

    let runtime_terminated = runtime.shutdown()?;

    if !generation_completed {
        let _ = send_audit(
            &tx,
            &SanitizationOperation {
                operation: "one_shot_generation_cancelled".to_string(),
                status: "warning".to_string(),
                details:
                    "One-shot generation was cancelled by the client. Runtime was shut down and cleanup continued."
                        .to_string(),
            },
        );
    }

    let mut response_buffer = SensitiveBytes::new(response_text);

    session.write_response(response_buffer.as_bytes())?;

    let response_probe = response_buffer.as_bytes().to_vec();

    let response_found_before =
        buffer_contains_pattern(response_buffer.as_bytes(), &response_probe);

    let (artifacts_detected, scan_operation) = scan_artifacts(&session.workspace)?;

    let mut sanitization_operations = Vec::new();

    emit_and_push(&tx, &mut sanitization_operations, scan_operation);

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "sensitive_bytes_prompt_storage".to_string(),
            status: "successful".to_string(),
            details:
                "Application-owned prompt is stored in a zeroizing byte buffer instead of a long-lived String."
                    .to_string(),
        },
    );

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "http_stream_prompt_buffer_zeroization".to_string(),
            status: "successful".to_string(),
            details:
                "Explicitly zeroized temporary Rust-owned prompt copy used for llama-server streaming request."
                    .to_string(),
        },
    );

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "managed_runtime_shutdown".to_string(),
            status: if runtime_terminated {
                "successful".to_string()
            } else {
                "failed".to_string()
            },
            details: "llama-server child process was terminated after inference.".to_string(),
        },
    );

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "prompt_ingest_channel".to_string(),
            status: "recorded".to_string(),
            details: format!(
                "Prompt was provided via '{}'. Browser prompts avoid shell history and process argv exposure.",
                config.prompt_source.as_str()
            ),
        },
    );

    let _ = send_runtime(&tx, "Sanitizing Rust-owned buffers...");

    config.prompt.sanitize();
    response_buffer.sanitize();

    let prompt_found_after = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);
    let response_found_after = buffer_contains_pattern(response_buffer.as_bytes(), &response_probe);

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        verify_buffer_zeroization("prompt_buffer", prompt_found_before, prompt_found_after),
    );

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        verify_buffer_zeroization(
            "response_buffer",
            response_found_before,
            response_found_after,
        ),
    );

    emit_and_push(
        &tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "explicit_sensitive_byte_buffer_zeroization".to_string(),
            status: "successful".to_string(),
            details:
                "Explicitly overwrote Rust-owned prompt and response byte buffers before drop."
                    .to_string(),
        },
    );

    let cleanup_report = if config.ephemeral {
        let _ = send_runtime(&tx, "Session mode: ephemeral");
        let _ = send_runtime(
            &tx,
            &format!("Detected {} workspace artifacts.", artifacts_detected.len()),
        );
        let _ = send_runtime(&tx, "Cleaning up workspace...");

        cleanup_ephemeral_workspace(
            &session.workspace,
            artifacts_detected,
            sanitization_operations,
        )
    } else {
        let _ = send_runtime(&tx, "Session mode: persistent");
        let _ = send_runtime(
            &tx,
            &format!("Detected {} workspace artifacts.", artifacts_detected.len()),
        );
        let _ = send_runtime(&tx, "Workspace retained at:");
        let _ = send_runtime(&tx, &session.workspace.display().to_string());

        CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
    };

    for operation in &cleanup_report.sanitization_operations {
        if operation.operation == "workspace_recursive_delete"
            || operation.operation == "post_cleanup_workspace_verification"
            || operation.operation == "workspace_retention_policy"
        {
            let _ = send_audit(&tx, operation);
        }
    }

    let lifecycle = SessionLifecycleMetadata::for_completed_session(&config, &cleanup_report);

    let report = PrivacyReport::new(
        session.id.clone(),
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        config.gpu_layers.clone(),
        runtime_terminated,
        cleanup_report.clone(),
    )
    .with_lifecycle(&lifecycle);

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;
        register_persistent_session(&config.home, &session, &config, &cleanup_report)?;
    }

    let _ = send_runtime(&tx, "--- Privacy Report v0 ---");
    let _ = send_report_text(&tx, &report_json);

    let _ = send_complete(&tx, generation_completed);

    Ok(())
}

fn emit_and_push(
    tx: &mpsc::Sender<StreamPayload>,
    operations: &mut Vec<SanitizationOperation>,
    operation: SanitizationOperation,
) {
    let _ = send_audit(tx, &operation);
    operations.push(operation);
}

fn run_cli_session(request: RunRequest) -> Result<RunResponse> {
    let exe_path = std::env::current_exe()?;

    let mode = request.mode.unwrap_or_else(|| "secure".to_string());
    let persistent = request.persistent.unwrap_or(false);

    let mut command = Command::new(exe_path);

    command
        .arg("--mode")
        .arg(mode)
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if persistent {
        command.arg("--persistent");
    }

    let mut child = command.spawn()?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Failed to open child stdin"))?;

        stdin.write_all(request.prompt.as_bytes())?;
    }

    drop(child.stdin.take());

    let output = child.wait_with_output()?;

    Ok(RunResponse {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn send_runtime(tx: &mpsc::Sender<StreamPayload>, message: &str) -> bool {
    send_payload(
        tx,
        StreamPayload {
            event_type: "runtime".to_string(),
            message: Some(message.to_string()),
            text: None,
            operation: None,
            status: None,
            details: None,
            success: None,
        },
    )
}

fn send_model_text(tx: &mpsc::Sender<StreamPayload>, text: &str) -> bool {
    send_payload(
        tx,
        StreamPayload {
            event_type: "model".to_string(),
            message: None,
            text: Some(text.to_string()),
            operation: None,
            status: None,
            details: None,
            success: None,
        },
    )
}

fn send_report_text(tx: &mpsc::Sender<StreamPayload>, text: &str) -> bool {
    send_payload(
        tx,
        StreamPayload {
            event_type: "report".to_string(),
            message: None,
            text: Some(text.to_string()),
            operation: None,
            status: None,
            details: None,
            success: None,
        },
    )
}

fn send_audit(tx: &mpsc::Sender<StreamPayload>, operation: &SanitizationOperation) -> bool {
    send_payload(
        tx,
        StreamPayload {
            event_type: "audit".to_string(),
            message: None,
            text: None,
            operation: Some(operation.operation.clone()),
            status: Some(operation.status.clone()),
            details: Some(operation.details.clone()),
            success: None,
        },
    )
}

fn send_complete(tx: &mpsc::Sender<StreamPayload>, success: bool) -> bool {
    send_payload(
        tx,
        StreamPayload {
            event_type: "complete".to_string(),
            message: None,
            text: None,
            operation: None,
            status: None,
            details: None,
            success: Some(success),
        },
    )
}

fn send_payload(tx: &mpsc::Sender<StreamPayload>, payload: StreamPayload) -> bool {
    tx.blocking_send(payload).is_ok()
}

async fn list_sessions(State(state): State<WebState>) -> Response {
    match SessionRegistry::load(&state.home) {
        Ok(registry) => Json(registry).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn update_retention_policy(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
    Json(request): Json<UpdateRetentionPolicyRequest>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || {
        update_registry_retention_policy(&home, &session_id, request)
    })
    .await
    {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn cleanup_session(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || cleanup_persistent_session(&home, &session_id)).await
    {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn reconcile_session(
    State(state): State<WebState>,
    Path(session_id): Path<String>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || reconcile_registry_session(&home, &session_id)).await
    {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn show_report(State(state): State<WebState>, Path(session_id): Path<String>) -> Response {
    let registry = match SessionRegistry::load(&state.home) {
        Ok(registry) => registry,
        Err(error) => {
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };

    let Some(entry) = registry.find(&session_id) else {
        return json_error(
            StatusCode::NOT_FOUND,
            format!("Session not found: {session_id}"),
        );
    };

    match fs::read_to_string(&entry.report_path) {
        Ok(report) => match serde_json::from_str::<serde_json::Value>(&report) {
            Ok(json) => Json(json).into_response(),
            Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json_error(
            StatusCode::NOT_FOUND,
            format!(
                "Report file not found for session {session_id}. It may have been archived, removed during lifecycle cleanup, or the registry may need reconciliation."
            ),
        ),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

fn json_error(status: StatusCode, message: String) -> Response {
    (status, Json(ErrorResponse { error: message })).into_response()
}

fn cleanup_persistent_session(
    home: &str,
    session_id: &str,
) -> Result<SessionLifecycleActionResponse> {
    cleanup_persistent_session_with_reason(
        home,
        session_id,
        CleanupReason::ManualOperatorRequest,
        "Manual lifecycle cleanup finished.",
        "Operator requested immediate lifecycle cleanup for a retained session.",
    )
}

fn cleanup_persistent_session_with_reason(
    home: &str,
    session_id: &str,
    reason: CleanupReason,
    completion_message: &str,
    request_details: &str,
) -> Result<SessionLifecycleActionResponse> {
    let mut registry = SessionRegistry::load(home)?;
    let entry = registry
        .find_mut(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {session_id}"))?;

    let workspace_path = entry.workspace.clone();
    let current_report_path = entry.report_path.clone();

    let archive_operation = archive_report_if_present(home, session_id, &current_report_path)?;

    entry.mark_cleanup_pending(reason.clone());
    registry.save(home)?;

    let workspace_path_buf = FsPath::new(&workspace_path).to_path_buf();
    let (artifacts_detected, scan_operation) = scan_artifacts(&workspace_path_buf)?;
    let mut operations = vec![scan_operation];

    operations.push(SanitizationOperation {
        operation: "lifecycle_cleanup_request".to_string(),
        status: "successful".to_string(),
        details: request_details.to_string(),
    });

    if let Some(operation) = archive_operation {
        operations.push(operation);
    }

    let cleanup_report =
        cleanup_ephemeral_workspace(&workspace_path_buf, artifacts_detected, operations);

    {
        let entry = registry
            .find_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found after cleanup: {session_id}"))?;

        if let Some(archived_path) =
            maybe_archived_report_path(home, session_id, &current_report_path)?
        {
            entry.report_path = archived_path;
        }

        entry.mark_cleanup_result(&cleanup_report, reason);
    }

    registry.save(home)?;

    let entry = registry
        .find(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found after cleanup save: {session_id}"))?;

    sync_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    Ok(build_lifecycle_action_response(entry, completion_message))
}

fn update_registry_retention_policy(
    home: &str,
    session_id: &str,
    request: UpdateRetentionPolicyRequest,
) -> Result<SessionLifecycleActionResponse> {
    let retention_policy = RetentionPolicy::from_str(&request.retention_policy)?;

    if retention_policy == RetentionPolicy::EphemeralImmediate {
        anyhow::bail!(
            "ephemeral_immediate is only valid for ephemeral sessions and cannot be assigned to a retained registry session"
        );
    }

    let retention_deadline = match retention_policy {
        RetentionPolicy::RetainUntilManualCleanup => None,
        RetentionPolicy::RetainForDuration => {
            let minutes = request.retain_for_minutes.ok_or_else(|| {
                anyhow::anyhow!("retain_for_minutes is required for retain_for_duration")
            })?;

            if minutes == 0 {
                anyhow::bail!("retain_for_minutes must be greater than 0");
            }

            Some((chrono::Utc::now() + chrono::Duration::minutes(minutes as i64)).to_rfc3339())
        }
        RetentionPolicy::EphemeralImmediate => None,
    };

    let mut registry = SessionRegistry::load(home)?;
    let entry = registry
        .find_mut(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {session_id}"))?;

    entry.apply_retention_policy(retention_policy.clone(), retention_deadline);
    registry.save(home)?;

    let entry = registry
        .find(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found after retention update: {session_id}"))?;

    sync_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    let message = match retention_policy {
        RetentionPolicy::RetainUntilManualCleanup => {
            "Updated session retention to manual cleanup.".to_string()
        }
        RetentionPolicy::RetainForDuration => format!(
            "Updated session retention to expire at {}.",
            entry
                .lifecycle
                .retention_deadline
                .as_deref()
                .unwrap_or("unknown")
        ),
        RetentionPolicy::EphemeralImmediate => unreachable!(),
    };

    Ok(build_lifecycle_action_response(entry, &message))
}

fn run_retention_sweep(home: &str) -> Result<Vec<String>> {
    let due_sessions = due_retention_cleanup_session_ids(home)?;
    let mut swept = Vec::new();

    for session_id in due_sessions {
        cleanup_persistent_session_with_reason(
            home,
            &session_id,
            CleanupReason::ScheduledRetentionExpiry,
            "Scheduled retention expiry cleanup finished.",
            "Scheduled retention expiry triggered lifecycle cleanup for this retained session.",
        )?;

        swept.push(session_id);
    }

    Ok(swept)
}

fn reconcile_registry_session(
    home: &str,
    session_id: &str,
) -> Result<SessionLifecycleActionResponse> {
    let mut registry = SessionRegistry::load(home)?;
    let message = {
        let entry = registry
            .find_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {session_id}"))?;

        let workspace_exists = FsPath::new(&entry.workspace).exists();
        let report_exists = FsPath::new(&entry.report_path).exists();

        if entry.cleanup_successful && !workspace_exists {
            entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
            "Registry matches a cleaned-up session. Workspace is gone and lifecycle cleanup succeeded."
                .to_string()
        } else if !workspace_exists && !entry.cleanup_successful {
            entry.mark_orphaned();
            "Workspace is missing even though the session was not recorded as cleaned successfully. Marked session as orphaned."
                .to_string()
        } else if workspace_exists && entry.cleanup_successful {
            entry.mark_orphaned();
            "Workspace still exists even though cleanup was previously recorded as successful. Marked session as orphaned for investigation."
                .to_string()
        } else if !report_exists && !entry.cleanup_successful {
            entry.mark_orphaned();
            "Report file is missing while lifecycle cleanup was not recorded as successful. Marked session as orphaned."
                .to_string()
        } else {
            entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
            "Registry paths are present and no reconciliation changes were needed.".to_string()
        }
    };

    registry.save(home)?;

    let entry = registry
        .find(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found after reconciliation: {session_id}"))?;

    sync_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    Ok(build_lifecycle_action_response(entry, &message))
}

fn archive_report_if_present(
    home: &str,
    session_id: &str,
    current_report_path: &str,
) -> Result<Option<SanitizationOperation>> {
    let source = FsPath::new(current_report_path);

    if !source.exists() {
        return Ok(None);
    }

    ensure_registry_dirs(home)?;

    let archived_path = archived_report_path(home, session_id);
    fs::copy(source, &archived_path)?;

    Ok(Some(SanitizationOperation {
        operation: "lifecycle_report_archive".to_string(),
        status: "successful".to_string(),
        details: format!(
            "Archived report before manual cleanup to {}.",
            archived_path.display()
        ),
    }))
}

fn maybe_archived_report_path(
    home: &str,
    session_id: &str,
    previous_report_path: &str,
) -> Result<Option<String>> {
    let archived_path = archived_report_path(home, session_id);

    if archived_path.exists() {
        return Ok(Some(archived_path.display().to_string()));
    }

    if FsPath::new(previous_report_path).exists() {
        return Ok(Some(previous_report_path.to_string()));
    }

    Ok(None)
}

fn build_lifecycle_action_response(
    entry: &SessionIndexEntry,
    message: &str,
) -> SessionLifecycleActionResponse {
    SessionLifecycleActionResponse {
        session_id: entry.session_id.clone(),
        lifecycle_state: entry.lifecycle.state.as_str().to_string(),
        retention_policy: entry.lifecycle.retention_policy.as_str().to_string(),
        retention_deadline: entry.lifecycle.retention_deadline.clone(),
        cleanup_reason: entry
            .lifecycle
            .cleanup_reason
            .as_ref()
            .map(|reason| reason.as_str().to_string()),
        cleanup_attempted: entry.cleanup_attempted,
        cleanup_successful: entry.cleanup_successful,
        workspace_deleted: entry.workspace_deleted,
        workspace_exists: FsPath::new(&entry.workspace).exists(),
        report_exists: FsPath::new(&entry.report_path).exists(),
        workspace: entry.workspace.clone(),
        report_path: entry.report_path.clone(),
        message: message.to_string(),
    }
}

fn home_dir() -> Result<String> {
    if let Ok(home) = std::env::var("HOME") {
        return Ok(home);
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        return Ok(user_profile);
    }

    anyhow::bail!("Could not determine home directory. HOME and USERPROFILE are both unset.")
}

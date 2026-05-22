use crate::audit::PrivacyReport;
use crate::chat::{ChatMessageRequest, ChatSessionManager, StartChatRequest};
use crate::cleanup::{
    cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation,
};
use crate::config::SessionConfig;
use crate::memory_scan::{buffer_contains_pattern, verify_buffer_zeroization};
use crate::registry::{register_persistent_session, SessionRegistry};
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
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use zeroize::Zeroize;

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

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    prompt: String,
    mode: Option<String>,
    persistent: Option<bool>,
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

pub async fn serve() -> Result<()> {
    let home = home_dir()?;

    let state = WebState {
        home: Arc::new(home),
        chat_manager: ChatSessionManager::new(),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/run", post(run_session))
        .route("/api/run/stream", post(run_session_stream))
        .route("/api/chat/start", post(start_chat_session))
        .route("/api/chat/:session_id/status", get(chat_session_status))
        .route("/api/chat/:session_id/end", post(end_chat_session))
        .route("/api/sessions", get(list_sessions))
        .route("/api/reports/:session_id", get(show_report))
        .route(
            "/api/chat/:session_id/message/stream",
            post(stream_chat_message),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3333));

    println!("NullContext web server listening on http://{addr}");
    println!("Health: http://{addr}/api/health");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

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

            let _ = tx.blocking_send(payload);
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
            send_payload(
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

            send_complete(&tx, false);
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

    let mut config =
        SessionConfig::from_web_request(home, request.prompt, request.mode, persistent)?;

    let session = Session::create()?;

    send_runtime(&tx, "Starting NullContext session...");
    send_runtime(&tx, &format!("Session ID: {}", session.id));
    send_runtime(&tx, &format!("Workspace: {}", session.workspace.display()));
    send_runtime(
        &tx,
        &format!("Security mode: {}", config.security_mode.as_str()),
    );
    send_runtime(
        &tx,
        &format!("Prompt source: {}", config.prompt_source.as_str()),
    );

    session.write_prompt(config.prompt.as_bytes())?;

    let prompt_probe = config.prompt.as_bytes().to_vec();

    let prompt_found_before = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);

    send_runtime(&tx, "Launching llama-server...");

    let mut runtime = ManagedRuntime::launch(&config)?;

    send_runtime(&tx, "Runtime healthy.");
    send_runtime(&tx, "Running streaming inference...");
    send_runtime(&tx, "--- Model Output ---");

    let response_text = stream_completion_from_llama(
        &runtime.completion_url(),
        config.prompt.as_str(),
        config.max_tokens.parse::<u32>()?,
        &tx,
    )?;

    let runtime_terminated = runtime.shutdown()?;

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

    send_runtime(&tx, "Sanitizing Rust-owned buffers...");

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
        send_runtime(&tx, "Session mode: ephemeral");
        send_runtime(
            &tx,
            &format!("Detected {} workspace artifacts.", artifacts_detected.len()),
        );
        send_runtime(&tx, "Cleaning up workspace...");

        cleanup_ephemeral_workspace(
            &session.workspace,
            artifacts_detected,
            sanitization_operations,
        )
    } else {
        send_runtime(&tx, "Session mode: persistent");
        send_runtime(
            &tx,
            &format!("Detected {} workspace artifacts.", artifacts_detected.len()),
        );
        send_runtime(&tx, "Workspace retained at:");
        send_runtime(&tx, &session.workspace.display().to_string());

        CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
    };

    for operation in &cleanup_report.sanitization_operations {
        if operation.operation == "workspace_recursive_delete"
            || operation.operation == "post_cleanup_workspace_verification"
            || operation.operation == "workspace_retention_policy"
        {
            send_audit(&tx, operation);
        }
    }

    let report = PrivacyReport::new(
        session.id.clone(),
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        config.gpu_layers.clone(),
        runtime_terminated,
        cleanup_report.clone(),
    );

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;
        register_persistent_session(&config.home, &session, &config, &cleanup_report)?;
    }

    send_runtime(&tx, "--- Privacy Report v0 ---");
    send_report_text(&tx, &report_json);

    send_complete(&tx, true);

    Ok(())
}

fn stream_completion_from_llama(
    completion_url: &str,
    prompt: &str,
    n_predict: u32,
    tx: &mpsc::Sender<StreamPayload>,
) -> Result<String> {
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
                send_model_text(tx, content);
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

    Ok(full_response)
}

fn emit_and_push(
    tx: &mpsc::Sender<StreamPayload>,
    operations: &mut Vec<SanitizationOperation>,
    operation: SanitizationOperation,
) {
    send_audit(tx, &operation);
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

fn send_runtime(tx: &mpsc::Sender<StreamPayload>, message: &str) {
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
    );
}

fn send_model_text(tx: &mpsc::Sender<StreamPayload>, text: &str) {
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
    );
}

fn send_report_text(tx: &mpsc::Sender<StreamPayload>, text: &str) {
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
    );
}

fn send_audit(tx: &mpsc::Sender<StreamPayload>, operation: &SanitizationOperation) {
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
    );
}

fn send_complete(tx: &mpsc::Sender<StreamPayload>, success: bool) {
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
    );
}

fn send_payload(tx: &mpsc::Sender<StreamPayload>, payload: StreamPayload) {
    let _ = tx.blocking_send(payload);
}

async fn list_sessions(State(state): State<WebState>) -> Response {
    match SessionRegistry::load(&state.home) {
        Ok(registry) => Json(registry).into_response(),
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
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

fn json_error(status: StatusCode, message: String) -> Response {
    (status, Json(ErrorResponse { error: message })).into_response()
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

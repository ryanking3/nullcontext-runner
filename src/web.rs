use crate::registry::SessionRegistry;
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
use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone)]
struct WebState {
    home: Arc<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputPhase {
    Runtime,
    Model,
    Report,
}

pub async fn serve() -> Result<()> {
    let home = home_dir()?;

    let state = WebState {
        home: Arc::new(home),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/run", post(run_session))
        .route("/api/run/stream", post(run_session_stream))
        .route("/api/sessions", get(list_sessions))
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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "nullcontext".to_string(),
    })
}

async fn run_session(Json(request): Json<RunRequest>) -> Response {
    match run_cli_session(request) {
        Ok(response) => Json(response).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn run_session_stream(
    Json(request): Json<RunRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<StreamPayload>(256);

    std::thread::spawn(move || {
        if let Err(error) = run_cli_session_streaming(request, tx.clone()) {
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

fn run_cli_session_streaming(request: RunRequest, tx: mpsc::Sender<StreamPayload>) -> Result<()> {
    let exe_path = std::env::current_exe()?;

    let mode = request.mode.unwrap_or_else(|| "secure".to_string());
    let persistent = request.persistent.unwrap_or(false);

    send_runtime(&tx, "Starting streamed NullContext session");

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

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdout"))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture child stderr"))?;

    let stdout_tx = tx.clone();
    let stdout_thread = std::thread::spawn(move || {
        stream_stdout(stdout, stdout_tx);
    });

    let stderr_tx = tx.clone();
    let stderr_thread = std::thread::spawn(move || {
        stream_stderr(stderr, stderr_tx);
    });

    let status = child.wait()?;

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    send_payload(
        &tx,
        StreamPayload {
            event_type: "complete".to_string(),
            message: None,
            text: None,
            operation: None,
            status: None,
            details: None,
            success: Some(status.success()),
        },
    );

    Ok(())
}

fn stream_stdout(stdout: impl std::io::Read, tx: mpsc::Sender<StreamPayload>) {
    let reader = BufReader::new(stdout);
    let mut phase = OutputPhase::Runtime;

    for line_result in reader.lines() {
        let Ok(line) = line_result else {
            send_error(&tx, "Failed to read stdout line");
            break;
        };

        let trimmed = line.trim();

        if trimmed == "--- Model Output ---" {
            phase = OutputPhase::Model;
            send_runtime(&tx, "--- Model Output ---");
            continue;
        }

        if trimmed == "--- Privacy Report v0 ---" {
            phase = OutputPhase::Report;
            send_runtime(&tx, "--- Privacy Report v0 ---");
            continue;
        }

        if let Some(operation) = parse_audit_line(trimmed) {
            send_payload(&tx, operation);
            continue;
        }

        if phase == OutputPhase::Model && is_cleanup_or_runtime_line(trimmed) {
            phase = OutputPhase::Runtime;
        }

        match phase {
            OutputPhase::Runtime => {
                if !trimmed.is_empty() {
                    send_runtime(&tx, &line);
                }
            }
            OutputPhase::Model => {
                if !trimmed.is_empty() {
                    send_model_text(&tx, &format!("{line}\n"));
                }
            }
            OutputPhase::Report => {
                send_report_text(&tx, &format!("{line}\n"));
            }
        }
    }
}

fn stream_stderr(stderr: impl std::io::Read, tx: mpsc::Sender<StreamPayload>) {
    let reader = BufReader::new(stderr);

    for line_result in reader.lines() {
        match line_result {
            Ok(line) => {
                send_payload(
                    &tx,
                    StreamPayload {
                        event_type: "stderr".to_string(),
                        message: Some(line),
                        text: None,
                        operation: None,
                        status: None,
                        details: None,
                        success: None,
                    },
                );
            }
            Err(_) => {
                send_error(&tx, "Failed to read stderr line");
                break;
            }
        }
    }
}

fn parse_audit_line(line: &str) -> Option<StreamPayload> {
    if !line.starts_with("[audit] ") {
        return None;
    }

    let payload = line.replace("[audit] ", "");
    let parts: Vec<&str> = payload.split(" | ").collect();

    if parts.len() < 3 {
        return Some(StreamPayload {
            event_type: "audit".to_string(),
            message: None,
            text: None,
            operation: Some("unknown".to_string()),
            status: Some("unknown".to_string()),
            details: Some(payload),
            success: None,
        });
    }

    Some(StreamPayload {
        event_type: "audit".to_string(),
        message: None,
        text: None,
        operation: Some(parts[0].trim().to_string()),
        status: Some(parts[1].trim().to_string()),
        details: Some(parts[2..].join(" | ").trim().to_string()),
        success: None,
    })
}

fn is_cleanup_or_runtime_line(line: &str) -> bool {
    line.starts_with("Sanitizing Rust-owned buffers")
        || line.starts_with("Session mode:")
        || line.starts_with("Detected ")
        || line.starts_with("Cleaning up workspace")
        || line.starts_with("Workspace retained at:")
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

fn send_error(tx: &mpsc::Sender<StreamPayload>, message: &str) {
    send_payload(
        tx,
        StreamPayload {
            event_type: "error".to_string(),
            message: Some(message.to_string()),
            text: None,
            operation: None,
            status: None,
            details: None,
            success: None,
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

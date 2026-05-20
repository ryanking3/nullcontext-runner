use crate::registry::SessionRegistry;
use anyhow::Result;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::Arc;
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

pub async fn serve() -> Result<()> {
    let home = home_dir()?;

    let state = WebState {
        home: Arc::new(home),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/run", post(run_session))
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

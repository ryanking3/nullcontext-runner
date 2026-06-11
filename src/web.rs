use crate::audit::{
    build_failed_launch_llama_runtime_report, build_llama_runtime_report, sync_report_lifecycle,
    PrivacyReport, RetrievalReport,
};
use crate::chat::{CancelChatResponse, ChatMessageRequest, ChatSessionManager, StartChatRequest};
use crate::cleanup::{
    cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation,
};
use crate::config::{load_model_registry, SessionConfig};
use crate::corpus::{CorpusCleanupReason, CorpusRetentionPolicy};
use crate::corpus_registry::{
    archived_corpus_report_path, due_retention_cleanup_corpus_ids, ensure_corpus_registry_dirs,
    list_corpora, reconcile_corpora_on_startup, resolve_corpus_report_availability,
    sync_corpus_report_lifecycle, CorpusIndexEntry, CorpusRegistry,
    CorpusStartupReconciliationSummary,
};
use crate::docs::{
    ingest_corpus, ingest_uploaded_corpus, IngestCorpusRequest, IngestCorpusResponse,
    IngestUploadedCorpusRequest, UploadedCorpusFile,
};
use crate::llama_stream::{stream_completion_from_llama, StreamTermination};
use crate::logging::{stderr_line, stdout_line};
use crate::memory_scan::{buffer_contains_pattern, verify_buffer_zeroization};
use crate::process_scan::{
    build_process_scan_report, scan_failed_start_cleanup_phase, scan_live_process_phase,
    scan_post_shutdown_process_phase, ProcessScanMarker,
};
use crate::registry::{
    archived_report_path, due_retention_cleanup_session_ids, ensure_registry_dirs,
    reconcile_registry_on_startup, register_persistent_session,
    resolve_session_report_availability, CleanupReason, RetentionPolicy, SessionIndexEntry,
    SessionLifecycleMetadata, SessionLifecycleState, SessionRegistry, StartupReconciliationSummary,
};
use crate::retrieval::{
    build_grounded_prompt, build_retrieval_report, query_corpus, QueryCorpusRequest,
    QueryCorpusResponse,
};
use crate::runtime::{observe_post_shutdown, ManagedRuntime, RuntimeLaunchFailure};
use crate::sensitive::SensitiveBytes;
use crate::session::Session;
use crate::validation_harness::run_controlled_canary_validation;
use anyhow::Result;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
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
    startup_status: Arc<StartupStatusResponse>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
}

#[derive(Debug, Clone, Serialize)]
struct StartupStatusResponse {
    sessions: StartupReconciliationResponse,
    corpora: StartupReconciliationResponse,
}

#[derive(Debug, Clone, Serialize)]
struct StartupReconciliationResponse {
    scanned: usize,
    changed: usize,
    orphaned: usize,
    abandoned_active: usize,
    cleanup_consistent: usize,
    unchanged: usize,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct SessionRegistrySnapshotResponse {
    sessions: Vec<SessionRegistryEntryResponse>,
}

#[derive(Debug, Serialize)]
struct SessionRegistryEntryResponse {
    session_id: String,
    started_at: String,
    security_mode: String,
    prompt_source: String,
    history_stored: bool,
    backend: String,
    model_id: String,
    model_name: String,
    model_path: String,
    workspace: String,
    report_path: String,
    artifacts_detected: usize,
    cleanup_attempted: bool,
    cleanup_successful: bool,
    workspace_deleted: bool,
    workspace_exists: bool,
    report_exists: bool,
    report_available: bool,
    report_storage: String,
    loadable_report_path: Option<String>,
    lifecycle: SessionLifecycleMetadata,
}

#[derive(Debug, Serialize)]
struct CorpusRegistrySnapshotResponse {
    corpora: Vec<CorpusRegistryEntryResponse>,
}

#[derive(Debug, Serialize)]
struct CorpusRegistryEntryResponse {
    corpus_id: String,
    name: String,
    created_at: String,
    persistent: bool,
    root_path: String,
    manifest_path: String,
    report_path: String,
    source_count: usize,
    chunk_count: usize,
    embedding_backend: Option<String>,
    embedding_model: Option<String>,
    ocr_backend: Option<String>,
    root_exists: bool,
    manifest_exists: bool,
    report_exists: bool,
    report_available: bool,
    report_storage: String,
    loadable_report_path: Option<String>,
    lifecycle: crate::corpus::CorpusLifecycleMetadata,
}

#[derive(Debug, Serialize)]
struct SessionLifecycleActionResponse {
    session_id: String,
    lifecycle_state: String,
    retention_policy: String,
    retention_deadline: Option<String>,
    cleanup_reason: Option<String>,
    cleanup_requested_at: Option<String>,
    cleanup_completed_at: Option<String>,
    state_note: Option<String>,
    updated_at: Option<String>,
    cleanup_attempted: bool,
    cleanup_successful: bool,
    workspace_deleted: bool,
    workspace_exists: bool,
    report_exists: bool,
    report_available: bool,
    report_storage: String,
    loadable_report_path: Option<String>,
    workspace: String,
    report_path: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct CorpusLifecycleActionResponse {
    corpus_id: String,
    lifecycle_state: String,
    retention_policy: String,
    retention_deadline: Option<String>,
    cleanup_reason: Option<String>,
    cleanup_requested_at: Option<String>,
    cleanup_completed_at: Option<String>,
    state_note: Option<String>,
    updated_at: Option<String>,
    root_exists: bool,
    manifest_exists: bool,
    report_exists: bool,
    report_available: bool,
    report_storage: String,
    loadable_report_path: Option<String>,
    root_path: String,
    manifest_path: String,
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
    model_id: Option<String>,
    corpus_id: Option<String>,
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
    let startup_session_summary = emit_startup_reconciliation(&home)?;
    let startup_corpus_summary = emit_startup_corpus_reconciliation(&home)?;
    ensure_corpus_registry_dirs(&home)?;

    let state = WebState {
        home: Arc::new(home),
        chat_manager: ChatSessionManager::new(),
        startup_status: Arc::new(StartupStatusResponse {
            sessions: build_session_startup_reconciliation_response(&startup_session_summary),
            corpora: build_corpus_startup_reconciliation_response(&startup_corpus_summary),
        }),
    };

    spawn_retention_scheduler(state.home.clone());

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/startup-status", get(startup_status))
        .route(
            "/api/corpora",
            get(list_corpora_route).post(ingest_corpus_route),
        )
        .route("/api/corpora/upload", post(upload_corpus_route))
        .route("/api/corpora/:corpus_id/report", get(show_corpus_report))
        .route("/api/corpora/:corpus_id/query", post(query_corpus_route))
        .route(
            "/api/corpora/:corpus_id/retention",
            post(update_corpus_retention_policy),
        )
        .route("/api/corpora/:corpus_id/cleanup", post(cleanup_corpus))
        .route("/api/corpora/:corpus_id/reconcile", post(reconcile_corpus))
        .route("/api/models", get(list_models))
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

    let addr = resolve_bind_addr()?;
    let listener = bind_listener(addr).await?;

    stdout_line(format!("NullContext web server listening on http://{addr}"));
    stdout_line(format!("Health: http://{addr}/api/health"));

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

            let result = tokio::task::spawn_blocking(move || {
                let sessions = run_retention_sweep(&home)?;
                let corpora = run_corpus_retention_sweep(&home)?;
                Ok::<_, anyhow::Error>((sessions, corpora))
            })
            .await;

            match result {
                Ok(Ok((swept, corpus_swept))) if !swept.is_empty() || !corpus_swept.is_empty() => {
                    if !swept.is_empty() {
                        stdout_line(format!(
                            "Retention sweep cleaned {} session(s): {}",
                            swept.len(),
                            swept.join(", ")
                        ));
                    }
                    if !corpus_swept.is_empty() {
                        stdout_line(format!(
                            "Retention sweep cleaned {} corpora: {}",
                            corpus_swept.len(),
                            corpus_swept.join(", ")
                        ));
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    stderr_line(format!("Retention sweep error: {error}"));
                }
                Err(error) => {
                    stderr_line(format!("Retention sweep task failed: {error}"));
                }
            }
        }
    });
}

fn emit_startup_reconciliation(home: &str) -> Result<StartupReconciliationSummary> {
    let summary = reconcile_registry_on_startup(home)?;
    sync_registry_report_lifecycle(home)?;

    stdout_line(format!(
        "Lifecycle reconciliation: scanned {} session(s), changed {}, orphaned {}, abandoned-active {}, cleanup-consistent {}, unchanged {}.",
        summary.scanned_sessions,
        summary.changed_sessions,
        summary.orphaned_sessions,
        summary.abandoned_active_sessions,
        summary.cleanup_succeeded_consistent,
        summary.unchanged_sessions
    ));

    for note in summary.notes.iter().take(8) {
        stdout_line(format!("  [lifecycle] {note}"));
    }

    if summary.notes.len() > 8 {
        stdout_line(format!(
            "  [lifecycle] ... and {} more session note(s)",
            summary.notes.len() - 8
        ));
    }

    Ok(summary)
}

fn emit_startup_corpus_reconciliation(home: &str) -> Result<CorpusStartupReconciliationSummary> {
    let summary = reconcile_corpora_on_startup(home)?;

    stdout_line(format!(
        "Corpus lifecycle reconciliation: scanned {} corpora, changed {}, orphaned {}, cleanup-consistent {}, unchanged {}.",
        summary.scanned_corpora,
        summary.changed_corpora,
        summary.orphaned_corpora,
        summary.cleanup_succeeded_consistent,
        summary.unchanged_corpora
    ));

    for note in summary.notes.iter().take(8) {
        stdout_line(format!("  [corpus-lifecycle] {note}"));
    }

    if summary.notes.len() > 8 {
        stdout_line(format!(
            "  [corpus-lifecycle] ... and {} more corpus note(s)",
            summary.notes.len() - 8
        ));
    }

    Ok(summary)
}

fn sync_registry_report_lifecycle(home: &str) -> Result<()> {
    let registry = SessionRegistry::load(home)?;

    for entry in registry.sessions {
        sync_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;
    }

    Ok(())
}

fn build_session_startup_reconciliation_response(
    summary: &StartupReconciliationSummary,
) -> StartupReconciliationResponse {
    StartupReconciliationResponse {
        scanned: summary.scanned_sessions,
        changed: summary.changed_sessions,
        orphaned: summary.orphaned_sessions,
        abandoned_active: summary.abandoned_active_sessions,
        cleanup_consistent: summary.cleanup_succeeded_consistent,
        unchanged: summary.unchanged_sessions,
        notes: summary.notes.clone(),
    }
}

fn build_corpus_startup_reconciliation_response(
    summary: &CorpusStartupReconciliationSummary,
) -> StartupReconciliationResponse {
    StartupReconciliationResponse {
        scanned: summary.scanned_corpora,
        changed: summary.changed_corpora,
        orphaned: summary.orphaned_corpora,
        abandoned_active: 0,
        cleanup_consistent: summary.cleanup_succeeded_consistent,
        unchanged: summary.unchanged_corpora,
        notes: summary.notes.clone(),
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "nullcontext".to_string(),
    })
}

async fn startup_status(State(state): State<WebState>) -> Json<StartupStatusResponse> {
    Json(state.startup_status.as_ref().clone())
}

async fn list_models(State(state): State<WebState>) -> Response {
    match load_model_registry(&state.home) {
        Ok(models) => Json(models).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn list_corpora_route(State(state): State<WebState>) -> Response {
    match list_corpora(&state.home) {
        Ok(registry) => Json(build_corpus_registry_snapshot(&state.home, registry)).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn ingest_corpus_route(
    State(state): State<WebState>,
    Json(request): Json<IngestCorpusRequest>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || ingest_corpus(&home, request)).await {
        Ok(Ok(response)) => Json::<IngestCorpusResponse>(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn upload_corpus_route(State(state): State<WebState>, mut multipart: Multipart) -> Response {
    let mut name = String::new();
    let mut persistent = None;
    let mut ocr_enabled = None;
    let mut files = Vec::new();

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let field_name = field.name().unwrap_or_default().to_string();

                match field_name.as_str() {
                    "name" => {
                        if let Ok(value) = field.text().await {
                            name = value;
                        }
                    }
                    "persistent" => {
                        if let Ok(value) = field.text().await {
                            persistent = Some(matches!(
                                value.trim().to_ascii_lowercase().as_str(),
                                "1" | "true" | "yes" | "on"
                            ));
                        }
                    }
                    "ocr_enabled" => {
                        if let Ok(value) = field.text().await {
                            ocr_enabled = Some(matches!(
                                value.trim().to_ascii_lowercase().as_str(),
                                "1" | "true" | "yes" | "on"
                            ));
                        }
                    }
                    "files" => {
                        let file_name = field
                            .file_name()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| "upload.bin".to_string());

                        match field.bytes().await {
                            Ok(bytes) => files.push(UploadedCorpusFile {
                                file_name,
                                bytes: bytes.to_vec(),
                            }),
                            Err(error) => {
                                return json_error(
                                    StatusCode::BAD_REQUEST,
                                    format!("Failed to read uploaded file bytes: {error}"),
                                );
                            }
                        }
                    }
                    _ => {
                        let _ = field.bytes().await;
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!("Failed to parse multipart upload: {error}"),
                );
            }
        }
    }

    let home = state.home.as_ref().clone();
    let request = IngestUploadedCorpusRequest {
        name,
        persistent,
        ocr_enabled,
        files,
    };

    match tokio::task::spawn_blocking(move || ingest_uploaded_corpus(&home, request)).await {
        Ok(Ok(response)) => Json::<IngestCorpusResponse>(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn query_corpus_route(
    State(state): State<WebState>,
    Path(corpus_id): Path<String>,
    Json(request): Json<QueryCorpusRequest>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || query_corpus(&home, &corpus_id, request)).await {
        Ok(Ok(response)) => Json::<QueryCorpusResponse>(response).into_response(),
        Ok(Err(error)) => {
            let message = error.to_string();
            let status = corpus_request_status(&message);
            json_error(status, message)
        }
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn show_corpus_report(
    State(state): State<WebState>,
    Path(corpus_id): Path<String>,
) -> Response {
    let registry = match CorpusRegistry::load(&state.home) {
        Ok(registry) => registry,
        Err(error) => {
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };

    let Some(entry) = registry.find(&corpus_id) else {
        return json_error(
            StatusCode::NOT_FOUND,
            format!("Corpus not found: {corpus_id}"),
        );
    };

    let availability = resolve_corpus_report_availability(&state.home, entry);

    let Some(report_path) = availability.loadable_path else {
        return json_error(
            StatusCode::NOT_FOUND,
            format!(
                "Corpus report file not found for {corpus_id}. NullContext could not find either the current report path or the archived lifecycle report. The report may have been removed during cleanup or the registry may need reconciliation."
            ),
        );
    };

    match fs::read_to_string(report_path) {
        Ok(report) => match serde_json::from_str::<serde_json::Value>(&report) {
            Ok(json) => Json(json).into_response(),
            Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        },
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn update_corpus_retention_policy(
    State(state): State<WebState>,
    Path(corpus_id): Path<String>,
    Json(request): Json<UpdateRetentionPolicyRequest>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || {
        update_registry_corpus_retention_policy(&home, &corpus_id, request)
    })
    .await
    {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn cleanup_corpus(State(state): State<WebState>, Path(corpus_id): Path<String>) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || cleanup_registered_corpus(&home, &corpus_id)).await {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn reconcile_corpus(
    State(state): State<WebState>,
    Path(corpus_id): Path<String>,
) -> Response {
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || reconcile_registry_corpus(&home, &corpus_id)).await {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn start_chat_session(
    State(state): State<WebState>,
    Json(request): Json<StartChatRequest>,
) -> Response {
    let manager = state.chat_manager.clone();
    let home = state.home.as_ref().clone();

    match tokio::task::spawn_blocking(move || manager.start_session(home, request)).await {
        Ok(Ok(response)) => Json(response).into_response(),
        Ok(Err(error)) => {
            let message = error.to_string();
            json_error(corpus_request_status(&message), message)
        }
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
    if request.corpus_id.is_some() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Corpus-backed retrieval is currently supported on /api/run/stream only.".to_string(),
        );
    }

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
    let corpus_id = request.corpus_id.clone();

    let mut config = SessionConfig::from_web_request(
        home,
        request.prompt,
        request.mode,
        persistent,
        request.model_id,
        request.chat_template,
        request.chat_context_token_budget,
        request.chat_context_turn_limit,
    )?;

    let mut retrieval_report = None;

    if let Some(corpus_id) = corpus_id {
        let _ = send_runtime(&tx, "Retrieving local corpus context...");
        let retrieval = query_corpus(
            &config.home,
            &corpus_id,
            QueryCorpusRequest {
                query: config.prompt.as_str().to_string(),
                top_k: Some(6),
            },
        )?;
        let grounded_prompt = build_grounded_prompt(&retrieval);
        retrieval_report = Some(build_retrieval_report(&retrieval));

        let _ = send_runtime(
            &tx,
            &format!(
                "Retrieved {} chunk(s) from corpus {} ({}).",
                retrieval.results.len(),
                retrieval.corpus_name,
                retrieval.corpus_id
            ),
        );
        let _ = send_audit(
            &tx,
            &SanitizationOperation {
                operation: "corpus_retrieval_context_injected".to_string(),
                status: "successful".to_string(),
                details: format!(
                    "Injected retrieval context from corpus '{}' ({}) using {} chunk(s) across {} source file(s).",
                    retrieval.corpus_name,
                    retrieval.corpus_id,
                    retrieval.results.len(),
                    retrieval_report
                        .as_ref()
                        .map(|report| report.source_paths.len())
                        .unwrap_or(0)
                ),
            },
        );

        config.prompt = SensitiveBytes::new(grounded_prompt);
    }

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
        &format!("Model: {} ({})", config.model_name, config.model_id),
    );
    let _ = send_runtime(&tx, &format!("Model path: {}", config.model_path));
    let _ = send_runtime(
        &tx,
        &format!("Prompt source: {}", config.prompt_source.as_str()),
    );

    session.write_prompt(config.prompt.as_bytes())?;

    let prompt_probe = config.prompt.as_bytes().to_vec();

    let prompt_found_before = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);

    let _ = send_runtime(&tx, "Launching llama-server...");

    let mut runtime = match ManagedRuntime::launch(&config) {
        Ok(runtime) => runtime,
        Err(error) => {
            if let Some(failure) = error.downcast_ref::<RuntimeLaunchFailure>() {
                handle_failed_streaming_startup(
                    &tx,
                    &session,
                    &mut config,
                    &prompt_probe,
                    prompt_found_before,
                    retrieval_report,
                    failure,
                )?;
                return Ok(());
            }

            return Err(error);
        }
    };
    let runtime_pid = runtime.pid();
    let runtime_endpoint = runtime.endpoint_url().to_string();

    let _ = send_runtime(&tx, "Runtime healthy.");
    let _ = send_runtime(&tx, &format!("Runtime endpoint: {runtime_endpoint}"));
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

    let live_process_scan = scan_live_process_phase(
        runtime_pid,
        &[
            ProcessScanMarker {
                kind: "prompt_marker",
                bytes: config.prompt.as_bytes(),
            },
            ProcessScanMarker {
                kind: "response_marker",
                bytes: response_text.as_bytes(),
            },
        ],
    );
    let runtime_usage = runtime.observe_usage();
    let runtime_shutdown = runtime.shutdown()?;
    let post_shutdown_observation = observe_post_shutdown(
        runtime_pid,
        config.gpu_layers.parse::<u32>().unwrap_or(0) > 0,
        Some(&config),
    );
    let post_shutdown_process_scan = scan_post_shutdown_process_phase(
        runtime_pid,
        &post_shutdown_observation,
        &[
            ProcessScanMarker {
                kind: "prompt_marker",
                bytes: config.prompt.as_bytes(),
            },
            ProcessScanMarker {
                kind: "response_marker",
                bytes: response_text.as_bytes(),
            },
        ],
    );

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
            status: if runtime_shutdown.stopped {
                "successful".to_string()
            } else {
                "failed".to_string()
            },
            details: format!(
                "llama-server child process shutdown completed using method {}. Exit code: {}.",
                runtime_shutdown.shutdown_method,
                runtime_shutdown
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
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
        runtime_shutdown.stopped,
        cleanup_report.clone(),
    )
    .with_lifecycle(&lifecycle)
    .with_process_scan(build_process_scan_report(
        Some(runtime_pid),
        vec![live_process_scan, post_shutdown_process_scan],
    ))
    .with_llama_runtime(build_llama_runtime_report(
        &config,
        Some(runtime_pid),
        Some(&runtime_endpoint),
        &runtime_shutdown,
        &runtime_usage,
        &post_shutdown_observation,
    ))
    .with_controlled_canary_run(run_controlled_canary_validation(&config));
    let report = if let Some(retrieval_report) = retrieval_report {
        report.with_retrieval(retrieval_report)
    } else {
        report
    };

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

fn handle_failed_streaming_startup(
    tx: &mpsc::Sender<StreamPayload>,
    session: &Session,
    config: &mut SessionConfig,
    prompt_probe: &[u8],
    prompt_found_before: bool,
    retrieval_report: Option<RetrievalReport>,
    failure: &RuntimeLaunchFailure,
) -> Result<()> {
    let _ = send_runtime(tx, "Runtime failed before becoming healthy.");

    let (artifacts_detected, scan_operation) = scan_artifacts(&session.workspace)?;
    let mut sanitization_operations = Vec::new();
    emit_and_push(tx, &mut sanitization_operations, scan_operation);

    emit_and_push(
        tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "managed_runtime_startup".to_string(),
            status: "failed".to_string(),
            details: failure.to_string(),
        },
    );

    let _ = send_runtime(tx, "Sanitizing Rust-owned prompt buffer...");
    config.prompt.sanitize();

    let prompt_found_after = buffer_contains_pattern(config.prompt.as_bytes(), prompt_probe);
    emit_and_push(
        tx,
        &mut sanitization_operations,
        verify_buffer_zeroization("prompt_buffer", prompt_found_before, prompt_found_after),
    );
    emit_and_push(
        tx,
        &mut sanitization_operations,
        SanitizationOperation {
            operation: "explicit_sensitive_byte_buffer_zeroization".to_string(),
            status: "successful".to_string(),
            details: "Explicitly overwrote the Rust-owned prompt byte buffer before drop after startup failure."
                .to_string(),
        },
    );

    let cleanup_report = if config.ephemeral {
        let _ = send_runtime(tx, "Session mode: ephemeral");
        let _ = send_runtime(
            tx,
            &format!(
                "Detected {} workspace artifacts after startup failure.",
                artifacts_detected.len()
            ),
        );
        let _ = send_runtime(tx, "Cleaning up workspace...");
        cleanup_ephemeral_workspace(
            &session.workspace,
            artifacts_detected,
            sanitization_operations,
        )
    } else {
        let _ = send_runtime(tx, "Session mode: persistent");
        let _ = send_runtime(
            tx,
            &format!(
                "Detected {} workspace artifacts after startup failure.",
                artifacts_detected.len()
            ),
        );
        let _ = send_runtime(tx, "Workspace retained at:");
        let _ = send_runtime(tx, &session.workspace.display().to_string());
        CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
    };

    for operation in &cleanup_report.sanitization_operations {
        if operation.operation == "workspace_recursive_delete"
            || operation.operation == "post_cleanup_workspace_verification"
            || operation.operation == "workspace_retention_policy"
        {
            let _ = send_audit(tx, operation);
        }
    }

    let lifecycle = SessionLifecycleMetadata::for_failed_startup(config, &cleanup_report);
    let report = PrivacyReport::new(
        session.id.clone(),
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        config.gpu_layers.clone(),
        failure.cleanup_succeeded,
        cleanup_report.clone(),
    )
    .with_lifecycle(&lifecycle)
    .with_process_scan(build_process_scan_report(
        Some(failure.runtime_pid),
        vec![scan_failed_start_cleanup_phase(
            failure.runtime_pid,
            &failure.post_cleanup_observation,
            &[ProcessScanMarker {
                kind: "prompt_marker",
                bytes: prompt_probe,
            }],
        )],
    ))
    .with_llama_runtime(build_failed_launch_llama_runtime_report(config, failure));
    let report = if let Some(retrieval_report) = retrieval_report {
        report.with_retrieval(retrieval_report)
    } else {
        report
    };

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;
        register_persistent_session(&config.home, session, config, &cleanup_report)?;
    }

    let _ = send_runtime(tx, "--- Privacy Report v0 ---");
    let _ = send_report_text(tx, &report_json);
    let _ = send_payload(
        tx,
        StreamPayload {
            event_type: "error".to_string(),
            message: Some(failure.to_string()),
            text: None,
            operation: None,
            status: None,
            details: None,
            success: None,
        },
    );
    let _ = send_complete(tx, false);

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
    if request.corpus_id.is_some() {
        anyhow::bail!("Corpus-backed retrieval is currently supported on /api/run/stream only.");
    }

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

    if let Some(model_id) = request.model_id {
        command.arg("--model").arg(model_id);
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
        Ok(registry) => {
            Json(build_session_registry_snapshot(&state.home, registry)).into_response()
        }
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

    if entry.lifecycle.state == SessionLifecycleState::Active {
        return json_error(
            StatusCode::CONFLICT,
            format!(
                "Session {session_id} is still active. Privacy reports are written when the active chat ends and sanitization completes."
            ),
        );
    }

    let availability = resolve_session_report_availability(&state.home, entry);

    let Some(report_path) = availability.loadable_path else {
        return json_error(
            StatusCode::NOT_FOUND,
            format!(
                "Report file not found for session {session_id}. NullContext could not find either the current report path or the archived lifecycle report. The report may have been removed during cleanup or the registry may need reconciliation."
            ),
        );
    };

    match fs::read_to_string(report_path) {
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

fn corpus_request_status(message: &str) -> StatusCode {
    if message.contains("Corpus not found in registry") {
        StatusCode::NOT_FOUND
    } else if message.contains("not ready for retrieval")
        || message.contains("Reconcile the corpus registry before using this corpus")
    {
        StatusCode::CONFLICT
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
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

    if entry.lifecycle.state == SessionLifecycleState::Active {
        anyhow::bail!(
            "Session {session_id} is still marked active. End the active chat session before running lifecycle cleanup."
        );
    }

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

    Ok(build_lifecycle_action_response(
        home,
        entry,
        completion_message,
    ))
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

    Ok(build_lifecycle_action_response(home, entry, &message))
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

fn run_corpus_retention_sweep(home: &str) -> Result<Vec<String>> {
    let due_corpora = due_retention_cleanup_corpus_ids(home)?;
    let mut swept = Vec::new();

    for corpus_id in due_corpora {
        cleanup_registered_corpus_with_reason(
            home,
            &corpus_id,
            CorpusCleanupReason::ScheduledRetentionExpiry,
            "Scheduled retention expiry cleanup finished.",
            "Scheduled retention expiry triggered lifecycle cleanup for this retained corpus.",
        )?;

        swept.push(corpus_id);
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

        if entry.lifecycle.state == SessionLifecycleState::Active {
            anyhow::bail!(
                "Session {session_id} is still marked active. End the active chat session or restart NullContext before reconciling it."
            );
        }

        let workspace_exists = FsPath::new(&entry.workspace).exists();
        let mut report_exists = FsPath::new(&entry.report_path).exists();
        let archived_report_path = archived_report_path(home, session_id);
        let recovered_archived_report = !report_exists && archived_report_path.exists();

        if recovered_archived_report {
            entry.report_path = archived_report_path.display().to_string();
            report_exists = true;
        }

        let can_restore_retained_state = matches!(
            entry.lifecycle.state,
            SessionLifecycleState::Orphaned | SessionLifecycleState::AbandonedActive
        ) && workspace_exists
            && report_exists
            && !entry.cleanup_successful;

        if entry.cleanup_successful && !workspace_exists {
            if recovered_archived_report {
                entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
                entry.lifecycle.state_note = Some(
                    "Manual reconciliation confirmed that lifecycle cleanup had already succeeded, the retained workspace is gone, and the registry was relinked to the archived session report."
                        .to_string(),
                );
                "Registry matched a cleaned-up session and manual reconciliation relinked the archived report."
                    .to_string()
            } else {
                entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
                entry.lifecycle.state_note = Some(
                    "Manual reconciliation confirmed that lifecycle cleanup had already succeeded and the retained workspace is gone."
                        .to_string(),
                );
                "Registry matches a cleaned-up session. Workspace is gone and lifecycle cleanup succeeded."
                    .to_string()
            }
        } else if !workspace_exists && !entry.cleanup_successful {
            entry.mark_orphaned_with_note(
                "Manual reconciliation found that the retained workspace is missing even though successful lifecycle cleanup was never recorded. The session was marked orphaned for review."
                    .to_string(),
            );
            "Workspace is missing even though the session was not recorded as cleaned successfully. Marked session as orphaned."
                .to_string()
        } else if workspace_exists && entry.cleanup_successful {
            entry.mark_orphaned_with_note(
                "Manual reconciliation found that the retained workspace still exists even though cleanup had been recorded as successful. The session was marked orphaned for review."
                    .to_string(),
            );
            "Workspace still exists even though cleanup was previously recorded as successful. Marked session as orphaned for investigation."
                .to_string()
        } else if !report_exists && !entry.cleanup_successful {
            entry.mark_orphaned_with_note(
                "Manual reconciliation found that the retained report is missing even though successful cleanup was never recorded. The session was marked orphaned for review."
                    .to_string(),
            );
            "Report file is missing while lifecycle cleanup was not recorded as successful. Marked session as orphaned."
                .to_string()
        } else if recovered_archived_report && can_restore_retained_state {
            entry.mark_completed_retained_with_note(
                "Manual reconciliation confirmed that the retained session artifacts are present again, relinked the registry entry to the archived report path, and restored the lifecycle state to completed_retained."
                    .to_string(),
            );
            "Manual reconciliation relinked the retained session to the archived report path and restored a healthy completed_retained lifecycle state."
                .to_string()
        } else if can_restore_retained_state {
            entry.mark_completed_retained_with_note(
                "Manual reconciliation confirmed that the retained session artifacts are present and restored the lifecycle state to completed_retained."
                    .to_string(),
            );
            "Manual reconciliation restored a healthy completed_retained lifecycle state for this retained session."
                .to_string()
        } else if recovered_archived_report {
            entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
            entry.lifecycle.state_note = Some(
                "Manual reconciliation found that the retained session report had moved to the archived report path and relinked the registry entry."
                    .to_string(),
            );
            "Manual reconciliation relinked the retained session to the archived report path."
                .to_string()
        } else {
            entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
            entry.lifecycle.state_note = Some(
                "Manual reconciliation confirmed that the registry entry still matches the retained session artifacts on disk."
                    .to_string(),
            );
            "Registry paths are present and no reconciliation changes were needed.".to_string()
        }
    };

    registry.save(home)?;

    let entry = registry
        .find(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found after reconciliation: {session_id}"))?;

    sync_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    Ok(build_lifecycle_action_response(home, entry, &message))
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
    home: &str,
    entry: &SessionIndexEntry,
    message: &str,
) -> SessionLifecycleActionResponse {
    let (report_exists, report_available, report_storage, loadable_report_path) =
        session_report_availability(home, entry);

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
        cleanup_requested_at: entry.lifecycle.cleanup_requested_at.clone(),
        cleanup_completed_at: entry.lifecycle.cleanup_completed_at.clone(),
        state_note: entry.lifecycle.state_note.clone(),
        updated_at: entry.lifecycle.updated_at.clone(),
        cleanup_attempted: entry.cleanup_attempted,
        cleanup_successful: entry.cleanup_successful,
        workspace_deleted: entry.workspace_deleted,
        workspace_exists: FsPath::new(&entry.workspace).exists(),
        report_exists,
        report_available,
        report_storage,
        loadable_report_path,
        workspace: entry.workspace.clone(),
        report_path: entry.report_path.clone(),
        message: message.to_string(),
    }
}

fn build_session_registry_snapshot(
    home: &str,
    registry: SessionRegistry,
) -> SessionRegistrySnapshotResponse {
    SessionRegistrySnapshotResponse {
        sessions: registry
            .sessions
            .into_iter()
            .map(|entry| build_session_registry_entry_response(home, entry))
            .collect(),
    }
}

fn build_session_registry_entry_response(
    home: &str,
    entry: SessionIndexEntry,
) -> SessionRegistryEntryResponse {
    let (report_exists, report_available, report_storage, loadable_report_path) =
        session_report_availability(home, &entry);

    SessionRegistryEntryResponse {
        workspace_exists: FsPath::new(&entry.workspace).exists(),
        report_exists,
        report_available,
        report_storage,
        loadable_report_path,
        session_id: entry.session_id,
        started_at: entry.started_at,
        security_mode: entry.security_mode,
        prompt_source: entry.prompt_source,
        history_stored: entry.history_stored,
        backend: entry.backend,
        model_id: entry.model_id,
        model_name: entry.model_name,
        model_path: entry.model_path,
        workspace: entry.workspace,
        report_path: entry.report_path,
        artifacts_detected: entry.artifacts_detected,
        cleanup_attempted: entry.cleanup_attempted,
        cleanup_successful: entry.cleanup_successful,
        workspace_deleted: entry.workspace_deleted,
        lifecycle: entry.lifecycle,
    }
}

fn build_corpus_registry_snapshot(
    home: &str,
    registry: CorpusRegistry,
) -> CorpusRegistrySnapshotResponse {
    CorpusRegistrySnapshotResponse {
        corpora: registry
            .corpora
            .into_iter()
            .map(|entry| build_corpus_registry_entry_response(home, entry))
            .collect(),
    }
}

fn build_corpus_registry_entry_response(
    home: &str,
    entry: CorpusIndexEntry,
) -> CorpusRegistryEntryResponse {
    let (report_exists, report_available, report_storage, loadable_report_path) =
        corpus_report_availability(home, &entry);

    CorpusRegistryEntryResponse {
        root_exists: FsPath::new(&entry.root_path).exists(),
        manifest_exists: FsPath::new(&entry.manifest_path).exists(),
        report_exists,
        report_available,
        report_storage,
        loadable_report_path,
        corpus_id: entry.corpus_id,
        name: entry.name,
        created_at: entry.created_at,
        persistent: entry.persistent,
        root_path: entry.root_path,
        manifest_path: entry.manifest_path,
        report_path: entry.report_path,
        source_count: entry.source_count,
        chunk_count: entry.chunk_count,
        embedding_backend: entry.embedding_backend,
        embedding_model: entry.embedding_model,
        ocr_backend: entry.ocr_backend,
        lifecycle: entry.lifecycle,
    }
}

fn cleanup_registered_corpus(home: &str, corpus_id: &str) -> Result<CorpusLifecycleActionResponse> {
    cleanup_registered_corpus_with_reason(
        home,
        corpus_id,
        CorpusCleanupReason::ManualOperatorRequest,
        "Manual corpus lifecycle cleanup finished.",
        "Operator requested immediate lifecycle cleanup for a retained corpus.",
    )
}

fn cleanup_registered_corpus_with_reason(
    home: &str,
    corpus_id: &str,
    reason: CorpusCleanupReason,
    completion_message: &str,
    request_details: &str,
) -> Result<CorpusLifecycleActionResponse> {
    let mut registry = CorpusRegistry::load(home)?;
    let entry = registry
        .find_mut(corpus_id)
        .ok_or_else(|| anyhow::anyhow!("Corpus not found: {corpus_id}"))?;

    let root_path = entry.root_path.clone();
    let manifest_path = entry.manifest_path.clone();
    let current_report_path = entry.report_path.clone();

    let archive_operation =
        archive_corpus_report_if_present(home, corpus_id, &current_report_path)?;

    entry.mark_cleanup_pending(reason.clone());
    sync_manifest_lifecycle_if_present(&manifest_path, &entry.lifecycle)?;
    registry.save(home)?;

    let root_path_buf = FsPath::new(&root_path).to_path_buf();
    let (artifacts_detected, scan_operation) = scan_artifacts(&root_path_buf)?;
    let mut operations = vec![scan_operation];

    operations.push(SanitizationOperation {
        operation: "corpus_lifecycle_cleanup_request".to_string(),
        status: "successful".to_string(),
        details: request_details.to_string(),
    });

    if let Some(operation) = archive_operation {
        operations.push(operation);
    }

    let cleanup_report =
        cleanup_ephemeral_workspace(&root_path_buf, artifacts_detected, operations);

    {
        let entry = registry
            .find_mut(corpus_id)
            .ok_or_else(|| anyhow::anyhow!("Corpus not found after cleanup: {corpus_id}"))?;

        if let Some(archived_path) =
            maybe_archived_corpus_report_path(home, corpus_id, &current_report_path)?
        {
            entry.report_path = archived_path;
        }

        entry.mark_cleanup_result(cleanup_report.successful, reason);
    }

    registry.save(home)?;

    let entry = registry
        .find(corpus_id)
        .ok_or_else(|| anyhow::anyhow!("Corpus not found after cleanup save: {corpus_id}"))?;

    sync_corpus_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    Ok(build_corpus_lifecycle_action_response(
        home,
        entry,
        completion_message,
    ))
}

fn update_registry_corpus_retention_policy(
    home: &str,
    corpus_id: &str,
    request: UpdateRetentionPolicyRequest,
) -> Result<CorpusLifecycleActionResponse> {
    let retention_policy = match request.retention_policy.as_str() {
        "retain_until_manual_cleanup" => CorpusRetentionPolicy::RetainUntilManualCleanup,
        "retain_for_duration" => CorpusRetentionPolicy::RetainForDuration,
        "ephemeral_immediate" => CorpusRetentionPolicy::EphemeralImmediate,
        value => anyhow::bail!("Invalid corpus retention policy: {value}"),
    };

    let retention_deadline = match retention_policy {
        CorpusRetentionPolicy::RetainUntilManualCleanup => None,
        CorpusRetentionPolicy::RetainForDuration => {
            let minutes = request.retain_for_minutes.ok_or_else(|| {
                anyhow::anyhow!("retain_for_minutes is required for retain_for_duration")
            })?;

            if minutes == 0 {
                anyhow::bail!("retain_for_minutes must be greater than 0");
            }

            Some((chrono::Utc::now() + chrono::Duration::minutes(minutes as i64)).to_rfc3339())
        }
        CorpusRetentionPolicy::EphemeralImmediate => None,
    };

    let mut registry = CorpusRegistry::load(home)?;
    let entry = registry
        .find_mut(corpus_id)
        .ok_or_else(|| anyhow::anyhow!("Corpus not found: {corpus_id}"))?;

    entry.apply_retention_policy(retention_policy.clone(), retention_deadline);
    sync_manifest_lifecycle_if_present(&entry.manifest_path, &entry.lifecycle)?;
    registry.save(home)?;

    let entry = registry
        .find(corpus_id)
        .ok_or_else(|| anyhow::anyhow!("Corpus not found after retention update: {corpus_id}"))?;

    sync_corpus_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    let message = match retention_policy {
        CorpusRetentionPolicy::RetainUntilManualCleanup => {
            "Updated corpus retention to manual cleanup.".to_string()
        }
        CorpusRetentionPolicy::RetainForDuration => format!(
            "Updated corpus retention to expire at {}.",
            entry
                .lifecycle
                .retention_deadline
                .as_deref()
                .unwrap_or("unknown")
        ),
        CorpusRetentionPolicy::EphemeralImmediate => {
            "Updated corpus retention to ephemeral immediate.".to_string()
        }
    };

    Ok(build_corpus_lifecycle_action_response(
        home, entry, &message,
    ))
}

fn reconcile_registry_corpus(home: &str, corpus_id: &str) -> Result<CorpusLifecycleActionResponse> {
    let mut registry = CorpusRegistry::load(home)?;
    let message = {
        let entry = registry
            .find_mut(corpus_id)
            .ok_or_else(|| anyhow::anyhow!("Corpus not found: {corpus_id}"))?;

        let root_exists = FsPath::new(&entry.root_path).exists();
        let mut report_exists = FsPath::new(&entry.report_path).exists();
        let archived_report_path = archived_corpus_report_path(home, corpus_id);
        let recovered_archived_report = !report_exists && archived_report_path.exists();

        if recovered_archived_report {
            entry.report_path = archived_report_path.display().to_string();
            report_exists = true;
        }

        let manifest_exists = FsPath::new(&entry.manifest_path).exists();
        let can_restore_ready_state = entry.lifecycle.state
            == crate::corpus::CorpusLifecycleState::Orphaned
            && root_exists
            && manifest_exists
            && report_exists;

        if entry.lifecycle.state == crate::corpus::CorpusLifecycleState::CleanupSucceeded
            && !root_exists
        {
            if recovered_archived_report {
                entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
                entry.lifecycle.state_note = Some(
                    "Manual reconciliation confirmed that lifecycle cleanup had already succeeded, the corpus root is gone, and the registry was relinked to the archived corpus report."
                        .to_string(),
                );
                "Registry matched a cleaned-up corpus and manual reconciliation relinked the archived report."
                    .to_string()
            } else {
                entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
                entry.lifecycle.state_note = Some(
                    "Manual reconciliation confirmed that lifecycle cleanup had already succeeded and the corpus root is gone."
                        .to_string(),
                );
                "Registry matches a cleaned-up corpus. Root directory is gone and lifecycle cleanup succeeded."
                    .to_string()
            }
        } else if !root_exists
            && entry.lifecycle.state != crate::corpus::CorpusLifecycleState::CleanupSucceeded
            && entry.lifecycle.state != crate::corpus::CorpusLifecycleState::CleanupFailed
        {
            entry.mark_orphaned_with_note(
                "Manual reconciliation found that the corpus root is missing even though successful lifecycle cleanup was never recorded. The corpus was marked orphaned for review."
                    .to_string(),
            );
            "Corpus root is missing even though cleanup was not recorded as successful. Marked corpus as orphaned."
                .to_string()
        } else if root_exists
            && entry.lifecycle.state == crate::corpus::CorpusLifecycleState::CleanupSucceeded
        {
            entry.mark_orphaned_with_note(
                "Manual reconciliation found that the corpus root still exists even though cleanup had been recorded as successful. The corpus was marked orphaned for review."
                    .to_string(),
            );
            "Corpus root still exists even though cleanup was previously recorded as successful. Marked corpus as orphaned for investigation."
                .to_string()
        } else if !report_exists
            && entry.lifecycle.state != crate::corpus::CorpusLifecycleState::CleanupSucceeded
            && entry.lifecycle.state != crate::corpus::CorpusLifecycleState::CleanupFailed
        {
            entry.mark_orphaned_with_note(
                "Manual reconciliation found that the retained corpus report is missing even though successful cleanup was never recorded. The corpus was marked orphaned for review."
                    .to_string(),
            );
            "Corpus report is missing while cleanup was not recorded as successful. Marked corpus as orphaned."
                .to_string()
        } else if recovered_archived_report && can_restore_ready_state {
            entry.mark_ready_with_note(
                "Manual reconciliation confirmed that the retained corpus artifacts are present again, relinked the registry entry to the archived report path, and restored the lifecycle state to ready."
                    .to_string(),
            );
            "Manual reconciliation relinked the retained corpus to the archived report path and restored a healthy ready lifecycle state."
                .to_string()
        } else if can_restore_ready_state {
            entry.mark_ready_with_note(
                "Manual reconciliation confirmed that the retained corpus artifacts are present and restored the lifecycle state to ready."
                    .to_string(),
            );
            "Manual reconciliation restored a healthy ready lifecycle state for this retained corpus."
                .to_string()
        } else if recovered_archived_report {
            entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
            entry.lifecycle.state_note = Some(
                "Manual reconciliation found that the retained corpus report had moved to the archived report path and relinked the registry entry."
                    .to_string(),
            );
            "Manual reconciliation relinked the retained corpus to the archived report path."
                .to_string()
        } else {
            entry.lifecycle.updated_at = Some(chrono::Utc::now().to_rfc3339());
            entry.lifecycle.state_note = Some(
                "Manual reconciliation confirmed that the corpus registry entry still matches the corpus artifacts on disk."
                    .to_string(),
            );
            "Registry paths are present and no corpus reconciliation changes were needed."
                .to_string()
        }
    };

    if let Some(entry) = registry.find(corpus_id) {
        sync_manifest_lifecycle_if_present(&entry.manifest_path, &entry.lifecycle)?;
    }
    registry.save(home)?;

    let entry = registry
        .find(corpus_id)
        .ok_or_else(|| anyhow::anyhow!("Corpus not found after reconciliation: {corpus_id}"))?;

    sync_corpus_report_lifecycle(FsPath::new(&entry.report_path), &entry.lifecycle)?;

    Ok(build_corpus_lifecycle_action_response(
        home, entry, &message,
    ))
}

fn archive_corpus_report_if_present(
    home: &str,
    corpus_id: &str,
    current_report_path: &str,
) -> Result<Option<SanitizationOperation>> {
    let source = FsPath::new(current_report_path);

    if !source.exists() {
        return Ok(None);
    }

    ensure_corpus_registry_dirs(home)?;

    let archived_path = archived_corpus_report_path(home, corpus_id);
    fs::copy(source, &archived_path)?;

    Ok(Some(SanitizationOperation {
        operation: "corpus_lifecycle_report_archive".to_string(),
        status: "successful".to_string(),
        details: format!(
            "Archived corpus report before cleanup to {}.",
            archived_path.display()
        ),
    }))
}

fn maybe_archived_corpus_report_path(
    home: &str,
    corpus_id: &str,
    previous_report_path: &str,
) -> Result<Option<String>> {
    let archived_path = archived_corpus_report_path(home, corpus_id);

    if archived_path.exists() {
        return Ok(Some(archived_path.display().to_string()));
    }

    if FsPath::new(previous_report_path).exists() {
        return Ok(Some(previous_report_path.to_string()));
    }

    Ok(None)
}

fn session_report_availability(
    home: &str,
    entry: &SessionIndexEntry,
) -> (bool, bool, String, Option<String>) {
    let availability = resolve_session_report_availability(home, entry);
    (
        availability.current_exists,
        availability.available,
        availability.storage.to_string(),
        availability
            .loadable_path
            .map(|path| path.display().to_string()),
    )
}

fn corpus_report_availability(
    home: &str,
    entry: &CorpusIndexEntry,
) -> (bool, bool, String, Option<String>) {
    let availability = resolve_corpus_report_availability(home, entry);
    (
        availability.current_exists,
        availability.available,
        availability.storage.to_string(),
        availability
            .loadable_path
            .map(|path| path.display().to_string()),
    )
}

fn sync_manifest_lifecycle_if_present(
    manifest_path: &str,
    lifecycle: &crate::corpus::CorpusLifecycleMetadata,
) -> Result<()> {
    let path = FsPath::new(manifest_path);

    if !path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(path)?;
    let mut manifest: crate::corpus::CorpusManifest = serde_json::from_str(&raw)?;
    manifest.lifecycle = lifecycle.clone();
    fs::write(path, serde_json::to_string_pretty(&manifest)?)?;

    Ok(())
}

fn build_corpus_lifecycle_action_response(
    home: &str,
    entry: &CorpusIndexEntry,
    message: &str,
) -> CorpusLifecycleActionResponse {
    let (report_exists, report_available, report_storage, loadable_report_path) =
        corpus_report_availability(home, entry);

    CorpusLifecycleActionResponse {
        corpus_id: entry.corpus_id.clone(),
        lifecycle_state: entry.lifecycle.state.as_str().to_string(),
        retention_policy: entry.lifecycle.retention_policy.as_str().to_string(),
        retention_deadline: entry.lifecycle.retention_deadline.clone(),
        cleanup_reason: entry
            .lifecycle
            .cleanup_reason
            .as_ref()
            .map(|reason| reason.as_str().to_string()),
        cleanup_requested_at: entry.lifecycle.cleanup_requested_at.clone(),
        cleanup_completed_at: entry.lifecycle.cleanup_completed_at.clone(),
        state_note: entry.lifecycle.state_note.clone(),
        updated_at: entry.lifecycle.updated_at.clone(),
        root_exists: FsPath::new(&entry.root_path).exists(),
        manifest_exists: FsPath::new(&entry.manifest_path).exists(),
        report_exists,
        report_available,
        report_storage,
        loadable_report_path,
        root_path: entry.root_path.clone(),
        manifest_path: entry.manifest_path.clone(),
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

fn resolve_bind_addr() -> Result<SocketAddr> {
    let port = env::var("NULLCONTEXT_PORT")
        .ok()
        .map(|value| value.parse::<u16>())
        .transpose()
        .map_err(|error| anyhow::anyhow!("Invalid NULLCONTEXT_PORT value: {error}"))?
        .unwrap_or(3333);

    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}

async fn bind_listener(addr: SocketAddr) -> Result<tokio::net::TcpListener> {
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            anyhow::bail!(
                "Failed to bind NullContext web server to http://{addr}: address already in use. Stop the existing listener or set NULLCONTEXT_PORT to a different localhost port and retry."
            )
        }
        Err(error) => Err(error.into()),
    }
}

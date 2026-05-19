mod audit;
mod cleanup;
mod config;
mod inference;
mod memory_scan;
mod registry;
mod runtime;
mod sensitive;
mod session;

use anyhow::Result;
use audit::PrivacyReport;
use cleanup::{
    cleanup_ephemeral_workspace, log_sanitization_operation, scan_artifacts, CleanupReport,
    SanitizationOperation,
};
use config::{AppCommand, SessionConfig};
use inference::run_inference;
use memory_scan::{buffer_contains_pattern, verify_buffer_zeroization};
use registry::{list_sessions, register_persistent_session, show_report};
use session::Session;

fn main() -> Result<()> {
    match AppCommand::from_env()? {
        AppCommand::Run(config) => run_session(config),
        AppCommand::ListSessions => {
            let home = if let Ok(home) = std::env::var("HOME") {
                home
            } else {
                std::env::var("USERPROFILE")?
            };
            list_sessions(&home)
        }
        AppCommand::ShowReport { session_id } => {
            let home = if let Ok(home) = std::env::var("HOME") {
                home
            } else {
                std::env::var("USERPROFILE")?
            };
            show_report(&home, &session_id)
        }
    }
}

fn run_session(mut config: SessionConfig) -> Result<()> {
    let session = Session::create()?;

    println!("Starting NullContext session...");
    println!("Session ID: {}", session.id);
    println!("Workspace: {}", session.workspace.display());
    println!("Security mode: {}", config.security_mode.as_str());
    println!("Prompt source: {}", config.prompt_source.as_str());

    session.write_prompt(config.prompt.as_bytes())?;

    let mut inference_result = run_inference(&config)?;

    session.write_response(inference_result.response.as_bytes())?;

    println!("\n--- Model Output ---\n");
    println!("{}", inference_result.response.as_str());

    let prompt_probe = config.prompt.as_bytes().to_vec();
    let response_probe = inference_result.response.as_bytes().to_vec();

    let prompt_found_before = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);

    let response_found_before =
        buffer_contains_pattern(inference_result.response.as_bytes(), &response_probe);

    let (artifacts_detected, scan_operation) = scan_artifacts(&session.workspace)?;

    log_sanitization_operation(&scan_operation);

    let mut sanitization_operations = vec![scan_operation];

    for operation in &inference_result.sanitization_operations {
        log_sanitization_operation(operation);
    }

    sanitization_operations.append(&mut inference_result.sanitization_operations);

    let prompt_ingest_operation = SanitizationOperation {
        operation: "prompt_ingest_channel".to_string(),
        status: "recorded".to_string(),
        details: format!(
            "Prompt was provided via '{}'. Use --stdin to avoid shell history and process argv exposure.",
            config.prompt_source.as_str()
        ),
    };

    log_sanitization_operation(&prompt_ingest_operation);
    sanitization_operations.push(prompt_ingest_operation);

    println!("\nSanitizing Rust-owned buffers...");

    config.prompt.sanitize();
    inference_result.response.sanitize();

    let prompt_found_after = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);

    let response_found_after =
        buffer_contains_pattern(inference_result.response.as_bytes(), &response_probe);

    let prompt_verification_operation =
        verify_buffer_zeroization("prompt_buffer", prompt_found_before, prompt_found_after);

    log_sanitization_operation(&prompt_verification_operation);
    sanitization_operations.push(prompt_verification_operation);

    let response_verification_operation = verify_buffer_zeroization(
        "response_buffer",
        response_found_before,
        response_found_after,
    );

    log_sanitization_operation(&response_verification_operation);
    sanitization_operations.push(response_verification_operation);

    let explicit_zeroization_operation = SanitizationOperation {
        operation: "explicit_sensitive_byte_buffer_zeroization".to_string(),
        status: "successful".to_string(),
        details: "Explicitly overwrote Rust-owned prompt and response byte buffers before drop."
            .to_string(),
    };

    log_sanitization_operation(&explicit_zeroization_operation);
    sanitization_operations.push(explicit_zeroization_operation);

    let cleanup_report = if config.ephemeral {
        println!("\nSession mode: ephemeral");
        println!("Detected {} workspace artifacts.", artifacts_detected.len());
        println!("Cleaning up workspace...");

        cleanup_ephemeral_workspace(
            &session.workspace,
            artifacts_detected,
            sanitization_operations,
        )
    } else {
        println!("\nSession mode: persistent");
        println!("Detected {} workspace artifacts.", artifacts_detected.len());
        println!("Workspace retained at:");
        println!("{}", session.workspace.display());

        CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
    };

    let report = PrivacyReport::new(
        session.id.clone(),
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        config.gpu_layers.clone(),
        inference_result.process_exited_cleanly,
        cleanup_report.clone(),
    );

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;

        register_persistent_session(&config.home, &session, &config, &cleanup_report)?;
    }

    println!("\n--- Privacy Report v0 ---");
    println!("{}", report_json);

    Ok(())
}

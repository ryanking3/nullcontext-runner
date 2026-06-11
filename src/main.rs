mod audit;
mod chat;
mod cleanup;
mod config;
mod corpus;
mod corpus_registry;
mod cuda_pressure;
mod docs;
mod embed;
mod gpu_inspection;
mod inference;
mod llama_stream;
mod logging;
mod memory_scan;
mod memory_validation;
mod process_scan;
mod ram_pressure;
mod registry;
mod retrieval;
mod runtime;
mod runtime_capabilities;
mod runtime_introspection;
mod sensitive;
mod session;
mod validation_harness;
mod web;

use crate::logging::stdout_line;
use anyhow::Result;
use audit::{build_failed_launch_llama_runtime_report, build_llama_runtime_report, PrivacyReport};
use cleanup::{
    cleanup_ephemeral_workspace, log_sanitization_operation, scan_artifacts, CleanupReport,
    SanitizationOperation,
};
use config::{AppCommand, SessionConfig};
use inference::run_inference;
use memory_scan::{buffer_contains_pattern, verify_buffer_zeroization};
use process_scan::{build_process_scan_report, scan_failed_start_cleanup_phase, ProcessScanMarker};
use registry::{list_sessions, register_persistent_session, show_report, SessionLifecycleMetadata};
use runtime::RuntimeLaunchFailure;
use session::Session;
use validation_harness::run_controlled_canary_validation;

fn main() -> Result<()> {
    match AppCommand::from_env()? {
        AppCommand::Run(config) => run_session(config),
        AppCommand::ListSessions => {
            let home = home_dir()?;
            list_sessions(&home)
        }
        AppCommand::ShowReport { session_id } => {
            let home = home_dir()?;
            show_report(&home, &session_id)
        }
        AppCommand::Serve => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(web::serve())
        }
    }
}

fn run_session(mut config: SessionConfig) -> Result<()> {
    let session = Session::create()?;

    stdout_line("Starting NullContext session...");
    stdout_line(format!("Session ID: {}", session.id));
    stdout_line(format!("Workspace: {}", session.workspace.display()));
    stdout_line(format!("Security mode: {}", config.security_mode.as_str()));
    stdout_line(format!(
        "Model: {} ({})",
        config.model_name, config.model_id
    ));
    stdout_line(format!("Model path: {}", config.model_path));
    stdout_line(format!("Prompt source: {}", config.prompt_source.as_str()));

    session.write_prompt(config.prompt.as_bytes())?;

    let prompt_probe = config.prompt.as_bytes().to_vec();
    let prompt_found_before = buffer_contains_pattern(config.prompt.as_bytes(), &prompt_probe);

    let mut inference_result = match run_inference(&config) {
        Ok(result) => result,
        Err(error) => {
            if let Some(failure) = error.downcast_ref::<RuntimeLaunchFailure>() {
                finalize_failed_startup_session(
                    &session,
                    &mut config,
                    &prompt_probe,
                    prompt_found_before,
                    failure,
                )?;
            }

            return Err(error);
        }
    };

    session.write_response(inference_result.response.as_bytes())?;

    stdout_line("\n--- Model Output ---\n");
    stdout_line(inference_result.response.as_str());

    let response_probe = inference_result.response.as_bytes().to_vec();

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

    stdout_line("\nSanitizing Rust-owned buffers...");

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
        stdout_line("\nSession mode: ephemeral");
        stdout_line(format!(
            "Detected {} workspace artifacts.",
            artifacts_detected.len()
        ));
        stdout_line("Cleaning up workspace...");

        cleanup_ephemeral_workspace(
            &session.workspace,
            artifacts_detected,
            sanitization_operations,
        )
    } else {
        stdout_line("\nSession mode: persistent");
        stdout_line(format!(
            "Detected {} workspace artifacts.",
            artifacts_detected.len()
        ));
        stdout_line("Workspace retained at:");
        stdout_line(session.workspace.display());

        CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
    };

    let lifecycle = SessionLifecycleMetadata::for_completed_session(&config, &cleanup_report);

    let report = PrivacyReport::new(
        session.id.clone(),
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        config.gpu_layers.clone(),
        inference_result.runtime_shutdown.stopped,
        cleanup_report.clone(),
    )
    .with_lifecycle(&lifecycle)
    .with_process_scan(inference_result.process_scan.clone())
    .with_llama_runtime(build_llama_runtime_report(
        &config,
        Some(inference_result.runtime_pid),
        Some(&inference_result.runtime_endpoint),
        &inference_result.runtime_shutdown,
        &inference_result.runtime_usage,
        &inference_result.post_shutdown_observation,
    ))
    .with_controlled_canary_run(run_controlled_canary_validation(&config));

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;

        register_persistent_session(&config.home, &session, &config, &cleanup_report)?;
    }

    stdout_line("\n--- Privacy Report v0 ---");
    stdout_line(&report_json);

    Ok(())
}

fn finalize_failed_startup_session(
    session: &Session,
    config: &mut SessionConfig,
    prompt_probe: &[u8],
    prompt_found_before: bool,
    failure: &RuntimeLaunchFailure,
) -> Result<()> {
    stdout_line("\nRuntime failed before becoming healthy.");

    let (artifacts_detected, scan_operation) = scan_artifacts(&session.workspace)?;
    log_sanitization_operation(&scan_operation);

    let mut sanitization_operations = vec![scan_operation];

    let startup_failure_operation = SanitizationOperation {
        operation: "managed_runtime_startup".to_string(),
        status: "failed".to_string(),
        details: failure.to_string(),
    };
    log_sanitization_operation(&startup_failure_operation);
    sanitization_operations.push(startup_failure_operation);

    stdout_line("\nSanitizing Rust-owned prompt buffer...");
    config.prompt.sanitize();

    let prompt_found_after = buffer_contains_pattern(config.prompt.as_bytes(), prompt_probe);
    let prompt_verification_operation =
        verify_buffer_zeroization("prompt_buffer", prompt_found_before, prompt_found_after);
    log_sanitization_operation(&prompt_verification_operation);
    sanitization_operations.push(prompt_verification_operation);

    let explicit_zeroization_operation = SanitizationOperation {
        operation: "explicit_sensitive_byte_buffer_zeroization".to_string(),
        status: "successful".to_string(),
        details: "Explicitly overwrote the Rust-owned prompt byte buffer before drop after startup failure."
            .to_string(),
    };
    log_sanitization_operation(&explicit_zeroization_operation);
    sanitization_operations.push(explicit_zeroization_operation);

    let cleanup_report = if config.ephemeral {
        stdout_line("\nSession mode: ephemeral");
        stdout_line(format!(
            "Detected {} workspace artifacts after startup failure.",
            artifacts_detected.len()
        ));
        stdout_line("Cleaning up workspace...");

        cleanup_ephemeral_workspace(
            &session.workspace,
            artifacts_detected,
            sanitization_operations,
        )
    } else {
        stdout_line("\nSession mode: persistent");
        stdout_line(format!(
            "Detected {} workspace artifacts after startup failure.",
            artifacts_detected.len()
        ));
        stdout_line("Workspace retained at:");
        stdout_line(session.workspace.display());

        CleanupReport::not_attempted(artifacts_detected, sanitization_operations)
    };

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

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;
        register_persistent_session(&config.home, session, config, &cleanup_report)?;
    }

    stdout_line("\n--- Privacy Report v0 ---");
    stdout_line(&report_json);

    Ok(())
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

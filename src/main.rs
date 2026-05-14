mod audit;
mod cleanup;
mod config;
mod inference;
mod runtime;
mod session;

use anyhow::Result;
use audit::PrivacyReport;
use cleanup::{cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation};
use config::SessionConfig;
use inference::run_inference;
use session::Session;

fn main() -> Result<()> {
    let config = SessionConfig::from_env()?;
    let session = Session::create()?;

    println!("Starting NullContext session...");
    println!("Session ID: {}", session.id);
    println!("Workspace: {}", session.workspace.display());
    println!("Security mode: {}", config.security_mode.as_str());

    session.write_prompt(&config.prompt)?;

    let inference_result = run_inference(&config)?;

    session.write_response(&inference_result.response)?;

    println!("\n--- Model Output ---\n");
    println!("{}", inference_result.response.as_str());

    let (artifacts_detected, scan_operation) = scan_artifacts(&session.workspace)?;

    let mut sanitization_operations = vec![scan_operation];

    sanitization_operations.push(SanitizationOperation {
        operation: "rust_owned_prompt_response_zeroize".to_string(),
        status: "scheduled".to_string(),
        details: "Rust-owned prompt and response buffers use zeroize-on-drop. This does not sanitize llama.cpp internal memory, OS swap, or shell history.".to_string(),
    });

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
        cleanup_report,
    );

    let report_json = report.to_pretty_json()?;

    if !config.ephemeral {
        session.write_report(&report_json)?;
    }

    println!("\n--- Privacy Report v0 ---");
    println!("{}", report_json);

    Ok(())
}

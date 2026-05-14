mod audit;
mod cleanup;
mod config;
mod inference;
mod runtime;
mod sensitive;
mod session;

use anyhow::Result;
use audit::PrivacyReport;
use cleanup::{cleanup_ephemeral_workspace, scan_artifacts, CleanupReport, SanitizationOperation};
use config::SessionConfig;
use inference::run_inference;
use session::Session;

fn main() -> Result<()> {
    let mut config = SessionConfig::from_env()?;

    let session = Session::create()?;

    println!("Starting NullContext session...");
    println!("Session ID: {}", session.id);
    println!("Workspace: {}", session.workspace.display());
    println!("Security mode: {}", config.security_mode.as_str());

    session.write_prompt(config.prompt.as_str())?;

    let mut inference_result = run_inference(&config)?;

    session.write_response(inference_result.response.as_str())?;

    println!("\n--- Model Output ---\n");
    println!("{}", inference_result.response.as_str());

    let (artifacts_detected, scan_operation) = scan_artifacts(&session.workspace)?;

    let mut sanitization_operations = vec![scan_operation];

    sanitization_operations.append(&mut inference_result.sanitization_operations);

    println!("\nSanitizing Rust-owned buffers...");

    config.prompt.sanitize();
    inference_result.response.sanitize();

    sanitization_operations.push(SanitizationOperation {
        operation: "explicit_rust_buffer_zeroization".to_string(),
        status: "successful".to_string(),
        details: "Explicitly overwrote Rust-owned prompt and response buffers before drop."
            .to_string(),
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

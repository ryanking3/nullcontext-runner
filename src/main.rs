mod audit;
mod cleanup;
mod config;
mod inference;
mod runtime;
mod session;

use anyhow::Result;
use audit::PrivacyReport;
use cleanup::cleanup_ephemeral_workspace;
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
    println!("{}", inference_result.response);

    let workspace_deleted = if config.ephemeral {
        false
    } else {
        println!("\nSession mode: persistent");
        println!("Workspace retained at:");
        println!("{}", session.workspace.display());

        false
    };

    let report = PrivacyReport::new(
        session.id.clone(),
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        config.gpu_layers,
        inference_result.process_exited_cleanly,
        workspace_deleted,
    );

    let report_json = report.to_pretty_json()?;

    session.write_report(&report_json)?;

    let workspace_deleted = if config.ephemeral {
        println!("\nSession mode: ephemeral");
        println!("Writing report before cleanup...");
        println!("Cleaning up workspace...");

        cleanup_ephemeral_workspace(&session.workspace)?
    } else {
        workspace_deleted
    };

    let final_report = PrivacyReport::new(
        session.id,
        session.started_at,
        !config.ephemeral,
        "llama-server".to_string(),
        config.security_mode.as_str().to_string(),
        "0".to_string(),
        inference_result.process_exited_cleanly,
        workspace_deleted,
    );

    println!("\n--- Privacy Report v0 ---");
    println!("{}", final_report.to_pretty_json()?);

    Ok(())
}

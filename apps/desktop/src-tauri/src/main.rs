use serde::Serialize;
use std::process::{Command, Stdio};

#[derive(Debug, Serialize)]
struct CommandResponse {
    stdout: String,
    stderr: String,
    success: bool,
}

#[tauri::command]
fn run_nullcontext_session(
    prompt: String,
    mode: String,
    persistent: bool,
) -> Result<CommandResponse, String> {
    let repo_root = repo_root()?;
    let binary_path = repo_root.join("target/debug/nullcontext-runner");

    let mut command = Command::new(binary_path);

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

    let mut child = command.spawn().map_err(|error| error.to_string())?;

    {
        use std::io::Write;

        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "Failed to open child stdin".to_string())?;

        stdin
            .write_all(prompt.as_bytes())
            .map_err(|error| error.to_string())?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;

    Ok(CommandResponse {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    })
}

#[tauri::command]
fn list_nullcontext_sessions() -> Result<CommandResponse, String> {
    let repo_root = repo_root()?;
    let binary_path = repo_root.join("target/debug/nullcontext-runner");

    let output = Command::new(binary_path)
        .arg("--list-sessions")
        .output()
        .map_err(|error| error.to_string())?;

    Ok(CommandResponse {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    })
}

#[tauri::command]
fn show_nullcontext_report(session_id: String) -> Result<CommandResponse, String> {
    let repo_root = repo_root()?;
    let binary_path = repo_root.join("target/debug/nullcontext-runner");

    let output = Command::new(binary_path)
        .arg("--show-report")
        .arg(session_id)
        .output()
        .map_err(|error| error.to_string())?;

    Ok(CommandResponse {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    })
}

fn repo_root() -> Result<std::path::PathBuf, String> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map(|path| path.to_path_buf())
        .ok_or_else(|| "Failed to resolve repository root".to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            run_nullcontext_session,
            list_nullcontext_sessions,
            show_nullcontext_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
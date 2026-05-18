use serde::Serialize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
struct StreamEvent {
    stream: String,
    chunk: String,
}

#[derive(Debug, Clone, Serialize)]
struct CommandResponse {
    stdout: String,
    stderr: String,
    success: bool,
}

#[tauri::command]
fn run_nullcontext_session_streaming(
    app: tauri::AppHandle,
    prompt: String,
    mode: String,
    persistent: bool,
) -> Result<(), String> {
    thread::spawn(move || {
        if let Err(error) = run_streaming_process(app.clone(), prompt, mode, persistent) {
            let _ = app.emit(
                "nullcontext://stream-error",
                StreamEvent {
                    stream: "error".to_string(),
                    chunk: error,
                },
            );
        }
    });

    Ok(())
}

fn run_streaming_process(
    app: tauri::AppHandle,
    prompt: String,
    mode: String,
    persistent: bool,
) -> Result<(), String> {
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
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "Failed to open child stdin".to_string())?;

        stdin
            .write_all(prompt.as_bytes())
            .map_err(|error| error.to_string())?;
    }

    drop(child.stdin.take());

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture child stdout".to_string())?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture child stderr".to_string())?;

    let stdout_app = app.clone();
    let stdout_thread = thread::spawn(move || {
        stream_reader(stdout, stdout_app, "stdout");
    });

    let stderr_app = app.clone();
    let stderr_thread = thread::spawn(move || {
        stream_reader(stderr, stderr_app, "stderr");
    });

    let status = child.wait().map_err(|error| error.to_string())?;

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    app.emit(
        "nullcontext://stream-complete",
        CommandResponse {
            stdout: String::new(),
            stderr: String::new(),
            success: status.success(),
        },
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

fn stream_reader<R: Read>(mut reader: R, app: tauri::AppHandle, stream_name: &str) {
    let mut buffer = [0u8; 512];

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => {
                let chunk = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();

                let _ = app.emit(
                    "nullcontext://stream-chunk",
                    StreamEvent {
                        stream: stream_name.to_string(),
                        chunk,
                    },
                );
            }
            Err(error) => {
                let _ = app.emit(
                    "nullcontext://stream-error",
                    StreamEvent {
                        stream: stream_name.to_string(),
                        chunk: format!("Failed reading stream: {error}"),
                    },
                );

                break;
            }
        }
    }
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
            run_nullcontext_session_streaming,
            list_nullcontext_sessions,
            show_nullcontext_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
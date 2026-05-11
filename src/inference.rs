use crate::config::SessionConfig;
use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug)]
pub struct InferenceResult {
    pub stdout: String,
    pub stderr: String,
    pub process_exited_cleanly: bool,
}

pub fn run_inference(config: &SessionConfig) -> Result<InferenceResult> {
    let output = Command::new(&config.llama_path)
        .arg("-m")
        .arg(&config.model_path)
        .arg("-p")
        .arg(&config.prompt)
        .arg("-n")
        .arg(&config.max_tokens)
        .arg("-ngl")
        .arg(&config.gpu_layers)
        .arg("-st")
        .output()
        .with_context(|| format!("Failed to run llama.cpp at {}", config.llama_path))?;

    Ok(InferenceResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        process_exited_cleanly: output.status.success(),
    })
}

use crate::config::SessionConfig;
use crate::runtime::ManagedRuntime;
use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct InferenceResult {
    pub response: String,
    pub process_exited_cleanly: bool,
}

#[derive(Serialize)]
struct CompletionRequest {
    prompt: String,
    n_predict: u32,
}

#[derive(Deserialize)]
struct CompletionResponse {
    content: String,
}

pub fn run_inference(config: &SessionConfig) -> Result<InferenceResult> {
    let mut runtime = ManagedRuntime::launch(config)?;

    println!("Running inference...");

    let response = send_completion_request(
        &runtime.completion_url(),
        &config.prompt,
        config.max_tokens.parse::<u32>()?,
    )?;

    let runtime_terminated = runtime.shutdown()?;

    Ok(InferenceResult {
        response,
        process_exited_cleanly: runtime_terminated,
    })
}

fn send_completion_request(completion_url: &str, prompt: &str, n_predict: u32) -> Result<String> {
    let client = Client::new();

    let response = client
        .post(completion_url)
        .json(&CompletionRequest {
            prompt: prompt.to_string(),
            n_predict,
        })
        .send()?
        .json::<CompletionResponse>()?;

    Ok(response.content)
}

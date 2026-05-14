use crate::cleanup::SanitizationOperation;
use crate::config::SessionConfig;
use crate::runtime::ManagedRuntime;
use crate::sensitive::SensitiveString;
use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[derive(Debug)]
pub struct InferenceResult {
    pub response: SensitiveString,
    pub process_exited_cleanly: bool,
    pub sanitization_operations: Vec<SanitizationOperation>,
}

#[derive(Serialize)]
struct CompletionRequest {
    prompt: String,
    n_predict: u32,
}

impl CompletionRequest {
    fn sanitize(&mut self) {
        self.prompt.zeroize();
    }
}

#[derive(Deserialize)]
struct CompletionResponse {
    content: String,
}

pub fn run_inference(config: &SessionConfig) -> Result<InferenceResult> {
    let mut runtime = ManagedRuntime::launch(config)?;

    println!("Running inference...");

    let (response, mut operations) = send_completion_request(
        &runtime.completion_url(),
        config.prompt.as_str(),
        config.max_tokens.parse::<u32>()?,
    )?;

    let runtime_terminated = runtime.shutdown()?;

    operations.push(SanitizationOperation {
        operation: "managed_runtime_shutdown".to_string(),
        status: if runtime_terminated {
            "successful".to_string()
        } else {
            "failed".to_string()
        },
        details: "llama-server child process was terminated after inference.".to_string(),
    });

    Ok(InferenceResult {
        response: SensitiveString::new(response),
        process_exited_cleanly: runtime_terminated,
        sanitization_operations: operations,
    })
}

fn send_completion_request(
    completion_url: &str,
    prompt: &str,
    n_predict: u32,
) -> Result<(String, Vec<SanitizationOperation>)> {
    let client = Client::new();

    let mut request = CompletionRequest {
        prompt: prompt.to_string(),
        n_predict,
    };

    let response = client
        .post(completion_url)
        .json(&request)
        .send()?
        .json::<CompletionResponse>()?;

    request.sanitize();

    let operations = vec![SanitizationOperation {
        operation: "http_request_prompt_buffer_zeroization".to_string(),
        status: "successful".to_string(),
        details: "Explicitly zeroized Rust-owned prompt copy used for llama-server HTTP request."
            .to_string(),
    }];

    Ok((response.content, operations))
}

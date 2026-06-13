use crate::audit::ProcessScanReport;
use crate::cleanup::SanitizationOperation;
use crate::config::SessionConfig;
use crate::logging::stdout_line;
use crate::process_scan::{
    build_process_scan_report, scan_live_process_phase, scan_post_shutdown_process_phase,
    ProcessScanMarker,
};
use crate::runtime::{
    observe_post_shutdown_with_stage_process_scan, ManagedRuntime, RuntimePostShutdownObservation,
    RuntimeProcessScanMarker, RuntimeShutdownOutcome, RuntimeUsageSnapshot,
};
use crate::sensitive::SensitiveBytes;
use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[derive(Debug)]
pub struct InferenceResult {
    pub response: SensitiveBytes,
    pub runtime_pid: u32,
    pub runtime_endpoint: String,
    pub runtime_shutdown: RuntimeShutdownOutcome,
    pub runtime_usage: RuntimeUsageSnapshot,
    pub post_shutdown_observation: RuntimePostShutdownObservation,
    pub process_scan: ProcessScanReport,
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
    let runtime_pid = runtime.pid();
    let runtime_endpoint = runtime.endpoint_url().to_string();

    stdout_line("Running inference...");

    let (response, mut operations) = send_completion_request(
        &runtime.completion_url(),
        config.prompt.as_str(),
        config.max_tokens.parse::<u32>()?,
    )?;

    let runtime_usage = runtime.observe_usage();
    let live_process_scan = scan_live_process_phase(
        runtime_pid,
        &[
            ProcessScanMarker {
                kind: "prompt_marker",
                bytes: config.prompt.as_bytes(),
            },
            ProcessScanMarker {
                kind: "response_marker",
                bytes: response.as_bytes(),
            },
        ],
    );
    let runtime_shutdown = runtime.shutdown()?;
    let stage_process_scan_markers = vec![
        RuntimeProcessScanMarker {
            kind: "prompt_marker".to_string(),
            bytes: config.prompt.as_bytes().to_vec(),
        },
        RuntimeProcessScanMarker {
            kind: "response_marker".to_string(),
            bytes: response.as_bytes().to_vec(),
        },
    ];
    let post_shutdown_observation = observe_post_shutdown_with_stage_process_scan(
        runtime_pid,
        config.gpu_layers.parse::<u32>().unwrap_or(0) > 0,
        Some(config),
        &stage_process_scan_markers,
    );
    let post_shutdown_process_scan = scan_post_shutdown_process_phase(
        runtime_pid,
        &post_shutdown_observation,
        &[
            ProcessScanMarker {
                kind: "prompt_marker",
                bytes: config.prompt.as_bytes(),
            },
            ProcessScanMarker {
                kind: "response_marker",
                bytes: response.as_bytes(),
            },
        ],
    );

    operations.push(SanitizationOperation {
        operation: "managed_runtime_shutdown".to_string(),
        status: if runtime_shutdown.stopped {
            "successful".to_string()
        } else {
            "failed".to_string()
        },
        details: format!(
            "llama-server child process shutdown completed using method {}. Exit code: {}.",
            runtime_shutdown.shutdown_method,
            runtime_shutdown
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
    });

    Ok(InferenceResult {
        response: SensitiveBytes::new(response),
        runtime_pid,
        runtime_endpoint,
        runtime_shutdown,
        runtime_usage,
        post_shutdown_observation,
        process_scan: build_process_scan_report(
            Some(runtime_pid),
            vec![live_process_scan, post_shutdown_process_scan],
        ),
        sanitization_operations: operations,
    })
}

fn send_completion_request(
    completion_url: &str,
    prompt: &str,
    n_predict: u32,
) -> Result<(String, Vec<SanitizationOperation>)> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

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

    let operations = vec![
        SanitizationOperation {
            operation: "sensitive_bytes_prompt_storage".to_string(),
            status: "successful".to_string(),
            details:
                "Application-owned prompt is stored in a zeroizing byte buffer instead of a long-lived String."
                    .to_string(),
        },
        SanitizationOperation {
            operation: "http_request_prompt_buffer_zeroization".to_string(),
            status: "successful".to_string(),
            details:
                "Explicitly zeroized temporary Rust-owned prompt copy used for llama-server HTTP request."
                    .to_string(),
        },
    ];

    Ok((response.content, operations))
}

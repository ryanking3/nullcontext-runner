use crate::audit::ControlledCanaryValidationRunReport;
use crate::config::SessionConfig;
use crate::process_scan::{
    build_process_scan_report, build_skipped_process_scan_report, scan_live_process_phase,
    scan_post_shutdown_process_phase, ProcessScanMarker,
};
use crate::runtime::{observe_post_shutdown_baseline, ManagedRuntime};
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Serialize)]
struct CanaryCompletionRequest {
    prompt: String,
    n_predict: u32,
}

impl CanaryCompletionRequest {
    fn sanitize(&mut self) {
        self.prompt.zeroize();
    }
}

#[derive(Deserialize)]
struct CanaryCompletionResponse {
    content: String,
}

pub fn run_controlled_canary_validation(
    config: &SessionConfig,
) -> ControlledCanaryValidationRunReport {
    let canary_id = format!("nc-canary-{}", Uuid::new_v4().simple());
    let canary_prompt = format!(
        "NullContext controlled memory-validation canary {canary_id}. Reply in one short sentence that includes the token NC-RESP-{canary_id} exactly once."
    );

    match run_controlled_canary_validation_impl(config, &canary_id, &canary_prompt) {
        Ok(report) => report,
        Err(error) => ControlledCanaryValidationRunReport {
            execution_status: "controlled_canary_helper_failed".to_string(),
            canary_id,
            runtime_pid: None,
            runtime_endpoint: None,
            response_bytes: None,
            summary:
                "NullContext attempted the controlled canary validation helper, but it did not complete successfully."
                    .to_string(),
            process_scan: build_skipped_process_scan_report(
                None,
                "The controlled canary helper did not complete, so no dedicated canary process scan report was produced.",
                "Without a completed controlled canary helper run, NullContext cannot compare dedicated canary marker persistence against the session cleanup evidence.",
                vec![format!("Controlled canary helper failed: {error}.")],
            ),
            notes: vec![
                "This failure only affects the dedicated validation helper. The main session report still reflects the user session's own cleanup evidence."
                    .to_string(),
            ],
        },
    }
}

fn run_controlled_canary_validation_impl(
    config: &SessionConfig,
    canary_id: &str,
    canary_prompt: &str,
) -> Result<ControlledCanaryValidationRunReport> {
    let mut runtime = ManagedRuntime::launch(config)
        .context("failed to launch helper runtime for controlled canary validation")?;
    let runtime_pid = runtime.pid();
    let runtime_endpoint = runtime.endpoint_url().to_string();

    let response = match send_canary_completion_request(
        &runtime.completion_url(),
        canary_prompt,
        config.max_tokens.parse::<u32>().unwrap_or(32).max(16),
    )
    .context("controlled canary completion request failed")
    {
        Ok(response) => response,
        Err(error) => {
            let shutdown_note = best_effort_shutdown_canary_runtime(&mut runtime);
            return Ok(ControlledCanaryValidationRunReport {
                execution_status: "controlled_canary_request_failed".to_string(),
                canary_id: canary_id.to_string(),
                runtime_pid: Some(runtime_pid),
                runtime_endpoint: Some(runtime_endpoint),
                response_bytes: None,
                summary:
                    "NullContext launched the controlled canary helper runtime, but the canary completion request failed before full scan evidence could be captured."
                        .to_string(),
                process_scan: build_skipped_process_scan_report(
                    Some(runtime_pid),
                    "The controlled canary helper runtime started, but the canary completion request failed before full marker scans could be completed.",
                    "Without a completed canary completion request, NullContext cannot compare dedicated canary prompt/response persistence for this helper run.",
                    vec![format!("Controlled canary request failed: {error}.")],
                ),
                notes: vec![shutdown_note],
            });
        }
    };

    let live_process_scan = scan_live_process_phase(
        runtime_pid,
        &[
            ProcessScanMarker {
                kind: "controlled_canary_prompt_marker",
                bytes: canary_prompt.as_bytes(),
            },
            ProcessScanMarker {
                kind: "controlled_canary_response_marker",
                bytes: response.as_bytes(),
            },
        ],
    );

    let shutdown = match runtime
        .shutdown()
        .context("failed to shut down controlled canary helper runtime")
    {
        Ok(shutdown) => shutdown,
        Err(error) => {
            return Ok(ControlledCanaryValidationRunReport {
                execution_status: "controlled_canary_shutdown_failed".to_string(),
                canary_id: canary_id.to_string(),
                runtime_pid: Some(runtime_pid),
                runtime_endpoint: Some(runtime_endpoint),
                response_bytes: Some(response.len()),
                summary:
                    "NullContext generated the controlled canary response and captured live scan evidence, but helper-runtime shutdown failed before post-shutdown validation completed."
                        .to_string(),
                process_scan: build_process_scan_report(Some(runtime_pid), vec![live_process_scan]),
                notes: vec![format!(
                    "Controlled canary helper shutdown failed after response generation: {error}."
                )],
            });
        }
    };
    let post_shutdown = observe_post_shutdown_baseline(
        runtime_pid,
        config.gpu_layers.parse::<u32>().unwrap_or(0) > 0,
    );
    let post_shutdown_process_scan = scan_post_shutdown_process_phase(
        runtime_pid,
        &post_shutdown,
        &[
            ProcessScanMarker {
                kind: "controlled_canary_prompt_marker",
                bytes: canary_prompt.as_bytes(),
            },
            ProcessScanMarker {
                kind: "controlled_canary_response_marker",
                bytes: response.as_bytes(),
            },
        ],
    );

    let process_scan = build_process_scan_report(
        Some(runtime_pid),
        vec![live_process_scan, post_shutdown_process_scan],
    );

    let mut notes = vec![
        format!(
            "Controlled canary helper runtime shut down using {} (exit code {}).",
            shutdown.shutdown_method,
            shutdown
                .exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        format!(
            "Controlled canary helper generated a response of {} bytes.",
            response.len()
        ),
        "The controlled canary helper runs after the main session ends and uses a fresh helper runtime with the same configured llama.cpp runtime/model settings."
            .to_string(),
    ];

    notes.extend(post_shutdown.observation_notes.clone());

    Ok(ControlledCanaryValidationRunReport {
        execution_status: "controlled_canary_completed".to_string(),
        canary_id: canary_id.to_string(),
        runtime_pid: Some(runtime_pid),
        runtime_endpoint: Some(runtime_endpoint),
        response_bytes: Some(response.len()),
        summary: format!(
            "NullContext ran a dedicated controlled canary helper runtime, scanned for prompt/response markers during live and post-shutdown phases, and recorded overall scan status {}.",
            process_scan.overall_status
        ),
        process_scan,
        notes,
    })
}

fn best_effort_shutdown_canary_runtime(runtime: &mut ManagedRuntime) -> String {
    match runtime.shutdown() {
        Ok(outcome) => format!(
            "NullContext still shut the controlled canary helper down using {} (exit code {}).",
            outcome.shutdown_method,
            outcome
                .exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        Err(error) => format!(
            "NullContext also failed to clean up the controlled canary helper runtime automatically: {error}."
        ),
    }
}

fn send_canary_completion_request(
    completion_url: &str,
    prompt: &str,
    n_predict: u32,
) -> Result<String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .context("failed to build controlled canary HTTP client")?;

    let mut request = CanaryCompletionRequest {
        prompt: prompt.to_string(),
        n_predict,
    };

    let response = client
        .post(completion_url)
        .json(&request)
        .send()
        .context("failed to send controlled canary completion request")?
        .error_for_status()
        .context("controlled canary helper returned an error status")?
        .json::<CanaryCompletionResponse>()
        .context("failed to decode controlled canary completion response")?;

    request.sanitize();

    Ok(response.content)
}

use crate::audit::{ControlledCanaryValidationPassReport, ControlledCanaryValidationRunReport};
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

const CONTROLLED_CANARY_PASS_COUNT: u32 = 3;

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
    let mut passes = Vec::new();

    for pass_index in 1..=CONTROLLED_CANARY_PASS_COUNT {
        passes.push(run_controlled_canary_validation_pass(config, pass_index));
    }

    build_controlled_canary_validation_run_report(passes)
}

fn run_controlled_canary_validation_pass(
    config: &SessionConfig,
    pass_index: u32,
) -> ControlledCanaryValidationPassReport {
    let canary_id = format!("nc-canary-p{}-{}", pass_index, Uuid::new_v4().simple());
    let canary_prompt = format!(
        "NullContext controlled memory-validation canary {canary_id}. Reply in one short sentence that includes the token NC-RESP-{canary_id} exactly once."
    );

    match run_controlled_canary_validation_pass_impl(config, pass_index, &canary_id, &canary_prompt)
    {
        Ok(report) => report,
        Err(error) => ControlledCanaryValidationPassReport {
            pass_index,
            execution_status: "controlled_canary_helper_failed".to_string(),
            canary_id,
            runtime_pid: None,
            runtime_endpoint: None,
            response_bytes: None,
            summary:
                "NullContext attempted this controlled canary validation pass, but it did not complete successfully."
                    .to_string(),
            process_scan: build_skipped_process_scan_report(
                None,
                "The controlled canary helper pass did not complete, so no dedicated canary process scan report was produced.",
                "Without a completed controlled canary helper pass, NullContext cannot compare dedicated canary marker persistence against the session cleanup evidence for this pass.",
                vec![format!(
                    "Controlled canary helper pass {} failed: {error}.",
                    pass_index
                )],
            ),
            notes: vec![
                "This failure only affects one dedicated validation pass. The main session report still reflects the user session's own cleanup evidence."
                    .to_string(),
            ],
        },
    }
}

fn run_controlled_canary_validation_pass_impl(
    config: &SessionConfig,
    pass_index: u32,
    canary_id: &str,
    canary_prompt: &str,
) -> Result<ControlledCanaryValidationPassReport> {
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
            return Ok(ControlledCanaryValidationPassReport {
                pass_index,
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
            return Ok(ControlledCanaryValidationPassReport {
                pass_index,
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
        format!("Controlled canary validation pass {pass_index} completed."),
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

    Ok(ControlledCanaryValidationPassReport {
        pass_index,
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

fn build_controlled_canary_validation_run_report(
    passes: Vec<ControlledCanaryValidationPassReport>,
) -> ControlledCanaryValidationRunReport {
    let requested_passes = CONTROLLED_CANARY_PASS_COUNT;
    let completed_passes = passes
        .iter()
        .filter(|pass| pass.execution_status == "controlled_canary_completed")
        .count() as u32;
    let failed_passes = requested_passes.saturating_sub(completed_passes);
    let aggregate_signal_status = aggregate_controlled_canary_signal_status(&passes);
    let aggregate_process_scan_status = aggregate_controlled_canary_process_scan_status(&passes);
    let selected_pass = select_representative_canary_pass(&passes);

    let (execution_status, summary, selection_reason) = if let Some(selected_pass) = selected_pass {
        (
            aggregate_controlled_canary_execution_status(&passes),
            format!(
                "NullContext ran {} controlled canary helper pass(es), completed {}, failed {}, and recorded aggregate signal status {}. Representative pass: {} with scan status {}.",
                requested_passes,
                completed_passes,
                failed_passes,
                aggregate_signal_status,
                selected_pass.pass_index,
                selected_pass.process_scan.overall_status
            ),
            representative_canary_selection_reason(selected_pass, &aggregate_signal_status),
        )
    } else {
        (
            "controlled_canary_not_run_yet".to_string(),
            "NullContext did not run any controlled canary helper passes for this report."
                .to_string(),
            "No representative controlled canary pass was selected because no passes were available."
                .to_string(),
        )
    };

    ControlledCanaryValidationRunReport {
        execution_status,
        requested_passes,
        completed_passes,
        failed_passes,
        aggregate_signal_status: aggregate_signal_status.clone(),
        aggregate_process_scan_status,
        canary_id: selected_pass
            .map(|pass| pass.canary_id.clone())
            .unwrap_or_else(|| "none".to_string()),
        selected_pass_index: selected_pass.map(|pass| pass.pass_index),
        selected_pass_canary_id: selected_pass.map(|pass| pass.canary_id.clone()),
        selection_reason,
        runtime_pid: selected_pass.and_then(|pass| pass.runtime_pid),
        runtime_endpoint: selected_pass.and_then(|pass| pass.runtime_endpoint.clone()),
        response_bytes: selected_pass.and_then(|pass| pass.response_bytes),
        summary,
        process_scan: selected_pass
            .map(|pass| pass.process_scan.clone())
            .unwrap_or_else(|| {
                build_skipped_process_scan_report(
                    None,
                    "No controlled canary helper passes were available, so no representative canary process scan report was selected.",
                    "Without any controlled canary helper passes, NullContext cannot compare dedicated canary marker persistence against the session cleanup evidence.",
                    vec!["No controlled canary helper passes were recorded.".to_string()],
                )
            }),
        passes,
        notes: controlled_canary_run_notes(&aggregate_signal_status),
    }
}

fn aggregate_controlled_canary_execution_status(
    passes: &[ControlledCanaryValidationPassReport],
) -> String {
    if passes.is_empty() {
        return "controlled_canary_not_run_yet".to_string();
    }

    if passes
        .iter()
        .any(|pass| pass.execution_status == "controlled_canary_completed")
    {
        if passes
            .iter()
            .all(|pass| pass.execution_status == "controlled_canary_completed")
        {
            "controlled_canary_completed".to_string()
        } else {
            "controlled_canary_completed_with_failures".to_string()
        }
    } else {
        "controlled_canary_all_passes_failed".to_string()
    }
}

fn aggregate_controlled_canary_signal_status(
    passes: &[ControlledCanaryValidationPassReport],
) -> String {
    if passes.is_empty() {
        return "controlled_canary_not_run_yet".to_string();
    }

    let statuses = passes
        .iter()
        .map(controlled_canary_pass_signal_status)
        .collect::<Vec<_>>();

    if statuses
        .iter()
        .any(|status| status == "controlled_canary_markers_detected")
    {
        return "controlled_canary_markers_detected_across_passes".to_string();
    }

    if statuses
        .iter()
        .all(|status| status == "controlled_canary_scan_clear_in_scanned_regions")
    {
        return "controlled_canary_all_completed_passes_clear".to_string();
    }

    if statuses.iter().any(|status| {
        status == "controlled_canary_scan_clear_in_scanned_regions"
            && statuses.iter().any(|other| other != status)
    }) {
        return "controlled_canary_mixed_clear_and_inconclusive".to_string();
    }

    if statuses.iter().all(|status| {
        status == "controlled_canary_scan_backend_unsupported"
            || status == "controlled_canary_not_run_yet"
    }) {
        return "controlled_canary_backend_unsupported_across_passes".to_string();
    }

    "controlled_canary_inconclusive_across_passes".to_string()
}

fn aggregate_controlled_canary_process_scan_status(
    passes: &[ControlledCanaryValidationPassReport],
) -> String {
    if passes.is_empty() {
        return "scan_not_completed".to_string();
    }

    if passes
        .iter()
        .any(|pass| pass.process_scan.overall_status == "markers_detected_in_scanned_memory")
    {
        return "markers_detected_in_scanned_memory".to_string();
    }

    if passes
        .iter()
        .all(|pass| pass.process_scan.overall_status == "no_markers_detected_in_scanned_regions")
    {
        return "no_markers_detected_in_scanned_regions".to_string();
    }

    if passes
        .iter()
        .all(|pass| pass.process_scan.overall_status == "scan_backend_unsupported_on_platform")
    {
        return "scan_backend_unsupported_on_platform".to_string();
    }

    "scan_attempt_incomplete".to_string()
}

fn select_representative_canary_pass(
    passes: &[ControlledCanaryValidationPassReport],
) -> Option<&ControlledCanaryValidationPassReport> {
    passes
        .iter()
        .max_by_key(|pass| representative_canary_pass_rank(pass))
}

fn representative_canary_pass_rank(pass: &ControlledCanaryValidationPassReport) -> (u8, u32) {
    (
        match controlled_canary_pass_signal_status(pass).as_str() {
            "controlled_canary_markers_detected" => 7,
            "controlled_canary_request_failed" => 6,
            "controlled_canary_shutdown_failed" => 5,
            "controlled_canary_scan_inconclusive" => 4,
            "controlled_canary_scan_not_completed" => 3,
            "controlled_canary_scan_backend_unsupported" => 2,
            "controlled_canary_scan_clear_in_scanned_regions" => 1,
            _ => 0,
        },
        u32::MAX.saturating_sub(pass.pass_index),
    )
}

fn representative_canary_selection_reason(
    pass: &ControlledCanaryValidationPassReport,
    aggregate_signal_status: &str,
) -> String {
    match aggregate_signal_status {
        "controlled_canary_markers_detected_across_passes" => {
            format!(
                "Pass {} was selected because it still detected controlled canary markers, which is the most security-relevant outcome across the repeated helper runs.",
                pass.pass_index
            )
        }
        "controlled_canary_all_completed_passes_clear" => {
            format!(
                "Pass {} was selected as a representative clear pass because every completed canary pass missed the markers in scanned readable regions.",
                pass.pass_index
            )
        }
        "controlled_canary_backend_unsupported_across_passes" => {
            format!(
                "Pass {} was selected as a representative unsupported-platform pass because every completed canary helper run hit the same direct process-scan backend limitation on this platform build.",
                pass.pass_index
            )
        }
        _ => format!(
            "Pass {} was selected as the most representative pessimistic pass for inspection because the repeated helper runs produced mixed or inconclusive outcomes.",
            pass.pass_index
        ),
    }
}

fn controlled_canary_run_notes(aggregate_signal_status: &str) -> Vec<String> {
    let aggregate_note = match aggregate_signal_status {
        "controlled_canary_markers_detected_across_passes" => {
            "Aggregate canary status is pessimistic: any pass that still detects markers outweighs cleaner passes."
                .to_string()
        }
        "controlled_canary_all_completed_passes_clear" => {
            "Aggregate canary status reflects repeated clear passes: every completed helper run missed its markers in scanned readable regions."
                .to_string()
        }
        "controlled_canary_backend_unsupported_across_passes" => {
            "Aggregate canary status reflects a platform limitation rather than mixed evidence: every completed helper run hit the same direct process-scan backend gap."
                .to_string()
        }
        _ => {
            "Aggregate canary status remains mixed or limited: repeated helper runs did not collapse into one uniformly clear or uniformly negative RAM-side result."
                .to_string()
        }
    };

    vec![
        aggregate_note,
        "The top-level process scan shown here is the representative pass selected for inspection, while the aggregate signal summarizes all passes."
            .to_string(),
    ]
}

fn controlled_canary_pass_signal_status(pass: &ControlledCanaryValidationPassReport) -> String {
    match pass.execution_status.as_str() {
        "controlled_canary_completed" => match pass.process_scan.overall_status.as_str() {
            "markers_detected_in_scanned_memory" => "controlled_canary_markers_detected",
            "no_markers_detected_in_scanned_regions" => {
                "controlled_canary_scan_clear_in_scanned_regions"
            }
            "scan_backend_unsupported_on_platform" => "controlled_canary_scan_backend_unsupported",
            "scan_attempt_failed" => "controlled_canary_scan_inconclusive",
            "scan_not_completed" => "controlled_canary_scan_not_completed",
            _ => "controlled_canary_scan_inconclusive",
        },
        "controlled_canary_request_failed" => "controlled_canary_request_failed",
        "controlled_canary_shutdown_failed" => "controlled_canary_shutdown_failed",
        "controlled_canary_helper_failed" => "controlled_canary_helper_failed",
        _ => "controlled_canary_inconclusive",
    }
    .to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_completed_pass(
        pass_index: u32,
        process_scan_overall_status: &str,
    ) -> ControlledCanaryValidationPassReport {
        let phase_status = match process_scan_overall_status {
            "scan_backend_unsupported_on_platform" => "scan_backend_unsupported_on_platform",
            "markers_detected_in_scanned_memory"
            | "no_markers_detected_in_scanned_regions" => "scan_completed",
            _ => "scan_attempt_failed",
        };
        let pattern_status = match process_scan_overall_status {
            "markers_detected_in_scanned_memory" => "detected_in_scanned_memory",
            "no_markers_detected_in_scanned_regions" => "not_detected_in_scanned_regions",
            _ => process_scan_overall_status,
        };
        let process_scan = build_process_scan_report(
            Some(1000 + pass_index),
            vec![crate::audit::ProcessScanPhaseReport {
                phase: "live_runtime".to_string(),
                status: phase_status.to_string(),
                method: "test".to_string(),
                target_pid: Some(1000 + pass_index),
                scope_summary: "test".to_string(),
                bytes_scanned: None,
                regions_scanned: None,
                regions_skipped: None,
                patterns: vec![crate::audit::ProcessScanPatternReport {
                    pattern_kind: "canary_marker".to_string(),
                    status: pattern_status.to_string(),
                    matches_found: None,
                    notes: "test".to_string(),
                }],
                notes: vec![],
            }],
        );

        ControlledCanaryValidationPassReport {
            pass_index,
            execution_status: "controlled_canary_completed".to_string(),
            canary_id: format!("canary-{pass_index}"),
            runtime_pid: Some(1000 + pass_index),
            runtime_endpoint: Some(format!("http://127.0.0.1:57{pass_index:03}")),
            response_bytes: Some(64),
            summary: "test".to_string(),
            process_scan,
            notes: vec![],
        }
    }

    #[test]
    fn unsupported_canary_runs_get_explicit_selection_reason() {
        let passes = vec![
            make_completed_pass(1, "scan_backend_unsupported_on_platform"),
            make_completed_pass(2, "scan_backend_unsupported_on_platform"),
            make_completed_pass(3, "scan_backend_unsupported_on_platform"),
        ];

        let report = build_controlled_canary_validation_run_report(passes);

        assert_eq!(
            report.aggregate_signal_status,
            "controlled_canary_backend_unsupported_across_passes"
        );
        assert!(
            report
                .selection_reason
                .contains("unsupported-platform pass"),
            "selection reason should describe the repeated unsupported backend case explicitly"
        );
        assert!(
            report
                .notes
                .first()
                .is_some_and(|note| note.contains("platform limitation")),
            "aggregate note should distinguish platform limitation from mixed evidence"
        );
    }

    #[test]
    fn clear_canary_runs_get_clear_aggregate_note() {
        let passes = vec![
            make_completed_pass(1, "no_markers_detected_in_scanned_regions"),
            make_completed_pass(2, "no_markers_detected_in_scanned_regions"),
            make_completed_pass(3, "no_markers_detected_in_scanned_regions"),
        ];

        let report = build_controlled_canary_validation_run_report(passes);

        assert_eq!(
            report.aggregate_signal_status,
            "controlled_canary_all_completed_passes_clear"
        );
        assert!(
            report
                .notes
                .first()
                .is_some_and(|note| note.contains("repeated clear passes")),
            "clear repeated runs should get a clear aggregate note instead of the generic pessimistic one"
        );
    }

    #[test]
    fn marker_detection_overrides_clear_passes_and_selects_detected_pass() {
        let passes = vec![
            make_completed_pass(1, "no_markers_detected_in_scanned_regions"),
            make_completed_pass(2, "markers_detected_in_scanned_memory"),
            make_completed_pass(3, "no_markers_detected_in_scanned_regions"),
        ];

        let report = build_controlled_canary_validation_run_report(passes);

        assert_eq!(
            report.aggregate_signal_status,
            "controlled_canary_markers_detected_across_passes"
        );
        assert_eq!(
            report.aggregate_process_scan_status,
            "markers_detected_in_scanned_memory"
        );
        assert_eq!(report.selected_pass_index, Some(2));
        assert!(report.selection_reason.contains("still detected controlled canary markers"));
        assert!(report
            .notes
            .first()
            .is_some_and(|note| note.contains("pessimistic")));
    }

    #[test]
    fn mixed_clear_and_unsupported_passes_remain_inconclusive() {
        let passes = vec![
            make_completed_pass(1, "no_markers_detected_in_scanned_regions"),
            make_completed_pass(2, "scan_backend_unsupported_on_platform"),
            make_completed_pass(3, "scan_backend_unsupported_on_platform"),
        ];

        let report = build_controlled_canary_validation_run_report(passes);

        assert_eq!(
            report.aggregate_signal_status,
            "controlled_canary_mixed_clear_and_inconclusive"
        );
        assert_eq!(report.aggregate_process_scan_status, "scan_attempt_incomplete");
        assert_eq!(report.selected_pass_index, Some(2));
        assert!(report.selection_reason.contains("mixed or inconclusive outcomes"));
    }
}

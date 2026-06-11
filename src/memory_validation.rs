use crate::audit::{
    ControlledCanaryValidationRunReport, MemoryValidationReport, MemoryValidationStageScorecard,
    PrivacyReport, ProcessScanReport, VramCleanupStrategyStageReport,
};

pub fn build_memory_validation_report(report: &PrivacyReport) -> MemoryValidationReport {
    let process_scan_signal_status =
        derive_process_scan_signal_status(report.process_scan.as_ref());
    let controlled_canary_run = report.memory_validation.controlled_canary_run.clone();
    let controlled_canary_signal_status =
        derive_controlled_canary_signal_status(&controlled_canary_run);

    let Some(llama_runtime) = report.llama_runtime.as_ref() else {
        return MemoryValidationReport {
            validation_status: "runtime_report_unavailable".to_string(),
            harness_scope: "session_evidence_scorecard".to_string(),
            canary_execution_status: controlled_canary_run.execution_status.clone(),
            process_scan_signal_status,
            best_stage_id: None,
            best_stage_label: None,
            best_stage_kind: None,
            best_stage_score: 0,
            best_stage_verdict: "runtime_report_unavailable".to_string(),
            summary:
                "NullContext could not derive a memory-validation scorecard because no llama runtime report was present."
                    .to_string(),
            controlled_canary_run,
            stage_scorecards: vec![],
            notes: vec![
                "This validation section scores runtime/process evidence and canary-helper status when available. A fuller Track E harness will later add repeated and more deeply instrumented canary execution."
                    .to_string(),
            ],
        };
    };

    if llama_runtime.vram_cleanup.stages.is_empty() {
        return MemoryValidationReport {
            validation_status: if llama_runtime.gpu_offload_requested {
                "stage_scoring_waiting_for_cleanup_stages".to_string()
            } else {
                "not_applicable_gpu_offload_not_requested".to_string()
            },
            harness_scope: "session_evidence_scorecard".to_string(),
            canary_execution_status: controlled_canary_run.execution_status.clone(),
            process_scan_signal_status: process_scan_signal_status.clone(),
            best_stage_id: None,
            best_stage_label: None,
            best_stage_kind: None,
            best_stage_score: 0,
            best_stage_verdict: if llama_runtime.gpu_offload_requested {
                "no_cleanup_stage_score_available".to_string()
            } else {
                "not_applicable".to_string()
            },
            summary: if llama_runtime.gpu_offload_requested {
                "NullContext recorded the validation scorecard contract, but this run did not produce experimental cleanup stages to score yet."
                    .to_string()
            } else {
                "NullContext did not score cleanup stages because GPU offload was not requested for this run."
                    .to_string()
            },
            controlled_canary_run,
            stage_scorecards: vec![],
            notes: vec![
                "This validation section can record dedicated canary-helper runs, but this specific report did not have cleanup stages available to score."
                    .to_string(),
            ],
        };
    }

    let stage_scorecards = llama_runtime
        .vram_cleanup
        .stages
        .iter()
        .map(|stage| {
            build_stage_scorecard(
                stage,
                &process_scan_signal_status,
                &controlled_canary_signal_status,
            )
        })
        .collect::<Vec<_>>();

    let best_stage = stage_scorecards
        .iter()
        .max_by_key(|scorecard| scorecard.validation_score)
        .cloned()
        .expect("stage scorecards should exist when cleanup stages exist");

    let validation_status = if process_scan_signal_status == "marker_persistence_detected"
        || controlled_canary_signal_status == "controlled_canary_markers_detected"
    {
        "stage_scoring_ready_marker_risk_still_present".to_string()
    } else {
        "stage_scoring_ready".to_string()
    };

    let summary = format!(
        "NullContext scored {} experimental cleanup stage(s) using session evidence plus controlled canary status {}. Best stage: {} with score {} and verdict {}.",
        stage_scorecards.len(),
        controlled_canary_run.execution_status,
        best_stage.stage_label,
        best_stage.validation_score,
        best_stage.validation_verdict
    );

    let mut notes = vec![
        "This validation harness slice combines session cleanup evidence with a dedicated helper-runtime canary run when available."
            .to_string(),
        "Stage scores are comparative operator guidance, not proof of full RAM or VRAM sanitization."
            .to_string(),
        process_scan_validation_note(&process_scan_signal_status),
        controlled_canary_validation_note(&controlled_canary_signal_status),
    ];

    if process_scan_signal_status == "marker_persistence_detected"
        || controlled_canary_signal_status == "controlled_canary_markers_detected"
    {
        notes.push(
            "Because direct scanning still detected configured markers either in the user session or the dedicated canary helper, even strong VRAM-stage scores should be treated as incomplete memory-clearing evidence."
                .to_string(),
        );
    }

    MemoryValidationReport {
        validation_status,
        harness_scope: "session_evidence_scorecard".to_string(),
        canary_execution_status: controlled_canary_run.execution_status.clone(),
        process_scan_signal_status,
        best_stage_id: Some(best_stage.stage_id.clone()),
        best_stage_label: Some(best_stage.stage_label.clone()),
        best_stage_kind: Some(best_stage.stage_kind.clone()),
        best_stage_score: best_stage.validation_score,
        best_stage_verdict: best_stage.validation_verdict.clone(),
        summary,
        controlled_canary_run,
        stage_scorecards,
        notes,
    }
}

fn derive_process_scan_signal_status(process_scan: Option<&ProcessScanReport>) -> String {
    match process_scan.map(|scan| scan.overall_status.as_str()) {
        Some("markers_detected_in_scanned_memory") => "marker_persistence_detected".to_string(),
        Some("no_markers_detected_in_scanned_regions") => {
            "marker_scan_clear_in_scanned_regions".to_string()
        }
        Some("scan_attempt_failed") => "marker_scan_inconclusive".to_string(),
        Some("scan_backend_unsupported_on_platform") => {
            "marker_scan_backend_unsupported".to_string()
        }
        Some("scan_skipped") | Some("scan_not_completed") => {
            "marker_scan_not_completed".to_string()
        }
        Some(_) => "marker_scan_context_mixed".to_string(),
        None => "process_scan_context_unavailable".to_string(),
    }
}

fn process_scan_validation_note(process_scan_signal_status: &str) -> String {
    match process_scan_signal_status {
        "marker_persistence_detected" => {
            "Direct process scanning detected configured markers in readable llama-server memory, which is a strong negative signal for this session."
                .to_string()
        }
        "marker_scan_clear_in_scanned_regions" => {
            "Direct process scanning did not find the configured markers in scanned readable regions. That is a useful positive signal, but not proof of full process-memory clearing."
                .to_string()
        }
        "marker_scan_inconclusive" => {
            "Direct process scanning was attempted but remained inconclusive, so the validation scorecard leans more heavily on runtime and VRAM evidence."
                .to_string()
        }
        "marker_scan_backend_unsupported" => {
            "This platform build does not yet provide the direct process-scan backend needed for stronger RAM-side validation."
                .to_string()
        }
        "marker_scan_not_completed" => {
            "No completed direct process scan was available for this run, so RAM-side validation evidence is limited."
                .to_string()
        }
        _ => {
            "No process-scan context was available, so this validation slice only scored the recorded cleanup-stage evidence."
                .to_string()
        }
    }
}

fn derive_controlled_canary_signal_status(
    controlled_canary_run: &ControlledCanaryValidationRunReport,
) -> String {
    match controlled_canary_run.execution_status.as_str() {
        "controlled_canary_completed" => {
            match derive_process_scan_signal_status(Some(&controlled_canary_run.process_scan))
                .as_str()
            {
                "marker_persistence_detected" => "controlled_canary_markers_detected".to_string(),
                "marker_scan_clear_in_scanned_regions" => {
                    "controlled_canary_scan_clear_in_scanned_regions".to_string()
                }
                "marker_scan_inconclusive" => "controlled_canary_scan_inconclusive".to_string(),
                "marker_scan_backend_unsupported" => {
                    "controlled_canary_scan_backend_unsupported".to_string()
                }
                "marker_scan_not_completed" => "controlled_canary_scan_not_completed".to_string(),
                _ => "controlled_canary_scan_context_mixed".to_string(),
            }
        }
        "controlled_canary_not_run_yet" => "controlled_canary_not_run_yet".to_string(),
        "controlled_canary_helper_failed" => "controlled_canary_helper_failed".to_string(),
        _ => "controlled_canary_inconclusive".to_string(),
    }
}

fn controlled_canary_validation_note(controlled_canary_signal_status: &str) -> String {
    match controlled_canary_signal_status {
        "controlled_canary_markers_detected" => {
            "The dedicated controlled canary helper still found its prompt or response markers in readable llama-server memory, which is a strong negative validation signal."
                .to_string()
        }
        "controlled_canary_scan_clear_in_scanned_regions" => {
            "The dedicated controlled canary helper did not find its markers in scanned readable regions. That is a stronger validation signal than passive session evidence alone."
                .to_string()
        }
        "controlled_canary_scan_backend_unsupported" => {
            "The dedicated controlled canary helper ran, but direct process-scan support is still missing on this platform build."
                .to_string()
        }
        "controlled_canary_helper_failed" => {
            "The dedicated controlled canary helper did not complete successfully, so validation still relies primarily on passive session evidence."
                .to_string()
        }
        "controlled_canary_not_run_yet" => {
            "No dedicated controlled canary helper run was available for this report."
                .to_string()
        }
        _ => {
            "The dedicated controlled canary helper completed with limited or inconclusive direct-scan evidence."
                .to_string()
        }
    }
}

fn build_stage_scorecard(
    stage: &VramCleanupStrategyStageReport,
    process_scan_signal_status: &str,
    controlled_canary_signal_status: &str,
) -> MemoryValidationStageScorecard {
    let mut score = 0_u32;
    let mut strengths = Vec::new();
    let mut gaps = Vec::new();

    match stage.evidence_improvement_status.as_str() {
        "evidence_improved_pid_no_longer_observed_after_strategy" => {
            score += 55;
            strengths.push(
                "The stage recheck no longer observed a matching GPU PID, which is the strongest current post-shutdown GPU signal."
                    .to_string(),
            );
        }
        "evidence_unchanged_not_observed" => {
            score += 48;
            strengths.push(
                "Neither baseline nor stage recheck observed a matching GPU PID.".to_string(),
            );
        }
        "evidence_improved_bytes_no_longer_visible_but_pid_still_observed" => {
            score += 36;
            strengths.push(
                "The stage recheck still observed the GPU PID, but per-process GPU memory bytes were no longer visible."
                    .to_string(),
            );
        }
        "evidence_improved_peak_bytes_lower_but_residency_still_observed" => {
            score += 28;
            strengths.push(
                "The stage recheck still observed GPU residency, but peak GPU bytes were lower than baseline."
                    .to_string(),
            );
        }
        "evidence_unchanged_pid_still_observed" => {
            score += 14;
            gaps.push(
                "GPU residency was still observed with no meaningful improvement over baseline."
                    .to_string(),
            );
        }
        "evidence_worsened_peak_bytes_higher_after_strategy" => {
            score += 4;
            gaps.push(
                "The stage reported higher peak GPU byte visibility than baseline.".to_string(),
            );
        }
        "evidence_worsened_gpu_visibility_increased_after_strategy" => {
            score += 2;
            gaps.push("The stage surfaced more GPU visibility than baseline.".to_string());
        }
        _ => {
            score += 6;
            gaps.push(
                "The recorded GPU evidence for this stage remained inconclusive or visibility-limited."
                    .to_string(),
            );
        }
    }

    if stage.action_status.contains("completed") {
        score += 10;
        strengths.push("The cleanup-stage action completed without being skipped.".to_string());
    } else if stage.action_status.contains("warning") {
        score += 4;
        gaps.push("The cleanup-stage action completed with warnings.".to_string());
    } else if stage.action_status.contains("failed") || stage.action_status.contains("unavailable")
    {
        gaps.push(
            "The cleanup-stage action failed or was unavailable, which weakens the evidence value of this stage."
                .to_string(),
        );
    }

    if stage.evidence_snapshot.gpu_samples_with_pid_observed == 0 {
        score += 12;
        strengths.push("The recheck window collected zero GPU-positive samples.".to_string());
    } else if stage.evidence_snapshot.gpu_samples_with_pid_observed <= 1 {
        score += 6;
        strengths.push("The recheck window observed at most one GPU-positive sample.".to_string());
    } else {
        gaps.push(format!(
            "The recheck window still observed {} GPU-positive sample(s).",
            stage.evidence_snapshot.gpu_samples_with_pid_observed
        ));
    }

    if stage.evidence_snapshot.gpu_peak_memory_bytes.is_none() {
        score += 8;
        strengths.push(
            "The stage snapshot did not expose per-process peak GPU memory bytes.".to_string(),
        );
    } else {
        gaps.push("The stage snapshot still exposed peak GPU memory bytes.".to_string());
    }

    match process_scan_signal_status {
        "marker_scan_clear_in_scanned_regions" => {
            score += 10;
            strengths.push(
                "Direct process scanning did not find configured markers in scanned readable regions."
                    .to_string(),
            );
        }
        "marker_persistence_detected" => {
            gaps.push(
                "Direct process scanning still detected configured markers in readable llama-server memory."
                    .to_string(),
            );
        }
        "marker_scan_inconclusive"
        | "marker_scan_backend_unsupported"
        | "marker_scan_not_completed"
        | "process_scan_context_unavailable"
        | "marker_scan_context_mixed" => {
            gaps.push(
                "Direct process-scan evidence was limited, incomplete, or unavailable for this run."
                    .to_string(),
            );
        }
        _ => {}
    }

    match controlled_canary_signal_status {
        "controlled_canary_scan_clear_in_scanned_regions" => {
            score += 12;
            strengths.push(
                "The dedicated controlled canary helper did not find its markers in scanned readable regions."
                    .to_string(),
            );
        }
        "controlled_canary_markers_detected" => {
            gaps.push(
                "The dedicated controlled canary helper still detected its markers in readable llama-server memory."
                    .to_string(),
            );
        }
        "controlled_canary_scan_backend_unsupported"
        | "controlled_canary_scan_not_completed"
        | "controlled_canary_helper_failed"
        | "controlled_canary_inconclusive"
        | "controlled_canary_not_run_yet" => {
            gaps.push(
                "Dedicated controlled canary evidence was limited, unavailable, or inconclusive for this report."
                    .to_string(),
            );
        }
        _ => {}
    }

    let validation_score = score.min(100);
    let validation_verdict = validation_verdict_for_score(validation_score);
    let summary = format!(
        "{} scored {}/100 with verdict {}.",
        stage.stage_label, validation_score, validation_verdict
    );

    MemoryValidationStageScorecard {
        stage_id: stage.stage_id.clone(),
        stage_label: stage.stage_label.clone(),
        stage_kind: stage.stage_kind.clone(),
        action_status: stage.action_status.clone(),
        vram_evidence_status: stage.evidence_improvement_status.clone(),
        process_scan_context_status: process_scan_signal_status.to_string(),
        controlled_canary_signal_status: controlled_canary_signal_status.to_string(),
        validation_score,
        validation_verdict,
        summary,
        strengths,
        gaps,
    }
}

fn validation_verdict_for_score(score: u32) -> String {
    match score {
        80..=100 => "strong_improvement_signal".to_string(),
        60..=79 => "moderate_improvement_signal".to_string(),
        40..=59 => "mixed_signal".to_string(),
        20..=39 => "limited_signal".to_string(),
        _ => "negative_or_inconclusive_signal".to_string(),
    }
}

use crate::audit::{
    MemoryValidationReport, MemoryValidationStageScorecard, PrivacyReport, ProcessScanReport,
    VramCleanupStrategyStageReport,
};
use crate::process_scan::{
    process_scan_signal_status_from_phase, process_scan_signal_status_from_report,
};

pub fn build_memory_validation_report(report: &PrivacyReport) -> MemoryValidationReport {
    let process_scan_signal_status =
        derive_process_scan_signal_status(report.process_scan.as_ref());
    let controlled_canary_run = report.memory_validation.controlled_canary_run.clone();
    let controlled_canary_signal_status = controlled_canary_run.aggregate_signal_status.clone();

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
        || controlled_canary_signal_status == "controlled_canary_markers_detected_across_passes"
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
        || controlled_canary_signal_status == "controlled_canary_markers_detected_across_passes"
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
    process_scan
        .map(process_scan_signal_status_from_report)
        .unwrap_or_else(|| "process_scan_context_unavailable".to_string())
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
        "marker_scan_process_not_observable_after_cleanup" => {
            "The runtime PID was no longer observable after cleanup, so NullContext had no remaining process target for a direct post-cleanup marker scan. That is useful shutdown evidence, but not the same as a clear direct marker miss."
                .to_string()
        }
        _ => {
            "No process-scan context was available, so this validation slice only scored the recorded cleanup-stage evidence."
                .to_string()
        }
    }
}

fn controlled_canary_validation_note(controlled_canary_signal_status: &str) -> String {
    match controlled_canary_signal_status {
        "controlled_canary_markers_detected_across_passes" => {
            "At least one dedicated controlled canary helper pass still found its prompt or response markers in readable llama-server memory, which is a strong negative validation signal."
                .to_string()
        }
        "controlled_canary_all_completed_passes_clear" => {
            "Every completed dedicated controlled canary helper pass missed its markers in scanned readable regions. That is a stronger validation signal than passive session evidence alone."
                .to_string()
        }
        "controlled_canary_backend_unsupported_across_passes" => {
            "The dedicated controlled canary helper passes ran, but direct process-scan support is still missing on this platform build."
                .to_string()
        }
        "controlled_canary_completed_with_failures"
        | "controlled_canary_mixed_clear_and_inconclusive"
        | "controlled_canary_inconclusive_across_passes"
        | "controlled_canary_request_failed"
        | "controlled_canary_shutdown_failed"
        | "controlled_canary_helper_failed"
        | "controlled_canary_all_passes_failed" => {
            "The repeated dedicated controlled canary helper passes produced mixed, failed, or inconclusive outcomes, so validation still relies heavily on passive session evidence."
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
    fallback_process_scan_signal_status: &str,
    controlled_canary_signal_status: &str,
) -> MemoryValidationStageScorecard {
    let mut score = 0_u32;
    let mut strengths = Vec::new();
    let mut gaps = Vec::new();
    let (process_scan_signal_status, process_scan_context_scope) =
        if let Some(report) = stage.helper_process_scan_report.as_ref() {
            (
                derive_process_scan_signal_status_from_report(report),
                "stage_local_helper_scan".to_string(),
            )
        } else if let Some(phase) = stage.process_scan_phase.as_ref() {
            (
                derive_process_scan_signal_status_from_phase(phase),
                "stage_local_cleanup_phase".to_string(),
            )
        } else if fallback_process_scan_signal_status != "process_scan_context_unavailable" {
            (
                fallback_process_scan_signal_status.to_string(),
                "session_fallback".to_string(),
            )
        } else {
            (
                fallback_process_scan_signal_status.to_string(),
                "process_scan_context_unavailable".to_string(),
            )
        };
    let cleanup_signal_support_status = stage.cleanup_signal_support_status.clone();
    let cleanup_signal_support_summary = stage.cleanup_signal_support_summary.clone();
    let cleanup_signal_support_scope_status = stage.cleanup_signal_support_scope_status.clone();
    let cleanup_signal_support_scope_summary = stage.cleanup_signal_support_scope_summary.clone();
    let contributing_cleanup_signals = stage.contributing_cleanup_signals.clone();

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

    match stage.marker_evidence_status.as_str() {
        "gpu_evidence_supported_by_clear_session_and_canary_scans" => {
            score += 18;
            strengths.push(
                "This stage's GPU outcome is reinforced by both clear session scanning and clear repeated canary passes."
                    .to_string(),
            );
        }
        "gpu_evidence_supported_by_partial_marker_clearance" => {
            score += 9;
            strengths.push(
                "This stage's GPU outcome has at least some supporting RAM-side marker-clearance evidence."
                    .to_string(),
            );
        }
        "gpu_evidence_improved_but_marker_persistence_detected" => {
            gaps.push(
                "GPU visibility improved for this stage, but marker-persistence evidence still remained negative."
                    .to_string(),
            );
        }
        "marker_persistence_detected_without_supporting_gpu_improvement" => {
            gaps.push(
                "This stage lacked both a clean GPU improvement signal and a clean marker-persistence outcome."
                    .to_string(),
            );
        }
        "gpu_evidence_without_clear_marker_confirmation"
        | "marker_evidence_context_mixed"
        | "marker_evidence_not_yet_contextualized" => {
            gaps.push(
                "This stage's GPU outcome was not backed by strong clear-marker confirmation."
                    .to_string(),
            );
        }
        _ => {
            gaps.push(
                "This stage recorded an unrecognized marker-evidence context, so RAM-side confirmation should be treated cautiously."
                    .to_string(),
            );
        }
    }

    match process_scan_signal_status.as_str() {
        "marker_persistence_detected" => gaps.push(
            "Direct process scanning still detected configured markers in readable llama-server memory."
                .to_string(),
        ),
        "marker_scan_process_not_observable_after_cleanup" => strengths.push(
            "After cleanup, the runtime PID was no longer observable, so no direct post-cleanup process target remained. That is a useful shutdown signal, but weaker than a clear marker miss in scanned memory."
                .to_string(),
        ),
        "marker_scan_inconclusive"
        | "marker_scan_backend_unsupported"
        | "marker_scan_not_completed"
        | "process_scan_context_unavailable"
        | "marker_scan_context_mixed" => gaps.push(
            "Direct process-scan evidence remained limited, incomplete, or unavailable."
                .to_string(),
        ),
        _ => {}
    }

    match process_scan_context_scope.as_str() {
        "stage_local_helper_scan" => strengths.push(
            "This stage carried its own helper-stage direct process scan instead of relying only on session-level fallback context."
                .to_string(),
        ),
        "stage_local_cleanup_phase" => strengths.push(
            "This stage carried its own cleanup-phase direct process scan context instead of relying only on session-level fallback context."
                .to_string(),
        ),
        "session_fallback" => gaps.push(
            "This stage did not have truly stage-local process-scan evidence, so its RAM-side context still relied on the broader session scan."
                .to_string(),
        ),
        _ => {}
    }

    match cleanup_signal_support_status.as_str() {
        "cleanup_signal_support_strong" => {
            score += 8;
            strengths.push(
                "Allocator reset, KV clear, and model unload cleanup signals were all observed for this runtime, which gives the cleanup interpretation stronger llama-internal support."
                    .to_string(),
            );
        }
        "cleanup_signal_support_partial" => {
            score += 3;
            strengths.push(
                "At least part of the allocator/KV/model cleanup path was observed directly, so the cleanup interpretation is stronger than host-tool evidence alone."
                    .to_string(),
            );
        }
        "cleanup_signal_support_declared_but_unobserved" => gaps.push(
            "This runtime declared allocator/KV cleanup-signal support, but this run did not directly observe those cleanup signals."
                .to_string(),
        ),
        "cleanup_signal_support_startup_failed_or_unavailable"
        | "cleanup_signal_support_unavailable" => gaps.push(
            "Allocator/KV cleanup-path evidence remained unavailable, so this stage still leans heavily on host-tool and process-scan evidence."
                .to_string(),
        ),
        _ => gaps.push(
            "Allocator/KV cleanup-path evidence remained limited or mixed for this stage."
                .to_string(),
        ),
    }

    match cleanup_signal_support_scope_status.as_str() {
        "cleanup_signal_scope_stage_local_helper_runtime" => strengths.push(
            "Allocator/KV cleanup evidence for this stage came from a stage-local helper runtime probe rather than only from the main runtime lifecycle."
                .to_string(),
        ),
        "cleanup_signal_scope_stage_local_helper_declared_but_not_observed" => gaps.push(
            "Allocator/KV cleanup evidence for this stage only reached stage-local helper-runtime declaration scope and was not directly observed during the helper probe."
                .to_string(),
        ),
        "cleanup_signal_scope_stage_local_helper_unavailable_due_to_startup_failure"
        | "cleanup_signal_scope_stage_local_helper_unavailable" => gaps.push(
            "A stage-local helper runtime probe existed for this stage, but NullContext still could not derive trustworthy internal cleanup scope from it."
                .to_string(),
        ),
        "cleanup_signal_scope_runtime_global_only" => gaps.push(
            "Allocator/KV cleanup evidence for this stage still comes from whole-runtime lifecycle observation rather than a stage-local internal cleanup hook."
                .to_string(),
        ),
        "cleanup_signal_scope_declared_but_not_observed" => gaps.push(
            "Allocator/KV cleanup evidence for this stage is still only declared by the runtime path and was not directly observed in this run."
                .to_string(),
        ),
        "cleanup_signal_scope_unavailable_due_to_startup_failure"
        | "cleanup_signal_scope_unavailable" => gaps.push(
            "NullContext could not derive trustworthy stage-specific allocator/KV cleanup scope for this stage."
                .to_string(),
        ),
        _ => {}
    }

    if !contributing_cleanup_signals.is_empty() {
        strengths.push(format!(
            "The internal cleanup interpretation for this stage was informed by these observed runtime cleanup signals: {}.",
            contributing_cleanup_signals.join(", ")
        ));
    }

    match controlled_canary_signal_status {
        "controlled_canary_markers_detected_across_passes" => gaps.push(
            "At least one dedicated controlled canary helper pass still detected its markers in readable llama-server memory."
                .to_string(),
        ),
        "controlled_canary_backend_unsupported_across_passes"
        | "controlled_canary_mixed_clear_and_inconclusive"
        | "controlled_canary_inconclusive_across_passes"
        | "controlled_canary_completed_with_failures"
        | "controlled_canary_request_failed"
        | "controlled_canary_shutdown_failed"
        | "controlled_canary_helper_failed"
        | "controlled_canary_all_passes_failed"
        | "controlled_canary_not_run_yet" => gaps.push(
            "Repeated dedicated controlled canary evidence was limited, unavailable, or inconclusive for this report."
                .to_string(),
        ),
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
        marker_evidence_status: stage.marker_evidence_status.clone(),
        process_scan_context_status: process_scan_signal_status,
        process_scan_context_scope,
        cleanup_signal_support_status,
        cleanup_signal_support_summary,
        cleanup_signal_support_scope_status,
        cleanup_signal_support_scope_summary,
        contributing_cleanup_signals,
        controlled_canary_signal_status: controlled_canary_signal_status.to_string(),
        validation_score,
        validation_verdict,
        summary,
        strengths,
        gaps,
    }
}

fn derive_process_scan_signal_status_from_report(report: &ProcessScanReport) -> String {
    process_scan_signal_status_from_report(report)
}

fn derive_process_scan_signal_status_from_phase(
    phase: &crate::audit::ProcessScanPhaseReport,
) -> String {
    process_scan_signal_status_from_phase(phase)
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

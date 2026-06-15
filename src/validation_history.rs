use crate::audit::{
    MemoryValidationHistoryReport, MemoryValidationStageRecommendationReport,
    MemoryValidationStageTrendReport, PrivacyReport,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationHistoryRegistry {
    #[serde(default)]
    pub entries: Vec<ValidationHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationHistoryEntry {
    pub session_id: String,
    pub recorded_at: String,
    pub started_at: String,
    pub scope_key: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub platform: Option<String>,
    pub gpu_offload_requested: Option<bool>,
    pub validation_status: String,
    pub process_scan_signal_status: String,
    pub canary_execution_status: String,
    pub canary_aggregate_signal_status: String,
    pub best_stage_score: u32,
    pub best_stage_verdict: String,
    #[serde(default)]
    pub stage_results: Vec<ValidationHistoryStageResultEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationHistoryStageResultEntry {
    pub stage_id: String,
    pub stage_label: String,
    pub stage_kind: String,
    pub vram_evidence_status: String,
    pub validation_score: u32,
    pub validation_verdict: String,
    pub marker_evidence_status: String,
    pub helper_process_scan_status: String,
}

pub fn apply_and_record_memory_validation_history(
    home: &str,
    mut report: PrivacyReport,
) -> PrivacyReport {
    match load_registry(home) {
        Ok(mut registry) => {
            let entry = ValidationHistoryEntry::from_report(&report);
            registry.upsert(entry);
            let history_report = build_history_report_from_registry(&registry, &report);
            report = report.with_memory_validation_history(history_report);

            if let Err(error) = save_registry(home, &registry) {
                report.memory_validation_history.history_status =
                    "history_persistence_failed".to_string();
                report.memory_validation_history.notes.push(format!(
                    "NullContext derived this cross-session validation summary, but failed to persist the updated validation-history registry: {error}."
                ));
            }

            report
        }
        Err(error) => {
            let scope_key = history_scope_key(&report);
            let scope_model_id = report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.model_id.clone());
            let scope_platform = history_scope_platform(&report);
            let scope_gpu_offload_requested = report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.gpu_offload_requested);

            report.with_memory_validation_history(MemoryValidationHistoryReport {
                history_status: "history_registry_unavailable".to_string(),
                scope_key,
                scope_model_id,
                scope_platform,
                scope_gpu_offload_requested,
                runs_recorded: 0,
                marker_detection_runs: 0,
                clear_canary_runs: 0,
                inconclusive_or_failed_runs: 0,
                strong_or_moderate_runs: 0,
                best_stage_score_min: None,
                best_stage_score_max: None,
                best_stage_score_avg: None,
                last_recorded_at: None,
                stage_trends: vec![],
                cleanup_stage_recommendation:
                    default_stage_recommendation_report("history_registry_unavailable"),
                summary:
                    "NullContext could not load the cross-session validation-history registry for this report."
                        .to_string(),
                notes: vec![format!(
                    "Validation-history registry load failed: {error}."
                )],
            })
        }
    }
}

impl ValidationHistoryRegistry {
    fn upsert(&mut self, entry: ValidationHistoryEntry) {
        self.entries
            .retain(|existing| existing.session_id != entry.session_id);
        self.entries.push(entry);
        self.entries
            .sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
    }
}

impl ValidationHistoryEntry {
    fn from_report(report: &PrivacyReport) -> Self {
        Self {
            session_id: report.session_id.clone(),
            recorded_at: Utc::now().to_rfc3339(),
            started_at: report.started_at.to_rfc3339(),
            scope_key: history_scope_key(report),
            model_id: report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.model_id.clone()),
            model_name: report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.model_name.clone()),
            platform: history_scope_platform(report),
            gpu_offload_requested: report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.gpu_offload_requested),
            validation_status: report.memory_validation.validation_status.clone(),
            process_scan_signal_status: report.memory_validation.process_scan_signal_status.clone(),
            canary_execution_status: report.memory_validation.canary_execution_status.clone(),
            canary_aggregate_signal_status: report
                .memory_validation
                .controlled_canary_run
                .aggregate_signal_status
                .clone(),
            best_stage_score: report.memory_validation.best_stage_score,
            best_stage_verdict: report.memory_validation.best_stage_verdict.clone(),
            stage_results: report
                .memory_validation
                .stage_scorecards
                .iter()
                .map(|scorecard| ValidationHistoryStageResultEntry {
                    stage_id: scorecard.stage_id.clone(),
                    stage_label: scorecard.stage_label.clone(),
                    stage_kind: scorecard.stage_kind.clone(),
                    vram_evidence_status: scorecard.vram_evidence_status.clone(),
                    validation_score: scorecard.validation_score,
                    validation_verdict: scorecard.validation_verdict.clone(),
                    marker_evidence_status: scorecard.marker_evidence_status.clone(),
                    helper_process_scan_status: report
                        .llama_runtime
                        .as_ref()
                        .and_then(|runtime| {
                            runtime
                                .vram_cleanup
                                .stages
                                .iter()
                                .find(|stage| stage.stage_id == scorecard.stage_id)
                                .and_then(|stage| stage.helper_process_scan_report.as_ref())
                        })
                        .map(|scan| helper_process_scan_signal_status(scan.overall_status.as_str()))
                        .unwrap_or_else(|| "helper_process_scan_not_recorded".to_string()),
                })
                .collect(),
        }
    }
}

fn build_history_report_from_registry(
    registry: &ValidationHistoryRegistry,
    report: &PrivacyReport,
) -> MemoryValidationHistoryReport {
    let scope_key = history_scope_key(report);
    let matching_entries = registry
        .entries
        .iter()
        .filter(|entry| entry.scope_key == scope_key)
        .collect::<Vec<_>>();

    if matching_entries.is_empty() {
        return MemoryValidationHistoryReport {
            history_status: "history_scope_empty".to_string(),
            scope_key,
            scope_model_id: report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.model_id.clone()),
            scope_platform: history_scope_platform(report),
            scope_gpu_offload_requested: report
                .llama_runtime
                .as_ref()
                .map(|runtime| runtime.gpu_offload_requested),
            runs_recorded: 0,
            marker_detection_runs: 0,
            clear_canary_runs: 0,
            inconclusive_or_failed_runs: 0,
            strong_or_moderate_runs: 0,
            best_stage_score_min: None,
            best_stage_score_max: None,
            best_stage_score_avg: None,
            last_recorded_at: None,
            stage_trends: vec![],
            cleanup_stage_recommendation: default_stage_recommendation_report(
                "history_scope_empty",
            ),
            summary:
                "NullContext defined the cross-session validation-history scope, but this is the first recorded run in that scope."
                    .to_string(),
            notes: vec![],
        };
    }

    let runs_recorded = matching_entries.len() as u32;
    let marker_detection_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.process_scan_signal_status == "marker_persistence_detected"
                || entry.canary_aggregate_signal_status
                    == "controlled_canary_markers_detected_across_passes"
        })
        .count() as u32;
    let clear_canary_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.canary_aggregate_signal_status == "controlled_canary_all_completed_passes_clear"
        })
        .count() as u32;
    let inconclusive_or_failed_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.validation_status != "stage_scoring_ready"
                || entry.canary_execution_status != "controlled_canary_completed"
        })
        .count() as u32;
    let strong_or_moderate_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.best_stage_verdict == "strong_improvement_signal"
                || entry.best_stage_verdict == "moderate_improvement_signal"
        })
        .count() as u32;

    let best_stage_score_min = matching_entries
        .iter()
        .map(|entry| entry.best_stage_score)
        .min();
    let best_stage_score_max = matching_entries
        .iter()
        .map(|entry| entry.best_stage_score)
        .max();
    let best_stage_score_avg = if matching_entries.is_empty() {
        None
    } else {
        Some(
            matching_entries
                .iter()
                .map(|entry| entry.best_stage_score as f64)
                .sum::<f64>()
                / matching_entries.len() as f64,
        )
    };
    let stage_trends = build_stage_trends(&matching_entries);
    let cleanup_stage_recommendation = build_stage_recommendation(&stage_trends);
    let recommendation_summary = cleanup_stage_recommendation.summary.clone();

    MemoryValidationHistoryReport {
        history_status: "history_recorded".to_string(),
        scope_key,
        scope_model_id: report
            .llama_runtime
            .as_ref()
            .map(|runtime| runtime.model_id.clone()),
        scope_platform: history_scope_platform(report),
        scope_gpu_offload_requested: report
            .llama_runtime
            .as_ref()
            .map(|runtime| runtime.gpu_offload_requested),
        runs_recorded,
        marker_detection_runs,
        clear_canary_runs,
        inconclusive_or_failed_runs,
        strong_or_moderate_runs,
        best_stage_score_min,
        best_stage_score_max,
        best_stage_score_avg,
        last_recorded_at: matching_entries.first().map(|entry| entry.recorded_at.clone()),
        stage_trends,
        cleanup_stage_recommendation,
        summary: format!(
            "NullContext has recorded {} validation run(s) for scope {}. {} run(s) still showed marker-detection evidence, {} run(s) achieved fully clear repeated canary passes, and the best-stage score average is {}. {}",
            runs_recorded,
            history_scope_label(report),
            marker_detection_runs,
            clear_canary_runs,
            best_stage_score_avg
                .map(|value| format!("{value:.1}/100"))
                .unwrap_or_else(|| "unavailable".to_string()),
            recommendation_summary
        ),
        notes: vec![
            "This scope groups validation history by model id, host platform, and whether GPU offload was requested."
                .to_string(),
            "Cross-session history is local-only and stores compact validation metadata rather than prompt/response content."
                .to_string(),
        ],
    }
}

#[derive(Debug, Default)]
struct StageTrendAccumulator {
    stage_label: String,
    stage_kind: String,
    runs_recorded: u32,
    total_validation_score: u64,
    best_validation_score: u32,
    improved_runs: u32,
    unchanged_runs: u32,
    worsened_runs: u32,
    inconclusive_runs: u32,
    strong_or_moderate_runs: u32,
    marker_detection_runs: u32,
    clear_marker_support_runs: u32,
    helper_scan_runs: u32,
    helper_scan_clear_runs: u32,
    helper_scan_marker_detection_runs: u32,
    latest_recorded_at: String,
    latest_vram_evidence_status: String,
    latest_validation_verdict: String,
    latest_marker_evidence_status: String,
}

fn build_stage_trends(
    matching_entries: &[&ValidationHistoryEntry],
) -> Vec<MemoryValidationStageTrendReport> {
    let mut by_stage: BTreeMap<String, StageTrendAccumulator> = BTreeMap::new();

    for entry in matching_entries {
        for stage in &entry.stage_results {
            let accumulator = by_stage.entry(stage.stage_id.clone()).or_default();
            if accumulator.stage_label.is_empty() {
                accumulator.stage_label = stage.stage_label.clone();
            }
            if accumulator.stage_kind.is_empty() {
                accumulator.stage_kind = stage.stage_kind.clone();
            }
            accumulator.runs_recorded = accumulator.runs_recorded.saturating_add(1);
            accumulator.total_validation_score = accumulator
                .total_validation_score
                .saturating_add(stage.validation_score as u64);
            accumulator.best_validation_score = accumulator
                .best_validation_score
                .max(stage.validation_score);
            match classify_stage_outcome(&stage.vram_evidence_status) {
                "improved" => {
                    accumulator.improved_runs = accumulator.improved_runs.saturating_add(1);
                }
                "unchanged" => {
                    accumulator.unchanged_runs = accumulator.unchanged_runs.saturating_add(1);
                }
                "worsened" => {
                    accumulator.worsened_runs = accumulator.worsened_runs.saturating_add(1);
                }
                _ => {
                    accumulator.inconclusive_runs = accumulator.inconclusive_runs.saturating_add(1);
                }
            }
            if stage.validation_verdict == "strong_improvement_signal"
                || stage.validation_verdict == "moderate_improvement_signal"
            {
                accumulator.strong_or_moderate_runs =
                    accumulator.strong_or_moderate_runs.saturating_add(1);
            }
            if stage
                .marker_evidence_status
                .contains("marker_persistence_detected")
                || stage.helper_process_scan_status == "helper_process_scan_marker_detected"
            {
                accumulator.marker_detection_runs =
                    accumulator.marker_detection_runs.saturating_add(1);
            }
            if stage.marker_evidence_status
                == "gpu_evidence_supported_by_clear_session_and_canary_scans"
            {
                accumulator.clear_marker_support_runs =
                    accumulator.clear_marker_support_runs.saturating_add(1);
            }
            if stage.helper_process_scan_status != "helper_process_scan_not_recorded" {
                accumulator.helper_scan_runs = accumulator.helper_scan_runs.saturating_add(1);
            }
            if stage.helper_process_scan_status == "helper_process_scan_clear" {
                accumulator.helper_scan_clear_runs =
                    accumulator.helper_scan_clear_runs.saturating_add(1);
            }
            if stage.helper_process_scan_status == "helper_process_scan_marker_detected" {
                accumulator.helper_scan_marker_detection_runs = accumulator
                    .helper_scan_marker_detection_runs
                    .saturating_add(1);
            }
            if entry.recorded_at >= accumulator.latest_recorded_at {
                accumulator.latest_recorded_at = entry.recorded_at.clone();
                accumulator.latest_vram_evidence_status = stage.vram_evidence_status.clone();
                accumulator.latest_validation_verdict = stage.validation_verdict.clone();
                accumulator.latest_marker_evidence_status = stage.marker_evidence_status.clone();
            }
        }
    }

    let mut stage_trends = by_stage
        .into_iter()
        .map(|(stage_id, accumulator)| {
            let avg_validation_score = if accumulator.runs_recorded == 0 {
                0.0
            } else {
                accumulator.total_validation_score as f64 / accumulator.runs_recorded as f64
            };
            let stage_label = accumulator.stage_label;
            let stage_kind = accumulator.stage_kind;
            let latest_vram_evidence_status = accumulator.latest_vram_evidence_status;
            let latest_validation_verdict = accumulator.latest_validation_verdict;
            let latest_marker_evidence_status = accumulator.latest_marker_evidence_status;
            let summary = format!(
                "{} was recorded in {} run(s), averaged {:.1}/100, improved {} time(s), stayed unchanged {} time(s), worsened {} time(s), and remained inconclusive {} time(s).",
                stage_label,
                accumulator.runs_recorded,
                avg_validation_score,
                accumulator.improved_runs,
                accumulator.unchanged_runs,
                accumulator.worsened_runs,
                accumulator.inconclusive_runs
            );
            let notes = vec![
                format!(
                    "Strong/moderate runs: {}, marker-detection runs: {}, clear marker support runs: {}.",
                    accumulator.strong_or_moderate_runs,
                    accumulator.marker_detection_runs,
                    accumulator.clear_marker_support_runs
                ),
                format!(
                    "Helper-stage scan runs: {}, clear helper scans: {}, helper marker detections: {}.",
                    accumulator.helper_scan_runs,
                    accumulator.helper_scan_clear_runs,
                    accumulator.helper_scan_marker_detection_runs
                ),
                format!(
                    "Latest VRAM evidence: {}. Latest verdict: {}. Latest marker evidence: {}.",
                    latest_vram_evidence_status.replace('_', " "),
                    latest_validation_verdict.replace('_', " "),
                    latest_marker_evidence_status.replace('_', " ")
                ),
            ];
            MemoryValidationStageTrendReport {
                stage_id,
                stage_label,
                stage_kind,
                runs_recorded: accumulator.runs_recorded,
                avg_validation_score,
                best_validation_score: accumulator.best_validation_score,
                improved_runs: accumulator.improved_runs,
                unchanged_runs: accumulator.unchanged_runs,
                worsened_runs: accumulator.worsened_runs,
                inconclusive_runs: accumulator.inconclusive_runs,
                strong_or_moderate_runs: accumulator.strong_or_moderate_runs,
                marker_detection_runs: accumulator.marker_detection_runs,
                clear_marker_support_runs: accumulator.clear_marker_support_runs,
                helper_scan_runs: accumulator.helper_scan_runs,
                helper_scan_clear_runs: accumulator.helper_scan_clear_runs,
                helper_scan_marker_detection_runs: accumulator.helper_scan_marker_detection_runs,
                latest_vram_evidence_status,
                latest_validation_verdict,
                latest_marker_evidence_status,
                summary,
                notes,
            }
        })
        .collect::<Vec<_>>();

    stage_trends.sort_by(|a, b| {
        b.avg_validation_score
            .total_cmp(&a.avg_validation_score)
            .then_with(|| b.runs_recorded.cmp(&a.runs_recorded))
            .then_with(|| b.strong_or_moderate_runs.cmp(&a.strong_or_moderate_runs))
    });

    stage_trends
}

fn build_stage_recommendation(
    stage_trends: &[MemoryValidationStageTrendReport],
) -> MemoryValidationStageRecommendationReport {
    if stage_trends.is_empty() {
        return default_stage_recommendation_report("no_stage_history_available");
    }

    let best = stage_trends
        .iter()
        .map(|trend| (trend, stage_effectiveness_score(trend)))
        .max_by(|(left_trend, left_score), (right_trend, right_score)| {
            left_score
                .total_cmp(right_score)
                .then_with(|| {
                    left_trend
                        .avg_validation_score
                        .total_cmp(&right_trend.avg_validation_score)
                })
                .then_with(|| left_trend.runs_recorded.cmp(&right_trend.runs_recorded))
                .then_with(|| {
                    right_trend
                        .marker_detection_runs
                        .cmp(&left_trend.marker_detection_runs)
                })
        });

    let Some((trend, effectiveness_score)) = best else {
        return default_stage_recommendation_report("no_stage_history_available");
    };

    let recommendation_status = if trend.runs_recorded < 2 {
        "recommendation_waiting_for_repeated_runs"
    } else if trend.marker_detection_runs > 0 {
        "recommendation_limited_by_marker_persistence"
    } else if trend.worsened_runs > 0 {
        "recommendation_limited_by_regressions"
    } else if trend.improved_runs == 0 && trend.strong_or_moderate_runs == 0 {
        "recommendation_mixed_no_clear_improvement"
    } else if trend.inconclusive_runs * 2 >= trend.runs_recorded {
        "recommendation_limited_by_inconclusive_history"
    } else {
        "recommendation_available"
    };

    let mut notes = vec![
        format!(
            "Compared across {} cleanup stage(s) recorded in this model/platform/GPU-offload scope.",
            stage_trends.len()
        ),
        "This recommendation is comparative operator guidance, not proof of full RAM or VRAM sanitization."
            .to_string(),
    ];

    if trend.marker_detection_runs > 0 {
        notes.push(
            "The leading stage still has repeated marker-detection evidence in its history, so treat the recommendation as limited rather than clean."
                .to_string(),
        );
    }
    if trend.worsened_runs > 0 {
        notes.push(
            "The leading stage also has at least one repeated worsened outcome, so it should not be treated as uniformly safe."
                .to_string(),
        );
    }
    if trend.inconclusive_runs > 0 {
        notes.push(
            "Some recorded runs for this stage were still inconclusive, which means the recommendation remains visibility-limited."
                .to_string(),
        );
    }

    let summary = match recommendation_status {
        "recommendation_available" => format!(
            "{} is the current best repeated cleanup stage for this scope based on {} run(s) with an average validation score of {:.1}/100 and no repeated marker detections.",
            trend.stage_label,
            trend.runs_recorded,
            trend.avg_validation_score
        ),
        "recommendation_waiting_for_repeated_runs" => format!(
            "{} is the current top-scoring cleanup stage, but NullContext has only recorded {} run(s) for it in this scope so far.",
            trend.stage_label,
            trend.runs_recorded
        ),
        "recommendation_limited_by_marker_persistence" => format!(
            "{} currently ranks highest by repeated evidence, but its history still includes {} marker-detection run(s).",
            trend.stage_label,
            trend.marker_detection_runs
        ),
        "recommendation_limited_by_regressions" => format!(
            "{} currently ranks highest overall, but its history still includes {} worsened run(s).",
            trend.stage_label,
            trend.worsened_runs
        ),
        "recommendation_limited_by_inconclusive_history" => format!(
            "{} currently ranks highest overall, but too many of its runs remained inconclusive to call it a clean winner yet.",
            trend.stage_label
        ),
        _ => format!(
            "{} currently ranks highest overall, but the repeated evidence still looks mixed rather than clearly improved.",
            trend.stage_label
        ),
    };

    MemoryValidationStageRecommendationReport {
        recommendation_status: recommendation_status.to_string(),
        stage_id: Some(trend.stage_id.clone()),
        stage_label: Some(trend.stage_label.clone()),
        stage_kind: Some(trend.stage_kind.clone()),
        compared_stage_count: stage_trends.len() as u32,
        runs_recorded: trend.runs_recorded,
        avg_validation_score: Some(trend.avg_validation_score),
        effectiveness_score: Some(effectiveness_score),
        improved_runs: trend.improved_runs,
        unchanged_runs: trend.unchanged_runs,
        worsened_runs: trend.worsened_runs,
        inconclusive_runs: trend.inconclusive_runs,
        marker_detection_runs: trend.marker_detection_runs,
        summary,
        notes,
    }
}

fn stage_effectiveness_score(trend: &MemoryValidationStageTrendReport) -> f64 {
    trend.avg_validation_score
        + (trend.improved_runs as f64 * 8.0)
        + (trend.unchanged_runs as f64 * 1.5)
        + (trend.strong_or_moderate_runs as f64 * 4.0)
        + (trend.clear_marker_support_runs as f64 * 3.0)
        + (trend.helper_scan_clear_runs as f64 * 2.0)
        - (trend.worsened_runs as f64 * 12.0)
        - (trend.marker_detection_runs as f64 * 10.0)
        - (trend.helper_scan_marker_detection_runs as f64 * 8.0)
        - (trend.inconclusive_runs as f64 * 2.0)
}

fn default_stage_recommendation_report(
    recommendation_status: &str,
) -> MemoryValidationStageRecommendationReport {
    let summary = match recommendation_status {
        "history_registry_unavailable" => {
            "NullContext could not derive a cleanup-stage recommendation because the validation-history registry was unavailable."
        }
        "history_scope_empty" => {
            "NullContext has not yet recorded enough history in this scope to recommend a cleanup stage."
        }
        _ => {
            "NullContext does not yet have enough repeated cleanup-stage history to recommend a best stage for this scope."
        }
    };

    MemoryValidationStageRecommendationReport {
        recommendation_status: recommendation_status.to_string(),
        stage_id: None,
        stage_label: None,
        stage_kind: None,
        compared_stage_count: 0,
        runs_recorded: 0,
        avg_validation_score: None,
        effectiveness_score: None,
        improved_runs: 0,
        unchanged_runs: 0,
        worsened_runs: 0,
        inconclusive_runs: 0,
        marker_detection_runs: 0,
        summary: summary.to_string(),
        notes: vec![
            "A repeated-evidence recommendation only becomes meaningful once multiple cleanup-stage outcomes have been recorded in the same scope."
                .to_string(),
        ],
    }
}

fn helper_process_scan_signal_status(overall_status: &str) -> String {
    match overall_status {
        "markers_detected_in_scanned_memory" => "helper_process_scan_marker_detected".to_string(),
        "no_markers_detected_in_scanned_regions" => "helper_process_scan_clear".to_string(),
        "scan_attempt_failed" => "helper_process_scan_inconclusive".to_string(),
        "scan_backend_unsupported_on_platform" => {
            "helper_process_scan_backend_unsupported".to_string()
        }
        "scan_skipped" | "scan_not_completed" => "helper_process_scan_not_completed".to_string(),
        _ => "helper_process_scan_mixed".to_string(),
    }
}

fn classify_stage_outcome(vram_evidence_status: &str) -> &'static str {
    match vram_evidence_status {
        "evidence_improved_pid_no_longer_observed_after_strategy"
        | "evidence_improved_bytes_no_longer_visible_but_pid_still_observed"
        | "evidence_improved_peak_bytes_lower_but_residency_still_observed" => "improved",
        "evidence_unchanged_not_observed" | "evidence_unchanged_pid_still_observed" => "unchanged",
        "evidence_worsened_peak_bytes_higher_after_strategy"
        | "evidence_worsened_gpu_visibility_increased_after_strategy" => "worsened",
        _ => "inconclusive",
    }
}

fn load_registry(home: &str) -> Result<ValidationHistoryRegistry> {
    let path = registry_path(home);
    if !path.exists() {
        return Ok(ValidationHistoryRegistry::default());
    }

    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read validation-history registry at {}",
            path.display()
        )
    })?;
    let registry = serde_json::from_str(&raw).with_context(|| {
        format!(
            "Failed to parse validation-history registry at {}",
            path.display()
        )
    })?;
    Ok(registry)
}

fn save_registry(home: &str, registry: &ValidationHistoryRegistry) -> Result<()> {
    let root = registry_root(home);
    fs::create_dir_all(&root)?;
    let path = registry_path(home);
    let temp = root.join("validation_history.json.tmp");

    let json = serde_json::to_string_pretty(registry)?;
    fs::write(&temp, json)?;
    fs::rename(&temp, &path).with_context(|| {
        format!(
            "Failed to persist validation-history registry at {}",
            path.display()
        )
    })?;

    Ok(())
}

fn registry_root(home: &str) -> PathBuf {
    Path::new(home).join(".nullcontext").join("validation")
}

fn registry_path(home: &str) -> PathBuf {
    registry_root(home).join("validation_history.json")
}

fn history_scope_key(report: &PrivacyReport) -> String {
    let model_id = report
        .llama_runtime
        .as_ref()
        .map(|runtime| runtime.model_id.as_str())
        .unwrap_or("unknown_model");
    let platform = history_scope_platform(report).unwrap_or_else(|| "unknown_platform".to_string());
    let gpu = report
        .llama_runtime
        .as_ref()
        .map(|runtime| runtime.gpu_offload_requested)
        .unwrap_or(false);

    format!("{model_id}::{platform}::gpu_{gpu}")
}

fn history_scope_platform(report: &PrivacyReport) -> Option<String> {
    report
        .process_scan
        .as_ref()
        .map(|scan| scan.platform.clone())
        .or_else(|| {
            let platform = report
                .memory_validation
                .controlled_canary_run
                .process_scan
                .platform
                .clone();
            if platform.is_empty() {
                None
            } else {
                Some(platform)
            }
        })
        .or_else(|| Some(std::env::consts::OS.to_string()))
}

fn history_scope_label(report: &PrivacyReport) -> String {
    let model_id = report
        .llama_runtime
        .as_ref()
        .map(|runtime| runtime.model_id.clone())
        .unwrap_or_else(|| "unknown_model".to_string());
    let platform = history_scope_platform(report).unwrap_or_else(|| "unknown_platform".to_string());
    let gpu = report
        .llama_runtime
        .as_ref()
        .map(|runtime| runtime.gpu_offload_requested)
        .unwrap_or(false);

    format!("{model_id} on {platform} (gpu_offload_requested={gpu})")
}

use crate::audit::{
    MemoryValidationHistoryReport, MemoryValidationStageEffectivenessEntry,
    MemoryValidationStageEffectivenessReport, MemoryValidationStageRecommendationReport,
    MemoryValidationStageTrendReport, PrivacyReport, ValidationReleaseGateReport,
};
use crate::logging::stdout_line;
use crate::registry::{resolve_session_report_availability, SessionRegistry};
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

#[derive(Debug, Clone, Serialize)]
struct ValidationHistoryRegistrySummary {
    registry_status: String,
    scopes_recorded: u32,
    total_runs_recorded: u32,
    latest_recorded_at: Option<String>,
    scopes: Vec<ValidationHistoryScopeSummary>,
    summary: String,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ValidationHistoryScopeSummary {
    scope_key: String,
    model_id: Option<String>,
    model_name: Option<String>,
    platform: Option<String>,
    gpu_offload_requested: Option<bool>,
    runs_recorded: u32,
    marker_detection_runs: u32,
    clear_canary_runs: u32,
    inconclusive_or_failed_runs: u32,
    strong_or_moderate_runs: u32,
    best_stage_score_min: Option<u32>,
    best_stage_score_max: Option<u32>,
    best_stage_score_avg: Option<f64>,
    latest_recorded_at: Option<String>,
    latest_session_id: Option<String>,
    latest_validation_status: Option<String>,
    latest_best_stage_verdict: Option<String>,
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
    #[serde(default = "default_canary_aggregate_process_scan_status")]
    pub canary_aggregate_process_scan_status: String,
    #[serde(default)]
    pub canary_requested_passes: u32,
    #[serde(default)]
    pub canary_completed_passes: u32,
    #[serde(default)]
    pub canary_failed_passes: u32,
    pub best_stage_score: u32,
    pub best_stage_verdict: String,
    #[serde(default)]
    pub stage_results: Vec<ValidationHistoryStageResultEntry>,
}

fn default_canary_aggregate_process_scan_status() -> String {
    "scan_not_completed".to_string()
}

fn default_process_scan_context_status() -> String {
    "process_scan_context_unavailable".to_string()
}

fn default_process_scan_context_scope() -> String {
    "process_scan_context_unavailable".to_string()
}

fn default_cleanup_signal_support_status() -> String {
    "cleanup_signal_support_unavailable".to_string()
}

fn default_cleanup_signal_support_scope_status() -> String {
    "cleanup_signal_scope_unavailable".to_string()
}

fn default_stage_effectiveness_report() -> MemoryValidationStageEffectivenessReport {
    MemoryValidationStageEffectivenessReport {
        summary_status: "cleanup_stage_effectiveness_not_derived".to_string(),
        consistently_helpful_count: 0,
        promising_but_limited_count: 0,
        ineffective_or_regressive_count: 0,
        marker_persistent_count: 0,
        waiting_for_repeated_history_count: 0,
        stages: vec![],
        summary:
            "NullContext had not yet derived explicit repeated cleanup-stage effectiveness classes for this scope."
                .to_string(),
        notes: vec![
            "Older reports may not include the repeated cleanup-stage effectiveness summary."
                .to_string(),
        ],
    }
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
    #[serde(default = "default_process_scan_context_status")]
    pub process_scan_context_status: String,
    #[serde(default = "default_process_scan_context_scope")]
    pub process_scan_context_scope: String,
    #[serde(default = "default_cleanup_signal_support_status")]
    pub cleanup_signal_support_status: String,
    #[serde(default = "default_cleanup_signal_support_scope_status")]
    pub cleanup_signal_support_scope_status: String,
    #[serde(default)]
    pub contributing_cleanup_signals: Vec<String>,
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
                controlled_canary_history: build_controlled_canary_history(&[]),
                cleanup_stage_effectiveness: default_stage_effectiveness_report(),
                cleanup_stage_recommendation:
                    default_stage_recommendation_report("history_registry_unavailable"),
                release_gate: default_release_gate_report(),
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

pub fn show_validation_history(home: &str, session_id: &str) -> Result<()> {
    let registry = load_registry(home)?;
    let session_registry = SessionRegistry::load(home)?;

    let entry = session_registry
        .find(session_id)
        .with_context(|| format!("Session not found in registry: {session_id}"))?;
    let availability = resolve_session_report_availability(home, entry);
    let Some(report_path) = availability.loadable_path else {
        anyhow::bail!(
            "No loadable report was found for session {session_id}. NullContext checked the current report path and any archived lifecycle report."
        );
    };

    let raw_report = fs::read_to_string(&report_path)
        .with_context(|| format!("Failed to read report at {}", report_path.display()))?;
    let report: PrivacyReport = serde_json::from_str(&raw_report)
        .with_context(|| format!("Failed to parse report at {}", report_path.display()))?;
    let history_report = build_history_report_from_registry(&registry, &report);

    stdout_line(serde_json::to_string_pretty(&history_report)?);

    Ok(())
}

pub fn list_validation_scopes(home: &str) -> Result<()> {
    let registry = load_registry(home)?;
    let summary = build_registry_summary(&registry);

    stdout_line(serde_json::to_string_pretty(&summary)?);

    Ok(())
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

fn build_registry_summary(
    registry: &ValidationHistoryRegistry,
) -> ValidationHistoryRegistrySummary {
    if registry.entries.is_empty() {
        return ValidationHistoryRegistrySummary {
            registry_status: "validation_history_registry_empty".to_string(),
            scopes_recorded: 0,
            total_runs_recorded: 0,
            latest_recorded_at: None,
            scopes: vec![],
            summary:
                "NullContext has not recorded any cross-session validation-history runs yet."
                    .to_string(),
            notes: vec![
                "Run one or more sessions with memory-validation reporting to populate this registry."
                    .to_string(),
            ],
        };
    }

    let mut by_scope: BTreeMap<String, Vec<&ValidationHistoryEntry>> = BTreeMap::new();
    for entry in &registry.entries {
        by_scope
            .entry(entry.scope_key.clone())
            .or_default()
            .push(entry);
    }

    let mut scopes = by_scope
        .into_iter()
        .map(|(scope_key, mut entries)| {
            entries.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
            let latest = entries.first().copied();
            let runs_recorded = entries.len() as u32;
            let marker_detection_runs = entries
                .iter()
                .filter(|entry| {
                    entry.process_scan_signal_status == "marker_persistence_detected"
                        || entry.canary_aggregate_signal_status
                            == "controlled_canary_markers_detected_across_passes"
                })
                .count() as u32;
            let clear_canary_runs = entries
                .iter()
                .filter(|entry| {
                    entry.canary_aggregate_signal_status
                        == "controlled_canary_all_completed_passes_clear"
                })
                .count() as u32;
            let inconclusive_or_failed_runs = entries
                .iter()
                .filter(|entry| {
                    entry.validation_status != "stage_scoring_ready"
                        || entry.canary_execution_status != "controlled_canary_completed"
                })
                .count() as u32;
            let strong_or_moderate_runs = entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.best_stage_verdict.as_str(),
                        "strong_improvement_signal" | "moderate_improvement_signal"
                    )
                })
                .count() as u32;
            let best_stage_score_min = entries.iter().map(|entry| entry.best_stage_score).min();
            let best_stage_score_max = entries.iter().map(|entry| entry.best_stage_score).max();
            let best_stage_score_avg = Some(
                entries
                    .iter()
                    .map(|entry| entry.best_stage_score as f64)
                    .sum::<f64>()
                    / entries.len() as f64,
            );

            ValidationHistoryScopeSummary {
                scope_key,
                model_id: latest.and_then(|entry| entry.model_id.clone()),
                model_name: latest.and_then(|entry| entry.model_name.clone()),
                platform: latest.and_then(|entry| entry.platform.clone()),
                gpu_offload_requested: latest.and_then(|entry| entry.gpu_offload_requested),
                runs_recorded,
                marker_detection_runs,
                clear_canary_runs,
                inconclusive_or_failed_runs,
                strong_or_moderate_runs,
                best_stage_score_min,
                best_stage_score_max,
                best_stage_score_avg,
                latest_recorded_at: latest.map(|entry| entry.recorded_at.clone()),
                latest_session_id: latest.map(|entry| entry.session_id.clone()),
                latest_validation_status: latest.map(|entry| entry.validation_status.clone()),
                latest_best_stage_verdict: latest.map(|entry| entry.best_stage_verdict.clone()),
            }
        })
        .collect::<Vec<_>>();

    scopes.sort_by(|a, b| b.latest_recorded_at.cmp(&a.latest_recorded_at));

    ValidationHistoryRegistrySummary {
        registry_status: "validation_history_registry_recorded".to_string(),
        scopes_recorded: scopes.len() as u32,
        total_runs_recorded: registry.entries.len() as u32,
        latest_recorded_at: registry.entries.first().map(|entry| entry.recorded_at.clone()),
        summary: format!(
            "NullContext has recorded {} validation run(s) across {} scope(s). Use a scope's latest_session_id with --show-validation-history <session-id> when you want the full repeated-history report for one saved run in that scope.",
            registry.entries.len(),
            scopes.len()
        ),
        notes: vec![
            "A scope groups runs by model id, host platform, and whether GPU offload was requested."
                .to_string(),
            "This registry summary is compact by design; use latest_session_id from a scope entry to inspect the full repeated-history report."
                .to_string(),
        ],
        scopes,
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
            canary_aggregate_process_scan_status: report
                .memory_validation
                .controlled_canary_run
                .aggregate_process_scan_status
                .clone(),
            canary_requested_passes: report
                .memory_validation
                .controlled_canary_run
                .requested_passes,
            canary_completed_passes: report
                .memory_validation
                .controlled_canary_run
                .completed_passes,
            canary_failed_passes: report.memory_validation.controlled_canary_run.failed_passes,
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
                    process_scan_context_status: scorecard.process_scan_context_status.clone(),
                    process_scan_context_scope: scorecard.process_scan_context_scope.clone(),
                    cleanup_signal_support_status: scorecard.cleanup_signal_support_status.clone(),
                    cleanup_signal_support_scope_status: scorecard
                        .cleanup_signal_support_scope_status
                        .clone(),
                    contributing_cleanup_signals: scorecard.contributing_cleanup_signals.clone(),
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
            controlled_canary_history: build_controlled_canary_history(&[]),
            cleanup_stage_effectiveness: default_stage_effectiveness_report(),
            cleanup_stage_recommendation: default_stage_recommendation_report(
                "history_scope_empty",
            ),
            release_gate: default_release_gate_report(),
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
    let controlled_canary_history = build_controlled_canary_history(&matching_entries);
    let cleanup_stage_effectiveness = build_stage_effectiveness_report(&stage_trends);
    let cleanup_stage_recommendation = build_stage_recommendation(&stage_trends);
    let release_gate =
        build_release_gate(&cleanup_stage_recommendation, &controlled_canary_history);
    let canary_summary = controlled_canary_history.summary.clone();
    let cleanup_stage_effectiveness_summary = cleanup_stage_effectiveness.summary.clone();
    let recommendation_summary = cleanup_stage_recommendation.summary.clone();
    let release_gate_summary = release_gate.summary.clone();

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
        controlled_canary_history,
        cleanup_stage_effectiveness,
        cleanup_stage_recommendation,
        release_gate,
        summary: format!(
            "NullContext has recorded {} validation run(s) for scope {}. {} run(s) still showed marker-detection evidence, {} run(s) achieved fully clear repeated canary passes, and the best-stage score average is {}. {} {} {} {}",
            runs_recorded,
            history_scope_label(report),
            marker_detection_runs,
            clear_canary_runs,
            best_stage_score_avg
                .map(|value| format!("{value:.1}/100"))
                .unwrap_or_else(|| "unavailable".to_string()),
            canary_summary,
            cleanup_stage_effectiveness_summary,
            recommendation_summary,
            release_gate_summary
        ),
        notes: vec![
            "This scope groups validation history by model id, host platform, and whether GPU offload was requested."
                .to_string(),
            "Cross-session history is local-only and stores compact validation metadata rather than prompt/response content."
                .to_string(),
        ],
    }
}

fn build_stage_effectiveness_report(
    stage_trends: &[MemoryValidationStageTrendReport],
) -> MemoryValidationStageEffectivenessReport {
    if stage_trends.is_empty() {
        return MemoryValidationStageEffectivenessReport {
            summary_status: "cleanup_stage_effectiveness_waiting_for_history".to_string(),
            consistently_helpful_count: 0,
            promising_but_limited_count: 0,
            ineffective_or_regressive_count: 0,
            marker_persistent_count: 0,
            waiting_for_repeated_history_count: 0,
            stages: vec![],
            summary:
                "No repeated cleanup-stage trends are available yet, so NullContext cannot classify which cleanup stages actually help in this scope."
                    .to_string(),
            notes: vec![
                "Repeated cleanup-stage effectiveness becomes meaningful only after at least one cleanup stage has history in the current model/platform/GPU-offload scope."
                    .to_string(),
            ],
        };
    }

    let mut consistently_helpful_count = 0_u32;
    let mut promising_but_limited_count = 0_u32;
    let mut ineffective_or_regressive_count = 0_u32;
    let mut marker_persistent_count = 0_u32;
    let mut waiting_for_repeated_history_count = 0_u32;

    let mut stages = stage_trends
        .iter()
        .map(|trend| {
            let effectiveness_class = classify_stage_effectiveness(trend).to_string();
            match effectiveness_class.as_str() {
                "effectiveness_consistently_helpful" => {
                    consistently_helpful_count = consistently_helpful_count.saturating_add(1);
                }
                "effectiveness_promising_but_limited" => {
                    promising_but_limited_count = promising_but_limited_count.saturating_add(1);
                }
                "effectiveness_marker_persistent" => {
                    marker_persistent_count = marker_persistent_count.saturating_add(1);
                }
                "effectiveness_waiting_for_repeated_history" => {
                    waiting_for_repeated_history_count =
                        waiting_for_repeated_history_count.saturating_add(1);
                }
                _ => {
                    ineffective_or_regressive_count =
                        ineffective_or_regressive_count.saturating_add(1);
                }
            }

            let summary = match effectiveness_class.as_str() {
                "effectiveness_consistently_helpful" => format!(
                    "{} repeatedly improves or preserves the strongest observed cleanup outcomes in this scope without repeated marker persistence or regressions.",
                    trend.stage_label
                ),
                "effectiveness_promising_but_limited" => format!(
                    "{} shows some repeated improvement signals, but its evidence is still limited by scope, incomplete marker support, or mixed cleanup-signal context.",
                    trend.stage_label
                ),
                "effectiveness_marker_persistent" => format!(
                    "{} still has repeated marker persistence in its history, so NullContext should not treat it as a stage that actually helps yet.",
                    trend.stage_label
                ),
                "effectiveness_waiting_for_repeated_history" => format!(
                    "{} does not have enough repeated history yet for NullContext to classify whether it actually helps in this scope.",
                    trend.stage_label
                ),
                _ => format!(
                    "{} is currently ineffective, regressive, or too inconsistent to count as a helpful cleanup stage in this scope.",
                    trend.stage_label
                ),
            };

            MemoryValidationStageEffectivenessEntry {
                stage_id: trend.stage_id.clone(),
                stage_label: trend.stage_label.clone(),
                stage_kind: trend.stage_kind.clone(),
                effectiveness_class,
                evidence_support_status: trend.evidence_support_status.clone(),
                cleanup_signal_scope_status: trend.latest_cleanup_signal_support_scope_status.clone(),
                runs_recorded: trend.runs_recorded,
                avg_validation_score: trend.avg_validation_score,
                improved_runs: trend.improved_runs,
                unchanged_runs: trend.unchanged_runs,
                worsened_runs: trend.worsened_runs,
                inconclusive_runs: trend.inconclusive_runs,
                marker_detection_runs: trend.marker_detection_runs,
                stage_local_scan_clear_runs: trend.stage_local_scan_clear_runs,
                summary,
            }
        })
        .collect::<Vec<_>>();

    stages.sort_by(|a, b| {
        stage_effectiveness_class_priority(&b.effectiveness_class)
            .cmp(&stage_effectiveness_class_priority(&a.effectiveness_class))
            .then_with(|| b.avg_validation_score.total_cmp(&a.avg_validation_score))
            .then_with(|| b.runs_recorded.cmp(&a.runs_recorded))
    });

    let summary_status = if consistently_helpful_count > 0 {
        "cleanup_stage_effectiveness_has_consistently_helpful_stage"
    } else if promising_but_limited_count > 0 {
        "cleanup_stage_effectiveness_only_promising_stages"
    } else if marker_persistent_count > 0 {
        "cleanup_stage_effectiveness_blocked_by_marker_persistence"
    } else if waiting_for_repeated_history_count == stage_trends.len() as u32 {
        "cleanup_stage_effectiveness_waiting_for_history"
    } else {
        "cleanup_stage_effectiveness_no_helpful_stage_confirmed"
    };

    let summary = format!(
        "Repeated cleanup-stage effectiveness in this scope currently classifies {} stage(s) as consistently helpful, {} as promising but limited, {} as ineffective or regressive, {} as marker-persistent, and {} as still waiting for enough repeated history.",
        consistently_helpful_count,
        promising_but_limited_count,
        ineffective_or_regressive_count,
        marker_persistent_count,
        waiting_for_repeated_history_count
    );

    let notes = vec![
        "This summary is meant to answer which cleanup stages actually help over time in the current model/platform/GPU-offload scope, not just which stage won one report."
            .to_string(),
        "A stage is only treated as consistently helpful here when repeated history stays free of repeated marker persistence, regressions, and major inconclusive gaps."
            .to_string(),
    ];

    MemoryValidationStageEffectivenessReport {
        summary_status: summary_status.to_string(),
        consistently_helpful_count,
        promising_but_limited_count,
        ineffective_or_regressive_count,
        marker_persistent_count,
        waiting_for_repeated_history_count,
        stages,
        summary,
        notes,
    }
}

fn classify_stage_effectiveness(trend: &MemoryValidationStageTrendReport) -> &'static str {
    if trend.runs_recorded < 2 {
        return "effectiveness_waiting_for_repeated_history";
    }
    if trend.marker_detection_runs > 0 {
        return "effectiveness_marker_persistent";
    }
    if trend.worsened_runs > 0 && trend.improved_runs == 0 && trend.strong_or_moderate_runs == 0 {
        return "effectiveness_ineffective_or_regressive";
    }
    if trend.stage_local_scan_clear_runs > 0
        && trend.improved_runs > 0
        && trend.worsened_runs == 0
        && trend.inconclusive_runs == 0
        && matches!(
            trend.evidence_support_status.as_str(),
            "recommendation_evidence_supported_by_stage_local_marker_clearance"
                | "recommendation_evidence_supported_by_marker_clearance_history"
        )
    {
        return "effectiveness_consistently_helpful";
    }
    if trend.improved_runs > 0
        || trend.strong_or_moderate_runs > 0
        || trend.clear_marker_support_runs > 0
        || trend.helper_scan_clear_runs > 0
    {
        return "effectiveness_promising_but_limited";
    }
    "effectiveness_ineffective_or_regressive"
}

fn stage_effectiveness_class_priority(effectiveness_class: &str) -> u8 {
    match effectiveness_class {
        "effectiveness_consistently_helpful" => 4,
        "effectiveness_promising_but_limited" => 3,
        "effectiveness_waiting_for_repeated_history" => 2,
        "effectiveness_ineffective_or_regressive" => 1,
        "effectiveness_marker_persistent" => 0,
        _ => 0,
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
    cleanup_signal_strong_runs: u32,
    cleanup_signal_partial_runs: u32,
    cleanup_signal_limited_runs: u32,
    cleanup_signal_runtime_global_only_runs: u32,
    cleanup_signal_stage_local_helper_runs: u32,
    cleanup_signal_declared_only_runs: u32,
    cleanup_signal_scope_unavailable_runs: u32,
    stage_local_scan_runs: u32,
    stage_local_scan_clear_runs: u32,
    stage_local_scan_marker_detection_runs: u32,
    stage_local_scan_limited_runs: u32,
    session_fallback_scan_runs: u32,
    latest_recorded_at: String,
    latest_vram_evidence_status: String,
    latest_validation_verdict: String,
    latest_marker_evidence_status: String,
    latest_cleanup_signal_support_status: String,
    latest_cleanup_signal_support_scope_status: String,
    latest_contributing_cleanup_signals: Vec<String>,
    latest_process_scan_context_status: String,
    latest_process_scan_context_scope: String,
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
            match stage.cleanup_signal_support_status.as_str() {
                "cleanup_signal_support_strong" => {
                    accumulator.cleanup_signal_strong_runs =
                        accumulator.cleanup_signal_strong_runs.saturating_add(1);
                }
                "cleanup_signal_support_partial" => {
                    accumulator.cleanup_signal_partial_runs =
                        accumulator.cleanup_signal_partial_runs.saturating_add(1);
                }
                _ => {
                    accumulator.cleanup_signal_limited_runs =
                        accumulator.cleanup_signal_limited_runs.saturating_add(1);
                }
            }
            match stage.cleanup_signal_support_scope_status.as_str() {
                "cleanup_signal_scope_stage_local_helper_runtime" => {
                    accumulator.cleanup_signal_stage_local_helper_runs = accumulator
                        .cleanup_signal_stage_local_helper_runs
                        .saturating_add(1);
                }
                "cleanup_signal_scope_runtime_global_only" => {
                    accumulator.cleanup_signal_runtime_global_only_runs = accumulator
                        .cleanup_signal_runtime_global_only_runs
                        .saturating_add(1);
                }
                "cleanup_signal_scope_declared_but_not_observed" => {
                    accumulator.cleanup_signal_declared_only_runs = accumulator
                        .cleanup_signal_declared_only_runs
                        .saturating_add(1);
                }
                _ => {
                    accumulator.cleanup_signal_scope_unavailable_runs = accumulator
                        .cleanup_signal_scope_unavailable_runs
                        .saturating_add(1);
                }
            }
            match stage.process_scan_context_scope.as_str() {
                "stage_local_helper_scan" | "stage_local_cleanup_phase" => {
                    accumulator.stage_local_scan_runs =
                        accumulator.stage_local_scan_runs.saturating_add(1);
                    match stage.process_scan_context_status.as_str() {
                        "marker_scan_clear_in_scanned_regions" => {
                            accumulator.stage_local_scan_clear_runs =
                                accumulator.stage_local_scan_clear_runs.saturating_add(1);
                        }
                        "marker_persistence_detected" => {
                            accumulator.stage_local_scan_marker_detection_runs = accumulator
                                .stage_local_scan_marker_detection_runs
                                .saturating_add(1);
                        }
                        _ => {
                            accumulator.stage_local_scan_limited_runs =
                                accumulator.stage_local_scan_limited_runs.saturating_add(1);
                        }
                    }
                }
                "session_fallback" => {
                    accumulator.session_fallback_scan_runs =
                        accumulator.session_fallback_scan_runs.saturating_add(1);
                }
                _ => {}
            }
            if entry.recorded_at >= accumulator.latest_recorded_at {
                accumulator.latest_recorded_at = entry.recorded_at.clone();
                accumulator.latest_vram_evidence_status = stage.vram_evidence_status.clone();
                accumulator.latest_validation_verdict = stage.validation_verdict.clone();
                accumulator.latest_marker_evidence_status = stage.marker_evidence_status.clone();
                accumulator.latest_cleanup_signal_support_status =
                    stage.cleanup_signal_support_status.clone();
                accumulator.latest_cleanup_signal_support_scope_status =
                    stage.cleanup_signal_support_scope_status.clone();
                accumulator.latest_contributing_cleanup_signals =
                    stage.contributing_cleanup_signals.clone();
                accumulator.latest_process_scan_context_status =
                    stage.process_scan_context_status.clone();
                accumulator.latest_process_scan_context_scope =
                    stage.process_scan_context_scope.clone();
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
            let latest_cleanup_signal_support_status =
                accumulator.latest_cleanup_signal_support_status;
            let latest_cleanup_signal_support_scope_status =
                accumulator.latest_cleanup_signal_support_scope_status;
            let latest_contributing_cleanup_signals =
                accumulator.latest_contributing_cleanup_signals;
            let latest_process_scan_context_status =
                accumulator.latest_process_scan_context_status;
            let latest_process_scan_context_scope =
                accumulator.latest_process_scan_context_scope;
            let evidence_support_status = if accumulator.runs_recorded < 2 {
                "recommendation_evidence_waiting_for_repeated_runs"
            } else if accumulator.marker_detection_runs > 0 {
                "recommendation_evidence_limited_by_marker_persistence"
            } else if accumulator.stage_local_scan_clear_runs > 0 {
                "recommendation_evidence_supported_by_stage_local_marker_clearance"
            } else if accumulator.clear_marker_support_runs > 0
                || accumulator.helper_scan_clear_runs > 0
            {
                "recommendation_evidence_supported_by_marker_clearance_history"
            } else if accumulator.inconclusive_runs * 2 >= accumulator.runs_recorded {
                "recommendation_evidence_limited_by_inconclusive_history"
            } else if accumulator.stage_local_scan_runs == 0
                && accumulator.session_fallback_scan_runs > 0
            {
                "recommendation_evidence_limited_to_session_fallback_scans"
            } else if accumulator.cleanup_signal_runtime_global_only_runs > 0
                && accumulator.cleanup_signal_runtime_global_only_runs
                    + accumulator.cleanup_signal_declared_only_runs
                    + accumulator.cleanup_signal_scope_unavailable_runs
                    == accumulator.runs_recorded
                && accumulator.clear_marker_support_runs == 0
                && accumulator.stage_local_scan_clear_runs == 0
            {
                "recommendation_evidence_limited_to_runtime_global_cleanup_signals"
            } else if accumulator.cleanup_signal_strong_runs > 0
                || accumulator.cleanup_signal_partial_runs > 0
            {
                "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance"
            } else if accumulator.strong_or_moderate_runs > 0 || accumulator.improved_runs > 0 {
                "recommendation_evidence_gpu_only_without_marker_support"
            } else {
                "recommendation_evidence_limited_mixed_history"
            };
            let evidence_support_summary = match evidence_support_status {
                "recommendation_evidence_supported_by_stage_local_marker_clearance" => format!(
                    "{} is backed by repeated stage-local clear marker scans, making this the strongest current cleanup-stage evidence class in the repeated trend table.",
                    stage_label
                ),
                "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance"
                    if accumulator.cleanup_signal_stage_local_helper_runs > 0 =>
                {
                    format!(
                        "{} is currently supported by repeated allocator/KV/model cleanup-path signals, and at least part of that support came from stage-local helper-runtime probes rather than only from whole-runtime inheritance.",
                        stage_label
                    )
                }
                "recommendation_evidence_supported_by_marker_clearance_history" => format!(
                    "{} is backed by repeated clear marker history, but that support is not yet entirely stage-local across all recorded runs.",
                    stage_label
                ),
                "recommendation_evidence_limited_to_runtime_global_cleanup_signals" => format!(
                    "{} is currently supported by repeated allocator/KV/model cleanup signals, but that support is still runtime-global-only rather than stage-local to the winning cleanup stage.",
                    stage_label
                ),
                "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance" => format!(
                    "{} is currently supported more by repeated allocator/KV/model cleanup-path signals than by repeated direct marker-clearance evidence, but that internal support may still be runtime-global rather than stage-local.",
                    stage_label
                ),
                "recommendation_evidence_gpu_only_without_marker_support" => format!(
                    "{} currently looks improved mostly from repeated GPU/process evidence trends; repeated direct marker-clearance support is still missing.",
                    stage_label
                ),
                "recommendation_evidence_limited_to_session_fallback_scans" => format!(
                    "{} still relies on session-fallback scan context rather than consistently isolated stage-local marker evidence.",
                    stage_label
                ),
                "recommendation_evidence_limited_by_inconclusive_history" => format!(
                    "{} still has too much inconclusive repeated history for NullContext to treat the stage trend as strongly supported.",
                    stage_label
                ),
                "recommendation_evidence_limited_by_marker_persistence" => format!(
                    "{} still has repeated marker persistence in its history, so this stage trend cannot be treated as clean evidence yet.",
                    stage_label
                ),
                "recommendation_evidence_waiting_for_repeated_runs" => format!(
                    "{} has not yet been exercised enough times in this scope for NullContext to classify its repeated evidence strongly.",
                    stage_label
                ),
                _ => format!(
                    "{} still has mixed repeated evidence, so NullContext cannot yet classify the stage trend as strongly supported.",
                    stage_label
                ),
            };
            let mut trend = MemoryValidationStageTrendReport {
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
                cleanup_signal_strong_runs: accumulator.cleanup_signal_strong_runs,
                cleanup_signal_partial_runs: accumulator.cleanup_signal_partial_runs,
                cleanup_signal_limited_runs: accumulator.cleanup_signal_limited_runs,
                cleanup_signal_runtime_global_only_runs: accumulator
                    .cleanup_signal_runtime_global_only_runs,
                cleanup_signal_stage_local_helper_runs: accumulator
                    .cleanup_signal_stage_local_helper_runs,
                cleanup_signal_declared_only_runs: accumulator
                    .cleanup_signal_declared_only_runs,
                cleanup_signal_scope_unavailable_runs: accumulator
                    .cleanup_signal_scope_unavailable_runs,
                stage_local_scan_runs: accumulator.stage_local_scan_runs,
                stage_local_scan_clear_runs: accumulator.stage_local_scan_clear_runs,
                stage_local_scan_marker_detection_runs: accumulator
                    .stage_local_scan_marker_detection_runs,
                stage_local_scan_limited_runs: accumulator.stage_local_scan_limited_runs,
                session_fallback_scan_runs: accumulator.session_fallback_scan_runs,
                latest_vram_evidence_status,
                latest_validation_verdict,
                latest_marker_evidence_status,
                latest_cleanup_signal_support_status,
                latest_cleanup_signal_support_scope_status,
                latest_contributing_cleanup_signals,
                latest_process_scan_context_status,
                latest_process_scan_context_scope,
                selection_fitness_status: "selection_fitness_not_derived".to_string(),
                selection_fitness_summary: String::new(),
                evidence_support_status: evidence_support_status.to_string(),
                evidence_support_summary,
                summary: String::new(),
                notes: vec![],
            };
            trend.selection_fitness_status =
                derive_stage_selection_fitness_status(&trend).to_string();
            trend.selection_fitness_summary = stage_selection_fitness_summary(&trend);
            let summary = format!(
                "{} was recorded in {} run(s), averaged {:.1}/100, improved {} time(s), stayed unchanged {} time(s), worsened {} time(s), and remained inconclusive {} time(s). Stage-local direct scans were recorded in {} run(s), with {} clear stage-local scan(s) and {} stage-local marker-detection run(s). Strong allocator/KV cleanup-signal support was present in {} run(s), stage-local helper-runtime cleanup-signal scope was recorded in {} run(s), and runtime-global-only cleanup-signal scope was recorded in {} run(s).",
                trend.stage_label,
                trend.runs_recorded,
                trend.avg_validation_score,
                trend.improved_runs,
                trend.unchanged_runs,
                trend.worsened_runs,
                trend.inconclusive_runs,
                trend.stage_local_scan_runs,
                trend.stage_local_scan_clear_runs,
                trend.stage_local_scan_marker_detection_runs,
                trend.cleanup_signal_strong_runs,
                trend.cleanup_signal_stage_local_helper_runs,
                trend.cleanup_signal_runtime_global_only_runs
            );
            let notes = vec![
                format!(
                    "Strong/moderate runs: {}, marker-detection runs: {}, clear marker support runs: {}.",
                    trend.strong_or_moderate_runs,
                    trend.marker_detection_runs,
                    trend.clear_marker_support_runs
                ),
                format!(
                    "Helper-stage scan runs: {}, clear helper scans: {}, helper marker detections: {}.",
                    trend.helper_scan_runs,
                    trend.helper_scan_clear_runs,
                    trend.helper_scan_marker_detection_runs
                ),
                format!(
                    "Cleanup-signal support: {} strong, {} partial, {} limited/unavailable.",
                    trend.cleanup_signal_strong_runs,
                    trend.cleanup_signal_partial_runs,
                    trend.cleanup_signal_limited_runs
                ),
                format!(
                    "Cleanup-signal scope: {} stage-local-helper run(s), {} runtime-global-only run(s), {} declared-but-unobserved run(s), {} unavailable/mixed run(s).",
                    trend.cleanup_signal_stage_local_helper_runs,
                    trend.cleanup_signal_runtime_global_only_runs,
                    trend.cleanup_signal_declared_only_runs,
                    trend.cleanup_signal_scope_unavailable_runs
                ),
                format!(
                    "Selection fitness: {}.",
                    trend.selection_fitness_status.replace('_', " ")
                ),
                format!(
                    "Repeated evidence support class: {}.",
                    trend.evidence_support_status.replace('_', " ")
                ),
                format!(
                    "Stage-local direct scans: {} total, {} clear, {} marker-detected, {} limited. Session-fallback scan usage: {} run(s).",
                    trend.stage_local_scan_runs,
                    trend.stage_local_scan_clear_runs,
                    trend.stage_local_scan_marker_detection_runs,
                    trend.stage_local_scan_limited_runs,
                    trend.session_fallback_scan_runs
                ),
                format!(
                    "Latest VRAM evidence: {}. Latest verdict: {}. Latest marker evidence: {}. Latest cleanup-signal support: {}. Latest cleanup-signal scope: {}. Latest process-scan context: {} via {}.",
                    trend.latest_vram_evidence_status.replace('_', " "),
                    trend.latest_validation_verdict.replace('_', " "),
                    trend.latest_marker_evidence_status.replace('_', " "),
                    trend.latest_cleanup_signal_support_status.replace('_', " "),
                    trend.latest_cleanup_signal_support_scope_status.replace('_', " "),
                    trend.latest_process_scan_context_status.replace('_', " "),
                    trend.latest_process_scan_context_scope.replace('_', " ")
                ),
                format!(
                    "Latest contributing cleanup signals: {}.",
                    if trend.latest_contributing_cleanup_signals.is_empty() {
                        "none".to_string()
                    } else {
                        trend.latest_contributing_cleanup_signals.join(", ")
                    }
                ),
            ];
            trend.summary = summary;
            trend.notes = notes;
            trend
        })
        .collect::<Vec<_>>();

    stage_trends.sort_by(|a, b| {
        stage_selection_fitness_priority(&b.selection_fitness_status)
            .cmp(&stage_selection_fitness_priority(
                &a.selection_fitness_status,
            ))
            .then_with(|| {
                stage_evidence_support_priority(&b.evidence_support_status)
                    .cmp(&stage_evidence_support_priority(&a.evidence_support_status))
            })
            .then_with(|| b.avg_validation_score.total_cmp(&a.avg_validation_score))
            .then_with(|| b.runs_recorded.cmp(&a.runs_recorded))
            .then_with(|| b.strong_or_moderate_runs.cmp(&a.strong_or_moderate_runs))
            .then_with(|| {
                b.stage_local_scan_clear_runs
                    .cmp(&a.stage_local_scan_clear_runs)
            })
            .then_with(|| {
                a.session_fallback_scan_runs
                    .cmp(&b.session_fallback_scan_runs)
            })
    });

    stage_trends
}

fn build_controlled_canary_history(
    matching_entries: &[&ValidationHistoryEntry],
) -> crate::audit::ControlledCanaryHistoryReport {
    if matching_entries.is_empty() {
        return crate::audit::ControlledCanaryHistoryReport {
            history_status: "controlled_canary_history_empty".to_string(),
            recommendation_status: "controlled_canary_not_exercised".to_string(),
            runs_with_canary_requested: 0,
            runs_with_completed_passes: 0,
            total_requested_passes: 0,
            total_completed_passes: 0,
            total_failed_passes: 0,
            clear_runs: 0,
            marker_detection_runs: 0,
            mixed_or_inconclusive_runs: 0,
            backend_unsupported_runs: 0,
            latest_execution_status: "controlled_canary_not_run_yet".to_string(),
            latest_aggregate_signal_status: "controlled_canary_not_run_yet".to_string(),
            summary:
                "NullContext has not yet recorded repeated dedicated controlled canary history for this scope."
                    .to_string(),
            notes: vec![
                "Dedicated helper-canary guidance becomes meaningful only after at least one run in this scope requests the canary harness."
                    .to_string(),
            ],
        };
    }

    let runs_with_canary_requested = matching_entries
        .iter()
        .filter(|entry| entry.canary_requested_passes > 0)
        .count() as u32;
    let runs_with_completed_passes = matching_entries
        .iter()
        .filter(|entry| entry.canary_completed_passes > 0)
        .count() as u32;
    let total_requested_passes = matching_entries
        .iter()
        .map(|entry| entry.canary_requested_passes)
        .sum::<u32>();
    let total_completed_passes = matching_entries
        .iter()
        .map(|entry| entry.canary_completed_passes)
        .sum::<u32>();
    let total_failed_passes = matching_entries
        .iter()
        .map(|entry| entry.canary_failed_passes)
        .sum::<u32>();
    let clear_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.canary_aggregate_signal_status == "controlled_canary_all_completed_passes_clear"
        })
        .count() as u32;
    let marker_detection_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.canary_aggregate_signal_status
                == "controlled_canary_markers_detected_across_passes"
        })
        .count() as u32;
    let backend_unsupported_runs = matching_entries
        .iter()
        .filter(|entry| {
            entry.canary_aggregate_signal_status
                == "controlled_canary_backend_unsupported_across_passes"
        })
        .count() as u32;
    let mixed_or_inconclusive_runs = matching_entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.canary_aggregate_signal_status.as_str(),
                "controlled_canary_mixed_clear_and_inconclusive"
                    | "controlled_canary_inconclusive_across_passes"
                    | "controlled_canary_request_failed"
                    | "controlled_canary_shutdown_failed"
                    | "controlled_canary_helper_failed"
                    | "controlled_canary_all_passes_failed"
                    | "controlled_canary_not_run_yet"
            ) || entry.canary_execution_status != "controlled_canary_completed"
        })
        .count() as u32;
    let latest_execution_status = matching_entries
        .first()
        .map(|entry| entry.canary_execution_status.clone())
        .unwrap_or_else(|| "controlled_canary_not_run_yet".to_string());
    let latest_aggregate_signal_status = matching_entries
        .first()
        .map(|entry| entry.canary_aggregate_signal_status.clone())
        .unwrap_or_else(|| "controlled_canary_not_run_yet".to_string());

    let recommendation_status = if runs_with_canary_requested == 0 {
        "controlled_canary_not_exercised"
    } else if marker_detection_runs > 0 {
        "controlled_canary_marker_persistence_detected_across_history"
    } else if clear_runs >= 2
        && mixed_or_inconclusive_runs == 0
        && backend_unsupported_runs == 0
        && runs_with_completed_passes >= 2
    {
        "controlled_canary_repeated_clear_history"
    } else if clear_runs == 1
        && runs_with_canary_requested == 1
        && mixed_or_inconclusive_runs == 0
        && backend_unsupported_runs == 0
    {
        "controlled_canary_single_clear_run_only"
    } else if backend_unsupported_runs == runs_with_canary_requested {
        "controlled_canary_backend_unsupported_across_history"
    } else if runs_with_completed_passes == 0 {
        "controlled_canary_no_completed_history"
    } else {
        "controlled_canary_mixed_or_inconclusive_history"
    };

    let summary = match recommendation_status {
        "controlled_canary_repeated_clear_history" => format!(
            "Dedicated helper-canary history is currently strongest in this scope: {} run(s) completed with fully clear repeated canary passes and no repeated marker detections.",
            clear_runs
        ),
        "controlled_canary_marker_persistence_detected_across_history" => format!(
            "Dedicated helper-canary history still detected markers in {} run(s), so the repeated canary evidence remains a strong negative signal.",
            marker_detection_runs
        ),
        "controlled_canary_single_clear_run_only" => {
            "Dedicated helper-canary history has one fully clear run so far, but it is not repeated enough yet to count as strong release-gating evidence."
                .to_string()
        }
        "controlled_canary_backend_unsupported_across_history" => {
            "Dedicated helper-canary runs were requested in this scope, but the direct-scan backend remained unsupported across the recorded history."
                .to_string()
        }
        "controlled_canary_no_completed_history" => {
            "Dedicated helper-canary runs were requested, but none completed cleanly enough yet to provide strong repeated evidence."
                .to_string()
        }
        _ => {
            "Dedicated helper-canary history is present for this scope, but the repeated results are still mixed or inconclusive."
                .to_string()
        }
    };

    let mut notes = vec![
        format!(
            "Requested passes: {}, completed passes: {}, failed passes: {} across {} run(s) with requested canary validation.",
            total_requested_passes,
            total_completed_passes,
            total_failed_passes,
            runs_with_canary_requested
        ),
        format!(
            "Latest canary execution: {}. Latest aggregate signal: {}.",
            latest_execution_status.replace('_', " "),
            latest_aggregate_signal_status.replace('_', " ")
        ),
    ];
    if backend_unsupported_runs > 0 {
        notes.push(
            "Some helper-canary history remained backend-limited, so absence of marker detections in those runs should not be treated as a clear pass."
                .to_string(),
        );
    }
    if mixed_or_inconclusive_runs > 0 {
        notes.push(
            "Mixed or inconclusive helper-canary runs still exist in this scope, which means the repeated canary story is not fully clean yet."
                .to_string(),
        );
    }

    crate::audit::ControlledCanaryHistoryReport {
        history_status: "controlled_canary_history_recorded".to_string(),
        recommendation_status: recommendation_status.to_string(),
        runs_with_canary_requested,
        runs_with_completed_passes,
        total_requested_passes,
        total_completed_passes,
        total_failed_passes,
        clear_runs,
        marker_detection_runs,
        mixed_or_inconclusive_runs,
        backend_unsupported_runs,
        latest_execution_status,
        latest_aggregate_signal_status,
        summary,
        notes,
    }
}

fn build_stage_recommendation(
    stage_trends: &[MemoryValidationStageTrendReport],
) -> MemoryValidationStageRecommendationReport {
    if stage_trends.is_empty() {
        return default_stage_recommendation_report("no_stage_history_available");
    }

    let mut ranked_stages = stage_trends
        .iter()
        .map(|trend| (trend, stage_effectiveness_score(trend)))
        .collect::<Vec<_>>();
    ranked_stages.sort_by(|(left_trend, left_score), (right_trend, right_score)| {
        stage_selection_fitness_priority(&right_trend.selection_fitness_status)
            .cmp(&stage_selection_fitness_priority(
                &left_trend.selection_fitness_status,
            ))
            .then_with(|| {
                stage_evidence_support_priority(&right_trend.evidence_support_status).cmp(
                    &stage_evidence_support_priority(&left_trend.evidence_support_status),
                )
            })
            .then_with(|| right_score.total_cmp(left_score))
            .then_with(|| {
                right_trend
                    .avg_validation_score
                    .total_cmp(&left_trend.avg_validation_score)
            })
            .then_with(|| right_trend.runs_recorded.cmp(&left_trend.runs_recorded))
            .then_with(|| {
                left_trend
                    .marker_detection_runs
                    .cmp(&right_trend.marker_detection_runs)
            })
    });

    let Some((trend, effectiveness_score)) = ranked_stages.first().copied() else {
        return default_stage_recommendation_report("no_stage_history_available");
    };
    let runner_up = ranked_stages.get(1).copied();
    let effectiveness_gap =
        runner_up.map(|(_, runner_up_score)| effectiveness_score - runner_up_score);
    let avg_validation_score_gap = runner_up.map(|(runner_up_trend, _)| {
        trend.avg_validation_score - runner_up_trend.avg_validation_score
    });
    let marker_detection_gap = runner_up.map(|(runner_up_trend, _)| {
        runner_up_trend.marker_detection_runs as i32 - trend.marker_detection_runs as i32
    });

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
    } else if trend.selection_fitness_status
        == "selection_fitness_demoted_runtime_global_or_fallback_only"
    {
        "recommendation_limited_by_low_fitness_evidence"
    } else if trend.selection_fitness_status == "selection_fitness_provisional_visibility_only" {
        "recommendation_limited_by_visibility_only_history"
    } else {
        "recommendation_available"
    };
    let clean_claim_status = if trend.runs_recorded < 2 {
        "clean_claim_blocked_by_insufficient_repeated_runs"
    } else if trend.marker_detection_runs > 0 {
        "clean_claim_blocked_by_marker_persistence"
    } else if trend.worsened_runs > 0 {
        "clean_claim_blocked_by_worsened_history"
    } else if trend.inconclusive_runs > 0 {
        "clean_claim_blocked_by_inconclusive_history"
    } else if trend.selection_fitness_status
        == "selection_fitness_demoted_runtime_global_or_fallback_only"
    {
        "clean_claim_blocked_by_runtime_global_or_fallback_evidence"
    } else if trend.selection_fitness_status
        == "selection_fitness_provisional_stage_local_cleanup_signal_backed"
    {
        "clean_claim_blocked_by_cleanup_signal_only_evidence"
    } else if trend.selection_fitness_status == "selection_fitness_provisional_visibility_only" {
        "clean_claim_blocked_by_visibility_only_evidence"
    } else if effectiveness_gap.is_some_and(|gap| gap <= 3.0) {
        "clean_claim_blocked_by_narrow_lead_over_runner_up"
    } else if recommendation_status != "recommendation_available" {
        "clean_claim_blocked_by_limited_recommendation_status"
    } else {
        "clean_claim_eligible_under_current_thresholds"
    };
    let evidence_support_status = if trend.runs_recorded < 2 {
        "recommendation_evidence_waiting_for_repeated_runs"
    } else if trend.marker_detection_runs > 0 {
        "recommendation_evidence_limited_by_marker_persistence"
    } else if trend.clear_marker_support_runs > 0 && trend.stage_local_scan_clear_runs > 0 {
        "recommendation_evidence_supported_by_stage_local_marker_clearance"
    } else if trend.clear_marker_support_runs > 0 || trend.helper_scan_clear_runs > 0 {
        "recommendation_evidence_supported_by_marker_clearance_history"
    } else if trend.inconclusive_runs * 2 >= trend.runs_recorded {
        "recommendation_evidence_limited_by_inconclusive_history"
    } else if trend.stage_local_scan_runs == 0 && trend.session_fallback_scan_runs > 0 {
        "recommendation_evidence_limited_to_session_fallback_scans"
    } else if trend.cleanup_signal_runtime_global_only_runs > 0
        && trend.cleanup_signal_runtime_global_only_runs
            + trend.cleanup_signal_declared_only_runs
            + trend.cleanup_signal_scope_unavailable_runs
            == trend.runs_recorded
        && trend.clear_marker_support_runs == 0
        && trend.stage_local_scan_clear_runs == 0
    {
        "recommendation_evidence_limited_to_runtime_global_cleanup_signals"
    } else if trend.cleanup_signal_strong_runs > 0 || trend.cleanup_signal_partial_runs > 0 {
        "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance"
    } else if trend.strong_or_moderate_runs > 0 || trend.improved_runs > 0 {
        "recommendation_evidence_gpu_only_without_marker_support"
    } else {
        "recommendation_evidence_limited_mixed_history"
    };

    let mut notes = vec![
        format!(
            "Compared across {} cleanup stage(s) recorded in this model/platform/GPU-offload scope.",
            stage_trends.len()
        ),
        "This recommendation is comparative operator guidance, not proof of full RAM or VRAM sanitization."
            .to_string(),
        "Repeated cleanup-stage ranking now explicitly prefers stronger selection-fitness classes before evidence-support classes and raw numeric effectiveness."
            .to_string(),
        format!(
            "Selection fitness: {}.",
            trend.selection_fitness_status.replace('_', " ")
        ),
        trend.selection_fitness_summary.clone(),
    ];
    if let Some((runner_up_trend, runner_up_score)) = runner_up {
        notes.push(format!(
            "Runner-up stage: {} with selection fitness {}, evidence class {}, effectiveness score {:.1}, and average validation score {:.1}/100.",
            runner_up_trend.stage_label,
            runner_up_trend.selection_fitness_status.replace('_', " "),
            runner_up_trend.evidence_support_status.replace('_', " "),
            runner_up_score,
            runner_up_trend.avg_validation_score
        ));
    } else {
        notes.push(
            "No runner-up stage exists yet in this scope, so the current recommendation is based on a single recorded cleanup stage trend."
                .to_string(),
        );
    }

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
    match evidence_support_status {
        "recommendation_evidence_supported_by_stage_local_marker_clearance" => notes.push(
            "The leading stage is backed by repeated stage-local clear marker scans, which is the strongest current recommendation-evidence class in this report."
                .to_string(),
        ),
        "recommendation_evidence_supported_by_marker_clearance_history" => notes.push(
            "The leading stage is backed by repeated clear marker history, but some of that support still comes from helper or broader repeated evidence instead of only stage-local scans."
                .to_string(),
        ),
        "recommendation_evidence_limited_to_runtime_global_cleanup_signals" => notes.push(
            "The leading stage is backed by repeated allocator/KV/model cleanup signals, but those signals still belong to whole-runtime lifecycle evidence rather than a stage-local internal cleanup hook."
                .to_string(),
        ),
        "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance" => notes.push(
            "The leading stage is currently supported more by allocator/KV/model cleanup-path signals than by repeated direct marker-clearance evidence."
                .to_string(),
        ),
        "recommendation_evidence_gpu_only_without_marker_support" => notes.push(
            "The leading stage is currently recommended mostly from GPU/process evidence trends rather than repeated direct marker-clearance evidence."
                .to_string(),
        ),
        "recommendation_evidence_limited_to_session_fallback_scans" => notes.push(
            "The leading stage still leans on session-fallback scan context, so the recommendation is not yet backed by consistently isolated stage-local marker evidence."
                .to_string(),
        ),
        "recommendation_evidence_limited_by_inconclusive_history" => notes.push(
            "Too much of the repeated history for the leading stage is still inconclusive to treat the recommendation evidence as strong."
                .to_string(),
        ),
        "recommendation_evidence_limited_by_marker_persistence" => notes.push(
            "Marker persistence is still present in the leading stage history, so the recommendation evidence is explicitly not clean."
                .to_string(),
        ),
        _ => {}
    }
    if trend.stage_local_scan_runs == 0 && trend.session_fallback_scan_runs > 0 {
        notes.push(
            "The leading stage still relies entirely on session-fallback process-scan context rather than truly stage-local RAM-side scan evidence."
                .to_string(),
        );
    } else if trend.stage_local_scan_runs > 0 {
        notes.push(format!(
            "The leading stage has {} stage-local direct scan run(s): {} clear, {} marker-detected, and {} limited.",
            trend.stage_local_scan_runs,
            trend.stage_local_scan_clear_runs,
            trend.stage_local_scan_marker_detection_runs,
            trend.stage_local_scan_limited_runs
        ));
    }
    if trend.cleanup_signal_strong_runs > 0 {
        notes.push(format!(
            "The leading stage is backed by strong allocator/KV/model cleanup-signal support in {} repeated run(s).",
            trend.cleanup_signal_strong_runs
        ));
    } else if trend.cleanup_signal_partial_runs > 0 {
        notes.push(format!(
            "The leading stage only has partial allocator/KV cleanup-signal support so far: {} partial run(s), {} limited/unavailable run(s).",
            trend.cleanup_signal_partial_runs,
            trend.cleanup_signal_limited_runs
        ));
    } else {
        notes.push(
            "The leading stage does not yet have direct allocator/KV cleanup-signal support in its repeated history."
                .to_string(),
        );
    }
    if let Some(gap) = effectiveness_gap {
        if gap <= 3.0 {
            notes.push(
                "The lead over the runner-up is narrow, so the recommendation should be treated as tentative rather than dominant."
                    .to_string(),
            );
        } else if gap >= 12.0 {
            notes.push(
                "The lead over the runner-up is materially wider than a small scoring fluctuation, which makes this recommendation more actionable."
                    .to_string(),
            );
        }
    }
    if let Some((runner_up_trend, _)) = runner_up {
        let leading_priority = stage_selection_fitness_priority(&trend.selection_fitness_status);
        let runner_up_priority =
            stage_selection_fitness_priority(&runner_up_trend.selection_fitness_status);
        if leading_priority > runner_up_priority {
            notes.push(
                "The leading stage outranked the runner-up partly because it belongs to a stronger selection-fitness class, not just because of raw score."
                    .to_string(),
            );
        } else if leading_priority < runner_up_priority {
            notes.push(
                "The runner-up had a stronger selection-fitness class, but the current ordering still favored the leading stage on the remaining ranking fields."
                    .to_string(),
            );
        }
    }

    let summary = match recommendation_status {
        "recommendation_available" => format!(
            "{} is the current best repeated cleanup stage for this scope based on {} run(s) with an average validation score of {:.1}/100 and no repeated marker detections{}.",
            trend.stage_label,
            trend.runs_recorded,
            trend.avg_validation_score,
            runner_up
                .map(|(runner_up_trend, _)| format!(
                    "; it currently leads {}",
                    runner_up_trend.stage_label
                ))
                .unwrap_or_default()
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
        "recommendation_limited_by_low_fitness_evidence" => format!(
            "{} currently ranks highest overall, but its repeated evidence is still dominated by runtime-global-only or session-fallback support, so the win remains demoted.",
            trend.stage_label
        ),
        "recommendation_limited_by_visibility_only_history" => format!(
            "{} currently ranks highest overall, but the repeated history is still mostly visibility-only rather than marker-backed.",
            trend.stage_label
        ),
        _ => format!(
            "{} currently ranks highest overall, but the repeated evidence still looks mixed rather than clearly improved.",
            trend.stage_label
        ),
    };
    let clean_claim_summary = match clean_claim_status {
        "clean_claim_eligible_under_current_thresholds" => format!(
            "{} is not only the current best repeated stage, but also the current cleanest stage candidate under the in-report thresholds.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_marker_persistence" => format!(
            "{} is currently the best repeated stage, but it is not a clean stage candidate because marker persistence still exists in its history.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_worsened_history" => format!(
            "{} is currently the best repeated stage, but it is not a clean stage candidate because its history still includes worsened runs.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_inconclusive_history" => format!(
            "{} is currently the best repeated stage, but it is not a clean stage candidate because some repeated runs remain inconclusive.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_runtime_global_or_fallback_evidence" => format!(
            "{} is currently the best repeated stage, but it is not a clean stage candidate because its support is still dominated by runtime-global-only or session-fallback evidence.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_cleanup_signal_only_evidence" => format!(
            "{} is currently the best repeated stage, but it is not a clean stage candidate because it still relies on cleanup-signal support without equally strong repeated direct marker-clearance evidence.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_visibility_only_evidence" => format!(
            "{} is currently the best repeated stage, but it is not a clean stage candidate because it still depends mostly on visibility-only evidence rather than direct marker clearance.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_narrow_lead_over_runner_up" => format!(
            "{} is currently ahead, but the lead over the runner-up is too narrow to treat it as a clearly cleaner stage yet.",
            trend.stage_label
        ),
        "clean_claim_blocked_by_insufficient_repeated_runs" => format!(
            "{} is currently the best stage available, but there are not yet enough repeated runs to treat it as a clean stage candidate.",
            trend.stage_label
        ),
        _ => format!(
            "{} is currently the best stage available, but the recommendation still does not support a stronger clean-stage claim yet.",
            trend.stage_label
        ),
    };
    let evidence_support_summary = match evidence_support_status {
        "recommendation_evidence_supported_by_stage_local_marker_clearance" => format!(
            "{} is currently backed by repeated stage-local clear marker scans, so the recommendation rests on direct marker-clearance evidence instead of GPU-only improvement trends.",
            trend.stage_label
        ),
        "recommendation_evidence_supported_by_marker_clearance_history" => format!(
            "{} is currently backed by repeated clear marker history, but that support is not yet entirely stage-local in every repeated run.",
            trend.stage_label
        ),
        "recommendation_evidence_limited_to_runtime_global_cleanup_signals" => format!(
            "{} is currently backed by repeated allocator/KV/model cleanup signals, but those signals are still runtime-global-only and do not yet prove that the winning cleanup stage itself triggered stage-local internal cleanup.",
            trend.stage_label
        ),
        "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance" => format!(
            "{} is currently supported by repeated allocator/KV/model cleanup-path signals, but it still lacks equally strong repeated direct marker-clearance evidence.",
            trend.stage_label
        ),
        "recommendation_evidence_gpu_only_without_marker_support" => format!(
            "{} is currently recommended mostly from repeated GPU/process improvement trends; direct repeated marker-clearance support is still missing.",
            trend.stage_label
        ),
        "recommendation_evidence_limited_by_marker_persistence" => format!(
            "{} still has repeated marker persistence in its history, so the recommendation cannot be treated as clean evidence yet.",
            trend.stage_label
        ),
        "recommendation_evidence_limited_by_inconclusive_history" => format!(
            "{} still has too much inconclusive repeated history for NullContext to treat the recommendation evidence as strong yet.",
            trend.stage_label
        ),
        "recommendation_evidence_limited_to_session_fallback_scans" => format!(
            "{} is still recommended partly from session-fallback scan context rather than consistently isolated stage-local marker evidence.",
            trend.stage_label
        ),
        "recommendation_evidence_waiting_for_repeated_runs" => format!(
            "{} is the current leading stage, but NullContext does not yet have enough repeated runs to classify its recommendation evidence strongly.",
            trend.stage_label
        ),
        _ => format!(
            "{} currently leads, but the repeated evidence is still mixed enough that NullContext cannot classify the recommendation support as strong yet.",
            trend.stage_label
        ),
    };

    MemoryValidationStageRecommendationReport {
        recommendation_status: recommendation_status.to_string(),
        clean_claim_status: clean_claim_status.to_string(),
        selection_fitness_status: trend.selection_fitness_status.clone(),
        selection_fitness_summary: trend.selection_fitness_summary.clone(),
        evidence_support_status: evidence_support_status.to_string(),
        evidence_support_summary,
        stage_id: Some(trend.stage_id.clone()),
        stage_label: Some(trend.stage_label.clone()),
        stage_kind: Some(trend.stage_kind.clone()),
        runner_up_stage_id: runner_up.map(|(runner_up_trend, _)| runner_up_trend.stage_id.clone()),
        runner_up_stage_label: runner_up
            .map(|(runner_up_trend, _)| runner_up_trend.stage_label.clone()),
        runner_up_stage_kind: runner_up
            .map(|(runner_up_trend, _)| runner_up_trend.stage_kind.clone()),
        compared_stage_count: stage_trends.len() as u32,
        runs_recorded: trend.runs_recorded,
        avg_validation_score: Some(trend.avg_validation_score),
        effectiveness_score: Some(effectiveness_score),
        runner_up_effectiveness_score: runner_up.map(|(_, score)| score),
        effectiveness_gap,
        avg_validation_score_gap,
        marker_detection_gap,
        improved_runs: trend.improved_runs,
        unchanged_runs: trend.unchanged_runs,
        worsened_runs: trend.worsened_runs,
        inconclusive_runs: trend.inconclusive_runs,
        marker_detection_runs: trend.marker_detection_runs,
        summary,
        clean_claim_summary,
        notes,
    }
}

fn build_release_gate(
    cleanup_stage_recommendation: &MemoryValidationStageRecommendationReport,
    controlled_canary_history: &crate::audit::ControlledCanaryHistoryReport,
) -> ValidationReleaseGateReport {
    let min_stage_runs_required = 2;
    let min_clear_canary_runs_required = 2;
    let max_marker_detection_runs_allowed_for_clean_claim = 0;
    let max_worsened_runs_allowed_for_clean_stage = 0;
    let max_inconclusive_runs_allowed_for_clean_stage = 0;
    let required_stage_evidence_support_statuses = vec![
        "recommendation_evidence_supported_by_stage_local_marker_clearance".to_string(),
        "recommendation_evidence_supported_by_marker_clearance_history".to_string(),
    ];
    let stage_evidence_support_meets_gate = required_stage_evidence_support_statuses
        .iter()
        .any(|status| status == &cleanup_stage_recommendation.evidence_support_status);

    let stage_gate_passed = cleanup_stage_recommendation.runs_recorded >= min_stage_runs_required
        && cleanup_stage_recommendation.marker_detection_runs
            <= max_marker_detection_runs_allowed_for_clean_claim
        && cleanup_stage_recommendation.worsened_runs <= max_worsened_runs_allowed_for_clean_stage
        && cleanup_stage_recommendation.inconclusive_runs
            <= max_inconclusive_runs_allowed_for_clean_stage
        && stage_evidence_support_meets_gate
        && matches!(
            cleanup_stage_recommendation.clean_claim_status.as_str(),
            "clean_claim_eligible_under_current_thresholds"
        );

    let cleanup_stage_gate_status = if cleanup_stage_recommendation.runs_recorded
        < min_stage_runs_required
    {
        "cleanup_stage_gate_waiting_for_more_repeated_runs"
    } else if cleanup_stage_recommendation.marker_detection_runs
        > max_marker_detection_runs_allowed_for_clean_claim
    {
        "cleanup_stage_gate_blocked_by_marker_persistence"
    } else if cleanup_stage_recommendation.worsened_runs > max_worsened_runs_allowed_for_clean_stage
    {
        "cleanup_stage_gate_blocked_by_worsened_runs"
    } else if cleanup_stage_recommendation.inconclusive_runs
        > max_inconclusive_runs_allowed_for_clean_stage
    {
        "cleanup_stage_gate_blocked_by_inconclusive_runs"
    } else if !stage_evidence_support_meets_gate {
        match cleanup_stage_recommendation
            .evidence_support_status
            .as_str()
        {
            "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance" => {
                "cleanup_stage_gate_blocked_by_cleanup_signal_only_evidence"
            }
            "recommendation_evidence_limited_to_runtime_global_cleanup_signals" => {
                "cleanup_stage_gate_blocked_by_runtime_global_cleanup_signal_evidence"
            }
            "recommendation_evidence_gpu_only_without_marker_support" => {
                "cleanup_stage_gate_blocked_by_gpu_only_recommendation_evidence"
            }
            "recommendation_evidence_limited_to_session_fallback_scans" => {
                "cleanup_stage_gate_blocked_by_fallback_scan_only_evidence"
            }
            "recommendation_evidence_limited_by_inconclusive_history" => {
                "cleanup_stage_gate_blocked_by_inconclusive_recommendation_evidence"
            }
            "recommendation_evidence_waiting_for_repeated_runs" => {
                "cleanup_stage_gate_waiting_for_recommendation_evidence_history"
            }
            "recommendation_evidence_limited_by_marker_persistence" => {
                "cleanup_stage_gate_blocked_by_marker_persistent_recommendation_evidence"
            }
            _ => "cleanup_stage_gate_blocked_by_non_marker_backed_recommendation_evidence",
        }
    } else if cleanup_stage_recommendation.clean_claim_status
        == "clean_claim_blocked_by_narrow_lead_over_runner_up"
    {
        "cleanup_stage_gate_blocked_by_narrow_lead_over_runner_up"
    } else if cleanup_stage_recommendation.recommendation_status != "recommendation_available" {
        "cleanup_stage_gate_limited_by_recommendation_status"
    } else {
        "cleanup_stage_gate_passed"
    };

    let controlled_canary_gate_passed = controlled_canary_history.clear_runs
        >= min_clear_canary_runs_required
        && controlled_canary_history.marker_detection_runs
            <= max_marker_detection_runs_allowed_for_clean_claim
        && controlled_canary_history.mixed_or_inconclusive_runs == 0
        && controlled_canary_history.backend_unsupported_runs == 0
        && controlled_canary_history.runs_with_completed_passes >= min_clear_canary_runs_required;

    let controlled_canary_gate_status = if controlled_canary_history.runs_with_canary_requested == 0
    {
        "controlled_canary_gate_not_exercised"
    } else if controlled_canary_history.runs_with_completed_passes < min_clear_canary_runs_required
    {
        "controlled_canary_gate_waiting_for_more_completed_history"
    } else if controlled_canary_history.marker_detection_runs
        > max_marker_detection_runs_allowed_for_clean_claim
    {
        "controlled_canary_gate_blocked_by_marker_persistence"
    } else if controlled_canary_history.backend_unsupported_runs > 0 {
        "controlled_canary_gate_blocked_by_backend_unsupported_runs"
    } else if controlled_canary_history.mixed_or_inconclusive_runs > 0 {
        "controlled_canary_gate_blocked_by_mixed_or_inconclusive_runs"
    } else if controlled_canary_history.clear_runs < min_clear_canary_runs_required {
        "controlled_canary_gate_waiting_for_more_clear_runs"
    } else {
        "controlled_canary_gate_passed"
    };

    let gate_status = if stage_gate_passed && controlled_canary_gate_passed {
        "release_gate_repeated_evidence_threshold_met"
    } else if !stage_gate_passed && !controlled_canary_gate_passed {
        "release_gate_blocked_on_stage_and_canary_thresholds"
    } else if !stage_gate_passed {
        "release_gate_blocked_on_cleanup_stage_thresholds"
    } else {
        "release_gate_blocked_on_controlled_canary_thresholds"
    };

    let summary = match gate_status {
        "release_gate_repeated_evidence_threshold_met" => {
            "Repeated validation evidence currently meets the in-report release-gating threshold for both the leading cleanup stage and the dedicated controlled canary history."
                .to_string()
        }
        "release_gate_blocked_on_stage_and_canary_thresholds" => {
            "Repeated validation evidence does not yet meet the in-report release-gating threshold for either the leading cleanup stage or the dedicated controlled canary history."
                .to_string()
        }
        "release_gate_blocked_on_cleanup_stage_thresholds" => {
            "Dedicated controlled canary history is stronger than the cleanup-stage recommendation right now, but the cleanup-stage threshold is not met yet."
                .to_string()
        }
        _ => {
            "The cleanup-stage recommendation is stronger than the dedicated controlled canary history right now, but the repeated controlled canary threshold is not met yet."
                .to_string()
        }
    };

    let release_readiness_status = if stage_gate_passed && controlled_canary_gate_passed {
        "release_readiness_repeated_evidence_ready_under_current_thresholds".to_string()
    } else if stage_gate_passed && !controlled_canary_gate_passed {
        "release_readiness_waiting_on_controlled_canary_history".to_string()
    } else if !stage_gate_passed && controlled_canary_gate_passed {
        "release_readiness_waiting_on_cleanup_stage_history".to_string()
    } else if cleanup_stage_gate_status.contains("waiting_for_more")
        || cleanup_stage_gate_status.contains("waiting_for_recommendation_evidence_history")
        || controlled_canary_gate_status.contains("waiting_for_more")
        || controlled_canary_gate_status == "controlled_canary_gate_not_exercised"
    {
        "release_readiness_waiting_for_more_history".to_string()
    } else if cleanup_stage_gate_status.contains("marker")
        || controlled_canary_gate_status.contains("marker")
    {
        "release_readiness_blocked_by_marker_persistence".to_string()
    } else if cleanup_stage_gate_status.contains("cleanup_signal_only")
        || cleanup_stage_gate_status.contains("runtime_global_cleanup_signal")
        || cleanup_stage_gate_status.contains("gpu_only")
        || cleanup_stage_gate_status.contains("fallback_scan_only")
    {
        "release_readiness_blocked_by_evidence_quality".to_string()
    } else if cleanup_stage_gate_status.contains("worsened")
        || cleanup_stage_gate_status.contains("inconclusive")
        || controlled_canary_gate_status.contains("mixed_or_inconclusive")
        || controlled_canary_gate_status.contains("backend_unsupported")
    {
        "release_readiness_blocked_by_inconsistent_or_unsupported_history".to_string()
    } else {
        "release_readiness_blocked_mixed".to_string()
    };

    let release_readiness_summary = match release_readiness_status.as_str() {
        "release_readiness_repeated_evidence_ready_under_current_thresholds" => {
            "Under the current in-report thresholds, repeated cleanup-stage evidence and repeated controlled-canary evidence are both strong enough to support a release-ready validation story for this scope."
                .to_string()
        }
        "release_readiness_waiting_on_controlled_canary_history" => {
            "Cleanup-stage history is currently stronger than the controlled-canary history for this scope; the release story is waiting on canary repetition rather than on stage scoring."
                .to_string()
        }
        "release_readiness_waiting_on_cleanup_stage_history" => {
            "Controlled-canary history is currently stronger than the cleanup-stage recommendation for this scope; the release story is waiting on repeated stage evidence rather than on canary repetition."
                .to_string()
        }
        "release_readiness_waiting_for_more_history" => {
            "The current validation story still needs more repeated history before it can be treated as release-ready for this scope."
                .to_string()
        }
        "release_readiness_blocked_by_marker_persistence" => {
            "Repeated validation is currently blocked by marker persistence, so the release story is not clean enough yet for this scope."
                .to_string()
        }
        "release_readiness_blocked_by_evidence_quality" => {
            "Repeated validation exists, but it is still backed by evidence classes that are weaker than the marker-backed threshold required for a clean release story."
                .to_string()
        }
        "release_readiness_blocked_by_inconsistent_or_unsupported_history" => {
            "Repeated validation is currently blocked by inconsistent, regressive, inconclusive, or backend-limited history, so the release story remains unstable for this scope."
                .to_string()
        }
        _ => {
            "Repeated validation remains mixed for this scope, so the release story still requires manual reading of the stage gate and controlled-canary gate details."
                .to_string()
        }
    };

    let notes = vec![
        format!(
            "Cleanup-stage threshold requires at least {} repeated runs, {} marker-detection runs, {} worsened runs, and {} inconclusive runs for the recommended stage.",
            min_stage_runs_required,
            max_marker_detection_runs_allowed_for_clean_claim,
            max_worsened_runs_allowed_for_clean_stage,
            max_inconclusive_runs_allowed_for_clean_stage
        ),
        format!(
            "Cleanup-stage release gating currently only accepts recommendation evidence classes backed by repeated marker-clearance history: {}.",
            required_stage_evidence_support_statuses
                .iter()
                .map(|status| status.replace('_', " "))
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        format!(
            "Controlled-canary threshold requires at least {} clear completed runs, {} marker-detection runs, and no mixed/inconclusive or backend-unsupported history.",
            min_clear_canary_runs_required,
            max_marker_detection_runs_allowed_for_clean_claim
        ),
        format!(
            "Current cleanup-stage gate: {}. Current controlled-canary gate: {}. Current recommendation evidence support: {}.",
            cleanup_stage_gate_status.replace('_', " "),
            controlled_canary_gate_status.replace('_', " "),
            cleanup_stage_recommendation
                .evidence_support_status
                .replace('_', " ")
        ),
    ];

    ValidationReleaseGateReport {
        gate_status: gate_status.to_string(),
        cleanup_stage_gate_status: cleanup_stage_gate_status.to_string(),
        controlled_canary_gate_status: controlled_canary_gate_status.to_string(),
        release_readiness_status,
        release_readiness_summary,
        min_stage_runs_required,
        min_clear_canary_runs_required,
        max_marker_detection_runs_allowed_for_clean_claim,
        max_worsened_runs_allowed_for_clean_stage,
        max_inconclusive_runs_allowed_for_clean_stage,
        required_stage_evidence_support_statuses,
        observed_stage_evidence_support_status: cleanup_stage_recommendation
            .evidence_support_status
            .clone(),
        stage_gate_passed,
        controlled_canary_gate_passed,
        summary,
        notes,
    }
}

fn default_release_gate_report() -> ValidationReleaseGateReport {
    ValidationReleaseGateReport {
        gate_status: "release_gate_not_derived".to_string(),
        cleanup_stage_gate_status: "cleanup_stage_gate_not_derived".to_string(),
        controlled_canary_gate_status: "controlled_canary_gate_not_derived".to_string(),
        release_readiness_status: "release_readiness_not_derived".to_string(),
        release_readiness_summary:
            "NullContext does not yet have enough repeated evidence to collapse this scope into one release-readiness verdict."
                .to_string(),
        min_stage_runs_required: 2,
        min_clear_canary_runs_required: 2,
        max_marker_detection_runs_allowed_for_clean_claim: 0,
        max_worsened_runs_allowed_for_clean_stage: 0,
        max_inconclusive_runs_allowed_for_clean_stage: 0,
        required_stage_evidence_support_statuses: vec![
            "recommendation_evidence_supported_by_stage_local_marker_clearance".to_string(),
            "recommendation_evidence_supported_by_marker_clearance_history".to_string(),
        ],
        observed_stage_evidence_support_status: "recommendation_evidence_not_derived".to_string(),
        stage_gate_passed: false,
        controlled_canary_gate_passed: false,
        summary:
            "NullContext does not yet have enough repeated evidence to derive explicit release-gating thresholds for this scope."
                .to_string(),
        notes: vec![
            "Release-gating guidance becomes meaningful only after repeated cleanup-stage and controlled-canary evidence exist in the same scope."
                .to_string(),
        ],
    }
}

fn stage_effectiveness_score(trend: &MemoryValidationStageTrendReport) -> f64 {
    trend.avg_validation_score
        + (trend.improved_runs as f64 * 8.0)
        + (trend.unchanged_runs as f64 * 1.5)
        + (trend.strong_or_moderate_runs as f64 * 4.0)
        + (trend.clear_marker_support_runs as f64 * 3.0)
        + (trend.helper_scan_clear_runs as f64 * 2.0)
        + (trend.cleanup_signal_strong_runs as f64 * 3.0)
        + (trend.cleanup_signal_partial_runs as f64 * 1.0)
        + (trend.stage_local_scan_clear_runs as f64 * 4.0)
        - (trend.worsened_runs as f64 * 12.0)
        - (trend.marker_detection_runs as f64 * 10.0)
        - (trend.helper_scan_marker_detection_runs as f64 * 8.0)
        - (trend.cleanup_signal_limited_runs as f64 * 1.0)
        - (trend.stage_local_scan_marker_detection_runs as f64 * 12.0)
        - (trend.stage_local_scan_limited_runs as f64 * 2.0)
        - (trend.session_fallback_scan_runs as f64 * 1.0)
        - (trend.inconclusive_runs as f64 * 2.0)
}

fn derive_stage_selection_fitness_status(trend: &MemoryValidationStageTrendReport) -> &'static str {
    if trend.runs_recorded < 2 {
        "selection_fitness_waiting_for_repeated_runs"
    } else if trend.marker_detection_runs > 0 || trend.worsened_runs > 0 {
        "selection_fitness_blocked_by_marker_persistence_or_regressions"
    } else if trend.stage_local_scan_clear_runs > 0 {
        "selection_fitness_preferred_stage_local_marker_backed"
    } else if trend.clear_marker_support_runs > 0 || trend.helper_scan_clear_runs > 0 {
        "selection_fitness_acceptable_marker_history_backed"
    } else if trend.inconclusive_runs * 2 >= trend.runs_recorded {
        "selection_fitness_limited_by_inconclusive_history"
    } else if (trend.cleanup_signal_strong_runs > 0 || trend.cleanup_signal_partial_runs > 0)
        && trend.cleanup_signal_stage_local_helper_runs > 0
        && trend.stage_local_scan_runs > 0
    {
        "selection_fitness_provisional_stage_local_cleanup_signal_backed"
    } else if (trend.stage_local_scan_runs == 0 && trend.session_fallback_scan_runs > 0)
        || (trend.cleanup_signal_runtime_global_only_runs > 0
            && trend.cleanup_signal_runtime_global_only_runs
                + trend.cleanup_signal_declared_only_runs
                + trend.cleanup_signal_scope_unavailable_runs
                == trend.runs_recorded)
    {
        "selection_fitness_demoted_runtime_global_or_fallback_only"
    } else {
        "selection_fitness_provisional_visibility_only"
    }
}

fn stage_selection_fitness_summary(trend: &MemoryValidationStageTrendReport) -> String {
    match trend.selection_fitness_status.as_str() {
        "selection_fitness_preferred_stage_local_marker_backed" => format!(
            "{} is currently a preferred repeated-stage candidate because it has repeated stage-local clear marker scans and no recorded regressions or marker persistence in this scope.",
            trend.stage_label
        ),
        "selection_fitness_acceptable_marker_history_backed" => format!(
            "{} is currently acceptable as a repeated-stage recommendation because it has clear marker history, but that support is not yet stage-local across every run.",
            trend.stage_label
        ),
        "selection_fitness_provisional_stage_local_cleanup_signal_backed" => format!(
            "{} is currently only provisional: it has some stage-local cleanup-signal support, but it still lacks equally strong repeated direct marker-clearance evidence.",
            trend.stage_label
        ),
        "selection_fitness_provisional_visibility_only" => format!(
            "{} is currently only provisional because the repeated history still leans more on visibility/process trends than on direct repeated marker-clearance evidence.",
            trend.stage_label
        ),
        "selection_fitness_demoted_runtime_global_or_fallback_only" => format!(
            "{} is currently demoted because its repeated history is still dominated by runtime-global-only cleanup signals or session-fallback scan context rather than stage-local proof.",
            trend.stage_label
        ),
        "selection_fitness_limited_by_inconclusive_history" => format!(
            "{} is currently limited because too much of its repeated history is still inconclusive to trust the selection strongly.",
            trend.stage_label
        ),
        "selection_fitness_blocked_by_marker_persistence_or_regressions" => format!(
            "{} is currently blocked from being treated as a clean repeated-stage candidate because its history still includes marker persistence or worsened outcomes.",
            trend.stage_label
        ),
        _ => format!(
            "{} has not yet been exercised enough times in this scope for NullContext to classify its repeated-stage fitness strongly.",
            trend.stage_label
        ),
    }
}

fn stage_evidence_support_priority(status: &str) -> u8 {
    match status {
        "recommendation_evidence_supported_by_stage_local_marker_clearance" => 7,
        "recommendation_evidence_supported_by_marker_clearance_history" => 6,
        "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance" => 5,
        "recommendation_evidence_limited_to_runtime_global_cleanup_signals" => 4,
        "recommendation_evidence_gpu_only_without_marker_support" => 3,
        "recommendation_evidence_limited_to_session_fallback_scans" => 2,
        "recommendation_evidence_limited_by_inconclusive_history" => 2,
        "recommendation_evidence_waiting_for_repeated_runs" => 1,
        "recommendation_evidence_limited_mixed_history" => 1,
        "recommendation_evidence_limited_by_marker_persistence" => 0,
        _ => 0,
    }
}

fn stage_selection_fitness_priority(status: &str) -> u8 {
    match status {
        "selection_fitness_preferred_stage_local_marker_backed" => 7,
        "selection_fitness_acceptable_marker_history_backed" => 6,
        "selection_fitness_provisional_stage_local_cleanup_signal_backed" => 5,
        "selection_fitness_provisional_visibility_only" => 4,
        "selection_fitness_demoted_runtime_global_or_fallback_only" => 3,
        "selection_fitness_limited_by_inconclusive_history" => 2,
        "selection_fitness_waiting_for_repeated_runs" => 1,
        "selection_fitness_blocked_by_marker_persistence_or_regressions" => 0,
        _ => 0,
    }
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
        clean_claim_status: "clean_claim_not_derived".to_string(),
        selection_fitness_status: "selection_fitness_not_derived".to_string(),
        selection_fitness_summary:
            "NullContext does not yet have enough repeated cleanup-stage history to classify whether the current recommendation would be preferred, provisional, demoted, or blocked."
                .to_string(),
        evidence_support_status: "recommendation_evidence_not_derived".to_string(),
        evidence_support_summary:
            "NullContext does not yet have enough repeated cleanup-stage history to classify whether the recommendation is marker-backed, only GPU-backed, or still too limited."
                .to_string(),
        stage_id: None,
        stage_label: None,
        stage_kind: None,
        runner_up_stage_id: None,
        runner_up_stage_label: None,
        runner_up_stage_kind: None,
        compared_stage_count: 0,
        runs_recorded: 0,
        avg_validation_score: None,
        effectiveness_score: None,
        runner_up_effectiveness_score: None,
        effectiveness_gap: None,
        avg_validation_score_gap: None,
        marker_detection_gap: None,
        improved_runs: 0,
        unchanged_runs: 0,
        worsened_runs: 0,
        inconclusive_runs: 0,
        marker_detection_runs: 0,
        summary: summary.to_string(),
        clean_claim_summary:
            "NullContext does not yet have enough repeated cleanup-stage history to support a stronger clean-stage claim for this scope."
                .to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::ControlledCanaryHistoryReport;

    fn make_trend(stage_id: &str, stage_label: &str) -> MemoryValidationStageTrendReport {
        MemoryValidationStageTrendReport {
            stage_id: stage_id.to_string(),
            stage_label: stage_label.to_string(),
            stage_kind: "test_stage".to_string(),
            runs_recorded: 3,
            avg_validation_score: 60.0,
            best_validation_score: 60,
            improved_runs: 2,
            unchanged_runs: 1,
            worsened_runs: 0,
            inconclusive_runs: 0,
            strong_or_moderate_runs: 2,
            marker_detection_runs: 0,
            clear_marker_support_runs: 0,
            helper_scan_runs: 0,
            helper_scan_clear_runs: 0,
            helper_scan_marker_detection_runs: 0,
            cleanup_signal_strong_runs: 0,
            cleanup_signal_partial_runs: 0,
            cleanup_signal_limited_runs: 0,
            cleanup_signal_runtime_global_only_runs: 0,
            cleanup_signal_stage_local_helper_runs: 0,
            cleanup_signal_declared_only_runs: 0,
            cleanup_signal_scope_unavailable_runs: 0,
            stage_local_scan_runs: 0,
            stage_local_scan_clear_runs: 0,
            stage_local_scan_marker_detection_runs: 0,
            stage_local_scan_limited_runs: 0,
            session_fallback_scan_runs: 0,
            latest_vram_evidence_status: "evidence_unchanged_not_observed".to_string(),
            latest_validation_verdict: "moderate_improvement_signal".to_string(),
            latest_marker_evidence_status: "marker_scan_clear_in_scanned_regions".to_string(),
            latest_cleanup_signal_support_status: "cleanup_signal_support_unavailable".to_string(),
            latest_cleanup_signal_support_scope_status: "cleanup_signal_scope_unavailable"
                .to_string(),
            latest_contributing_cleanup_signals: vec![],
            latest_process_scan_context_status: "marker_scan_clear_in_scanned_regions".to_string(),
            latest_process_scan_context_scope: "process_scan_context_unavailable".to_string(),
            selection_fitness_status: "selection_fitness_not_derived".to_string(),
            selection_fitness_summary: String::new(),
            evidence_support_status: "recommendation_evidence_limited_mixed_history".to_string(),
            evidence_support_summary: String::new(),
            summary: String::new(),
            notes: vec![],
        }
    }

    fn finalize_selection_fitness(
        mut trend: MemoryValidationStageTrendReport,
    ) -> MemoryValidationStageTrendReport {
        trend.selection_fitness_status = derive_stage_selection_fitness_status(&trend).to_string();
        trend.selection_fitness_summary = stage_selection_fitness_summary(&trend);
        trend
    }

    fn make_stage_result_entry(
        stage_id: &str,
        stage_label: &str,
    ) -> ValidationHistoryStageResultEntry {
        ValidationHistoryStageResultEntry {
            stage_id: stage_id.to_string(),
            stage_label: stage_label.to_string(),
            stage_kind: "test_stage".to_string(),
            vram_evidence_status: "evidence_unchanged_not_observed".to_string(),
            validation_score: 60,
            validation_verdict: "moderate_improvement_signal".to_string(),
            marker_evidence_status: "marker_evidence_limited_or_visibility_only".to_string(),
            process_scan_context_status: "process_scan_context_unavailable".to_string(),
            process_scan_context_scope: "process_scan_context_unavailable".to_string(),
            cleanup_signal_support_status: "cleanup_signal_support_unavailable".to_string(),
            cleanup_signal_support_scope_status: "cleanup_signal_scope_unavailable".to_string(),
            contributing_cleanup_signals: vec![],
            helper_process_scan_status: "helper_process_scan_not_recorded".to_string(),
        }
    }

    fn make_history_entry(
        session_id: &str,
        recorded_at: &str,
        stage_results: Vec<ValidationHistoryStageResultEntry>,
    ) -> ValidationHistoryEntry {
        ValidationHistoryEntry {
            session_id: session_id.to_string(),
            recorded_at: recorded_at.to_string(),
            started_at: recorded_at.to_string(),
            scope_key: "test-scope".to_string(),
            model_id: Some("test-model".to_string()),
            model_name: Some("Test Model".to_string()),
            platform: Some("test-platform".to_string()),
            gpu_offload_requested: Some(true),
            validation_status: "stage_scoring_ready".to_string(),
            process_scan_signal_status: "marker_scan_clear_in_scanned_regions".to_string(),
            canary_execution_status: "controlled_canary_completed".to_string(),
            canary_aggregate_signal_status: "controlled_canary_clear_across_passes".to_string(),
            canary_aggregate_process_scan_status: "no_markers_detected_in_scanned_regions"
                .to_string(),
            canary_requested_passes: 2,
            canary_completed_passes: 2,
            canary_failed_passes: 0,
            best_stage_score: 60,
            best_stage_verdict: "moderate_improvement_signal".to_string(),
            stage_results,
        }
    }

    fn make_controlled_canary_history(clear_runs: u32) -> ControlledCanaryHistoryReport {
        ControlledCanaryHistoryReport {
            history_status: "controlled_canary_history_ready".to_string(),
            recommendation_status: "controlled_canary_recommendation_available".to_string(),
            runs_with_canary_requested: clear_runs,
            runs_with_completed_passes: clear_runs,
            total_requested_passes: clear_runs * 2,
            total_completed_passes: clear_runs * 2,
            total_failed_passes: 0,
            clear_runs,
            marker_detection_runs: 0,
            mixed_or_inconclusive_runs: 0,
            backend_unsupported_runs: 0,
            latest_execution_status: "controlled_canary_completed".to_string(),
            latest_aggregate_signal_status: "controlled_canary_clear_across_passes".to_string(),
            summary: "test controlled canary history".to_string(),
            notes: vec![],
        }
    }

    #[test]
    fn recommendation_prefers_marker_backed_stage_over_fallback_heavy_stage() {
        let mut marker_backed = make_trend("marker-backed", "Marker Backed Stage");
        marker_backed.avg_validation_score = 72.0;
        marker_backed.best_validation_score = 72;
        marker_backed.stage_local_scan_runs = 3;
        marker_backed.stage_local_scan_clear_runs = 2;
        marker_backed.clear_marker_support_runs = 2;
        marker_backed.latest_process_scan_context_scope = "stage_local_cleanup_phase".to_string();
        marker_backed.evidence_support_status =
            "recommendation_evidence_supported_by_stage_local_marker_clearance".to_string();
        let marker_backed = finalize_selection_fitness(marker_backed);

        let mut fallback_heavy = make_trend("fallback-heavy", "Fallback Heavy Stage");
        fallback_heavy.avg_validation_score = 95.0;
        fallback_heavy.best_validation_score = 95;
        fallback_heavy.improved_runs = 3;
        fallback_heavy.strong_or_moderate_runs = 3;
        fallback_heavy.session_fallback_scan_runs = 3;
        fallback_heavy.latest_process_scan_context_scope = "session_fallback".to_string();
        fallback_heavy.evidence_support_status =
            "recommendation_evidence_limited_to_session_fallback_scans".to_string();
        let fallback_heavy = finalize_selection_fitness(fallback_heavy);

        let recommendation = build_stage_recommendation(&[fallback_heavy, marker_backed]);

        assert_eq!(
            recommendation.stage_id.as_deref(),
            Some("marker-backed"),
            "marker-backed stage should outrank a fallback-heavy stage even when the fallback stage has a higher raw score"
        );
        assert_eq!(
            recommendation.selection_fitness_status,
            "selection_fitness_preferred_stage_local_marker_backed"
        );
    }

    #[test]
    fn recommendation_blocks_clean_claim_for_cleanup_signal_only_stage() {
        let mut cleanup_signal_only = make_trend("cleanup-signal-only", "Cleanup Signal Stage");
        cleanup_signal_only.avg_validation_score = 83.0;
        cleanup_signal_only.best_validation_score = 83;
        cleanup_signal_only.cleanup_signal_strong_runs = 3;
        cleanup_signal_only.cleanup_signal_stage_local_helper_runs = 3;
        cleanup_signal_only.stage_local_scan_runs = 3;
        cleanup_signal_only.stage_local_scan_limited_runs = 3;
        cleanup_signal_only.latest_process_scan_context_scope =
            "stage_local_helper_scan".to_string();
        cleanup_signal_only.latest_cleanup_signal_support_status =
            "cleanup_signal_support_strong".to_string();
        cleanup_signal_only.latest_cleanup_signal_support_scope_status =
            "cleanup_signal_scope_stage_local_helper_runtime".to_string();
        cleanup_signal_only.evidence_support_status =
            "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance"
                .to_string();
        let cleanup_signal_only = finalize_selection_fitness(cleanup_signal_only);

        let recommendation = build_stage_recommendation(&[cleanup_signal_only]);

        assert_eq!(
            recommendation.selection_fitness_status,
            "selection_fitness_provisional_stage_local_cleanup_signal_backed"
        );
        assert_eq!(
            recommendation.clean_claim_status,
            "clean_claim_blocked_by_cleanup_signal_only_evidence"
        );
        assert_eq!(
            recommendation.recommendation_status,
            "recommendation_available",
            "cleanup-signal-only stages can still be the best available stage while remaining blocked from a stronger clean-stage claim"
        );
    }

    #[test]
    fn selection_fitness_demotes_runtime_global_only_history() {
        let mut runtime_global_only = make_trend("runtime-global", "Runtime Global Stage");
        runtime_global_only.cleanup_signal_runtime_global_only_runs = 3;
        runtime_global_only.cleanup_signal_scope_unavailable_runs = 0;
        runtime_global_only.cleanup_signal_declared_only_runs = 0;
        runtime_global_only.latest_cleanup_signal_support_status =
            "cleanup_signal_support_strong".to_string();
        runtime_global_only.latest_cleanup_signal_support_scope_status =
            "cleanup_signal_scope_runtime_global_only".to_string();
        runtime_global_only.evidence_support_status =
            "recommendation_evidence_limited_to_runtime_global_cleanup_signals".to_string();
        let runtime_global_only = finalize_selection_fitness(runtime_global_only);

        assert_eq!(
            runtime_global_only.selection_fitness_status,
            "selection_fitness_demoted_runtime_global_or_fallback_only"
        );
        assert!(runtime_global_only
            .selection_fitness_summary
            .contains("runtime-global-only cleanup signals"));
    }

    #[test]
    fn build_stage_trends_aggregates_and_orders_stage_history() {
        let mut marker_backed_result = make_stage_result_entry("stage-a", "Stage A");
        marker_backed_result.vram_evidence_status =
            "evidence_improved_pid_no_longer_observed_after_strategy".to_string();
        marker_backed_result.validation_score = 82;
        marker_backed_result.validation_verdict = "strong_improvement_signal".to_string();
        marker_backed_result.marker_evidence_status =
            "gpu_evidence_supported_by_clear_session_and_canary_scans".to_string();
        marker_backed_result.process_scan_context_status =
            "marker_scan_clear_in_scanned_regions".to_string();
        marker_backed_result.process_scan_context_scope = "stage_local_cleanup_phase".to_string();
        marker_backed_result.cleanup_signal_support_status =
            "cleanup_signal_support_strong".to_string();
        marker_backed_result.cleanup_signal_support_scope_status =
            "cleanup_signal_scope_stage_local_helper_runtime".to_string();
        marker_backed_result.contributing_cleanup_signals =
            vec!["allocator_reset".to_string(), "kv_clear".to_string()];
        marker_backed_result.helper_process_scan_status = "helper_process_scan_clear".to_string();

        let mut fallback_result = make_stage_result_entry("stage-b", "Stage B");
        fallback_result.vram_evidence_status =
            "evidence_improved_peak_bytes_lower_but_residency_still_observed".to_string();
        fallback_result.validation_score = 91;
        fallback_result.process_scan_context_scope = "session_fallback".to_string();
        fallback_result.cleanup_signal_support_status = "cleanup_signal_support_strong".to_string();
        fallback_result.cleanup_signal_support_scope_status =
            "cleanup_signal_scope_runtime_global_only".to_string();

        let entry_one = make_history_entry(
            "session-one",
            "2026-07-05T10:00:00Z",
            vec![marker_backed_result.clone(), fallback_result.clone()],
        );

        let mut marker_backed_result_two = marker_backed_result.clone();
        marker_backed_result_two.validation_score = 70;
        marker_backed_result_two.validation_verdict = "moderate_improvement_signal".to_string();
        marker_backed_result_two.helper_process_scan_status =
            "helper_process_scan_not_recorded".to_string();

        let mut fallback_result_two = fallback_result.clone();
        fallback_result_two.validation_score = 93;

        let entry_two = make_history_entry(
            "session-two",
            "2026-07-05T11:00:00Z",
            vec![marker_backed_result_two, fallback_result_two],
        );

        let trends = build_stage_trends(&[&entry_one, &entry_two]);

        assert_eq!(trends.len(), 2);
        assert_eq!(trends[0].stage_id, "stage-a");
        assert_eq!(trends[0].runs_recorded, 2);
        assert_eq!(trends[0].stage_local_scan_clear_runs, 2);
        assert_eq!(trends[0].helper_scan_clear_runs, 1);
        assert_eq!(trends[0].cleanup_signal_stage_local_helper_runs, 2);
        assert_eq!(
            trends[0].selection_fitness_status,
            "selection_fitness_preferred_stage_local_marker_backed"
        );
        assert_eq!(
            trends[0].evidence_support_status,
            "recommendation_evidence_supported_by_stage_local_marker_clearance"
        );

        assert_eq!(trends[1].stage_id, "stage-b");
        assert_eq!(trends[1].session_fallback_scan_runs, 2);
        assert_eq!(trends[1].cleanup_signal_runtime_global_only_runs, 2);
        assert_eq!(
            trends[1].selection_fitness_status,
            "selection_fitness_demoted_runtime_global_or_fallback_only"
        );
    }

    #[test]
    fn release_gate_blocks_cleanup_signal_only_evidence_even_with_strong_canary_history() {
        let mut recommendation = default_stage_recommendation_report("test");
        recommendation.recommendation_status = "recommendation_available".to_string();
        recommendation.clean_claim_status =
            "clean_claim_blocked_by_cleanup_signal_only_evidence".to_string();
        recommendation.selection_fitness_status =
            "selection_fitness_provisional_stage_local_cleanup_signal_backed".to_string();
        recommendation.evidence_support_status =
            "recommendation_evidence_supported_by_cleanup_signals_without_marker_clearance"
                .to_string();
        recommendation.runs_recorded = 3;
        recommendation.marker_detection_runs = 0;
        recommendation.worsened_runs = 0;
        recommendation.inconclusive_runs = 0;

        let gate = build_release_gate(&recommendation, &make_controlled_canary_history(2));

        assert!(!gate.stage_gate_passed);
        assert!(gate.controlled_canary_gate_passed);
        assert_eq!(
            gate.cleanup_stage_gate_status,
            "cleanup_stage_gate_blocked_by_cleanup_signal_only_evidence"
        );
        assert_eq!(
            gate.release_readiness_status,
            "release_readiness_waiting_on_cleanup_stage_history"
        );
    }

    #[test]
    fn release_gate_passes_for_marker_backed_stage_and_clear_canary_history() {
        let mut recommendation = default_stage_recommendation_report("test");
        recommendation.recommendation_status = "recommendation_available".to_string();
        recommendation.clean_claim_status =
            "clean_claim_eligible_under_current_thresholds".to_string();
        recommendation.selection_fitness_status =
            "selection_fitness_preferred_stage_local_marker_backed".to_string();
        recommendation.evidence_support_status =
            "recommendation_evidence_supported_by_stage_local_marker_clearance".to_string();
        recommendation.runs_recorded = 3;
        recommendation.marker_detection_runs = 0;
        recommendation.worsened_runs = 0;
        recommendation.inconclusive_runs = 0;

        let gate = build_release_gate(&recommendation, &make_controlled_canary_history(2));

        assert!(gate.stage_gate_passed);
        assert!(gate.controlled_canary_gate_passed);
        assert_eq!(gate.cleanup_stage_gate_status, "cleanup_stage_gate_passed");
        assert_eq!(
            gate.release_readiness_status,
            "release_readiness_repeated_evidence_ready_under_current_thresholds"
        );
        assert_eq!(
            gate.gate_status,
            "release_gate_repeated_evidence_threshold_met"
        );
    }
}

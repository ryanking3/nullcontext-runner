use crate::audit::{MemoryValidationHistoryReport, PrivacyReport};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
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
        summary: format!(
            "NullContext has recorded {} validation run(s) for scope {}. {} run(s) still showed marker-detection evidence, {} run(s) achieved fully clear repeated canary passes, and the best-stage score average is {}.",
            runs_recorded,
            history_scope_label(report),
            marker_detection_runs,
            clear_canary_runs,
            best_stage_score_avg
                .map(|value| format!("{value:.1}/100"))
                .unwrap_or_else(|| "unavailable".to_string())
        ),
        notes: vec![
            "This scope groups validation history by model id, host platform, and whether GPU offload was requested."
                .to_string(),
            "Cross-session history is local-only and stores compact validation metadata rather than prompt/response content."
                .to_string(),
        ],
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

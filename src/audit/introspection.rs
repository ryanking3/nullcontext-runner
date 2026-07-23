use super::{LlamaRuntimeCleanupSignalEntryReport, LlamaRuntimeIntrospectionEventReport};
use crate::runtime_introspection::RuntimeIntrospectionSignal;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn map_runtime_introspection_signal(
    signal: &RuntimeIntrospectionSignal,
    signal_aliases: &BTreeMap<String, Vec<String>>,
) -> LlamaRuntimeIntrospectionEventReport {
    let canonical_event = canonical_signal_id_for_event(&signal.event, signal_aliases);
    let canonical_event_for_classification = canonical_event.clone();
    let classification_event = canonical_event_for_classification
        .as_deref()
        .unwrap_or(signal.event.as_str());
    LlamaRuntimeIntrospectionEventReport {
        event: signal.event.clone(),
        canonical_event,
        status: signal.status.clone(),
        source: signal.source_stream.clone(),
        lifecycle_phase: introspection_event_phase(classification_event).to_string(),
        evidence_scope: introspection_event_scope(classification_event).to_string(),
        cleanup_relevance: introspection_event_cleanup_relevance(classification_event).to_string(),
        details: signal.details.clone(),
    }
}

pub(super) fn build_signal_alias_lookup(
    manifest_aliases: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut merged = BTreeMap::from([
        (
            "allocator_initialized".to_string(),
            vec!["allocator_initialized".to_string()],
        ),
        (
            "allocator_teardown_observed".to_string(),
            vec![
                "allocator_teardown".to_string(),
                "allocator_teardown_observed".to_string(),
            ],
        ),
        (
            "allocator_reset_observed".to_string(),
            vec![
                "allocator_reset".to_string(),
                "allocator_reset_observed".to_string(),
            ],
        ),
        (
            "kv_cache_initialized".to_string(),
            vec!["kv_cache_initialized".to_string()],
        ),
        (
            "kv_cache_reused".to_string(),
            vec!["kv_cache_reused".to_string()],
        ),
        (
            "kv_cache_clear_observed".to_string(),
            vec![
                "kv_cache_clear".to_string(),
                "kv_cache_clear_observed".to_string(),
            ],
        ),
        (
            "model_unload_observed".to_string(),
            vec![
                "model_unload".to_string(),
                "model_unload_observed".to_string(),
            ],
        ),
    ]);

    for (canonical_signal_id, aliases) in manifest_aliases {
        let entry = merged.entry(canonical_signal_id.clone()).or_default();
        entry.extend(aliases.iter().cloned());
    }

    for (canonical_signal_id, aliases) in &mut merged {
        if !aliases.iter().any(|value| value == canonical_signal_id) {
            aliases.push(canonical_signal_id.clone());
        }
        aliases.sort();
        aliases.dedup();
    }

    merged
}

pub(super) fn observed_signal_matches(
    signal: &RuntimeIntrospectionSignal,
    signal_aliases: &BTreeMap<String, Vec<String>>,
    canonical_signal_id: &str,
) -> bool {
    if signal.status == "failed" {
        return false;
    }

    canonical_signal_id_for_event(&signal.event, signal_aliases)
        .as_deref()
        .map(|value| value == canonical_signal_id)
        .unwrap_or(false)
}

pub(super) fn canonical_or_raw_signal_id(
    signal: &RuntimeIntrospectionSignal,
    signal_aliases: &BTreeMap<String, Vec<String>>,
) -> String {
    canonical_signal_id_for_event(&signal.event, signal_aliases)
        .unwrap_or_else(|| signal.event.clone())
}

pub(super) fn canonical_signal_id_for_event(
    event: &str,
    signal_aliases: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    signal_aliases
        .iter()
        .find_map(|(canonical_signal_id, aliases)| {
            aliases
                .iter()
                .any(|alias| alias == event)
                .then(|| canonical_signal_id.clone())
        })
}

pub(super) fn introspection_event_phase(event: &str) -> &'static str {
    match event {
        "allocator_initialized" | "kv_cache_initialized" => "setup",
        "kv_cache_reused" => "reuse",
        "allocator_teardown_observed"
        | "allocator_reset_observed"
        | "kv_cache_clear_observed"
        | "model_unload_observed" => "cleanup",
        "introspection_signal_parse_failed" => "parse_failure",
        _ if event.contains("init")
            || event.contains("load")
            || event.contains("alloc")
            || event.contains("create") =>
        {
            "setup"
        }
        _ if event.contains("reuse") || event.contains("reused") => "reuse",
        _ if event.contains("clear")
            || event.contains("reset")
            || event.contains("teardown")
            || event.contains("unload")
            || event.contains("cleanup")
            || event.contains("free")
            || event.contains("release")
            || event.contains("destroy") =>
        {
            "cleanup"
        }
        _ => "unknown",
    }
}

pub(super) fn introspection_event_scope(event: &str) -> &'static str {
    match event {
        "allocator_initialized" | "allocator_teardown_observed" | "allocator_reset_observed" => {
            "allocator"
        }
        "kv_cache_initialized" | "kv_cache_reused" | "kv_cache_clear_observed" => "kv_cache",
        "model_unload_observed" => "model_lifecycle",
        "introspection_signal_parse_failed" => "parser",
        _ if event.starts_with("allocator_") || event.contains("allocator") => "allocator",
        _ if event.starts_with("kv_cache_")
            || event.starts_with("kv_")
            || event.contains("kv_cache")
            || event.contains("context_cache") =>
        {
            "kv_cache"
        }
        _ if event.starts_with("model_") || event.contains("model") => "model_lifecycle",
        _ => "unknown",
    }
}

pub(super) fn introspection_event_cleanup_relevance(event: &str) -> &'static str {
    match event {
        "allocator_reset_observed"
        | "kv_cache_clear_observed"
        | "model_unload_observed"
        | "allocator_teardown_observed" => "direct_cleanup_path_signal",
        "allocator_initialized" | "kv_cache_initialized" | "kv_cache_reused" => {
            "setup_or_reuse_signal_only"
        }
        "introspection_signal_parse_failed" => "signal_capture_failure",
        _ if event.contains("clear")
            || event.contains("reset")
            || event.contains("teardown")
            || event.contains("unload")
            || event.contains("cleanup")
            || event.contains("free")
            || event.contains("release")
            || event.contains("destroy") =>
        {
            "direct_cleanup_path_signal"
        }
        _ if event.contains("init")
            || event.contains("load")
            || event.contains("alloc")
            || event.contains("reuse")
            || event.contains("create") =>
        {
            "setup_or_reuse_signal_only"
        }
        _ => "unknown_cleanup_relevance",
    }
}

pub(super) fn fallback_signal_entry(
    signal_id: &str,
    signal_label: &str,
    startup_failed: bool,
) -> LlamaRuntimeCleanupSignalEntryReport {
    let observation_status = if startup_failed {
        "signal_collection_interrupted_by_startup_failure"
    } else {
        "signal_not_observed"
    };
    let evidence_status = if startup_failed {
        "signal_evidence_unavailable_due_to_startup_failure"
    } else {
        "signal_support_unknown_in_fallback_path"
    };

    LlamaRuntimeCleanupSignalEntryReport {
        signal_id: signal_id.to_string(),
        signal_label: signal_label.to_string(),
        declared_support_status: "support_unknown_in_fallback_path".to_string(),
        observation_status: observation_status.to_string(),
        evidence_status: evidence_status.to_string(),
        observed_count: 0,
        observed_sources: vec![],
        observed_phases: vec![],
        sample_observed_status: None,
        sample_observed_details: None,
        summary: if startup_failed {
            format!(
                "{} could not be evaluated because runtime startup failed before normal signal collection.",
                signal_label
            )
        } else {
            format!(
                "{} remained on the stock fallback path, so NullContext could not verify direct support for this runtime signal.",
                signal_label
            )
        },
    }
}

pub(super) fn signal_entry(
    signal_id: &str,
    signal_label: &str,
    declared_support: bool,
    observed_signals: &[RuntimeIntrospectionSignal],
    signal_aliases: &BTreeMap<String, Vec<String>>,
    startup_failed: bool,
) -> LlamaRuntimeCleanupSignalEntryReport {
    let matched_signals = observed_signals
        .iter()
        .filter(|signal| observed_signal_matches(signal, signal_aliases, signal_id))
        .collect::<Vec<_>>();
    let observed = !matched_signals.is_empty();
    let declared_support_status = if declared_support {
        "declared_signal_support_available"
    } else {
        "declared_signal_support_unavailable"
    };
    let observation_status = if startup_failed {
        "signal_collection_interrupted_by_startup_failure"
    } else if observed {
        "signal_observed"
    } else {
        "signal_not_observed"
    };
    let evidence_status = if startup_failed {
        "signal_evidence_unavailable_due_to_startup_failure"
    } else if observed {
        "direct_signal_observed"
    } else if declared_support {
        "declared_support_but_signal_not_observed"
    } else {
        "no_declared_support_and_no_signal_observed"
    };
    let mut observed_sources = matched_signals
        .iter()
        .map(|signal| signal.source_stream.clone())
        .collect::<Vec<_>>();
    observed_sources.sort();
    observed_sources.dedup();
    let mut observed_phases = matched_signals
        .iter()
        .map(|signal| introspection_event_phase(&signal.event).to_string())
        .collect::<Vec<_>>();
    observed_phases.sort();
    observed_phases.dedup();
    let sample_observed_status = matched_signals.last().map(|signal| signal.status.clone());
    let sample_observed_details = matched_signals.iter().rev().find_map(|signal| {
        let trimmed = signal.details.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let summary = if startup_failed {
        format!(
            "{} could not be evaluated because runtime startup failed before normal signal collection.",
            signal_label
        )
    } else if observed {
        format!(
            "{} was observed directly {} time(s) during this runtime lifecycle via {}.",
            signal_label,
            matched_signals.len(),
            if observed_sources.is_empty() {
                "no captured source metadata".to_string()
            } else {
                observed_sources.join(", ")
            }
        )
    } else if declared_support {
        format!(
            "{} was declared as supported by this runtime path, but no direct signal was observed for this session.",
            signal_label
        )
    } else {
        format!(
            "{} was neither declared as supported nor observed directly for this session.",
            signal_label
        )
    };

    LlamaRuntimeCleanupSignalEntryReport {
        signal_id: signal_id.to_string(),
        signal_label: signal_label.to_string(),
        declared_support_status: declared_support_status.to_string(),
        observation_status: observation_status.to_string(),
        evidence_status: evidence_status.to_string(),
        observed_count: matched_signals.len() as u32,
        observed_sources,
        observed_phases,
        sample_observed_status,
        sample_observed_details,
        summary,
    }
}

pub(super) fn signal_declared(
    declared_cleanup_signal_ids: &[String],
    declared_signal_ids: &[String],
    signal_id: &str,
) -> bool {
    declared_cleanup_signal_ids
        .iter()
        .any(|value| value == signal_id)
        || declared_signal_ids.iter().any(|value| value == signal_id)
}

pub(super) fn build_additional_runtime_signal_entries(
    declared_signal_ids: &[String],
    declared_cleanup_signal_ids: &[String],
    observed_signals: &[RuntimeIntrospectionSignal],
    signal_aliases: &BTreeMap<String, Vec<String>>,
    startup_failed: bool,
) -> Vec<LlamaRuntimeCleanupSignalEntryReport> {
    let known_signal_ids = [
        "allocator_initialized",
        "allocator_teardown_observed",
        "allocator_reset_observed",
        "kv_cache_initialized",
        "kv_cache_reused",
        "kv_cache_clear_observed",
        "model_unload_observed",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    declared_signal_ids
        .iter()
        .chain(declared_cleanup_signal_ids.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|signal_id| !known_signal_ids.contains(signal_id.as_str()))
        .map(|signal_id| {
            signal_entry(
                &signal_id,
                &signal_label_for_id(&signal_id),
                signal_declared(declared_cleanup_signal_ids, declared_signal_ids, &signal_id),
                observed_signals,
                signal_aliases,
                startup_failed,
            )
        })
        .collect()
}

pub(super) fn build_additional_cleanup_signal_entries(
    declared_cleanup_signal_ids: &[String],
    declared_signal_ids: &[String],
    observed_signals: &[RuntimeIntrospectionSignal],
    signal_aliases: &BTreeMap<String, Vec<String>>,
    startup_failed: bool,
) -> Vec<LlamaRuntimeCleanupSignalEntryReport> {
    let known_cleanup_signal_ids = [
        "allocator_reset_observed",
        "kv_cache_clear_observed",
        "model_unload_observed",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    declared_cleanup_signal_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|signal_id| !known_cleanup_signal_ids.contains(signal_id.as_str()))
        .map(|signal_id| {
            signal_entry(
                &signal_id,
                &signal_label_for_id(&signal_id),
                signal_declared(declared_cleanup_signal_ids, declared_signal_ids, &signal_id),
                observed_signals,
                signal_aliases,
                startup_failed,
            )
        })
        .collect()
}

pub(super) fn observed_signal_sources(signals: &[RuntimeIntrospectionSignal]) -> Vec<String> {
    let mut sources = signals
        .iter()
        .map(|signal| signal.source_stream.clone())
        .collect::<Vec<_>>();
    sources.sort();
    sources.dedup();
    sources
}

fn signal_label_for_id(signal_id: &str) -> String {
    signal_id
        .split('_')
        .filter(|segment| !matches!(*segment, "observed"))
        .map(|segment| {
            if segment.eq_ignore_ascii_case("kv") {
                return "KV".to_string();
            }
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

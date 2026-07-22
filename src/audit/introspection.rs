use super::LlamaRuntimeIntrospectionEventReport;
use crate::runtime_introspection::RuntimeIntrospectionSignal;
use std::collections::BTreeMap;

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

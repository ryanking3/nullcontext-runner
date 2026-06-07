use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimeIntrospectionCapabilities {
    pub capability_source: String,
    pub manifest_path: Option<String>,
    pub runtime_build_profile: String,
    pub instrumentation_backend: String,
    pub allocator_introspection_status: String,
    pub kv_cache_introspection_status: String,
    pub model_unload_signal_status: String,
    pub allocator_reset_signal_status: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RuntimeIntrospectionManifest {
    runtime_build_profile: Option<String>,
    instrumentation_backend: Option<String>,
    allocator_introspection_status: Option<String>,
    kv_cache_introspection_status: Option<String>,
    model_unload_signal_status: Option<String>,
    allocator_reset_signal_status: Option<String>,
    notes: Option<Vec<String>>,
}

pub fn detect_runtime_introspection_capabilities(
    llama_path: &str,
) -> Result<RuntimeIntrospectionCapabilities> {
    let manifest_path = runtime_introspection_manifest_path(llama_path);

    if !manifest_path.exists() {
        return Ok(stock_runtime_capabilities());
    }

    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "Failed to read runtime introspection manifest at {}",
            manifest_path.display()
        )
    })?;
    let manifest: RuntimeIntrospectionManifest = serde_json::from_str(&raw).with_context(|| {
        format!(
            "Failed to parse runtime introspection manifest at {}",
            manifest_path.display()
        )
    })?;

    let mut notes = manifest.notes.unwrap_or_default();
    notes.push(format!(
        "Loaded runtime introspection capabilities from {}.",
        manifest_path.display()
    ));

    Ok(RuntimeIntrospectionCapabilities {
        capability_source: "sidecar_manifest".to_string(),
        manifest_path: Some(manifest_path.display().to_string()),
        runtime_build_profile: manifest
            .runtime_build_profile
            .unwrap_or_else(|| "instrumented_external_llama_server".to_string()),
        instrumentation_backend: manifest
            .instrumentation_backend
            .unwrap_or_else(|| "manifest_declared".to_string()),
        allocator_introspection_status: manifest
            .allocator_introspection_status
            .unwrap_or_else(|| "allocator_introspection_status_unspecified".to_string()),
        kv_cache_introspection_status: manifest
            .kv_cache_introspection_status
            .unwrap_or_else(|| "kv_cache_introspection_status_unspecified".to_string()),
        model_unload_signal_status: manifest
            .model_unload_signal_status
            .unwrap_or_else(|| "model_unload_signal_status_unspecified".to_string()),
        allocator_reset_signal_status: manifest
            .allocator_reset_signal_status
            .unwrap_or_else(|| "allocator_reset_signal_status_unspecified".to_string()),
        notes,
    })
}

fn runtime_introspection_manifest_path(llama_path: &str) -> PathBuf {
    let runtime_path = Path::new(llama_path);
    let extension = runtime_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if extension.is_empty() {
        runtime_path.with_extension("nullcontext-introspection.json")
    } else {
        runtime_path.with_extension(format!("{extension}.nullcontext-introspection.json"))
    }
}

fn stock_runtime_capabilities() -> RuntimeIntrospectionCapabilities {
    RuntimeIntrospectionCapabilities {
        capability_source: "stock_runtime_fallback".to_string(),
        manifest_path: None,
        runtime_build_profile: "stock_external_llama_server".to_string(),
        instrumentation_backend: "none".to_string(),
        allocator_introspection_status: "allocator_introspection_unavailable".to_string(),
        kv_cache_introspection_status: "kv_cache_introspection_unavailable".to_string(),
        model_unload_signal_status: "model_unload_not_observed_directly".to_string(),
        allocator_reset_signal_status: "allocator_reset_not_observed_directly".to_string(),
        notes: vec![
            "No runtime introspection sidecar manifest was found next to the configured llama-server binary."
                .to_string(),
            "NullContext is treating this runtime as a stock external llama-server build.".to_string(),
        ],
    }
}

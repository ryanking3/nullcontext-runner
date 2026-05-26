#![allow(dead_code)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CorpusLifecycleState {
    #[default]
    Draft,
    Building,
    Ready,
    IngestionFailed,
    CleanupPending,
    CleanupSucceeded,
    CleanupFailed,
    Orphaned,
}

impl CorpusLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Building => "building",
            Self::Ready => "ready",
            Self::IngestionFailed => "ingestion_failed",
            Self::CleanupPending => "cleanup_pending",
            Self::CleanupSucceeded => "cleanup_succeeded",
            Self::CleanupFailed => "cleanup_failed",
            Self::Orphaned => "orphaned",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CorpusRetentionPolicy {
    EphemeralImmediate,
    #[default]
    RetainUntilManualCleanup,
    RetainForDuration,
}

impl CorpusRetentionPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EphemeralImmediate => "ephemeral_immediate",
            Self::RetainUntilManualCleanup => "retain_until_manual_cleanup",
            Self::RetainForDuration => "retain_for_duration",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CorpusCleanupReason {
    EphemeralPolicy,
    ManualOperatorRequest,
    ScheduledRetentionExpiry,
    StartupOrphanReconciliation,
}

impl CorpusCleanupReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EphemeralPolicy => "ephemeral_policy",
            Self::ManualOperatorRequest => "manual_operator_request",
            Self::ScheduledRetentionExpiry => "scheduled_retention_expiry",
            Self::StartupOrphanReconciliation => "startup_orphan_reconciliation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusLifecycleMetadata {
    #[serde(default)]
    pub state: CorpusLifecycleState,
    #[serde(default)]
    pub retention_policy: CorpusRetentionPolicy,
    #[serde(default)]
    pub retention_deadline: Option<String>,
    #[serde(default)]
    pub cleanup_requested_at: Option<String>,
    #[serde(default)]
    pub cleanup_completed_at: Option<String>,
    #[serde(default)]
    pub cleanup_reason: Option<CorpusCleanupReason>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusArtifactPaths {
    pub root_dir: String,
    pub manifest_path: String,
    pub sources_path: String,
    pub pages_path: String,
    pub chunks_path: String,
    pub embeddings_path: String,
    pub ingestion_report_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusManifest {
    pub corpus_id: String,
    pub name: String,
    pub created_at: String,
    pub persistent: bool,
    pub embedding_backend: Option<String>,
    pub embedding_model: Option<String>,
    pub ocr_backend: Option<String>,
    pub chunk_strategy: String,
    pub source_count: usize,
    pub chunk_count: usize,
    pub artifact_paths: CorpusArtifactPaths,
    #[serde(default)]
    pub lifecycle: CorpusLifecycleMetadata,
}

impl CorpusManifest {
    pub fn new(name: impl Into<String>, home: &str, persistent: bool) -> Self {
        let corpus_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let artifact_paths = build_corpus_artifact_paths(home, &corpus_id, persistent);
        let retention_policy = if persistent {
            CorpusRetentionPolicy::RetainUntilManualCleanup
        } else {
            CorpusRetentionPolicy::EphemeralImmediate
        };

        Self {
            corpus_id,
            name: name.into(),
            created_at,
            persistent,
            embedding_backend: None,
            embedding_model: None,
            ocr_backend: None,
            chunk_strategy: "unconfigured".to_string(),
            source_count: 0,
            chunk_count: 0,
            artifact_paths,
            lifecycle: CorpusLifecycleMetadata {
                state: CorpusLifecycleState::Draft,
                retention_policy,
                retention_deadline: None,
                cleanup_requested_at: None,
                cleanup_completed_at: None,
                cleanup_reason: None,
                updated_at: Some(Utc::now().to_rfc3339()),
            },
        }
    }
}

pub fn ensure_corpus_artifact_dirs(paths: &CorpusArtifactPaths) -> std::io::Result<()> {
    std::fs::create_dir_all(Path::new(&paths.root_dir))
}

pub fn corpus_registry_root(home: &str) -> PathBuf {
    Path::new(home).join(".nullcontext").join("corpora")
}

pub fn corpus_data_root(home: &str) -> PathBuf {
    corpus_registry_root(home).join("data")
}

pub fn persistent_corpus_dir(home: &str, corpus_id: &str) -> PathBuf {
    corpus_data_root(home).join(corpus_id)
}

pub fn ephemeral_corpus_dir(corpus_id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("nullcontext")
        .join("corpora")
        .join(corpus_id)
}

pub fn build_corpus_artifact_paths(
    home: &str,
    corpus_id: &str,
    persistent: bool,
) -> CorpusArtifactPaths {
    let root = if persistent {
        persistent_corpus_dir(home, corpus_id)
    } else {
        ephemeral_corpus_dir(corpus_id)
    };

    CorpusArtifactPaths {
        root_dir: root.display().to_string(),
        manifest_path: root.join("manifest.json").display().to_string(),
        sources_path: root.join("sources.json").display().to_string(),
        pages_path: root.join("pages.json").display().to_string(),
        chunks_path: root.join("chunks.json").display().to_string(),
        embeddings_path: root.join("embeddings.json").display().to_string(),
        ingestion_report_path: root.join("ingestion_report.json").display().to_string(),
    }
}

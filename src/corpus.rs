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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusLifecycleReport {
    pub state: String,
    pub retention_policy: String,
    pub retention_deadline: Option<String>,
    pub cleanup_requested_at: Option<String>,
    pub cleanup_completed_at: Option<String>,
    pub cleanup_reason: Option<String>,
    pub state_note: Option<String>,
    pub updated_at: Option<String>,
    pub policy_summary: String,
    pub decision_summary: String,
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
    pub state_note: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl CorpusLifecycleMetadata {
    pub fn to_report(&self) -> CorpusLifecycleReport {
        CorpusLifecycleReport {
            state: self.state.as_str().to_string(),
            retention_policy: self.retention_policy.as_str().to_string(),
            retention_deadline: self.retention_deadline.clone(),
            cleanup_requested_at: self.cleanup_requested_at.clone(),
            cleanup_completed_at: self.cleanup_completed_at.clone(),
            cleanup_reason: self
                .cleanup_reason
                .as_ref()
                .map(|reason| reason.as_str().to_string()),
            state_note: self.state_note.clone(),
            updated_at: self.updated_at.clone(),
            policy_summary: corpus_lifecycle_policy_summary(self),
            decision_summary: corpus_lifecycle_decision_summary(self),
        }
    }
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
                state_note: Some(
                    "Corpus manifest has been created and is waiting for ingestion to populate retrieval artifacts."
                        .to_string(),
                ),
                updated_at: Some(Utc::now().to_rfc3339()),
            },
        }
    }
}

fn corpus_lifecycle_policy_summary(metadata: &CorpusLifecycleMetadata) -> String {
    match metadata.retention_policy {
        CorpusRetentionPolicy::EphemeralImmediate => {
            "Corpus artifacts live in temporary storage and should be cleaned promptly after the grounded workflow finishes."
                .to_string()
        }
        CorpusRetentionPolicy::RetainUntilManualCleanup => {
            "Corpus artifacts are retained until an operator explicitly requests cleanup."
                .to_string()
        }
        CorpusRetentionPolicy::RetainForDuration => {
            if let Some(deadline) = &metadata.retention_deadline {
                format!(
                    "Corpus artifacts are retained until {deadline}, after which scheduled cleanup may run."
                )
            } else {
                "Corpus is configured for scheduled retention expiry, but no deadline is currently recorded."
                    .to_string()
            }
        }
    }
}

fn corpus_lifecycle_decision_summary(metadata: &CorpusLifecycleMetadata) -> String {
    if let Some(note) = &metadata.state_note {
        return note.clone();
    }

    match metadata.state {
        CorpusLifecycleState::Draft => {
            "Corpus manifest has been created but ingestion has not completed yet.".to_string()
        }
        CorpusLifecycleState::Building => {
            "Corpus ingestion is currently building artifacts.".to_string()
        }
        CorpusLifecycleState::Ready => {
            "Corpus artifacts are available for retrieval under the current lifecycle policy."
                .to_string()
        }
        CorpusLifecycleState::IngestionFailed => {
            "Corpus ingestion failed before a usable retrieval corpus was finalized."
                .to_string()
        }
        CorpusLifecycleState::CleanupPending => {
            "Corpus cleanup has been requested but has not yet completed.".to_string()
        }
        CorpusLifecycleState::CleanupSucceeded => {
            let reason = metadata
                .cleanup_reason
                .as_ref()
                .map(corpus_cleanup_reason_summary)
                .unwrap_or("Corpus cleanup completed successfully.");

            reason.to_string()
        }
        CorpusLifecycleState::CleanupFailed => {
            let reason = metadata
                .cleanup_reason
                .as_ref()
                .map(corpus_cleanup_reason_summary)
                .unwrap_or("Corpus cleanup attempted but did not complete successfully.");

            format!("{reason} Corpus cleanup failed or requires operator follow-up.")
        }
        CorpusLifecycleState::Orphaned => {
            "Lifecycle reconciliation detected an inconsistency between the corpus registry and on-disk artifacts. Operator review is recommended."
                .to_string()
        }
    }
}

fn corpus_cleanup_reason_summary(reason: &CorpusCleanupReason) -> &'static str {
    match reason {
        CorpusCleanupReason::EphemeralPolicy => {
            "Cleanup ran because the corpus policy was ephemeral-at-end."
        }
        CorpusCleanupReason::ManualOperatorRequest => {
            "Cleanup ran because an operator explicitly requested lifecycle cleanup."
        }
        CorpusCleanupReason::ScheduledRetentionExpiry => {
            "Cleanup ran because the scheduled retention deadline expired."
        }
        CorpusCleanupReason::StartupOrphanReconciliation => {
            "Lifecycle reconciliation changed the corpus state during startup recovery."
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

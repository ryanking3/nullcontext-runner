#![allow(dead_code)]

use crate::corpus::{
    corpus_registry_root, CorpusCleanupReason, CorpusLifecycleMetadata, CorpusLifecycleState,
    CorpusManifest, CorpusRetentionPolicy,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusRegistry {
    pub corpora: Vec<CorpusIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusIndexEntry {
    pub corpus_id: String,
    pub name: String,
    pub created_at: String,
    pub persistent: bool,
    pub root_path: String,
    pub manifest_path: String,
    pub source_count: usize,
    pub chunk_count: usize,
    pub embedding_backend: Option<String>,
    pub embedding_model: Option<String>,
    pub ocr_backend: Option<String>,
    #[serde(default)]
    pub report_path: String,
    #[serde(default)]
    pub lifecycle: CorpusLifecycleMetadata,
}

impl CorpusRegistry {
    pub fn load(home: &str) -> Result<Self> {
        let path = corpus_registry_path(home);

        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read corpus registry at {}", path.display()))?;

        let parsed = serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse corpus registry at {}", path.display()))?;

        Ok(parsed)
    }

    pub fn save(&self, home: &str) -> Result<()> {
        ensure_corpus_registry_dirs(home)?;

        let root = corpus_registry_root(home);
        let path = corpus_registry_path(home);
        let temp = root.join("index.json.tmp");

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&temp, json)?;
        fs::rename(&temp, &path)
            .with_context(|| format!("Failed to write corpus registry at {}", path.display()))?;

        Ok(())
    }

    pub fn register(&mut self, entry: CorpusIndexEntry) {
        self.corpora
            .retain(|corpus| corpus.corpus_id != entry.corpus_id);
        self.corpora.push(entry);
        self.corpora.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    }

    pub fn find(&self, corpus_id: &str) -> Option<&CorpusIndexEntry> {
        self.corpora
            .iter()
            .find(|corpus| corpus.corpus_id == corpus_id)
    }

    pub fn find_mut(&mut self, corpus_id: &str) -> Option<&mut CorpusIndexEntry> {
        self.corpora
            .iter_mut()
            .find(|corpus| corpus.corpus_id == corpus_id)
    }
}

impl CorpusIndexEntry {
    pub fn from_manifest(manifest: &CorpusManifest) -> Self {
        Self {
            corpus_id: manifest.corpus_id.clone(),
            name: manifest.name.clone(),
            created_at: manifest.created_at.clone(),
            persistent: manifest.persistent,
            root_path: manifest.artifact_paths.root_dir.clone(),
            manifest_path: manifest.artifact_paths.manifest_path.clone(),
            source_count: manifest.source_count,
            chunk_count: manifest.chunk_count,
            embedding_backend: manifest.embedding_backend.clone(),
            embedding_model: manifest.embedding_model.clone(),
            ocr_backend: manifest.ocr_backend.clone(),
            report_path: manifest.artifact_paths.ingestion_report_path.clone(),
            lifecycle: manifest.lifecycle.clone(),
        }
    }

    pub fn mark_cleanup_pending(&mut self, reason: CorpusCleanupReason) {
        self.lifecycle.state = CorpusLifecycleState::CleanupPending;
        self.lifecycle.cleanup_requested_at = Some(current_timestamp());
        self.lifecycle.cleanup_reason = Some(reason);
        self.lifecycle.state_note = Some(
            "Lifecycle cleanup was requested and NullContext is preparing to archive the corpus report and remove retained corpus artifacts."
                .to_string(),
        );
        self.lifecycle.updated_at = self.lifecycle.cleanup_requested_at.clone();
    }

    pub fn mark_cleanup_result(&mut self, successful: bool, reason: CorpusCleanupReason) {
        self.lifecycle.state = if successful {
            CorpusLifecycleState::CleanupSucceeded
        } else {
            CorpusLifecycleState::CleanupFailed
        };
        self.lifecycle.cleanup_completed_at = Some(current_timestamp());
        self.lifecycle.cleanup_reason = Some(reason);
        self.lifecycle.state_note = Some(if successful {
            "Lifecycle cleanup finished and NullContext recorded the retained corpus cleanup result."
                .to_string()
        } else {
            "Lifecycle cleanup finished with a failed or partial result. Operator follow-up may still be required for this corpus."
                .to_string()
        });
        self.lifecycle.updated_at = self.lifecycle.cleanup_completed_at.clone();
        self.source_count = 0;
        self.chunk_count = 0;
    }

    pub fn mark_orphaned(&mut self) {
        self.mark_orphaned_with_note(
            "Lifecycle reconciliation detected an inconsistency between the corpus registry entry and on-disk corpus artifacts."
                .to_string(),
        );
    }

    pub fn mark_orphaned_with_note(&mut self, note: String) {
        self.lifecycle.state = CorpusLifecycleState::Orphaned;
        self.lifecycle.cleanup_reason = Some(CorpusCleanupReason::StartupOrphanReconciliation);
        self.lifecycle.state_note = Some(note);
        self.lifecycle.updated_at = Some(current_timestamp());
    }

    pub fn apply_retention_policy(
        &mut self,
        retention_policy: CorpusRetentionPolicy,
        retention_deadline: Option<String>,
    ) {
        self.lifecycle.retention_policy = retention_policy;
        self.lifecycle.retention_deadline = retention_deadline;
        self.lifecycle.updated_at = Some(current_timestamp());
    }
}

pub fn register_corpus(home: &str, manifest: &CorpusManifest) -> Result<()> {
    let mut registry = CorpusRegistry::load(home)?;
    registry.register(CorpusIndexEntry::from_manifest(manifest));
    registry.save(home)
}

pub fn ensure_corpus_registry_dirs(home: &str) -> Result<()> {
    fs::create_dir_all(corpus_registry_root(home))?;
    fs::create_dir_all(corpus_registry_root(home).join("data"))?;
    fs::create_dir_all(corpus_registry_root(home).join("reports"))?;
    Ok(())
}

pub fn list_corpora(home: &str) -> Result<CorpusRegistry> {
    CorpusRegistry::load(home)
}

pub fn validate_corpus_ready(entry: &CorpusIndexEntry) -> Result<()> {
    if entry.lifecycle.state != CorpusLifecycleState::Ready {
        anyhow::bail!(
            "Corpus {} is not ready for retrieval. Current lifecycle state: {}.",
            entry.corpus_id,
            entry.lifecycle.state.as_str()
        );
    }

    if !Path::new(&entry.root_path).exists() {
        anyhow::bail!(
            "Corpus root is missing for registry entry: {}. Reconcile the corpus registry before using this corpus.",
            entry.corpus_id
        );
    }

    if !Path::new(&entry.manifest_path).exists() {
        anyhow::bail!(
            "Corpus manifest is missing for registry entry: {}. Reconcile the corpus registry before using this corpus.",
            entry.corpus_id
        );
    }

    Ok(())
}

pub fn corpus_registry_path(home: &str) -> PathBuf {
    corpus_registry_root(home).join("index.json")
}

pub fn corpus_manifest_path(home: &str, corpus_id: &str) -> PathBuf {
    corpus_registry_root(home)
        .join("data")
        .join(corpus_id)
        .join("manifest.json")
}

pub fn resolve_corpus_retention_policy(persistent: bool) -> CorpusRetentionPolicy {
    if persistent {
        CorpusRetentionPolicy::RetainUntilManualCleanup
    } else {
        CorpusRetentionPolicy::EphemeralImmediate
    }
}

pub fn corpus_exists(home: &str, corpus_id: &str) -> bool {
    Path::new(&corpus_manifest_path(home, corpus_id)).exists()
}

pub fn due_retention_cleanup_corpus_ids(home: &str) -> Result<Vec<String>> {
    let registry = CorpusRegistry::load(home)?;
    let now = Utc::now();

    Ok(registry
        .corpora
        .iter()
        .filter(|entry| {
            entry.lifecycle.retention_policy == CorpusRetentionPolicy::RetainForDuration
                && entry.lifecycle.state == CorpusLifecycleState::Ready
                && entry
                    .lifecycle
                    .retention_deadline
                    .as_deref()
                    .and_then(parse_timestamp)
                    .is_some_and(|deadline| deadline <= now)
        })
        .map(|entry| entry.corpus_id.clone())
        .collect())
}

pub fn archived_corpus_report_path(home: &str, corpus_id: &str) -> PathBuf {
    corpus_registry_root(home)
        .join("reports")
        .join(format!("{corpus_id}.json"))
}

pub struct CorpusStartupReconciliationSummary {
    pub scanned_corpora: usize,
    pub changed_corpora: usize,
    pub orphaned_corpora: usize,
    pub cleanup_succeeded_consistent: usize,
    pub unchanged_corpora: usize,
    pub notes: Vec<String>,
}

pub fn reconcile_corpora_on_startup(home: &str) -> Result<CorpusStartupReconciliationSummary> {
    let mut registry = CorpusRegistry::load(home)?;
    let mut summary = CorpusStartupReconciliationSummary {
        scanned_corpora: registry.corpora.len(),
        changed_corpora: 0,
        orphaned_corpora: 0,
        cleanup_succeeded_consistent: 0,
        unchanged_corpora: 0,
        notes: Vec::new(),
    };

    for entry in &mut registry.corpora {
        let message = reconcile_corpus_entry(entry);

        match message {
            CorpusReconciliationOutcome::Changed(note) => {
                summary.changed_corpora += 1;
                if entry.lifecycle.state == CorpusLifecycleState::Orphaned {
                    summary.orphaned_corpora += 1;
                }
                summary.notes.push(format!("{}: {}", entry.corpus_id, note));
            }
            CorpusReconciliationOutcome::CleanupConsistent(note) => {
                summary.cleanup_succeeded_consistent += 1;
                summary.unchanged_corpora += 1;
                summary.notes.push(format!("{}: {}", entry.corpus_id, note));
            }
            CorpusReconciliationOutcome::Unchanged(note) => {
                summary.unchanged_corpora += 1;
                summary.notes.push(format!("{}: {}", entry.corpus_id, note));
            }
        }
    }

    if summary.changed_corpora > 0 {
        registry.save(home)?;
    }

    Ok(summary)
}

pub fn sync_corpus_report_lifecycle(
    report_path: &Path,
    lifecycle: &CorpusLifecycleMetadata,
) -> Result<()> {
    if !report_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(report_path)?;
    let mut value: Value = serde_json::from_str(&raw)?;
    value["lifecycle"] = serde_json::to_value(lifecycle.to_report())?;
    fs::write(report_path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

enum CorpusReconciliationOutcome {
    Changed(String),
    CleanupConsistent(String),
    Unchanged(String),
}

fn reconcile_corpus_entry(entry: &mut CorpusIndexEntry) -> CorpusReconciliationOutcome {
    let root_exists = Path::new(&entry.root_path).exists();
    let report_exists = Path::new(&entry.report_path).exists();

    if entry.lifecycle.state == CorpusLifecycleState::CleanupPending {
        entry.mark_orphaned_with_note(
            "Startup found this corpus still marked cleanup_pending, so the previous cleanup attempt likely ended before completion and needs operator review."
                .to_string(),
        );
        return CorpusReconciliationOutcome::Changed(
            "Startup found a corpus still marked cleanup_pending; reclassified as orphaned."
                .to_string(),
        );
    }

    if entry.lifecycle.state == CorpusLifecycleState::CleanupSucceeded && !root_exists {
        if !report_exists {
            entry.mark_orphaned_with_note(
                "Cleanup had been recorded as successful and the corpus root is gone, but the retained report is also missing. The corpus was marked orphaned for investigation."
                    .to_string(),
            );
            return CorpusReconciliationOutcome::Changed(
                "Cleanup had been recorded as successful and the corpus root is gone, but the retained report is missing; marked orphaned for investigation."
                    .to_string(),
            );
        }

        return CorpusReconciliationOutcome::CleanupConsistent(
            "Cleanup had already succeeded and startup confirmed the corpus root remains removed."
                .to_string(),
        );
    }

    if !root_exists
        && entry.lifecycle.state != CorpusLifecycleState::CleanupSucceeded
        && entry.lifecycle.state != CorpusLifecycleState::CleanupFailed
    {
        entry.mark_orphaned_with_note(
            "The corpus root is missing even though successful lifecycle cleanup was never recorded. The corpus was marked orphaned for investigation."
                .to_string(),
        );
        return CorpusReconciliationOutcome::Changed(
            "Corpus root is missing even though lifecycle cleanup was not recorded as successful; marked orphaned."
                .to_string(),
        );
    }

    if root_exists && entry.lifecycle.state == CorpusLifecycleState::CleanupSucceeded {
        entry.mark_orphaned_with_note(
            "Lifecycle cleanup had been recorded as successful, but the corpus root still exists on disk. The corpus was marked orphaned for investigation."
                .to_string(),
        );
        return CorpusReconciliationOutcome::Changed(
            "Corpus root still exists even though cleanup was previously recorded as successful; marked orphaned."
                .to_string(),
        );
    }

    if !report_exists
        && entry.lifecycle.state != CorpusLifecycleState::CleanupSucceeded
        && entry.lifecycle.state != CorpusLifecycleState::CleanupFailed
    {
        entry.mark_orphaned_with_note(
            "The retained corpus report is missing even though successful cleanup was never recorded. The corpus was marked orphaned for investigation."
                .to_string(),
        );
        return CorpusReconciliationOutcome::Changed(
            "Corpus report is missing while cleanup was not recorded as successful; marked orphaned."
                .to_string(),
        );
    }

    entry.lifecycle.updated_at = Some(current_timestamp());
    entry.lifecycle.state_note = Some(
        "Startup reconciliation confirmed that the corpus registry entry still matches the corpus artifacts on disk."
            .to_string(),
    );
    CorpusReconciliationOutcome::Unchanged(
        "Corpus paths are present and no reconciliation changes were needed.".to_string(),
    )
}

#![allow(dead_code)]

use crate::corpus::{
    corpus_registry_root, CorpusLifecycleMetadata, CorpusManifest, CorpusRetentionPolicy,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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
            lifecycle: manifest.lifecycle.clone(),
        }
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
    Ok(())
}

pub fn list_corpora(home: &str) -> Result<CorpusRegistry> {
    CorpusRegistry::load(home)
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

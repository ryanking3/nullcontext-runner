#![allow(dead_code)]

use crate::audit::RetrievalReport;
use crate::corpus::CorpusManifest;
use crate::corpus_registry::{validate_corpus_ready, CorpusRegistry};
use crate::docs::ChunkRecord;
use crate::embed::{
    cosine_similarity, embed_text, EmbeddingRecord, EMBEDDING_BACKEND, EMBEDDING_MODEL,
};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct QueryCorpusRequest {
    pub query: String,
    pub top_k: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryCorpusResponse {
    pub corpus_id: String,
    pub corpus_name: String,
    pub embedding_backend: String,
    pub embedding_model: String,
    pub query: String,
    pub top_k: usize,
    pub results: Vec<RetrievalResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalResult {
    pub chunk_id: String,
    pub source_id: String,
    pub source_path: String,
    pub page_number: Option<u32>,
    pub score: f32,
    pub text_preview: String,
    pub text: String,
}

pub fn build_grounded_prompt(response: &QueryCorpusResponse) -> String {
    let mut sections = Vec::new();
    sections.push(format!(
        "[NullContext Retrieval Context]\nCorpus: {} ({})\nTop-K: {}\n",
        response.corpus_name, response.corpus_id, response.top_k
    ));

    for (index, result) in response.results.iter().enumerate() {
        let page_suffix = result
            .page_number
            .map(|page| format!(" page {}", page))
            .unwrap_or_default();
        sections.push(format!(
            "[Source {}]\nPath: {}{}\nScore: {:.4}\n{}\n",
            index + 1,
            result.source_path,
            page_suffix,
            result.score,
            result.text
        ));
    }

    sections.push(format!(
        "[User Question]\n{}\n\nAnswer using the retrieval context when relevant. If the corpus does not contain the answer, say so plainly.",
        response.query
    ));

    sections.join("\n")
}

pub fn build_retrieval_report(response: &QueryCorpusResponse) -> RetrievalReport {
    let mut source_paths = Vec::new();
    let mut page_hits = Vec::new();

    for result in &response.results {
        if !source_paths.contains(&result.source_path) {
            source_paths.push(result.source_path.clone());
        }

        if let Some(page_number) = result.page_number {
            let page_hit = format!("{}#page-{}", result.source_path, page_number);
            if !page_hits.contains(&page_hit) {
                page_hits.push(page_hit);
            }
        }
    }

    RetrievalReport {
        corpus_id: response.corpus_id.clone(),
        corpus_name: response.corpus_name.clone(),
        retrieval_mode: "one_shot".to_string(),
        query: response.query.clone(),
        top_k: response.top_k,
        grounded_turns: 1,
        retrieved_chunks: response.results.len(),
        source_paths,
        page_hits,
        context_injected: true,
    }
}

pub fn build_active_chat_retrieval_report(
    corpus_id: &str,
    corpus_name: &str,
    reports: &[RetrievalReport],
) -> Option<RetrievalReport> {
    let latest = reports.last()?;
    let mut source_paths = BTreeSet::new();
    let mut page_hits = BTreeSet::new();
    let mut retrieved_chunks = 0usize;

    for report in reports {
        retrieved_chunks += report.retrieved_chunks;

        for source_path in &report.source_paths {
            source_paths.insert(source_path.clone());
        }

        for page_hit in &report.page_hits {
            page_hits.insert(page_hit.clone());
        }
    }

    Some(RetrievalReport {
        corpus_id: corpus_id.to_string(),
        corpus_name: corpus_name.to_string(),
        retrieval_mode: "active_chat".to_string(),
        query: latest.query.clone(),
        top_k: latest.top_k,
        grounded_turns: reports.len(),
        retrieved_chunks,
        source_paths: source_paths.into_iter().collect(),
        page_hits: page_hits.into_iter().collect(),
        context_injected: true,
    })
}

pub fn query_corpus(
    home: &str,
    corpus_id: &str,
    request: QueryCorpusRequest,
) -> Result<QueryCorpusResponse> {
    let registry = CorpusRegistry::load(home)?;
    let entry = registry
        .find(corpus_id)
        .ok_or_else(|| anyhow!("Corpus not found in registry: {corpus_id}"))?;
    validate_corpus_ready(entry)?;

    let manifest = load_json::<CorpusManifest>(&entry.manifest_path)?;
    let chunks = load_json::<Vec<ChunkRecord>>(&manifest.artifact_paths.chunks_path)?;
    let embeddings = load_json::<Vec<EmbeddingRecord>>(&manifest.artifact_paths.embeddings_path)?;

    let top_k = request.top_k.unwrap_or(6).clamp(1, 24);
    let query_vector = embed_text(&request.query);
    let chunk_map = chunks
        .into_iter()
        .map(|chunk| (chunk.chunk_id.clone(), chunk))
        .collect::<HashMap<_, _>>();

    let mut scored = embeddings
        .into_iter()
        .filter_map(|embedding| {
            let chunk = chunk_map.get(&embedding.chunk_id)?;
            Some(RetrievalResult {
                chunk_id: chunk.chunk_id.clone(),
                source_id: chunk.source_id.clone(),
                source_path: chunk.source_path.clone(),
                page_number: chunk.page_number,
                score: cosine_similarity(&query_vector, &embedding.vector),
                text_preview: chunk.text_preview.clone(),
                text: chunk.text.clone(),
            })
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_k);

    Ok(QueryCorpusResponse {
        corpus_id: manifest.corpus_id,
        corpus_name: manifest.name,
        embedding_backend: manifest
            .embedding_backend
            .unwrap_or_else(|| EMBEDDING_BACKEND.to_string()),
        embedding_model: manifest
            .embedding_model
            .unwrap_or_else(|| EMBEDDING_MODEL.to_string()),
        query: request.query,
        top_k,
        results: scored,
    })
}

fn load_json<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T> {
    let raw = fs::read_to_string(Path::new(path))
        .with_context(|| format!("Failed to read corpus artifact {}", path))?;
    let parsed = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse corpus artifact {}", path))?;
    Ok(parsed)
}

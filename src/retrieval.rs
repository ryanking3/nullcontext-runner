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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{ensure_corpus_artifact_dirs, CorpusLifecycleState};
    use crate::corpus_registry::register_corpus;
    use crate::embed::embed_chunks;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestHome {
        path: PathBuf,
    }

    impl TestHome {
        fn new() -> Self {
            let unique = format!(
                "nullcontext-retrieval-tests-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time should be after unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(path.join(".nullcontext"))
                .expect("test home should create .nullcontext directory");
            Self { path }
        }

        fn home(&self) -> String {
            self.path.to_string_lossy().into_owned()
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_json<T: Serialize>(path: &str, value: &T) {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).expect("artifact directory should exist");
        }
        let body = serde_json::to_string_pretty(value).expect("json should serialize");
        fs::write(path, body).expect("json fixture should be writable");
    }

    fn ready_manifest_with_chunks(
        home: &TestHome,
        chunks: Vec<ChunkRecord>,
        include_embedding_metadata: bool,
    ) -> CorpusManifest {
        let mut manifest = CorpusManifest::new("Fixture Corpus", &home.home(), true);
        manifest.lifecycle.state = CorpusLifecycleState::Ready;
        if !include_embedding_metadata {
            manifest.embedding_backend = None;
            manifest.embedding_model = None;
        }
        manifest.source_count = 2;
        manifest.chunk_count = chunks.len();

        ensure_corpus_artifact_dirs(&manifest.artifact_paths)
            .expect("artifact directories should be creatable");
        write_json(&manifest.artifact_paths.manifest_path, &manifest);
        write_json(&manifest.artifact_paths.chunks_path, &chunks);
        write_json(
            &manifest.artifact_paths.embeddings_path,
            &embed_chunks(&chunks),
        );
        register_corpus(&home.home(), &manifest).expect("registry entry should be writable");
        manifest
    }

    #[test]
    fn query_corpus_returns_ranked_results_and_fallback_embedding_metadata() {
        let home = TestHome::new();
        let chunks = vec![
            ChunkRecord {
                chunk_id: "chunk-1".to_string(),
                source_id: "source-1".to_string(),
                source_path: "C:/fixtures/alpha.txt".to_string(),
                page_number: None,
                chunk_index: 0,
                token_estimate: 3,
                text_preview: "alpha alpha alpha".to_string(),
                text: "alpha alpha alpha".to_string(),
            },
            ChunkRecord {
                chunk_id: "chunk-2".to_string(),
                source_id: "source-2".to_string(),
                source_path: "C:/fixtures/report.pdf".to_string(),
                page_number: Some(3),
                chunk_index: 0,
                token_estimate: 2,
                text_preview: "alpha beta".to_string(),
                text: "alpha beta".to_string(),
            },
            ChunkRecord {
                chunk_id: "chunk-3".to_string(),
                source_id: "source-2".to_string(),
                source_path: "C:/fixtures/report.pdf".to_string(),
                page_number: Some(4),
                chunk_index: 1,
                token_estimate: 2,
                text_preview: "gamma delta".to_string(),
                text: "gamma delta".to_string(),
            },
        ];
        let manifest = ready_manifest_with_chunks(&home, chunks, false);

        let response = query_corpus(
            &home.home(),
            &manifest.corpus_id,
            QueryCorpusRequest {
                query: "alpha".to_string(),
                top_k: Some(2),
            },
        )
        .expect("query should succeed");

        assert_eq!(response.corpus_id, manifest.corpus_id);
        assert_eq!(response.corpus_name, "Fixture Corpus");
        assert_eq!(response.embedding_backend, EMBEDDING_BACKEND);
        assert_eq!(response.embedding_model, EMBEDDING_MODEL);
        assert_eq!(response.top_k, 2);
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].chunk_id, "chunk-1");
        assert_eq!(response.results[0].source_path, "C:/fixtures/alpha.txt");
        assert_eq!(response.results[0].page_number, None);
        assert_eq!(response.results[1].chunk_id, "chunk-2");
        assert_eq!(response.results[1].source_path, "C:/fixtures/report.pdf");
        assert_eq!(response.results[1].page_number, Some(3));
        assert!(response.results[0].score >= response.results[1].score);
    }

    #[test]
    fn query_corpus_clamps_requested_top_k_to_twenty_four_results() {
        let home = TestHome::new();
        let chunks = (0..30)
            .map(|index| ChunkRecord {
                chunk_id: format!("chunk-{}", index + 1),
                source_id: format!("source-{}", index + 1),
                source_path: format!("C:/fixtures/doc-{}.txt", index + 1),
                page_number: None,
                chunk_index: 0,
                token_estimate: 1,
                text_preview: format!("alpha {}", index + 1),
                text: format!("alpha {}", index + 1),
            })
            .collect::<Vec<_>>();
        let manifest = ready_manifest_with_chunks(&home, chunks, true);

        let response = query_corpus(
            &home.home(),
            &manifest.corpus_id,
            QueryCorpusRequest {
                query: "alpha".to_string(),
                top_k: Some(99),
            },
        )
        .expect("query should succeed");

        assert_eq!(response.top_k, 24);
        assert_eq!(response.results.len(), 24);
    }

    #[test]
    fn retrieval_reports_deduplicate_provenance_and_aggregate_active_chat_turns() {
        let response = QueryCorpusResponse {
            corpus_id: "corpus-1".to_string(),
            corpus_name: "Fixture Corpus".to_string(),
            embedding_backend: EMBEDDING_BACKEND.to_string(),
            embedding_model: EMBEDDING_MODEL.to_string(),
            query: "alpha".to_string(),
            top_k: 3,
            results: vec![
                RetrievalResult {
                    chunk_id: "chunk-1".to_string(),
                    source_id: "source-1".to_string(),
                    source_path: "C:/fixtures/report.pdf".to_string(),
                    page_number: Some(2),
                    score: 0.9,
                    text_preview: "alpha".to_string(),
                    text: "alpha".to_string(),
                },
                RetrievalResult {
                    chunk_id: "chunk-2".to_string(),
                    source_id: "source-1".to_string(),
                    source_path: "C:/fixtures/report.pdf".to_string(),
                    page_number: Some(2),
                    score: 0.8,
                    text_preview: "beta".to_string(),
                    text: "beta".to_string(),
                },
                RetrievalResult {
                    chunk_id: "chunk-3".to_string(),
                    source_id: "source-2".to_string(),
                    source_path: "C:/fixtures/notes.txt".to_string(),
                    page_number: None,
                    score: 0.7,
                    text_preview: "gamma".to_string(),
                    text: "gamma".to_string(),
                },
            ],
        };

        let one_shot = build_retrieval_report(&response);
        assert_eq!(one_shot.retrieval_mode, "one_shot");
        assert_eq!(
            one_shot.source_paths,
            vec![
                "C:/fixtures/report.pdf".to_string(),
                "C:/fixtures/notes.txt".to_string()
            ]
        );
        assert_eq!(
            one_shot.page_hits,
            vec!["C:/fixtures/report.pdf#page-2".to_string()]
        );

        let follow_up = RetrievalReport {
            corpus_id: "corpus-1".to_string(),
            corpus_name: "Fixture Corpus".to_string(),
            retrieval_mode: "one_shot".to_string(),
            query: "beta".to_string(),
            top_k: 1,
            grounded_turns: 1,
            retrieved_chunks: 1,
            source_paths: vec!["C:/fixtures/appendix.pdf".to_string()],
            page_hits: vec!["C:/fixtures/appendix.pdf#page-9".to_string()],
            context_injected: true,
        };

        let active_chat = build_active_chat_retrieval_report(
            "corpus-1",
            "Fixture Corpus",
            &[one_shot, follow_up],
        )
        .expect("active chat aggregation should exist");

        assert_eq!(active_chat.retrieval_mode, "active_chat");
        assert_eq!(active_chat.query, "beta");
        assert_eq!(active_chat.top_k, 1);
        assert_eq!(active_chat.grounded_turns, 2);
        assert_eq!(active_chat.retrieved_chunks, 4);
        assert_eq!(
            active_chat.source_paths,
            vec![
                "C:/fixtures/appendix.pdf".to_string(),
                "C:/fixtures/notes.txt".to_string(),
                "C:/fixtures/report.pdf".to_string()
            ]
        );
        assert_eq!(
            active_chat.page_hits,
            vec![
                "C:/fixtures/appendix.pdf#page-9".to_string(),
                "C:/fixtures/report.pdf#page-2".to_string()
            ]
        );
    }
}

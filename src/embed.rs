#![allow(dead_code)]

use crate::docs::ChunkRecord;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const EMBEDDING_DIMENSIONS: usize = 256;
pub const EMBEDDING_BACKEND: &str = "hashed_text_v1";
pub const EMBEDDING_MODEL: &str = "local_hash_embedding_256";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub chunk_id: String,
    pub vector: Vec<f32>,
}

pub fn embed_chunks(chunks: &[ChunkRecord]) -> Vec<EmbeddingRecord> {
    chunks
        .iter()
        .map(|chunk| EmbeddingRecord {
            chunk_id: chunk.chunk_id.clone(),
            vector: embed_text(&chunk.text),
        })
        .collect()
}

pub fn embed_text(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; EMBEDDING_DIMENSIONS];

    for token in tokenize(text) {
        let index = hash_token(&token) % EMBEDDING_DIMENSIONS;
        vector[index] += 1.0;
    }

    normalize(&mut vector);
    vector
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;

    for (l, r) in left.iter().zip(right.iter()) {
        dot += l * r;
        left_norm += l * l;
        right_norm += r * r;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }

    dot / (left_norm.sqrt() * right_norm.sqrt())
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|char: char| !char.is_alphanumeric())
        .filter(|token| !token.trim().is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn hash_token(token: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish() as usize
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();

    if norm == 0.0 {
        return;
    }

    for value in vector.iter_mut() {
        *value /= norm;
    }
}

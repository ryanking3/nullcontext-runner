use crate::corpus::{
    ensure_corpus_artifact_dirs, CorpusLifecycleReport, CorpusLifecycleState, CorpusManifest,
};
use crate::corpus_registry::{register_corpus, CorpusIndexEntry};
use crate::embed::{embed_chunks, EMBEDDING_BACKEND, EMBEDDING_MODEL};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use lopdf::Document;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;
use walkdir::WalkDir;

const CHUNK_SIZE_CHARS: usize = 1200;
const CHUNK_OVERLAP_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Text,
    Markdown,
    Pdf,
}

impl SourceKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Ready,
    Partial,
    Failed,
}

impl ExtractionStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusSourceRecord {
    pub source_id: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
    pub included: bool,
    pub extraction_status: String,
    pub extraction_message: Option<String>,
    pub text_bytes: usize,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPageRecord {
    pub source_id: String,
    pub source_path: String,
    pub page_number: u32,
    pub native_text_bytes: usize,
    pub ocr_text_bytes: usize,
    pub used_ocr: bool,
    pub status: String,
    pub warning: Option<String>,
    pub text_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub chunk_id: String,
    pub source_id: String,
    pub source_path: String,
    pub page_number: Option<u32>,
    pub chunk_index: usize,
    pub token_estimate: usize,
    pub text_preview: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusIngestionReport {
    pub corpus_id: String,
    pub created_at: String,
    pub persistent: bool,
    pub source_paths_requested: Vec<String>,
    pub files_discovered: usize,
    pub files_ingested: usize,
    pub files_failed: usize,
    pub pdf_pages_seen: usize,
    pub pdf_pages_ocrd: usize,
    pub chunk_count: usize,
    pub ocr_enabled: bool,
    pub warnings: Vec<String>,
    pub lifecycle: Option<CorpusLifecycleReport>,
    pub upload_staging: Option<UploadStagingReport>,
    pub residual_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStagingReport {
    pub staging_root: String,
    pub staged_files: usize,
    pub staged_bytes: u64,
    pub source_filenames: Vec<String>,
    pub cleaned_up: bool,
    pub cleanup_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestCorpusRequest {
    pub name: String,
    pub paths: Vec<String>,
    pub persistent: Option<bool>,
    pub ocr_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestCorpusResponse {
    pub corpus: CorpusIndexEntry,
    pub report: CorpusIngestionReport,
}

#[derive(Debug, Clone)]
pub struct UploadedCorpusFile {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct IngestUploadedCorpusRequest {
    pub name: String,
    pub persistent: Option<bool>,
    pub ocr_enabled: Option<bool>,
    pub files: Vec<UploadedCorpusFile>,
}

pub fn ingest_corpus(home: &str, request: IngestCorpusRequest) -> Result<IngestCorpusResponse> {
    let persistent = request.persistent.unwrap_or(true);
    let ocr_enabled = request.ocr_enabled.unwrap_or(true);

    let mut manifest = CorpusManifest::new(request.name, home, persistent);
    manifest.lifecycle.state = CorpusLifecycleState::Building;
    manifest.lifecycle.state_note = Some(
        "Corpus ingestion is running and NullContext is building the manifest, extracted text, chunks, embeddings, and report artifacts."
            .to_string(),
    );
    manifest.lifecycle.updated_at = Some(Utc::now().to_rfc3339());
    manifest.chunk_strategy = format!("char_window:{}:{}", CHUNK_SIZE_CHARS, CHUNK_OVERLAP_CHARS);
    manifest.ocr_backend = Some(if ocr_enabled {
        "tesseract_cli".to_string()
    } else {
        "disabled".to_string()
    });
    manifest.embedding_backend = Some(EMBEDDING_BACKEND.to_string());
    manifest.embedding_model = Some(EMBEDDING_MODEL.to_string());

    ensure_corpus_artifact_dirs(&manifest.artifact_paths)?;
    write_manifest(&manifest)?;

    let ingestion_result = run_ingestion(&manifest, &request.paths, ocr_enabled);

    match ingestion_result {
        Ok(mut result) => {
            manifest.source_count = result.sources.len();
            manifest.chunk_count = result.chunks.len();
            manifest.lifecycle.state = if result.report.files_failed > 0 {
                CorpusLifecycleState::Ready
            } else {
                CorpusLifecycleState::Ready
            };
            manifest.lifecycle.state_note = Some(if result.report.files_failed > 0 {
                "Corpus ingestion completed with some source-level failures, but NullContext finalized a usable retrieval corpus from the successfully processed inputs."
                    .to_string()
            } else {
                "Corpus ingestion completed successfully and retrieval artifacts are ready under the current lifecycle policy."
                    .to_string()
            });
            manifest.lifecycle.updated_at = Some(Utc::now().to_rfc3339());
            let embeddings = embed_chunks(&result.chunks);

            result.report.chunk_count = manifest.chunk_count;
            result.report.files_ingested = manifest
                .source_count
                .saturating_sub(result.report.files_failed);
            result.report.lifecycle = Some(manifest.lifecycle.to_report());
            result.report.upload_staging = None;

            write_json(&manifest.artifact_paths.sources_path, &result.sources)?;
            write_json(&manifest.artifact_paths.pages_path, &result.pages)?;
            write_json(&manifest.artifact_paths.chunks_path, &result.chunks)?;
            write_json(&manifest.artifact_paths.embeddings_path, &embeddings)?;
            write_json(
                &manifest.artifact_paths.ingestion_report_path,
                &result.report,
            )?;
            write_manifest(&manifest)?;
            register_corpus(home, &manifest)?;

            Ok(IngestCorpusResponse {
                corpus: CorpusIndexEntry::from_manifest(&manifest),
                report: result.report,
            })
        }
        Err(error) => {
            manifest.lifecycle.state = CorpusLifecycleState::IngestionFailed;
            manifest.lifecycle.state_note = Some(
                "Corpus ingestion failed before NullContext could finalize a usable retrieval corpus."
                    .to_string(),
            );
            manifest.lifecycle.updated_at = Some(Utc::now().to_rfc3339());
            let _ = write_manifest(&manifest);
            let _ = register_corpus(home, &manifest);
            Err(error)
        }
    }
}

pub fn ingest_uploaded_corpus(
    home: &str,
    request: IngestUploadedCorpusRequest,
) -> Result<IngestCorpusResponse> {
    if request.files.is_empty() {
        return Err(anyhow!(
            "No uploaded files were provided. Supported types: .txt, .md, .pdf"
        ));
    }

    let staging = stage_uploaded_files(&request.files)?;
    let original_names = request
        .files
        .iter()
        .map(|file| format!("browser_upload:{}", file.file_name))
        .collect::<Vec<_>>();

    let ingest_result = ingest_corpus(
        home,
        IngestCorpusRequest {
            name: request.name,
            paths: vec![staging.root.display().to_string()],
            persistent: request.persistent,
            ocr_enabled: request.ocr_enabled,
        },
    );

    let cleanup_result = cleanup_upload_staging(&staging.root);

    match ingest_result {
        Ok(mut response) => {
            rewrite_uploaded_source_paths(&response.corpus.root_path, &staging.path_map)?;

            response.report.source_paths_requested = original_names;
            response.report.upload_staging = Some(UploadStagingReport {
                staging_root: staging.root.display().to_string(),
                staged_files: staging.staged_files,
                staged_bytes: staging.staged_bytes,
                source_filenames: staging.path_map.values().cloned().collect::<Vec<_>>(),
                cleaned_up: cleanup_result.is_ok(),
                cleanup_error: cleanup_result.err().map(|error| error.to_string()),
            });

            if let Some(staging_report) = &response.report.upload_staging {
                if !staging_report.cleaned_up {
                    response.report.warnings.push(format!(
                        "Upload staging cleanup failed for {}.",
                        staging_report.staging_root
                    ));
                }
            }

            let ingestion_report_path =
                Path::new(&response.corpus.root_path).join("ingestion_report.json");
            write_json(
                &ingestion_report_path.display().to_string(),
                &response.report,
            )?;

            Ok(response)
        }
        Err(error) => {
            let _ = cleanup_result;
            Err(error)
        }
    }
}

struct IngestionArtifacts {
    sources: Vec<CorpusSourceRecord>,
    pages: Vec<PdfPageRecord>,
    chunks: Vec<ChunkRecord>,
    report: CorpusIngestionReport,
}

fn run_ingestion(
    manifest: &CorpusManifest,
    requested_paths: &[String],
    ocr_enabled: bool,
) -> Result<IngestionArtifacts> {
    let discovered = discover_supported_files(requested_paths)?;

    if discovered.is_empty() {
        return Err(anyhow!(
            "No supported files were found. Supported types: .txt, .md, .pdf"
        ));
    }

    let mut sources = Vec::new();
    let mut pages = Vec::new();
    let mut chunks = Vec::new();
    let mut warnings = Vec::new();
    let mut files_failed = 0usize;
    let mut pdf_pages_seen = 0usize;
    let mut pdf_pages_ocrd = 0usize;

    for (index, file) in discovered.iter().enumerate() {
        let source_id = format!("source-{}", index + 1);
        let metadata = fs::metadata(file)
            .with_context(|| format!("Failed to read metadata for {}", file.display()))?;
        let kind = detect_source_kind(file)
            .ok_or_else(|| anyhow!("Unsupported file type discovered: {}", file.display()))?;

        match kind {
            SourceKind::Text | SourceKind::Markdown => {
                let text = fs::read_to_string(file)
                    .with_context(|| format!("Failed to read text file {}", file.display()))?;

                let source_chunks = build_chunks(&source_id, file, None, &text, chunks.len());
                let chunk_count = source_chunks.len();
                chunks.extend(source_chunks);

                sources.push(CorpusSourceRecord {
                    source_id,
                    path: file.display().to_string(),
                    kind: kind.as_str().to_string(),
                    size_bytes: metadata.len(),
                    included: true,
                    extraction_status: ExtractionStatus::Ready.as_str().to_string(),
                    extraction_message: None,
                    text_bytes: text.len(),
                    chunk_count,
                });
            }
            SourceKind::Pdf => {
                let pdf_result = ingest_pdf(
                    file,
                    &source_id,
                    &manifest.artifact_paths.root_dir,
                    ocr_enabled,
                )
                .with_context(|| format!("Failed to ingest PDF {}", file.display()))?;

                pdf_pages_seen += pdf_result.page_records.len();
                pdf_pages_ocrd += pdf_result
                    .page_records
                    .iter()
                    .filter(|page| page.used_ocr)
                    .count();
                if let Some(message) = &pdf_result.source.extraction_message {
                    warnings.push(format!("{}: {message}", file.display()));
                }
                if pdf_result.source.extraction_status == ExtractionStatus::Failed.as_str() {
                    files_failed += 1;
                }

                chunks.extend(pdf_result.chunks);
                pages.extend(pdf_result.page_records);
                sources.push(CorpusSourceRecord {
                    chunk_count: pdf_result.source.chunk_count,
                    ..pdf_result.source
                });
            }
        }
    }

    let report = CorpusIngestionReport {
        corpus_id: manifest.corpus_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        persistent: manifest.persistent,
        source_paths_requested: requested_paths.to_vec(),
        files_discovered: discovered.len(),
        files_ingested: 0,
        files_failed,
        pdf_pages_seen,
        pdf_pages_ocrd,
        chunk_count: chunks.len(),
        ocr_enabled,
        warnings,
        lifecycle: Some(manifest.lifecycle.to_report()),
        upload_staging: None,
        residual_risk: "Extracted text, chunk artifacts, and any OCR-derived text may persist in corpus artifacts until cleanup. OCR rasterization may briefly create temporary page images during ingestion. OS/filesystem caches and process memory are not fully sanitized.".to_string(),
    };

    Ok(IngestionArtifacts {
        sources,
        pages,
        chunks,
        report,
    })
}

struct UploadStagingArea {
    root: PathBuf,
    path_map: HashMap<String, String>,
    staged_files: usize,
    staged_bytes: u64,
}

fn stage_uploaded_files(files: &[UploadedCorpusFile]) -> Result<UploadStagingArea> {
    let root = std::env::temp_dir()
        .join("nullcontext")
        .join("uploads")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&root)?;

    let mut path_map = HashMap::new();
    let mut staged_bytes = 0u64;

    for (index, file) in files.iter().enumerate() {
        let safe_name = sanitize_upload_filename(index, &file.file_name);
        let staged_path = root.join(&safe_name);
        fs::write(&staged_path, &file.bytes)?;
        staged_bytes += file.bytes.len() as u64;
        path_map.insert(staged_path.display().to_string(), file.file_name.clone());
    }

    Ok(UploadStagingArea {
        root,
        path_map,
        staged_files: files.len(),
        staged_bytes,
    })
}

fn cleanup_upload_staging(root: &Path) -> Result<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }

    Ok(())
}

fn sanitize_upload_filename(index: usize, file_name: &str) -> String {
    let candidate = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload.bin");
    let sanitized = candidate
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();

    format!("{:04}-{}", index + 1, sanitized)
}

fn rewrite_uploaded_source_paths(
    corpus_root: &str,
    path_map: &HashMap<String, String>,
) -> Result<()> {
    let root = Path::new(corpus_root);
    let sources_path = root.join("sources.json");
    let pages_path = root.join("pages.json");
    let chunks_path = root.join("chunks.json");

    let mut sources = read_json::<Vec<CorpusSourceRecord>>(&sources_path)?;
    let mut pages = read_json::<Vec<PdfPageRecord>>(&pages_path)?;
    let mut chunks = read_json::<Vec<ChunkRecord>>(&chunks_path)?;

    for source in &mut sources {
        if let Some(file_name) = path_map.get(&source.path) {
            source.path = format!("browser_upload:{file_name}");
        }
    }

    for page in &mut pages {
        if let Some(file_name) = path_map.get(&page.source_path) {
            page.source_path = format!("browser_upload:{file_name}");
        }
    }

    for chunk in &mut chunks {
        if let Some(file_name) = path_map.get(&chunk.source_path) {
            chunk.source_path = format!("browser_upload:{file_name}");
        }
    }

    write_json(&sources_path.display().to_string(), &sources)?;
    write_json(&pages_path.display().to_string(), &pages)?;
    write_json(&chunks_path.display().to_string(), &chunks)?;

    Ok(())
}

struct PdfIngestionResult {
    source: CorpusSourceRecord,
    page_records: Vec<PdfPageRecord>,
    chunks: Vec<ChunkRecord>,
}

fn ingest_pdf(
    path: &Path,
    source_id: &str,
    corpus_root: &str,
    ocr_enabled: bool,
) -> Result<PdfIngestionResult> {
    let document = Document::load(path)?;
    let pages = document.get_pages();
    let mut page_records = Vec::new();
    let mut chunks = Vec::new();
    let mut total_text_bytes = 0usize;
    let mut extraction_failures = 0usize;
    let mut warnings = Vec::new();

    for page_number in pages.keys() {
        let native_text = document.extract_text(&[*page_number]).unwrap_or_default();
        let native_text = normalize_text(&native_text);
        let native_quality_poor = native_text.trim().chars().count() < 40;

        let (final_text, ocr_text, used_ocr, warning) = if ocr_enabled && native_quality_poor {
            match run_page_ocr(path, *page_number, corpus_root) {
                Ok(ocr_text)
                    if ocr_text.trim().chars().count() > native_text.trim().chars().count() =>
                {
                    (
                        normalize_text(&ocr_text),
                        ocr_text,
                        true,
                        Some("Used OCR because native page extraction was sparse.".to_string()),
                    )
                }
                Ok(ocr_text) if native_text.trim().is_empty() && !ocr_text.trim().is_empty() => (
                    normalize_text(&ocr_text),
                    ocr_text,
                    true,
                    Some("Used OCR because native page extraction was empty.".to_string()),
                ),
                Ok(ocr_text) => (
                    native_text.clone(),
                    ocr_text,
                    false,
                    Some(
                        "Attempted OCR, but kept native text because it was more complete."
                            .to_string(),
                    ),
                ),
                Err(error) => (
                    native_text.clone(),
                    String::new(),
                    false,
                    Some(format!("OCR attempt failed: {error}")),
                ),
            }
        } else {
            (native_text.clone(), String::new(), false, None)
        };

        let status = if final_text.trim().is_empty() {
            extraction_failures += 1;
            warnings.push(format!("Page {} extracted no usable text.", page_number));
            ExtractionStatus::Failed
        } else if warning.is_some() {
            ExtractionStatus::Partial
        } else {
            ExtractionStatus::Ready
        };

        total_text_bytes += final_text.len();
        let preview = text_preview(&final_text);
        page_records.push(PdfPageRecord {
            source_id: source_id.to_string(),
            source_path: path.display().to_string(),
            page_number: *page_number,
            native_text_bytes: native_text.len(),
            ocr_text_bytes: ocr_text.len(),
            used_ocr,
            status: status.as_str().to_string(),
            warning: warning.clone(),
            text_preview: preview,
        });

        chunks.extend(build_chunks(
            source_id,
            path,
            Some(*page_number),
            &final_text,
            chunks.len(),
        ));
    }

    let extraction_status = if extraction_failures == page_records.len() {
        ExtractionStatus::Failed
    } else if extraction_failures > 0 || page_records.iter().any(|page| page.used_ocr) {
        ExtractionStatus::Partial
    } else {
        ExtractionStatus::Ready
    };

    let extraction_message = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join(" "))
    };

    Ok(PdfIngestionResult {
        source: CorpusSourceRecord {
            source_id: source_id.to_string(),
            path: path.display().to_string(),
            kind: SourceKind::Pdf.as_str().to_string(),
            size_bytes: fs::metadata(path)?.len(),
            included: true,
            extraction_status: extraction_status.as_str().to_string(),
            extraction_message,
            text_bytes: total_text_bytes,
            chunk_count: chunks.len(),
        },
        page_records,
        chunks,
    })
}

fn build_chunks(
    source_id: &str,
    source_path: &Path,
    page_number: Option<u32>,
    text: &str,
    existing_chunk_count: usize,
) -> Vec<ChunkRecord> {
    let cleaned = normalize_text(text);

    if cleaned.trim().is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = cleaned.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut chunk_index = 0usize;

    while start < chars.len() {
        let end = usize::min(start + CHUNK_SIZE_CHARS, chars.len());
        let text: String = chars[start..end].iter().collect();
        let token_estimate = text.split_whitespace().count();
        chunks.push(ChunkRecord {
            chunk_id: format!("chunk-{}", existing_chunk_count + chunks.len() + 1),
            source_id: source_id.to_string(),
            source_path: source_path.display().to_string(),
            page_number,
            chunk_index,
            token_estimate,
            text_preview: text_preview(&text),
            text,
        });

        if end == chars.len() {
            break;
        }

        let next_start = end.saturating_sub(CHUNK_OVERLAP_CHARS);
        if next_start <= start {
            break;
        }
        start = next_start;
        chunk_index += 1;
    }

    chunks
}

fn discover_supported_files(requested_paths: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path in requested_paths {
        let path = PathBuf::from(path);

        if path.is_file() {
            if detect_source_kind(&path).is_some() {
                files.push(path);
            }
            continue;
        }

        if path.is_dir() {
            for entry in WalkDir::new(&path) {
                let entry = entry?;
                let entry_path = entry.path();
                if entry.file_type().is_file() && detect_source_kind(entry_path).is_some() {
                    files.push(entry_path.to_path_buf());
                }
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn detect_source_kind(path: &Path) -> Option<SourceKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();

    match extension.as_str() {
        "txt" => Some(SourceKind::Text),
        "md" => Some(SourceKind::Markdown),
        "pdf" => Some(SourceKind::Pdf),
        _ => None,
    }
}

fn run_page_ocr(pdf_path: &Path, page_number: u32, corpus_root: &str) -> Result<String> {
    let ocr_dir = Path::new(corpus_root).join("ocr-temp");
    fs::create_dir_all(&ocr_dir)?;

    let base = ocr_dir.join(format!("page-{page_number}"));
    let png_path = base.with_extension("png");

    let raster_status = Command::new("pdftoppm")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-singlefile")
        .arg("-png")
        .arg(pdf_path)
        .arg(&base)
        .status()
        .with_context(|| "Failed to start pdftoppm. Install poppler to enable PDF OCR fallback.")?;

    if !raster_status.success() {
        return Err(anyhow!(
            "pdftoppm failed while rasterizing page {page_number}"
        ));
    }

    let output = Command::new("tesseract")
        .arg(&png_path)
        .arg("stdout")
        .output()
        .with_context(|| {
            "Failed to start tesseract. Install tesseract to enable PDF OCR fallback."
        })?;

    let _ = fs::remove_file(&png_path);

    if !output.status.success() {
        return Err(anyhow!(
            "tesseract failed while OCR-processing page {page_number}"
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn write_manifest(manifest: &CorpusManifest) -> Result<()> {
    write_json(&manifest.artifact_paths.manifest_path, manifest)
}

fn write_json<T: Serialize>(path: &str, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json).with_context(|| format!("Failed to write {}", path))?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let parsed = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn text_preview(text: &str) -> String {
    let preview: String = text.chars().take(160).collect();
    preview.replace('\n', " ")
}

fn normalize_text(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

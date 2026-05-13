use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactRecord {
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizationOperation {
    pub operation: String,
    pub status: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupReport {
    pub attempted: bool,
    pub successful: bool,
    pub workspace_deleted: bool,
    pub files_removed: usize,
    pub directories_removed: usize,
    pub artifacts_detected: Vec<ArtifactRecord>,
    pub sanitization_operations: Vec<SanitizationOperation>,
    pub error: Option<String>,
}

impl CleanupReport {
    pub fn not_attempted(
        artifacts_detected: Vec<ArtifactRecord>,
        sanitization_operations: Vec<SanitizationOperation>,
    ) -> Self {
        Self {
            attempted: false,
            successful: false,
            workspace_deleted: false,
            files_removed: 0,
            directories_removed: 0,
            artifacts_detected,
            sanitization_operations,
            error: None,
        }
    }
}

pub fn scan_artifacts(path: &Path) -> Result<(Vec<ArtifactRecord>, SanitizationOperation)> {
    let mut artifacts = Vec::new();

    if !path.exists() {
        return Ok((
            artifacts,
            SanitizationOperation {
                operation: "workspace_artifact_scan".to_string(),
                status: "successful".to_string(),
                details: "Workspace did not exist".to_string(),
            },
        ));
    }

    recursively_scan(path.to_path_buf(), &mut artifacts)?;

    let operation = SanitizationOperation {
        operation: "workspace_artifact_scan".to_string(),
        status: "successful".to_string(),
        details: format!("Scanned {} artifacts", artifacts.len()),
    };

    Ok((artifacts, operation))
}

pub fn cleanup_ephemeral_workspace(
    path: &Path,
    artifacts_detected: Vec<ArtifactRecord>,
    mut sanitization_operations: Vec<SanitizationOperation>,
) -> CleanupReport {
    let mut report = CleanupReport {
        attempted: true,
        successful: false,
        workspace_deleted: false,
        files_removed: artifacts_detected
            .iter()
            .filter(|a| a.kind == "file")
            .count(),
        directories_removed: artifacts_detected
            .iter()
            .filter(|a| a.kind == "directory")
            .count(),
        artifacts_detected,
        sanitization_operations: vec![],
        error: None,
    };

    if path.exists() {
        match fs::remove_dir_all(path) {
            Ok(_) => {
                sanitization_operations.push(SanitizationOperation {
                    operation: "workspace_recursive_delete".to_string(),
                    status: "successful".to_string(),
                    details: "Workspace directory removed".to_string(),
                });
            }
            Err(error) => {
                sanitization_operations.push(SanitizationOperation {
                    operation: "workspace_recursive_delete".to_string(),
                    status: "failed".to_string(),
                    details: format!("Failed to remove workspace: {error}"),
                });

                report.error = Some(format!("Failed to remove workspace: {error}"));

                report.sanitization_operations = sanitization_operations;

                return report;
            }
        }
    }

    report.workspace_deleted = !path.exists();
    report.successful = report.workspace_deleted && report.error.is_none();
    report.sanitization_operations = sanitization_operations;

    report
}

fn recursively_scan(path: PathBuf, artifacts: &mut Vec<ArtifactRecord>) -> Result<()> {
    let metadata = fs::metadata(&path)?;

    let kind = if metadata.is_file() {
        "file"
    } else if metadata.is_dir() {
        "directory"
    } else {
        "unknown"
    };

    artifacts.push(ArtifactRecord {
        path: path.display().to_string(),
        kind: kind.to_string(),
        size_bytes: metadata.len(),
    });

    if metadata.is_dir() {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            recursively_scan(entry.path(), artifacts)?;
        }
    }

    Ok(())
}

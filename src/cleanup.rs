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
pub struct CleanupReport {
    pub attempted: bool,
    pub successful: bool,
    pub workspace_deleted: bool,
    pub files_removed: usize,
    pub directories_removed: usize,
    pub artifacts: Vec<ArtifactRecord>,
    pub error: Option<String>,
}

impl CleanupReport {
    pub fn not_attempted() -> Self {
        Self {
            attempted: false,
            successful: false,
            workspace_deleted: false,
            files_removed: 0,
            directories_removed: 0,
            artifacts: vec![],
            error: None,
        }
    }
}

pub fn cleanup_ephemeral_workspace(path: &Path) -> CleanupReport {
    let mut report = CleanupReport {
        attempted: true,
        successful: false,
        workspace_deleted: false,
        files_removed: 0,
        directories_removed: 0,
        artifacts: vec![],
        error: None,
    };

    match scan_artifacts(path) {
        Ok(artifacts) => {
            report.files_removed = artifacts.iter().filter(|a| a.kind == "file").count();

            report.directories_removed = artifacts.iter().filter(|a| a.kind == "directory").count();

            report.artifacts = artifacts;
        }
        Err(error) => {
            report.error = Some(format!("Failed to scan artifacts before cleanup: {error}"));
        }
    }

    if path.exists() {
        if let Err(error) = fs::remove_dir_all(path) {
            report.error = Some(format!("Failed to remove workspace: {error}"));
            return report;
        }
    }

    report.workspace_deleted = !path.exists();
    report.successful = report.workspace_deleted && report.error.is_none();

    report
}

fn scan_artifacts(path: &Path) -> Result<Vec<ArtifactRecord>> {
    let mut artifacts = Vec::new();

    if !path.exists() {
        return Ok(artifacts);
    }

    recursively_scan(path.to_path_buf(), &mut artifacts)?;

    Ok(artifacts)
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

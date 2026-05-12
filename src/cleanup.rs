use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct CleanupReport {
    pub attempted: bool,
    pub successful: bool,
    pub workspace_deleted: bool,
    pub files_removed: usize,
    pub directories_removed: usize,
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
        error: None,
    };

    match count_artifacts(path) {
        Ok((files, dirs)) => {
            report.files_removed = files;
            report.directories_removed = dirs;
        }
        Err(error) => {
            report.error = Some(format!("Failed to count artifacts before cleanup: {error}"));
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

fn count_artifacts(path: &Path) -> Result<(usize, usize)> {
    let mut files = 0;
    let mut dirs = 0;

    if !path.exists() {
        return Ok((files, dirs));
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            files += 1;
        } else if metadata.is_dir() {
            dirs += 1;

            let (nested_files, nested_dirs) = count_artifacts(&entry.path())?;
            files += nested_files;
            dirs += nested_dirs;
        }
    }

    Ok((files, dirs))
}

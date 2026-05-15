use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug)]
pub struct Session {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub workspace: PathBuf,
}

impl Session {
    pub fn create() -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let started_at = Utc::now();
        let workspace = create_session_workspace(&id)?;

        Ok(Self {
            id,
            started_at,
            workspace,
        })
    }

    pub fn write_prompt(&self, prompt: &[u8]) -> Result<()> {
        fs::write(self.workspace.join("prompt.txt"), prompt)?;
        Ok(())
    }

    pub fn write_response(&self, response: &[u8]) -> Result<()> {
        fs::write(self.workspace.join("response.txt"), response)?;
        Ok(())
    }

    pub fn write_report(&self, report_json: &str) -> Result<()> {
        fs::write(self.workspace.join("report.json"), report_json)?;
        Ok(())
    }
}

fn create_session_workspace(session_id: &str) -> Result<PathBuf> {
    let base = PathBuf::from("/tmp/nullcontext");

    fs::create_dir_all(&base)?;

    let session_dir = base.join(session_id);

    fs::create_dir_all(&session_dir)?;

    Ok(session_dir)
}

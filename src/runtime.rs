use crate::config::SessionConfig;
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub struct ManagedRuntime {
    child: Child,
    base_url: String,
}

impl ManagedRuntime {
    pub fn launch(config: &SessionConfig) -> Result<Self> {
        println!("Launching llama-server...");

        let child = Command::new(&config.llama_path)
            .arg("-m")
            .arg(&config.model_path)
            .arg("-ngl")
            .arg(&config.gpu_layers)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg("8080")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to launch llama-server")?;

        let runtime = Self {
            child,
            base_url: "http://127.0.0.1:8080".to_string(),
        };

        runtime.wait_until_ready(Duration::from_secs(30))?;

        println!("Runtime healthy.");

        Ok(runtime)
    }

    pub fn completion_url(&self) -> String {
        format!("{}/completion", self.base_url)
    }

    pub fn shutdown(&mut self) -> Result<bool> {
        println!("Shutting down runtime...");

        match self.child.try_wait()? {
            Some(_status) => Ok(true),
            None => {
                self.child.kill()?;
                self.child.wait()?;
                Ok(true)
            }
        }
    }

    fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        println!("Waiting for runtime readiness...");

        let client = Client::new();
        let health_url = format!("{}/health", self.base_url);
        let started_at = Instant::now();

        while started_at.elapsed() < timeout {
            match client.get(&health_url).send() {
                Ok(response) if response.status().is_success() => {
                    return Ok(());
                }
                _ => {
                    thread::sleep(Duration::from_millis(250));
                }
            }
        }

        bail!("llama-server did not become ready within {:?}", timeout)
    }
}

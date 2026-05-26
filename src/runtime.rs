use crate::config::SessionConfig;
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ManagedRuntime {
    child: Child,
    base_url: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeShutdownOutcome {
    pub stopped: bool,
    pub shutdown_method: String,
    pub exit_code: Option<i32>,
    pub graceful_shutdown_supported: bool,
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to launch llama-server at {}", config.llama_path))?;

        let mut runtime = Self {
            child,
            base_url: "http://127.0.0.1:8080".to_string(),
        };

        runtime.wait_until_ready(Duration::from_secs(60))?;

        println!("Runtime healthy.");

        Ok(runtime)
    }

    pub fn completion_url(&self) -> String {
        format!("{}/completion", self.base_url)
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn shutdown(&mut self) -> Result<RuntimeShutdownOutcome> {
        println!("Shutting down runtime...");

        match self.child.try_wait()? {
            Some(status) => Ok(RuntimeShutdownOutcome {
                stopped: true,
                shutdown_method: "already_exited".to_string(),
                exit_code: status.code(),
                graceful_shutdown_supported: false,
            }),
            None => {
                self.child.kill()?;
                let status = self.child.wait()?;
                Ok(RuntimeShutdownOutcome {
                    stopped: true,
                    shutdown_method: "forced_kill_wait".to_string(),
                    exit_code: status.code(),
                    graceful_shutdown_supported: false,
                })
            }
        }
    }

    fn wait_until_ready(&mut self, timeout: Duration) -> Result<()> {
        println!("Waiting for runtime readiness...");

        let client = Client::new();
        let health_url = format!("{}/health", self.base_url);
        let started_at = Instant::now();

        while started_at.elapsed() < timeout {
            if let Some(status) = self.child.try_wait()? {
                let stderr = read_child_stderr(&mut self.child);
                let stdout = read_child_stdout(&mut self.child);

                bail!(
                    "llama-server exited before becoming ready. status: {}\nstdout:\n{}\nstderr:\n{}",
                    status,
                    stdout,
                    stderr
                );
            }

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

fn read_child_stderr(child: &mut Child) -> String {
    let Some(stderr) = child.stderr.as_mut() else {
        return String::new();
    };

    let mut output = String::new();
    let _ = stderr.read_to_string(&mut output);
    output
}

fn read_child_stdout(child: &mut Child) -> String {
    let Some(stdout) = child.stdout.as_mut() else {
        return String::new();
    };

    let mut output = String::new();
    let _ = stdout.read_to_string(&mut output);
    output
}

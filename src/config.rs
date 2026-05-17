use crate::sensitive::SensitiveBytes;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use zeroize::Zeroize;

#[derive(Debug)]
pub enum AppCommand {
    Run(SessionConfig),
    ListSessions,
    ShowReport { session_id: String },
}

#[derive(Debug, Clone)]
pub enum SecurityMode {
    Standard,
    Secure,
    AirGapped,
}

impl SecurityMode {
    pub fn from_str(value: &str) -> Result<Self> {
        match value {
            "standard" => Ok(Self::Standard),
            "secure" => Ok(Self::Secure),
            "air-gapped" => Ok(Self::AirGapped),
            _ => bail!("Invalid security mode: {value}"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Secure => "secure",
            Self::AirGapped => "air-gapped",
        }
    }
}

#[derive(Debug, Clone)]
pub enum PromptSource {
    CliArgs,
    Stdin,
    Default,
}

impl PromptSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CliArgs => "cli_args",
            Self::Stdin => "stdin",
            Self::Default => "default",
        }
    }
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    llama_path: Option<String>,
    model_path: Option<String>,
    default_mode: Option<String>,
    max_tokens: Option<u32>,
    gpu_layers: Option<u32>,
}

impl FileConfig {
    fn load(home: &str) -> Result<Self> {
        let config_path = PathBuf::from(format!("{home}/.nullcontext/config.toml"));

        if !config_path.exists() {
            return Ok(Self {
                llama_path: None,
                model_path: None,
                default_mode: None,
                max_tokens: None,
                gpu_layers: None,
            });
        }

        let raw = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file at {}", config_path.display()))?;

        let parsed: Self = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse config file at {}", config_path.display()))?;

        Ok(parsed)
    }
}

#[derive(Debug)]
pub struct SessionConfig {
    pub home: String,
    pub llama_path: String,
    pub model_path: String,
    pub prompt: SensitiveBytes,
    pub prompt_source: PromptSource,
    pub max_tokens: String,
    pub gpu_layers: String,
    pub ephemeral: bool,
    pub security_mode: SecurityMode,
}

impl AppCommand {
    pub fn from_env() -> Result<Self> {
        let home = env::var("HOME")?;
        let mut args: Vec<String> = env::args().skip(1).collect();

        if args.contains(&"--list-sessions".to_string()) {
            args.zeroize();
            return Ok(Self::ListSessions);
        }

        if let Some(index) = args.iter().position(|arg| arg == "--show-report") {
            if index + 1 >= args.len() {
                args.zeroize();
                bail!("--show-report requires a session id");
            }

            let session_id = args[index + 1].clone();
            args.zeroize();

            return Ok(Self::ShowReport { session_id });
        }

        let config = SessionConfig::from_args(home, args)?;

        Ok(Self::Run(config))
    }
}

impl SessionConfig {
    fn from_args(home: String, mut args: Vec<String>) -> Result<Self> {
        let file_config = FileConfig::load(&home)?;

        let persistent = args.contains(&"--persistent".to_string());
        let use_stdin = args.contains(&"--stdin".to_string());

        let mut security_mode =
            SecurityMode::from_str(file_config.default_mode.as_deref().unwrap_or("secure"))?;

        let mut filtered_args: Vec<String> = Vec::new();

        let mut i = 0;

        while i < args.len() {
            match args[i].as_str() {
                "--persistent" => {
                    i += 1;
                }
                "--stdin" => {
                    i += 1;
                }
                "--mode" => {
                    if i + 1 >= args.len() {
                        args.zeroize();
                        filtered_args.zeroize();
                        bail!("--mode requires a value");
                    }

                    security_mode = SecurityMode::from_str(&args[i + 1])?;
                    i += 2;
                }
                arg => {
                    filtered_args.push(arg.to_string());
                    i += 1;
                }
            }
        }

        let (prompt, prompt_source) = if use_stdin {
            let mut stdin_prompt = String::new();

            io::stdin()
                .read_to_string(&mut stdin_prompt)
                .context("Failed to read prompt from stdin")?;

            (stdin_prompt, PromptSource::Stdin)
        } else {
            let cli_prompt = filtered_args.join(" ");

            if cli_prompt.trim().is_empty() {
                ("Hello from NullContext".to_string(), PromptSource::Default)
            } else {
                (cli_prompt, PromptSource::CliArgs)
            }
        };

        let prompt_bytes = SensitiveBytes::new(prompt);

        filtered_args.zeroize();
        args.zeroize();

        let ephemeral = match security_mode {
            SecurityMode::Standard => !persistent,
            SecurityMode::Secure => true,
            SecurityMode::AirGapped => true,
        };

        Ok(Self {
            home: home.clone(),
            llama_path: file_config
                .llama_path
                .unwrap_or_else(|| format!("{home}/dev/llama.cpp/build/bin/llama-server")),
            model_path: file_config
                .model_path
                .unwrap_or_else(|| format!("{home}/models/qwen2.5-0.5b-instruct-q4_k_m.gguf")),
            prompt: prompt_bytes,
            prompt_source,
            max_tokens: file_config.max_tokens.unwrap_or(256).to_string(),
            gpu_layers: file_config.gpu_layers.unwrap_or(0).to_string(),
            ephemeral,
            security_mode,
        })
    }
}

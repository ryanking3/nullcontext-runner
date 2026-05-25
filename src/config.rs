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
    Serve,
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

fn home_dir() -> Result<String> {
    if let Ok(home) = std::env::var("HOME") {
        return Ok(home);
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        return Ok(user_profile);
    }

    bail!("Could not determine home directory. HOME and USERPROFILE are both unset.")
}

#[derive(Debug, Clone)]
pub enum PromptSource {
    CliArgs,
    Stdin,
    Default,
    Web,
}

impl PromptSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CliArgs => "cli_args",
            Self::Stdin => "stdin",
            Self::Default => "default",
            Self::Web => "web",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ChatTemplate {
    Generic,
    ChatMl,
    Llama3Instruct,
}

impl ChatTemplate {
    pub fn from_str(value: &str) -> Result<Self> {
        match value {
            "generic" => Ok(Self::Generic),
            "chatml" => Ok(Self::ChatMl),
            "llama3" | "llama3-instruct" => Ok(Self::Llama3Instruct),
            _ => bail!("Invalid chat template: {value}"),
        }
    }

    pub fn resolve(config_value: Option<&str>, model_path: &str) -> Result<Self> {
        if let Some(value) = config_value {
            if value == "auto" {
                return Ok(Self::detect(model_path));
            }

            return Self::from_str(value);
        }

        Ok(Self::detect(model_path))
    }

    fn detect(model_path: &str) -> Self {
        let model_path = model_path.to_ascii_lowercase();

        if model_path.contains("qwen") || model_path.contains("chatml") {
            Self::ChatMl
        } else if model_path.contains("llama-3")
            || model_path.contains("llama3")
            || model_path.contains("meta-llama-3")
        {
            Self::Llama3Instruct
        } else {
            Self::Generic
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
    chat_template: Option<String>,
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
                chat_template: None,
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
    pub chat_template: ChatTemplate,
    pub prompt: SensitiveBytes,
    pub prompt_source: PromptSource,
    pub max_tokens: String,
    pub gpu_layers: String,
    pub ephemeral: bool,
    pub security_mode: SecurityMode,
}

impl AppCommand {
    pub fn from_env() -> Result<Self> {
        let home = home_dir()?;
        let mut args: Vec<String> = env::args().skip(1).collect();

        if args.first().map(|arg| arg.as_str()) == Some("serve") {
            args.zeroize();
            return Ok(Self::Serve);
        }

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
    pub fn from_web_request(
        home: String,
        prompt: String,
        mode: Option<String>,
        persistent: bool,
    ) -> Result<Self> {
        let file_config = FileConfig::load(&home)?;

        let security_mode = SecurityMode::from_str(
            mode.as_deref()
                .or(file_config.default_mode.as_deref())
                .unwrap_or("secure"),
        )?;

        let ephemeral = match security_mode {
            SecurityMode::Standard => !persistent,
            SecurityMode::Secure => true,
            SecurityMode::AirGapped => true,
        };

        let llama_path = file_config
            .llama_path
            .unwrap_or_else(|| format!("{home}/dev/llama.cpp/build/bin/llama-server"));
        let model_path = file_config
            .model_path
            .unwrap_or_else(|| format!("{home}/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"));
        let chat_template =
            ChatTemplate::resolve(file_config.chat_template.as_deref(), &model_path)?;

        Ok(Self {
            home: home.clone(),
            llama_path,
            model_path,
            chat_template,
            prompt: SensitiveBytes::new(prompt),
            prompt_source: PromptSource::Web,
            max_tokens: file_config.max_tokens.unwrap_or(128).to_string(),
            gpu_layers: file_config.gpu_layers.unwrap_or(0).to_string(),
            ephemeral,
            security_mode,
        })
    }

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

        let llama_path = file_config
            .llama_path
            .unwrap_or_else(|| format!("{home}/dev/llama.cpp/build/bin/llama-server"));
        let model_path = file_config
            .model_path
            .unwrap_or_else(|| format!("{home}/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"));
        let chat_template =
            ChatTemplate::resolve(file_config.chat_template.as_deref(), &model_path)?;

        Ok(Self {
            home: home.clone(),
            llama_path,
            model_path,
            chat_template,
            prompt: prompt_bytes,
            prompt_source,
            max_tokens: file_config.max_tokens.unwrap_or(128).to_string(),
            gpu_layers: file_config.gpu_layers.unwrap_or(0).to_string(),
            ephemeral,
            security_mode,
        })
    }
}

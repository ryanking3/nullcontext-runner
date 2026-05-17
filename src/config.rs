use crate::sensitive::SensitiveBytes;
use anyhow::{bail, Context, Result};
use std::env;
use std::io::{self, Read};
use zeroize::Zeroize;

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

#[derive(Debug)]
pub struct SessionConfig {
    pub llama_path: String,
    pub model_path: String,
    pub prompt: SensitiveBytes,
    pub prompt_source: PromptSource,
    pub max_tokens: String,
    pub gpu_layers: String,
    pub ephemeral: bool,
    pub security_mode: SecurityMode,
}

impl SessionConfig {
    pub fn from_env() -> Result<Self> {
        let home = env::var("HOME")?;
        let mut args: Vec<String> = env::args().skip(1).collect();

        let persistent = args.contains(&"--persistent".to_string());
        let use_stdin = args.contains(&"--stdin".to_string());

        let mut security_mode = SecurityMode::Secure;
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

        let (mut prompt, prompt_source) = if use_stdin {
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

        let prompt_bytes = SensitiveBytes::new(prompt.clone());

        prompt.zeroize();
        filtered_args.zeroize();
        args.zeroize();

        let ephemeral = match security_mode {
            SecurityMode::Standard => !persistent,
            SecurityMode::Secure => true,
            SecurityMode::AirGapped => true,
        };

        Ok(Self {
            llama_path: format!("{}/dev/llama.cpp/build/bin/llama-server", home),
            model_path: format!("{}/models/qwen2.5-0.5b-instruct-q4_k_m.gguf", home),
            prompt: prompt_bytes,
            prompt_source,
            max_tokens: "256".to_string(),
            gpu_layers: "0".to_string(),
            ephemeral,
            security_mode,
        })
    }
}

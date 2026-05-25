use crate::sensitive::SensitiveBytes;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
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

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::ChatMl => "chatml",
            Self::Llama3Instruct => "llama3-instruct",
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
struct FileModelConfig {
    id: String,
    name: Option<String>,
    description: Option<String>,
    model_path: String,
    max_tokens: Option<u32>,
    gpu_layers: Option<u32>,
    chat_template: Option<String>,
    chat_context_token_budget: Option<u32>,
    chat_context_turn_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    llama_path: Option<String>,
    model_path: Option<String>,
    default_model: Option<String>,
    default_mode: Option<String>,
    max_tokens: Option<u32>,
    gpu_layers: Option<u32>,
    chat_template: Option<String>,
    chat_context_token_budget: Option<u32>,
    chat_context_turn_limit: Option<usize>,
    models: Option<Vec<FileModelConfig>>,
}

impl FileConfig {
    fn load(home: &str) -> Result<Self> {
        let config_path = PathBuf::from(format!("{home}/.nullcontext/config.toml"));

        if !config_path.exists() {
            return Ok(Self {
                llama_path: None,
                model_path: None,
                default_model: None,
                default_mode: None,
                max_tokens: None,
                gpu_layers: None,
                chat_template: None,
                chat_context_token_budget: None,
                chat_context_turn_limit: None,
                models: None,
            });
        }

        let raw = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file at {}", config_path.display()))?;

        let parsed: Self = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse config file at {}", config_path.display()))?;

        Ok(parsed)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RegisteredModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub model_path: String,
    pub max_tokens: u32,
    pub gpu_layers: u32,
    pub chat_template: String,
    pub chat_context_token_budget: usize,
    pub chat_context_turn_limit: usize,
    pub default_selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelRegistrySnapshot {
    pub default_model_id: String,
    pub models: Vec<RegisteredModel>,
}

#[derive(Debug)]
pub struct SessionConfig {
    pub home: String,
    pub llama_path: String,
    pub model_id: String,
    pub model_name: String,
    pub model_path: String,
    pub chat_template: ChatTemplate,
    pub chat_context_token_budget: usize,
    pub chat_context_turn_limit: usize,
    pub prompt: SensitiveBytes,
    pub prompt_source: PromptSource,
    pub max_tokens: String,
    pub gpu_layers: String,
    pub ephemeral: bool,
    pub security_mode: SecurityMode,
}

pub fn load_model_registry(home: &str) -> Result<ModelRegistrySnapshot> {
    let file_config = FileConfig::load(home)?;
    build_model_registry(home, &file_config)
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
        model_id_override: Option<String>,
        chat_template_override: Option<String>,
        chat_context_token_budget_override: Option<u32>,
        chat_context_turn_limit_override: Option<usize>,
    ) -> Result<Self> {
        let file_config = FileConfig::load(&home)?;
        let model_registry = build_model_registry(&home, &file_config)?;
        let selected_model = resolve_selected_model(&model_registry, model_id_override.as_deref())?;

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
            .unwrap_or_else(|| default_llama_path(&home));
        let model_path = selected_model.model_path.clone();
        let chat_template = ChatTemplate::resolve(
            chat_template_override
                .as_deref()
                .or(Some(selected_model.chat_template.as_str())),
            &model_path,
        )?;
        let chat_context_token_budget = resolve_positive_u32(
            chat_context_token_budget_override,
            Some(selected_model.chat_context_token_budget as u32),
            selected_model.chat_context_token_budget as u32,
            "chat_context_token_budget",
        )? as usize;
        let chat_context_turn_limit = resolve_positive_usize(
            chat_context_turn_limit_override,
            Some(selected_model.chat_context_turn_limit),
            selected_model.chat_context_turn_limit,
            "chat_context_turn_limit",
        )?;

        Ok(Self {
            home: home.clone(),
            llama_path,
            model_id: selected_model.id.clone(),
            model_name: selected_model.name.clone(),
            model_path,
            chat_template,
            chat_context_token_budget,
            chat_context_turn_limit,
            prompt: SensitiveBytes::new(prompt),
            prompt_source: PromptSource::Web,
            max_tokens: selected_model.max_tokens.to_string(),
            gpu_layers: selected_model.gpu_layers.to_string(),
            ephemeral,
            security_mode,
        })
    }

    fn from_args(home: String, mut args: Vec<String>) -> Result<Self> {
        let file_config = FileConfig::load(&home)?;
        let model_registry = build_model_registry(&home, &file_config)?;

        let persistent = args.contains(&"--persistent".to_string());
        let use_stdin = args.contains(&"--stdin".to_string());
        let mut model_id_override: Option<String> = None;

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
                "--model" => {
                    if i + 1 >= args.len() {
                        args.zeroize();
                        filtered_args.zeroize();
                        bail!("--model requires a model id");
                    }

                    model_id_override = Some(args[i + 1].clone());
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
        let selected_model = resolve_selected_model(&model_registry, model_id_override.as_deref())?;

        let llama_path = file_config
            .llama_path
            .unwrap_or_else(|| default_llama_path(&home));
        let model_path = selected_model.model_path.clone();
        let chat_template =
            ChatTemplate::resolve(Some(selected_model.chat_template.as_str()), &model_path)?;
        let chat_context_token_budget = resolve_positive_u32(
            None,
            Some(selected_model.chat_context_token_budget as u32),
            selected_model.chat_context_token_budget as u32,
            "chat_context_token_budget",
        )? as usize;
        let chat_context_turn_limit = resolve_positive_usize(
            None,
            Some(selected_model.chat_context_turn_limit),
            selected_model.chat_context_turn_limit,
            "chat_context_turn_limit",
        )?;

        Ok(Self {
            home: home.clone(),
            llama_path,
            model_id: selected_model.id.clone(),
            model_name: selected_model.name.clone(),
            model_path,
            chat_template,
            chat_context_token_budget,
            chat_context_turn_limit,
            prompt: prompt_bytes,
            prompt_source,
            max_tokens: selected_model.max_tokens.to_string(),
            gpu_layers: selected_model.gpu_layers.to_string(),
            ephemeral,
            security_mode,
        })
    }
}

fn build_model_registry(home: &str, file_config: &FileConfig) -> Result<ModelRegistrySnapshot> {
    let configured_models = file_config.models.as_deref().unwrap_or(&[]);
    let mut models = if configured_models.is_empty() {
        vec![build_legacy_registered_model(home, file_config)?]
    } else {
        let mut seen_ids = HashSet::new();
        let mut built = Vec::with_capacity(configured_models.len());

        for file_model in configured_models {
            if !seen_ids.insert(file_model.id.clone()) {
                bail!("Duplicate model id in config.toml: {}", file_model.id);
            }

            built.push(build_registered_model(file_model, file_config)?);
        }

        built
    };

    let default_model_id = file_config
        .default_model
        .clone()
        .unwrap_or_else(|| models[0].id.clone());

    if !models.iter().any(|model| model.id == default_model_id) {
        bail!("Configured default_model does not match any known model id: {default_model_id}");
    }

    for model in &mut models {
        model.default_selected = model.id == default_model_id;
    }

    Ok(ModelRegistrySnapshot {
        default_model_id,
        models,
    })
}

fn build_registered_model(
    file_model: &FileModelConfig,
    defaults: &FileConfig,
) -> Result<RegisteredModel> {
    let max_tokens = resolve_positive_u32(
        file_model.max_tokens,
        defaults.max_tokens,
        128,
        &format!("models.{}.max_tokens", file_model.id),
    )?;
    let chat_context_token_budget = resolve_positive_u32(
        file_model.chat_context_token_budget,
        defaults.chat_context_token_budget,
        2048,
        &format!("models.{}.chat_context_token_budget", file_model.id),
    )? as usize;
    let chat_context_turn_limit = resolve_positive_usize(
        file_model.chat_context_turn_limit,
        defaults.chat_context_turn_limit,
        12,
        &format!("models.{}.chat_context_turn_limit", file_model.id),
    )?;
    let chat_template = file_model
        .chat_template
        .clone()
        .or_else(|| defaults.chat_template.clone())
        .unwrap_or_else(|| "auto".to_string());

    validate_chat_template_setting(&chat_template, &file_model.model_path)?;

    Ok(RegisteredModel {
        id: file_model.id.clone(),
        name: file_model
            .name
            .clone()
            .unwrap_or_else(|| default_model_name(&file_model.id, &file_model.model_path)),
        description: file_model.description.clone(),
        model_path: file_model.model_path.clone(),
        max_tokens,
        gpu_layers: file_model.gpu_layers.or(defaults.gpu_layers).unwrap_or(0),
        chat_template,
        chat_context_token_budget,
        chat_context_turn_limit,
        default_selected: false,
    })
}

fn build_legacy_registered_model(home: &str, file_config: &FileConfig) -> Result<RegisteredModel> {
    let model_path = file_config
        .model_path
        .clone()
        .unwrap_or_else(|| default_model_path(home));
    let max_tokens = resolve_positive_u32(None, file_config.max_tokens, 128, "max_tokens")?;
    let chat_context_token_budget = resolve_positive_u32(
        None,
        file_config.chat_context_token_budget,
        2048,
        "chat_context_token_budget",
    )? as usize;
    let chat_context_turn_limit = resolve_positive_usize(
        None,
        file_config.chat_context_turn_limit,
        12,
        "chat_context_turn_limit",
    )?;
    let chat_template = file_config
        .chat_template
        .clone()
        .unwrap_or_else(|| "auto".to_string());

    validate_chat_template_setting(&chat_template, &model_path)?;

    let default_id = file_config
        .default_model
        .clone()
        .unwrap_or_else(|| "default".to_string());

    Ok(RegisteredModel {
        id: default_id.clone(),
        name: default_model_name(&default_id, &model_path),
        description: Some(
            "Legacy single-model config synthesized into the model registry.".to_string(),
        ),
        model_path,
        max_tokens,
        gpu_layers: file_config.gpu_layers.unwrap_or(0),
        chat_template,
        chat_context_token_budget,
        chat_context_turn_limit,
        default_selected: true,
    })
}

fn resolve_selected_model<'a>(
    model_registry: &'a ModelRegistrySnapshot,
    model_id_override: Option<&str>,
) -> Result<&'a RegisteredModel> {
    let model_id = model_id_override.unwrap_or(model_registry.default_model_id.as_str());

    model_registry
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown model id: {model_id}"))
}

fn validate_chat_template_setting(value: &str, model_path: &str) -> Result<()> {
    ChatTemplate::resolve(Some(value), model_path).map(|_| ())
}

fn default_llama_path(home: &str) -> String {
    format!("{home}/dev/llama.cpp/build/bin/llama-server")
}

fn default_model_path(home: &str) -> String {
    format!("{home}/models/qwen2.5-0.5b-instruct-q4_k_m.gguf")
}

fn default_model_name(id: &str, model_path: &str) -> String {
    Path::new(model_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('_', " "))
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| id.to_string())
}

fn resolve_positive_u32(
    override_value: Option<u32>,
    config_value: Option<u32>,
    default_value: u32,
    field_name: &str,
) -> Result<u32> {
    let value = override_value.or(config_value).unwrap_or(default_value);

    if value == 0 {
        bail!("{field_name} must be greater than 0");
    }

    Ok(value)
}

fn resolve_positive_usize(
    override_value: Option<usize>,
    config_value: Option<usize>,
    default_value: usize,
    field_name: &str,
) -> Result<usize> {
    let value = override_value.or(config_value).unwrap_or(default_value);

    if value == 0 {
        bail!("{field_name} must be greater than 0");
    }

    Ok(value)
}

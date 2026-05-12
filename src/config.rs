use anyhow::Result;
use std::env;

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub llama_path: String,
    pub model_path: String,
    pub prompt: String,
    pub max_tokens: String,
    pub gpu_layers: String,
    pub ephemeral: bool,
}

impl SessionConfig {
    pub fn from_env() -> Result<Self> {
        let home = env::var("HOME")?;

        let args: Vec<String> = env::args().skip(1).collect();

        let persistent = args.contains(&"--persistent".to_string());

        let filtered_args: Vec<String> = args.into_iter().filter(|a| a != "--persistent").collect();

        let prompt = filtered_args.join(" ");

        Ok(Self {
            llama_path: format!("{}/dev/llama.cpp/build/bin/llama-server", home),
            model_path: format!("{}/models/qwen2.5-0.5b-instruct-q4_k_m.gguf", home),
            prompt: if prompt.trim().is_empty() {
                "Hello from NullContext".to_string()
            } else {
                prompt
            },
            max_tokens: "256".to_string(),
            gpu_layers: "0".to_string(),
            ephemeral: !persistent,
        })
    }
}

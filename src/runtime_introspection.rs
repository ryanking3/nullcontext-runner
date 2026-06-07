use serde::Deserialize;

const INTROSPECTION_PREFIX: &str = "NULLCONTEXT_INTROSPECTION:";

#[derive(Debug, Clone)]
pub struct RuntimeIntrospectionSignal {
    pub event: String,
    pub status: String,
    pub source_stream: String,
    pub details: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeIntrospectionSignalLine {
    event: String,
    status: Option<String>,
    details: Option<String>,
}

pub fn parse_runtime_introspection_signals(
    stdout: &str,
    stderr: &str,
) -> Vec<RuntimeIntrospectionSignal> {
    let mut signals = Vec::new();
    signals.extend(parse_stream("stdout", stdout));
    signals.extend(parse_stream("stderr", stderr));
    signals
}

fn parse_stream(source_stream: &str, content: &str) -> Vec<RuntimeIntrospectionSignal> {
    let mut signals = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let Some(raw_payload) = trimmed.strip_prefix(INTROSPECTION_PREFIX) else {
            continue;
        };

        let payload = raw_payload.trim();

        match serde_json::from_str::<RuntimeIntrospectionSignalLine>(payload) {
            Ok(parsed) => signals.push(RuntimeIntrospectionSignal {
                event: parsed.event,
                status: parsed.status.unwrap_or_else(|| "observed".to_string()),
                source_stream: source_stream.to_string(),
                details: parsed.details.unwrap_or_default(),
            }),
            Err(error) => signals.push(RuntimeIntrospectionSignal {
                event: "introspection_signal_parse_failed".to_string(),
                status: "failed".to_string(),
                source_stream: source_stream.to_string(),
                details: format!(
                    "Failed to parse runtime introspection signal payload '{}': {}",
                    payload, error
                ),
            }),
        }
    }

    signals
}

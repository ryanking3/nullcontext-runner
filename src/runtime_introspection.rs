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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_signals_from_both_runtime_streams() {
        let stdout = r#"
            ordinary runtime output
            NULLCONTEXT_INTROSPECTION: {"event":"allocator_reset_observed","status":"observed","details":"after shutdown"}
        "#;
        let stderr = r#"
            NULLCONTEXT_INTROSPECTION: {"event":"kv_cache_clear_observed"}
        "#;

        let signals = parse_runtime_introspection_signals(stdout, stderr);

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].event, "allocator_reset_observed");
        assert_eq!(signals[0].status, "observed");
        assert_eq!(signals[0].source_stream, "stdout");
        assert_eq!(signals[0].details, "after shutdown");
        assert_eq!(signals[1].event, "kv_cache_clear_observed");
        assert_eq!(signals[1].status, "observed");
        assert_eq!(signals[1].source_stream, "stderr");
        assert!(signals[1].details.is_empty());
    }

    #[test]
    fn records_malformed_introspection_payload_without_dropping_later_signals() {
        let stdout = r#"
            NULLCONTEXT_INTROSPECTION: not-json
            NULLCONTEXT_INTROSPECTION: {"event":"model_unload_observed","status":"observed"}
        "#;

        let signals = parse_runtime_introspection_signals(stdout, "");

        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].event, "introspection_signal_parse_failed");
        assert_eq!(signals[0].status, "failed");
        assert_eq!(signals[0].source_stream, "stdout");
        assert!(signals[0].details.contains("not-json"));
        assert_eq!(signals[1].event, "model_unload_observed");
        assert_eq!(signals[1].status, "observed");
    }
}

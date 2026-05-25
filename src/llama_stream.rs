use anyhow::Result;
use reqwest::blocking::Client;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamTermination {
    Completed,
    CancelRequested,
    StreamClosed,
}

#[derive(Serialize)]
struct StreamingCompletionRequest {
    prompt: String,
    n_predict: u32,
    stream: bool,
}

impl StreamingCompletionRequest {
    fn sanitize(&mut self) {
        self.prompt.zeroize();
    }
}

pub fn stream_completion_from_llama<C, E>(
    completion_url: &str,
    prompt: &str,
    n_predict: u32,
    is_cancel_requested: C,
    mut emit_text: E,
) -> Result<(String, StreamTermination)>
where
    C: Fn() -> bool,
    E: FnMut(&str) -> bool,
{
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    let mut request = StreamingCompletionRequest {
        prompt: prompt.to_string(),
        n_predict,
        stream: true,
    };

    let response = client.post(completion_url).json(&request).send()?;

    request.sanitize();

    let reader = BufReader::new(response);
    let mut full_response = String::new();

    for line_result in reader.lines() {
        if is_cancel_requested() {
            return Ok((full_response, StreamTermination::CancelRequested));
        }

        let line = line_result?;

        if !line.starts_with("data:") {
            continue;
        }

        let data = line.trim_start_matches("data:").trim();

        if data.is_empty() || data == "[DONE]" {
            continue;
        }

        let parsed: serde_json::Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(content) = parsed.get("content").and_then(|value| value.as_str()) {
            if !content.is_empty() {
                if is_cancel_requested() {
                    return Ok((full_response, StreamTermination::CancelRequested));
                }

                full_response.push_str(content);

                if !emit_text(content) {
                    if is_cancel_requested() {
                        return Ok((full_response, StreamTermination::CancelRequested));
                    }

                    return Ok((full_response, StreamTermination::StreamClosed));
                }
            }
        }

        let stopped = parsed
            .get("stop")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        if stopped {
            break;
        }
    }

    if is_cancel_requested() {
        return Ok((full_response, StreamTermination::CancelRequested));
    }

    Ok((full_response, StreamTermination::Completed))
}

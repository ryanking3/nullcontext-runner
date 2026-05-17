use crate::cleanup::SanitizationOperation;

pub fn buffer_contains_pattern(buffer: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() || pattern.len() > buffer.len() {
        return false;
    }

    buffer
        .windows(pattern.len())
        .any(|window| window == pattern)
}

pub fn verify_buffer_zeroization(
    label: &str,
    before_sanitize: bool,
    after_sanitize: bool,
) -> SanitizationOperation {
    let status = if before_sanitize && !after_sanitize {
        "successful"
    } else if !before_sanitize {
        "warning"
    } else {
        "failed"
    };

    let details = if before_sanitize && !after_sanitize {
        format!("{label} contained sensitive bytes before sanitization and did not contain them after sanitization.")
    } else if !before_sanitize {
        format!("{label} did not contain the expected sensitive bytes before sanitization; verification may be inconclusive.")
    } else {
        format!("{label} still contained sensitive bytes after sanitization.")
    };

    SanitizationOperation {
        operation: format!("{label}_ram_zeroization_verification"),
        status: status.to_string(),
        details,
    }
}

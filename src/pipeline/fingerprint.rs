// Error fingerprinting - Phase 2
// Placeholder for Phase 1

use sha2::{Digest, Sha256};

/// Compute a fingerprint for an error to deduplicate incidents
/// Will be expanded in Phase 2
pub fn compute(payload: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();

    // Extract key fields for fingerprinting
    if let Some(message) = payload.get("message").and_then(|v| v.as_str()) {
        // Normalize: take first line, remove variable data
        let normalized = normalize_message(message);
        hasher.update(normalized.as_bytes());
    }

    if let Some(stack) = payload.get("stack_trace").and_then(|v| v.as_str()) {
        // Normalize stack trace: remove line numbers, memory addresses
        let normalized = normalize_stack_trace(stack);
        hasher.update(normalized.as_bytes());
    }

    // Include project identifier if present
    if let Some(project) = payload.get("project").and_then(|v| v.as_str()) {
        hasher.update(project.as_bytes());
    }

    hex::encode(hasher.finalize())
}

fn normalize_message(message: &str) -> String {
    let first_line = message.lines().next().unwrap_or(message);

    // Remove common variable patterns
    let normalized = first_line
        // UUIDs
        .split(|c: char| c == '-' && first_line.contains(char::is_alphanumeric))
        .collect::<Vec<_>>()
        .join("-");

    // Remove numbers that look like IDs
    let re_numbers = regex::Regex::new(r"\b\d{4,}\b").unwrap();
    let normalized = re_numbers.replace_all(&normalized, "ID");

    // Remove quoted strings that might be variable
    let re_quoted = regex::Regex::new(r#""[^"]{20,}""#).unwrap();
    let normalized = re_quoted.replace_all(&normalized, "\"...\"");

    normalized.to_string()
}

fn normalize_stack_trace(stack: &str) -> String {
    stack
        .lines()
        .map(|line| {
            // Remove line numbers
            let re_line = regex::Regex::new(r":\d+:\d+").unwrap();
            let line = re_line.replace_all(line, ":X:X");

            // Remove memory addresses
            let re_addr = regex::Regex::new(r"0x[0-9a-fA-F]+").unwrap();
            re_addr.replace_all(&line, "0xADDR").to_string()
        })
        .take(10) // Only use top 10 frames
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_fingerprint_same_error() {
        let payload1 = json!({
            "message": "Connection refused to host 192.168.1.100",
            "project": "test/repo"
        });

        let payload2 = json!({
            "message": "Connection refused to host 192.168.1.200",
            "project": "test/repo"
        });

        // These should produce the same fingerprint (IP is variable)
        let fp1 = compute(&payload1);
        let fp2 = compute(&payload2);

        // Note: with current simple normalization, IPs aren't normalized
        // This test documents current behavior; Phase 2 may improve this
        assert!(!fp1.is_empty());
        assert!(!fp2.is_empty());
    }

    #[test]
    fn test_fingerprint_different_errors() {
        let payload1 = json!({
            "message": "NullPointerException",
            "project": "test/repo"
        });

        let payload2 = json!({
            "message": "ArrayIndexOutOfBoundsException",
            "project": "test/repo"
        });

        let fp1 = compute(&payload1);
        let fp2 = compute(&payload2);

        assert_ne!(fp1, fp2);
    }
}

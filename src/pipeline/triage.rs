//! Triage Pipeline Stage
//!
//! Uses the reader LLM to determine if an error is fixable and gather initial context.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::integrations::llm::TrackedLlm;

/// Result of triage analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageResult {
    /// Whether the error appears to be fixable automatically
    pub fixable: bool,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f32,
    /// Reason for the fixability determination
    pub reason: String,
    /// Error category (null_reference, type_error, bounds_error, etc.)
    pub category: String,
    /// Suggested files to examine
    pub suggested_files: Vec<String>,
    /// Brief description of the error for humans
    pub description: String,
    /// Suggested fix approach (if fixable)
    pub fix_approach: Option<String>,
}

const TRIAGE_SYSTEM_PROMPT: &str = r#"You are an expert software engineer analyzing production errors to determine if they can be automatically fixed.

Your job is to:
1. Understand what went wrong
2. Categorize the error type
3. Determine if it's likely fixable with a code change
4. Identify which files likely need to be examined

Errors that are typically FIXABLE:
- Null/undefined reference errors (add null checks)
- Type mismatches (fix type annotations or conversions)
- Off-by-one errors (fix loop bounds or array indices)
- Missing error handling (add try/catch)
- Import/export errors (fix import paths)
- Simple logic errors (wrong operator, missing condition)

Errors that are typically NOT FIXABLE automatically:
- Configuration issues (wrong environment variables)
- External service failures (database down, API unreachable)
- Data corruption issues
- Race conditions or complex concurrency bugs
- Performance issues (need profiling)
- Security vulnerabilities (need careful review)

Respond in JSON format only."#;

/// Run triage on an error sample
pub async fn run(
    llm: &TrackedLlm,
    sample: &serde_json::Value,
    incident_id: Uuid,
    project_id: Uuid,
) -> Result<TriageResult> {
    run_with_context(llm, sample, incident_id, project_id, "").await
}

/// Run triage on an error sample with CRISPR.md context
pub async fn run_with_context(
    llm: &TrackedLlm,
    sample: &serde_json::Value,
    incident_id: Uuid,
    project_id: Uuid,
    crispr_context: &str,
) -> Result<TriageResult> {
    let context_section = if crispr_context.is_empty() {
        String::new()
    } else {
        format!(
            r#"

{}

Use the above context to inform your analysis. Pay attention to:
- Any special notes from developers about this codebase
- Patterns from previous fixes that might apply
- The service's purpose to understand the error's impact

"#,
            crispr_context
        )
    };

    let prompt = format!(
        r#"Analyze this production error and determine if it can be automatically fixed.
{context_section}
Error details:
```json
{}
```

Respond with a JSON object containing:
- "fixable": boolean - can this be fixed with a code change?
- "confidence": number 0-1 - how confident are you?
- "reason": string - why is it fixable/not fixable?
- "category": string - error type (null_reference, type_error, bounds_error, missing_import, logic_error, external_service, configuration, other)
- "suggested_files": array of strings - file paths that likely need examination (extract from stack trace)
- "description": string - brief human-readable description
- "fix_approach": string or null - if fixable, how would you approach the fix?

JSON response:"#,
        serde_json::to_string_pretty(sample)?
    );

    let response = llm
        .complete(
            &prompt,
            Some(TRIAGE_SYSTEM_PROMPT),
            "triage",
            Some(incident_id),
            Some(project_id),
        )
        .await?;

    // Parse the JSON response
    let result: TriageResult = parse_json_response(&response.content)?;

    Ok(result)
}

/// Parse JSON from LLM response, handling markdown code blocks
fn parse_json_response(content: &str) -> Result<TriageResult> {
    // Try to extract JSON from markdown code blocks
    let json_str = if content.contains("```json") {
        content
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(content)
            .trim()
    } else if content.contains("```") {
        content
            .split("```")
            .nth(1)
            .unwrap_or(content)
            .trim()
    } else {
        content.trim()
    };

    // Try to parse
    match serde_json::from_str(json_str) {
        Ok(result) => Ok(result),
        Err(e) => {
            // Try to find JSON object in the response
            if let Some(start) = json_str.find('{') {
                if let Some(end) = json_str.rfind('}') {
                    let json_obj = &json_str[start..=end];
                    return Ok(serde_json::from_str(json_obj)?);
                }
            }
            Err(anyhow::anyhow!("Failed to parse triage response: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_response() {
        let response = r#"```json
{
    "fixable": true,
    "confidence": 0.9,
    "reason": "Null reference can be fixed with a null check",
    "category": "null_reference",
    "suggested_files": ["src/user.ts"],
    "description": "Null reference when accessing user property",
    "fix_approach": "Add null check before accessing property"
}
```"#;

        let result = parse_json_response(response).unwrap();
        assert!(result.fixable);
        assert_eq!(result.category, "null_reference");
    }
}

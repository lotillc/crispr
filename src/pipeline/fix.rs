//! Fix Generation Pipeline Stage
//!
//! Uses the writer LLM to generate code fixes based on context.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::integrations::llm::TrackedLlm;

use super::context::FixContext;

/// Generated fix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFix {
    /// Files to modify
    pub changes: Vec<FileChange>,
    /// Explanation of the fix
    pub explanation: String,
    /// Confidence in the fix (0.0 to 1.0)
    pub confidence: f32,
    /// Potential risks or caveats
    pub risks: Vec<String>,
    /// Suggested tests to run
    pub suggested_tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path
    pub path: String,
    /// New content for the file (full file)
    pub content: String,
    /// Brief description of what changed
    pub description: String,
    /// Original SHA (for update verification)
    pub original_sha: Option<String>,
}

const FIX_SYSTEM_PROMPT: &str = r#"You are an expert software engineer fixing production bugs. Your fixes must be:

1. MINIMAL - Only change what's necessary to fix the bug
2. SAFE - Don't introduce new bugs or break existing functionality
3. IDIOMATIC - Follow the existing code style and patterns
4. COMPLETE - Include all necessary changes (imports, types, etc.)

When generating fixes:
- Prefer defensive programming (null checks, error handling)
- Don't change unrelated code
- Keep the same formatting and style as the original
- If unsure, add appropriate logging or error handling

Respond with JSON containing the complete fixed file contents."#;

/// Generate a fix for the error
pub async fn generate(
    llm: &TrackedLlm,
    context: &FixContext,
    incident_id: Uuid,
    project_id: Uuid,
) -> Result<GeneratedFix> {
    // Build prompt with context
    let files_context: String = context
        .files
        .iter()
        .map(|f| format!("### File: {}\n```\n{}\n```", f.path, f.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        r#"Fix this production error:

## Error Information
- Category: {}
- Description: {}
- Suggested approach: {}

## Error Sample
```json
{}
```

## Context Analysis
{}

## Relevant Files
{}

## Instructions
Generate a fix for this error. Return a JSON object with:
- "changes": array of file changes, each with:
  - "path": file path
  - "content": complete new file content (not a diff)
  - "description": what you changed and why
- "explanation": overall explanation of the fix
- "confidence": 0-1 confidence score
- "risks": array of potential risks or things to watch
- "suggested_tests": array of test commands or scenarios to verify the fix

IMPORTANT: Return the COMPLETE file content, not a diff. Include ALL lines of the file.

JSON response:"#,
        context.triage.category,
        context.triage.description,
        context.triage.fix_approach.as_deref().unwrap_or("Not specified"),
        serde_json::to_string_pretty(&context.sample)?,
        context.analysis.as_deref().unwrap_or("No additional analysis"),
        files_context,
    );

    let response = llm
        .complete(
            &prompt,
            Some(FIX_SYSTEM_PROMPT),
            "fix",
            Some(incident_id),
            Some(project_id),
        )
        .await?;

    // Parse the fix
    let mut fix: GeneratedFix = parse_json_response(&response.content)?;

    // Add original SHAs to changes
    for change in &mut fix.changes {
        if let Some(orig_file) = context.files.iter().find(|f| f.path == change.path) {
            change.original_sha = Some(orig_file.sha.clone());
        }
    }

    Ok(fix)
}

/// Parse JSON from LLM response
fn parse_json_response(content: &str) -> Result<GeneratedFix> {
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
            Err(anyhow::anyhow!("Failed to parse fix response: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fix_response() {
        let response = r#"```json
{
    "changes": [
        {
            "path": "src/user.ts",
            "content": "const user = getUser();\nif (user) {\n  console.log(user.name);\n}",
            "description": "Added null check for user"
        }
    ],
    "explanation": "Added null check to prevent TypeError",
    "confidence": 0.9,
    "risks": ["May need to handle the null case differently"],
    "suggested_tests": ["npm test"]
}
```"#;

        let result = parse_json_response(response).unwrap();
        assert_eq!(result.changes.len(), 1);
        assert!(result.confidence > 0.8);
    }
}

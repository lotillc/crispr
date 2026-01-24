//! Fix Generation Pipeline Stage
//!
//! Uses the writer LLM to generate code fixes based on context.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::integrations::llm::TrackedLlm;

use super::context::FixContext;

/// Generated fix that may span multiple files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFix {
    /// Files to modify (one or more)
    pub changes: Vec<FileChange>,
    /// Explanation of the fix
    pub explanation: String,
    /// Confidence in the fix (0.0 to 1.0)
    pub confidence: f32,
    /// Potential risks or caveats
    pub risks: Vec<String>,
    /// Suggested tests to run
    pub suggested_tests: Vec<String>,
    /// Generated unit tests that verify the fix
    #[serde(default)]
    pub generated_tests: Vec<GeneratedTest>,
}

impl GeneratedFix {
    /// Returns true if this fix modifies multiple files
    pub fn is_multi_file(&self) -> bool {
        self.changes.len() > 1
    }

    /// Get a list of all file paths being modified
    pub fn affected_files(&self) -> Vec<&str> {
        self.changes.iter().map(|c| c.path.as_str()).collect()
    }

    /// Get the number of files being modified
    pub fn file_count(&self) -> usize {
        self.changes.len()
    }

    /// Get a summary of the fix for logging
    pub fn summary(&self) -> String {
        let file_count = self.changes.len();
        let files_desc = if file_count == 1 {
            format!("1 file ({})", self.changes[0].path)
        } else {
            let file_list: Vec<&str> = self.changes.iter().take(3).map(|c| c.path.as_str()).collect();
            let suffix = if file_count > 3 {
                format!(" and {} more", file_count - 3)
            } else {
                String::new()
            };
            format!("{} files ({}{})", file_count, file_list.join(", "), suffix)
        };
        format!(
            "Fix affecting {}: {} (confidence: {:.0}%)",
            files_desc,
            self.explanation.chars().take(100).collect::<String>(),
            self.confidence * 100.0
        )
    }

    /// Validate that all file changes reference files from the provided context
    pub fn validate_against_context(&self, context_files: &[&str]) -> std::result::Result<(), Vec<String>> {
        let invalid_paths: Vec<String> = self
            .changes
            .iter()
            .filter(|c| !context_files.contains(&c.path.as_str()))
            .map(|c| c.path.clone())
            .collect();

        if invalid_paths.is_empty() {
            Ok(())
        } else {
            Err(invalid_paths)
        }
    }

    /// Check if any changes would be empty or no-ops
    pub fn has_empty_changes(&self) -> bool {
        self.changes.iter().any(|c| c.content.trim().is_empty())
    }

    /// Get total number of generated tests
    pub fn test_count(&self) -> usize {
        self.generated_tests.len()
    }

    /// Check if the fix has generated tests
    pub fn has_tests(&self) -> bool {
        !self.generated_tests.is_empty()
    }
}

/// A generated unit test file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTest {
    /// Suggested test file path (e.g., "src/user.test.ts" for "src/user.ts")
    pub path: String,
    /// The test source code
    pub content: String,
    /// Description of what the test verifies
    pub description: String,
    /// The test framework used (e.g., "jest", "pytest", "go test")
    pub framework: String,
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

impl FileChange {
    /// Check if the change appears to be substantive (not just whitespace)
    pub fn has_substantive_change(&self, original_content: &str) -> bool {
        let normalize = |s: &str| {
            s.lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        };
        normalize(&self.content) != normalize(original_content)
    }
}

const FIX_SYSTEM_PROMPT: &str = r#"You are an expert software engineer fixing production bugs. Your fixes must be:

1. MINIMAL - Only change what's necessary to fix the bug
2. SAFE - Don't introduce new bugs or break existing functionality
3. IDIOMATIC - Follow the existing code style and patterns
4. COMPLETE - Include all necessary changes across ALL affected files
5. TESTED - Generate unit tests to verify the fix works correctly

MULTI-FILE FIXES:
Many bugs require changes across multiple files. You MUST identify and fix ALL related files when:
- A type definition needs to change (update the type AND all usages)
- An interface/API changes (update the definition AND all callers)
- A shared function signature changes (update the function AND all call sites)
- Error handling needs to propagate (update the source AND consumers)
- A constant or configuration changes (update the definition AND references)
- A data model changes (update the model, DTOs, API handlers, and client code)

TEST GENERATION:
You MUST generate unit tests for every fix. Tests should:
- Use the appropriate test framework for the language:
  * TypeScript/JavaScript: Jest or Vitest
  * Python: pytest
  * Go: go test (testing package)
  * Rust: built-in test module (#[cfg(test)])
  * Java/Kotlin: JUnit
  * Ruby: RSpec or Minitest
  * PHP: PHPUnit
  * C#: xUnit or NUnit
- Include a test that would have CAUGHT the original bug (regression test)
- Include a test that verifies the fix works correctly
- Follow the naming convention for test files in that language:
  * TypeScript/JavaScript: *.test.ts, *.spec.ts, *.test.js, *.spec.js
  * Python: test_*.py or *_test.py
  * Go: *_test.go
  * Rust: inline #[cfg(test)] module or tests/*.rs
  * Java: *Test.java
  * Ruby: *_spec.rb or *_test.rb
- Mock external dependencies appropriately
- Be self-contained and runnable

When generating fixes:
- Prefer defensive programming (null checks, error handling)
- Don't change unrelated code
- Keep the same formatting and style as the original
- If unsure, add appropriate logging or error handling
- Always check if changes in one file require changes in related files

Respond with JSON containing the complete fixed file contents for ALL files that need changes, plus generated tests."#;

/// Get the recommended test framework for a given language
fn get_test_framework_hint(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "typescript" | "ts" => "Jest or Vitest",
        "javascript" | "js" => "Jest or Vitest",
        "python" | "py" => "pytest",
        "go" | "golang" => "go test (testing package)",
        "rust" | "rs" => "built-in test module (#[cfg(test)])",
        "java" => "JUnit 5",
        "kotlin" | "kt" => "JUnit 5 or Kotest",
        "ruby" | "rb" => "RSpec",
        "php" => "PHPUnit",
        "csharp" | "c#" | "cs" => "xUnit or NUnit",
        "swift" => "XCTest",
        "scala" => "ScalaTest or Specs2",
        _ => "appropriate test framework for the language",
    }
}

/// Get an example test file path for a given language
fn get_test_file_example(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "typescript" | "ts" => "src/user.test.ts",
        "javascript" | "js" => "src/user.test.js",
        "python" | "py" => "tests/test_user.py",
        "go" | "golang" => "user_test.go",
        "rust" | "rs" => "src/user.rs (inline tests) or tests/user_test.rs",
        "java" => "src/test/java/UserTest.java",
        "kotlin" | "kt" => "src/test/kotlin/UserTest.kt",
        "ruby" | "rb" => "spec/user_spec.rb",
        "php" => "tests/UserTest.php",
        "csharp" | "c#" | "cs" => "Tests/UserTests.cs",
        "swift" => "Tests/UserTests.swift",
        "scala" => "src/test/scala/UserSpec.scala",
        _ => "appropriate test file path",
    }
}

/// Derive a test file path from a source file path
pub fn derive_test_file_path(source_path: &str, language: &str) -> String {
    let lang = language.to_lowercase();

    // Get the file name without extension and the directory
    let path = std::path::Path::new(source_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("test");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    match lang.as_str() {
        "typescript" | "ts" => {
            if parent.is_empty() {
                format!("{}.test.ts", stem)
            } else {
                format!("{}/{}.test.ts", parent, stem)
            }
        }
        "javascript" | "js" => {
            if parent.is_empty() {
                format!("{}.test.js", stem)
            } else {
                format!("{}/{}.test.js", parent, stem)
            }
        }
        "python" | "py" => {
            // Python tests typically go in a tests/ directory
            format!("tests/test_{}.py", stem)
        }
        "go" | "golang" => {
            if parent.is_empty() {
                format!("{}_test.go", stem)
            } else {
                format!("{}/{}_test.go", parent, stem)
            }
        }
        "rust" | "rs" => {
            // Rust tests can be inline or in tests/ directory
            format!("tests/{}_test.rs", stem)
        }
        "java" => {
            // Java tests go in src/test/java mirroring the source structure
            let test_path = parent.replace("src/main/java", "src/test/java");
            format!("{}/{}Test.java", test_path, stem)
        }
        "kotlin" | "kt" => {
            let test_path = parent.replace("src/main/kotlin", "src/test/kotlin");
            format!("{}/{}Test.kt", test_path, stem)
        }
        "ruby" | "rb" => {
            format!("spec/{}_spec.rb", stem)
        }
        "php" => {
            format!("tests/{}Test.php", stem)
        }
        "csharp" | "c#" | "cs" => {
            format!("Tests/{}Tests.cs", stem)
        }
        _ => {
            // Default: add .test before extension
            if parent.is_empty() {
                format!("{}.test", stem)
            } else {
                format!("{}/{}.test", parent, stem)
            }
        }
    }
}

/// Feedback context for retry attempts
#[derive(Debug, Clone, Default)]
pub struct RetryContext {
    /// User-provided feedback about why the previous fix wasn't satisfactory
    pub feedback: Option<String>,
    /// Comments from PR reviews
    pub pr_comments: Vec<PrComment>,
    /// Number of previous attempts
    pub attempt_number: i32,
}

/// A comment from a PR review
#[derive(Debug, Clone)]
pub struct PrComment {
    pub author: String,
    pub body: String,
    pub file_path: Option<String>,
    pub line: Option<i32>,
}

/// Generate a fix for the error
pub async fn generate(
    llm: &TrackedLlm,
    context: &FixContext,
    incident_id: Uuid,
    project_id: Uuid,
    retry_context: Option<&RetryContext>,
) -> Result<GeneratedFix> {
    // Build prompt with context
    let files_context: String = context
        .files
        .iter()
        .map(|f| format!("### File: {}\n```\n{}\n```", f.path, f.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    // Build a list of available files for context
    let available_files: String = context
        .files
        .iter()
        .map(|f| format!("  - {}", f.path))
        .collect::<Vec<_>>()
        .join("\n");

    // Detect language for test framework guidance
    let language = context.project.language.as_deref().unwrap_or("unknown");
    let test_framework_hint = get_test_framework_hint(language);

    // Build feedback section if this is a retry
    let feedback_section = if let Some(retry) = retry_context {
        let mut sections = Vec::new();

        if retry.attempt_number > 1 {
            sections.push(format!("## Retry Attempt #{}\nThis is a retry - previous fixes were not satisfactory.", retry.attempt_number));
        }

        if let Some(feedback) = &retry.feedback {
            sections.push(format!("## User Feedback on Previous Fix\n{}", feedback));
        }

        if !retry.pr_comments.is_empty() {
            let comments: String = retry.pr_comments.iter()
                .map(|c| {
                    let location = match (&c.file_path, c.line) {
                        (Some(path), Some(line)) => format!(" ({}:{})", path, line),
                        (Some(path), None) => format!(" ({})", path),
                        _ => String::new(),
                    };
                    format!("- @{}{}: {}", c.author, location, c.body)
                })
                .collect::<Vec<_>>()
                .join("\n");

            sections.push(format!(
                r#"## PR Review Comments
The following comments were left on the previous PR. Consider them as context but use judgment:
- Not every comment is a command - some may just be acknowledgments or questions
- Focus on actionable feedback that improves the fix
- Ignore comments that are just confirmations or unrelated discussion

{}
"#,
                comments
            ));
        }

        if sections.is_empty() {
            String::new()
        } else {
            sections.join("\n\n") + "\n\n"
        }
    } else {
        String::new()
    };

    let prompt = format!(
        r#"Fix this production error:

{}## Error Information
- Category: {}
- Description: {}
- Suggested approach: {}
- Language: {}

## Error Sample
```json
{}
```

## Context Analysis
{}

## Available Files
The following files are available for modification:
{}

## File Contents
{}

## Instructions
Generate a fix for this error AND unit tests to verify the fix. You may modify MULTIPLE FILES if needed.

IMPORTANT: Many bugs span multiple files. Consider whether you need to:
- Update type definitions AND their usages
- Modify shared functions AND their callers
- Change interfaces AND implementations
- Update error handling across the call chain
- Synchronize related files (e.g., model + handler + API)

Return a JSON object with:
- "changes": array of file changes (one or more files), each with:
  - "path": file path (must be one of the available files listed above)
  - "content": complete new file content (not a diff)
  - "description": what you changed and why
- "explanation": overall explanation of the fix (mention if changes span multiple files)
- "confidence": 0-1 confidence score (lower if the fix is partial or uncertain)
- "risks": array of potential risks or things to watch
- "suggested_tests": array of test commands or scenarios to verify the fix
- "generated_tests": array of generated test files, each with:
  - "path": suggested test file path (e.g., "{}")
  - "content": complete test file source code
  - "description": what the test verifies
  - "framework": test framework used (e.g., "{}")

## Test Generation Requirements
You MUST generate at least one test file that:
1. Would have CAUGHT the original bug (a regression test that fails without the fix)
2. Verifies the fix works correctly (passes with the fix applied)
3. Uses the appropriate test framework: {}
4. Follows the file naming convention for tests in this language
5. Mocks external dependencies appropriately
6. Is self-contained and runnable

CRITICAL REQUIREMENTS:
1. Return the COMPLETE file content for each changed file, not a diff
2. Include ALL lines of each file, even unchanged lines
3. If the bug requires changes in multiple files, include ALL affected files
4. Only include files that actually need changes
5. Maintain consistent changes across files (e.g., if renaming a function, rename it everywhere)
6. ALWAYS include generated_tests array with at least one test

JSON response:"#,
        feedback_section,
        context.triage.category,
        context.triage.description,
        context.triage.fix_approach.as_deref().unwrap_or("Not specified"),
        language,
        serde_json::to_string_pretty(&context.sample)?,
        context.analysis.as_deref().unwrap_or("No additional analysis"),
        available_files,
        files_context,
        get_test_file_example(language),
        test_framework_hint,
        test_framework_hint,
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

    #[test]
    fn test_parse_multi_file_fix_response() {
        let response = r#"```json
{
    "changes": [
        {
            "path": "src/models/user.ts",
            "content": "export interface User {\n  id: string;\n  name: string;\n  email: string | null;\n}",
            "description": "Made email field nullable"
        },
        {
            "path": "src/handlers/user.ts",
            "content": "import { User } from '../models/user';\n\nexport function getEmail(user: User): string {\n  return user.email ?? 'no-email@example.com';\n}",
            "description": "Added null coalescing for email"
        },
        {
            "path": "src/api/user.ts",
            "content": "import { getEmail } from '../handlers/user';\n\nexport function formatUser(user: User): string {\n  return `${user.name} <${getEmail(user)}>`;\n}",
            "description": "Updated to use safe email getter"
        }
    ],
    "explanation": "Fixed null pointer error by making email nullable and adding safe accessors across all affected files",
    "confidence": 0.85,
    "risks": ["Existing code may expect non-null email"],
    "suggested_tests": ["npm test", "npm run test:integration"]
}
```"#;

        let result = parse_json_response(response).unwrap();
        assert_eq!(result.changes.len(), 3);
        assert!(result.is_multi_file());
        assert_eq!(result.file_count(), 3);
        assert!(result.affected_files().contains(&"src/models/user.ts"));
        assert!(result.affected_files().contains(&"src/handlers/user.ts"));
        assert!(result.affected_files().contains(&"src/api/user.ts"));
    }

    #[test]
    fn test_generated_fix_summary() {
        let single_file_fix = GeneratedFix {
            changes: vec![FileChange {
                path: "src/user.ts".to_string(),
                content: "content".to_string(),
                description: "fix".to_string(),
                original_sha: None,
            }],
            explanation: "Fixed the bug".to_string(),
            confidence: 0.9,
            risks: vec![],
            suggested_tests: vec![],
            generated_tests: vec![],
        };
        assert!(!single_file_fix.is_multi_file());
        assert!(single_file_fix.summary().contains("1 file"));

        let multi_file_fix = GeneratedFix {
            changes: vec![
                FileChange {
                    path: "src/a.ts".to_string(),
                    content: "content".to_string(),
                    description: "fix".to_string(),
                    original_sha: None,
                },
                FileChange {
                    path: "src/b.ts".to_string(),
                    content: "content".to_string(),
                    description: "fix".to_string(),
                    original_sha: None,
                },
            ],
            explanation: "Fixed the bug across multiple files".to_string(),
            confidence: 0.8,
            risks: vec![],
            suggested_tests: vec![],
            generated_tests: vec![],
        };
        assert!(multi_file_fix.is_multi_file());
        assert!(multi_file_fix.summary().contains("2 files"));
    }

    #[test]
    fn test_validate_against_context() {
        let fix = GeneratedFix {
            changes: vec![
                FileChange {
                    path: "src/user.ts".to_string(),
                    content: "content".to_string(),
                    description: "fix".to_string(),
                    original_sha: None,
                },
                FileChange {
                    path: "src/handler.ts".to_string(),
                    content: "content".to_string(),
                    description: "fix".to_string(),
                    original_sha: None,
                },
            ],
            explanation: "Fix".to_string(),
            confidence: 0.9,
            risks: vec![],
            suggested_tests: vec![],
            generated_tests: vec![],
        };

        // All files are in context
        let context_files = vec!["src/user.ts", "src/handler.ts", "src/other.ts"];
        assert!(fix.validate_against_context(&context_files).is_ok());

        // Missing file in context
        let partial_context = vec!["src/user.ts"];
        let result = fix.validate_against_context(&partial_context);
        assert!(result.is_err());
        let invalid = result.unwrap_err();
        assert_eq!(invalid, vec!["src/handler.ts".to_string()]);
    }

    #[test]
    fn test_file_change_has_substantive_change() {
        let change = FileChange {
            path: "test.ts".to_string(),
            content: "const x = 1;\nconst y = 2;".to_string(),
            description: "test".to_string(),
            original_sha: None,
        };

        // Same content (with different whitespace) - no substantive change
        assert!(!change.has_substantive_change("const x = 1;\n  const y = 2;  "));

        // Different content - has substantive change
        assert!(change.has_substantive_change("const x = 1;"));
    }

    #[test]
    fn test_derive_test_file_path() {
        assert_eq!(derive_test_file_path("src/user.ts", "typescript"), "src/user.test.ts");
        assert_eq!(derive_test_file_path("src/handlers/user.ts", "typescript"), "src/handlers/user.test.ts");
        assert_eq!(derive_test_file_path("src/user.py", "python"), "tests/test_user.py");
        assert_eq!(derive_test_file_path("handlers/user.go", "go"), "handlers/user_test.go");
        assert_eq!(derive_test_file_path("src/user.rs", "rust"), "tests/user_test.rs");
    }

    #[test]
    fn test_get_test_framework_hint() {
        assert_eq!(get_test_framework_hint("typescript"), "Jest or Vitest");
        assert_eq!(get_test_framework_hint("python"), "pytest");
        assert_eq!(get_test_framework_hint("go"), "go test (testing package)");
        assert_eq!(get_test_framework_hint("rust"), "built-in test module (#[cfg(test)])");
    }

    #[test]
    fn test_parse_fix_with_generated_tests() {
        let response = r#"```json
{
    "changes": [
        {
            "path": "src/user.ts",
            "content": "export function getUser(id: string): User | null { return users.get(id) ?? null; }",
            "description": "Return null instead of undefined for missing users"
        }
    ],
    "explanation": "Fixed undefined return to return null explicitly",
    "confidence": 0.95,
    "risks": [],
    "suggested_tests": ["npm test"],
    "generated_tests": [
        {
            "path": "src/user.test.ts",
            "content": "import { getUser } from './user';\n\ndescribe('getUser', () => {\n  it('returns null for missing user', () => {\n    expect(getUser('nonexistent')).toBeNull();\n  });\n});",
            "description": "Test that getUser returns null for missing users",
            "framework": "Jest"
        }
    ]
}
```"#;

        let result = parse_json_response(response).unwrap();
        assert_eq!(result.changes.len(), 1);
        assert!(result.has_tests());
        assert_eq!(result.test_count(), 1);
        assert_eq!(result.generated_tests[0].framework, "Jest");
    }
}

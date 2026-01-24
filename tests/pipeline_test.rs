//! Integration tests for CRISPR pipeline stages
//!
//! Tests cover:
//! - Fingerprinting (error deduplication)
//! - Triage decisions (fixable vs unfixable)
//! - Context gathering
//! - Fix generation
//! - PR creation flow
//!
//! These tests do not require a database connection.

mod common;

use serde_json::{json, Value};
use uuid::Uuid;

use common::fixtures::{ErrorFixtures, SourceCodeFixtures};
use common::mocks::{
    MockFileChange, MockFixResponse, MockGeneratedTest, MockGitHubClient, MockLlmManager,
    MockTriageResponse,
};

// ============================================================================
// Fingerprinting Tests
// ============================================================================

/// Compute a fingerprint for an error (same logic as in pipeline/fingerprint.rs)
fn compute_fingerprint(payload: &Value) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();

    if let Some(message) = payload.get("message").and_then(|v| v.as_str()) {
        let normalized = normalize_message(message);
        hasher.update(normalized.as_bytes());
    }

    if let Some(stack) = payload.get("stack_trace").and_then(|v| v.as_str()) {
        let normalized = normalize_stack_trace(stack);
        hasher.update(normalized.as_bytes());
    }

    if let Some(project) = payload.get("project").and_then(|v| v.as_str()) {
        hasher.update(project.as_bytes());
    }

    hex::encode(hasher.finalize())
}

fn normalize_message(message: &str) -> String {
    let first_line = message.lines().next().unwrap_or(message);
    // Remove numbers that look like IDs
    let re_numbers = regex::Regex::new(r"\b\d{4,}\b").unwrap();
    let normalized = re_numbers.replace_all(first_line, "ID");
    normalized.to_string()
}

fn normalize_stack_trace(stack: &str) -> String {
    stack
        .lines()
        .map(|line| {
            let re_line = regex::Regex::new(r":\d+:\d+").unwrap();
            let line = re_line.replace_all(line, ":X:X");
            let re_addr = regex::Regex::new(r"0x[0-9a-fA-F]+").unwrap();
            re_addr.replace_all(&line, "0xADDR").to_string()
        })
        .take(10)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_fingerprint_same_error_different_ids() {
    // Same error type with different IDs should produce same fingerprint
    let payload1 = json!({
        "message": "User not found: 12345",
        "project": "acme/webapp"
    });

    let payload2 = json!({
        "message": "User not found: 67890",
        "project": "acme/webapp"
    });

    let fp1 = compute_fingerprint(&payload1);
    let fp2 = compute_fingerprint(&payload2);

    assert_eq!(fp1, fp2);
}

#[test]
fn test_fingerprint_different_errors() {
    let payload1 = json!({
        "message": "NullPointerException",
        "project": "acme/webapp"
    });

    let payload2 = json!({
        "message": "ArrayIndexOutOfBoundsException",
        "project": "acme/webapp"
    });

    let fp1 = compute_fingerprint(&payload1);
    let fp2 = compute_fingerprint(&payload2);

    assert_ne!(fp1, fp2);
}

#[test]
fn test_fingerprint_same_error_different_projects() {
    // Same error in different projects should have different fingerprints
    let payload1 = json!({
        "message": "Connection refused",
        "project": "acme/webapp"
    });

    let payload2 = json!({
        "message": "Connection refused",
        "project": "acme/backend"
    });

    let fp1 = compute_fingerprint(&payload1);
    let fp2 = compute_fingerprint(&payload2);

    assert_ne!(fp1, fp2);
}

#[test]
fn test_fingerprint_normalizes_line_numbers() {
    // Same stack trace with different line numbers should produce same fingerprint
    let payload1 = json!({
        "message": "Error",
        "stack_trace": "at handler.js:10:5\nat server.js:20:10",
        "project": "acme/webapp"
    });

    let payload2 = json!({
        "message": "Error",
        "stack_trace": "at handler.js:15:8\nat server.js:25:12",
        "project": "acme/webapp"
    });

    let fp1 = compute_fingerprint(&payload1);
    let fp2 = compute_fingerprint(&payload2);

    assert_eq!(fp1, fp2);
}

#[test]
fn test_fingerprint_typescript_error() {
    let error = ErrorFixtures::typescript_null_reference();
    let fp = compute_fingerprint(&error);

    assert!(!fp.is_empty());
    assert_eq!(fp.len(), 64); // SHA256 hex length
}

#[test]
fn test_fingerprint_stability() {
    // Same input should always produce same fingerprint
    let error = ErrorFixtures::python_type_error();

    let fp1 = compute_fingerprint(&error);
    let fp2 = compute_fingerprint(&error);
    let fp3 = compute_fingerprint(&error);

    assert_eq!(fp1, fp2);
    assert_eq!(fp2, fp3);
}

// ============================================================================
// Triage Tests
// ============================================================================

#[tokio::test]
async fn test_triage_fixable_null_reference() {
    let mock_llm = MockLlmManager::new();

    // Configure specific response for null reference errors
    mock_llm
        .set_triage_response(
            "Cannot read property",
            MockTriageResponse {
                fixable: true,
                confidence: 0.95,
                reason: "Null reference error can be fixed with optional chaining or null check"
                    .to_string(),
                category: "null_reference".to_string(),
                suggested_files: vec!["src/controllers/user.ts".to_string()],
                description: "TypeError when accessing property on undefined".to_string(),
                fix_approach: Some(
                    "Add optional chaining (?.) or explicit null check".to_string(),
                ),
            },
        )
        .await;

    let error = ErrorFixtures::typescript_null_reference();
    let response = mock_llm
        .get_triage_response(error["message"].as_str().unwrap())
        .await;

    let triage: Value = serde_json::from_str(&response).unwrap();

    assert!(triage["fixable"].as_bool().unwrap());
    assert!(triage["confidence"].as_f64().unwrap() > 0.9);
    assert_eq!(triage["category"], "null_reference");
    assert!(triage["fix_approach"].as_str().is_some());
}

#[tokio::test]
async fn test_triage_fixable_type_error() {
    let mock_llm = MockLlmManager::new();

    mock_llm
        .set_triage_response(
            "TypeError",
            MockTriageResponse {
                fixable: true,
                confidence: 0.85,
                reason: "Type error can be fixed with proper type handling".to_string(),
                category: "type_error".to_string(),
                suggested_files: vec!["src/services/user_service.py".to_string()],
                description: "Type mismatch in string concatenation".to_string(),
                fix_approach: Some("Add type guards or default values".to_string()),
            },
        )
        .await;

    let error = ErrorFixtures::python_type_error();
    let response = mock_llm
        .get_triage_response(error["message"].as_str().unwrap())
        .await;

    let triage: Value = serde_json::from_str(&response).unwrap();

    assert!(triage["fixable"].as_bool().unwrap());
    assert_eq!(triage["category"], "type_error");
}

#[tokio::test]
async fn test_triage_unfixable_configuration() {
    let mock_llm = MockLlmManager::new().with_unfixable_triage();

    let error = ErrorFixtures::configuration_error();
    let response = mock_llm
        .get_triage_response(error["message"].as_str().unwrap())
        .await;

    let triage: Value = serde_json::from_str(&response).unwrap();

    assert!(!triage["fixable"].as_bool().unwrap());
    assert_eq!(triage["category"], "configuration");
}

#[tokio::test]
async fn test_triage_unfixable_external_service() {
    let mock_llm = MockLlmManager::new();

    mock_llm
        .set_triage_response(
            "Connection refused",
            MockTriageResponse {
                fixable: false,
                confidence: 0.9,
                reason: "External service failure cannot be fixed with code changes".to_string(),
                category: "external_service".to_string(),
                suggested_files: vec![],
                description: "Database connection failure".to_string(),
                fix_approach: None,
            },
        )
        .await;

    let error = ErrorFixtures::external_service_error();
    let response = mock_llm
        .get_triage_response(error["message"].as_str().unwrap())
        .await;

    let triage: Value = serde_json::from_str(&response).unwrap();

    assert!(!triage["fixable"].as_bool().unwrap());
    assert_eq!(triage["category"], "external_service");
}

#[test]
fn test_triage_response_json_parsing() {
    // Test parsing LLM response with markdown code blocks
    let response_with_markdown = r#"```json
{
    "fixable": true,
    "confidence": 0.9,
    "reason": "Can be fixed with null check",
    "category": "null_reference",
    "suggested_files": ["src/user.ts"],
    "description": "Null reference error",
    "fix_approach": "Add null check"
}
```"#;

    // Parse JSON from markdown
    let json_str = response_with_markdown
        .split("```json")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .unwrap()
        .trim();

    let triage: Value = serde_json::from_str(json_str).unwrap();

    assert!(triage["fixable"].as_bool().unwrap());
    assert_eq!(triage["category"], "null_reference");
}

#[test]
fn test_triage_categories() {
    // Valid triage categories
    let fixable_categories = [
        "null_reference",
        "type_error",
        "bounds_error",
        "missing_import",
        "logic_error",
    ];

    let unfixable_categories = ["external_service", "configuration", "other"];

    for cat in fixable_categories {
        assert!(
            is_typically_fixable(cat),
            "Category {} should be fixable",
            cat
        );
    }

    for cat in unfixable_categories {
        assert!(
            !is_typically_fixable(cat),
            "Category {} should not be fixable",
            cat
        );
    }
}

fn is_typically_fixable(category: &str) -> bool {
    matches!(
        category,
        "null_reference" | "type_error" | "bounds_error" | "missing_import" | "logic_error"
    )
}

// ============================================================================
// Context Gathering Tests
// ============================================================================

/// Parse file paths from stack trace
fn parse_stack_trace_files(stack: &str) -> Vec<String> {
    let mut files = Vec::new();

    // Match JavaScript/TypeScript/Java format: file.ext:line
    let js_re = regex::Regex::new(r"([^\s(]+\.[a-zA-Z]{2,4}):\d+").unwrap();
    // Match Python format: File "path/file.py", line N
    let py_re = regex::Regex::new(r#"File "([^"]+\.py)", line \d+"#).unwrap();

    // Parse JavaScript/TypeScript/Java/Go style
    for cap in js_re.captures_iter(stack) {
        if let Some(path) = cap.get(1) {
            let path = path.as_str();
            // Filter out vendor/node_modules paths
            if !path.contains("node_modules")
                && !path.contains("vendor")
                && !path.contains("site-packages")
            {
                if !files.contains(&path.to_string()) {
                    files.push(path.to_string());
                }
            }
        }
    }

    // Parse Python style
    for cap in py_re.captures_iter(stack) {
        if let Some(path) = cap.get(1) {
            let path = path.as_str();
            // Filter out vendor/site-packages paths
            if !path.contains("site-packages") && !path.contains("vendor") {
                if !files.contains(&path.to_string()) {
                    files.push(path.to_string());
                }
            }
        }
    }

    files
}

#[test]
fn test_context_extracts_files_from_stack_trace() {
    let error = ErrorFixtures::typescript_null_reference();
    let stack = error["stack_trace"].as_str().unwrap();

    let files = parse_stack_trace_files(stack);

    assert!(files.contains(&"src/controllers/user.ts".to_string()));
    // Should not include node_modules
    assert!(!files.iter().any(|f| f.contains("node_modules")));
}

#[test]
fn test_context_extracts_files_from_python_stack() {
    let error = ErrorFixtures::python_type_error();
    let stack = error["stack_trace"].as_str().unwrap();

    let files = parse_stack_trace_files(stack);

    assert!(!files.is_empty());
    // Python uses slightly different path format, so check for .py files
    assert!(files.iter().any(|f| f.ends_with(".py")));
}

#[tokio::test]
async fn test_context_fetches_files_from_github() {
    let mock_github = MockGitHubClient::new();

    // Add test files
    mock_github
        .add_file(
            "acme",
            "webapp",
            "src/controllers/user.ts",
            SourceCodeFixtures::typescript_user_handler(),
        )
        .await;
    mock_github
        .add_file(
            "acme",
            "webapp",
            "src/models/user.ts",
            SourceCodeFixtures::typescript_user_model(),
        )
        .await;

    // Verify files are accessible
    let file = mock_github
        .get_file("acme", "webapp", "src/controllers/user.ts")
        .await;

    assert!(file.is_some());
    assert!(file.unwrap().content.contains("UserController"));

    // Check API calls were recorded
    let calls = mock_github.get_api_calls().await;
    assert!(!calls.is_empty());
}

#[test]
fn test_context_finds_related_files() {
    // Given a handler file, should suggest related model/service files
    let handler_path = "src/handlers/user.ts";

    let related = find_related_files(handler_path);

    assert!(related.contains(&"src/models/user.ts".to_string()));
    assert!(related.contains(&"src/services/user.ts".to_string()));
}

fn find_related_files(path: &str) -> Vec<String> {
    let mut related = Vec::new();

    if path.contains("/handlers/") {
        related.push(path.replace("/handlers/", "/models/"));
        related.push(path.replace("/handlers/", "/services/"));
    }

    if path.contains("/controllers/") {
        related.push(path.replace("/controllers/", "/models/"));
        related.push(path.replace("/controllers/", "/services/"));
    }

    related
}

#[test]
fn test_context_filters_vendor_paths() {
    let paths = vec![
        "src/user.ts",
        "node_modules/express/index.js",
        "vendor/autoload.php",
        ".pyenv/lib/python.py",
        "src/handler.go",
    ];

    let filtered: Vec<&str> = paths
        .into_iter()
        .filter(|p| should_include_path(p))
        .collect();

    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains(&"src/user.ts"));
    assert!(filtered.contains(&"src/handler.go"));
}

fn should_include_path(path: &str) -> bool {
    let excluded = [
        "node_modules",
        "vendor",
        "site-packages",
        ".pyenv",
        ".virtualenv",
    ];

    !excluded.iter().any(|p| path.contains(p)) && !path.is_empty()
}

// ============================================================================
// Fix Generation Tests
// ============================================================================

#[tokio::test]
async fn test_fix_generation_single_file() {
    let mock_llm = MockLlmManager::new();

    mock_llm
        .set_fix_response(
            "null_reference",
            MockFixResponse {
                changes: vec![MockFileChange {
                    path: "src/controllers/user.ts".to_string(),
                    content: r#"import { User } from '../models/user';

export class UserController {
  async getProfile(req: Request, res: Response) {
    const userId = req.params.id;
    const user = await this.userService.findById(userId);

    // Fixed: Added null check
    if (!user) {
      return res.status(404).json({ error: 'User not found' });
    }

    const profile = {
      name: user.name,
      email: user.email,
      createdAt: user.createdAt
    };

    res.json(profile);
  }
}"#
                    .to_string(),
                    description: "Added null check for user lookup".to_string(),
                }],
                explanation: "Added null check to handle case when user is not found".to_string(),
                confidence: 0.9,
                risks: vec!["Callers expect different response format".to_string()],
                suggested_tests: vec!["npm test".to_string()],
                generated_tests: vec![MockGeneratedTest {
                    path: "src/controllers/user.test.ts".to_string(),
                    content: r#"describe('UserController', () => {
  it('returns 404 for non-existent user', async () => {
    // Test implementation
  });
});"#
                        .to_string(),
                    description: "Test null user handling".to_string(),
                    framework: "jest".to_string(),
                }],
            },
        )
        .await;

    let response = mock_llm.get_fix_response("null_reference").await;
    let fix: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(fix["changes"].as_array().unwrap().len(), 1);
    assert!(fix["confidence"].as_f64().unwrap() > 0.8);
    assert!(!fix["generated_tests"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_fix_generation_multi_file() {
    let mock_llm = MockLlmManager::new();

    mock_llm
        .set_fix_response(
            "type_error",
            MockFixResponse {
                changes: vec![
                    MockFileChange {
                        path: "src/models/user.ts".to_string(),
                        content: "// Updated model with optional fields".to_string(),
                        description: "Made email field optional".to_string(),
                    },
                    MockFileChange {
                        path: "src/handlers/user.ts".to_string(),
                        content: "// Updated handler with null checks".to_string(),
                        description: "Added null coalescing for optional fields".to_string(),
                    },
                ],
                explanation: "Fixed type error by making field optional and adding null handling"
                    .to_string(),
                confidence: 0.85,
                risks: vec!["Existing code may expect non-null values".to_string()],
                suggested_tests: vec!["npm test".to_string()],
                generated_tests: vec![],
            },
        )
        .await;

    let response = mock_llm.get_fix_response("type_error").await;
    let fix: Value = serde_json::from_str(&response).unwrap();

    let changes = fix["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);

    // Verify both files are modified
    let paths: Vec<&str> = changes
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"src/models/user.ts"));
    assert!(paths.contains(&"src/handlers/user.ts"));
}

#[tokio::test]
async fn test_fix_generation_with_tests() {
    let mock_llm = MockLlmManager::new();

    // Default response includes generated tests
    let response = mock_llm.get_fix_response("null_reference").await;
    let fix: Value = serde_json::from_str(&response).unwrap();

    let tests = fix["generated_tests"].as_array().unwrap();
    assert!(!tests.is_empty());

    let test = &tests[0];
    assert!(test["path"].as_str().unwrap().contains(".test."));
    assert!(test["framework"].as_str().is_some());
    assert!(test["content"].as_str().is_some());
}

#[tokio::test]
async fn test_fix_generation_low_confidence() {
    let mock_llm = MockLlmManager::new().with_failed_fix();

    let response = mock_llm.get_fix_response("complex_error").await;
    let fix: Value = serde_json::from_str(&response).unwrap();

    assert!(fix["confidence"].as_f64().unwrap() < 0.5);
    assert!(fix["changes"].as_array().unwrap().is_empty());
}

#[test]
fn test_fix_response_json_parsing() {
    let response = r#"```json
{
    "changes": [
        {
            "path": "src/user.ts",
            "content": "const user = getUser();\nif (user) {\n  console.log(user.name);\n}",
            "description": "Added null check"
        }
    ],
    "explanation": "Added null check to prevent TypeError",
    "confidence": 0.9,
    "risks": ["May need different null handling"],
    "suggested_tests": ["npm test"],
    "generated_tests": []
}
```"#;

    // Parse JSON from markdown
    let json_str = response
        .split("```json")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .unwrap()
        .trim();

    let fix: Value = serde_json::from_str(json_str).unwrap();

    assert_eq!(fix["changes"].as_array().unwrap().len(), 1);
    assert!(fix["confidence"].as_f64().unwrap() > 0.8);
}

#[test]
fn test_fix_validates_file_paths() {
    // Fix should only modify files that were in the context
    let context_files = vec!["src/user.ts", "src/handler.ts"];
    let fix_paths = vec!["src/user.ts", "src/handler.ts"];

    let all_valid = fix_paths.iter().all(|p| context_files.contains(p));
    assert!(all_valid);

    // Invalid path
    let invalid_paths = vec!["src/user.ts", "src/other.ts"];
    let has_invalid = invalid_paths.iter().any(|p| !context_files.contains(p));
    assert!(has_invalid);
}

// ============================================================================
// PR Creation Tests
// ============================================================================

#[tokio::test]
async fn test_pr_creation_flow() {
    let mock_github = MockGitHubClient::new();

    // Create branch
    let branch = mock_github
        .create_branch("acme", "webapp", "crispr/fix-null-ref-12345", "abc123")
        .await
        .unwrap();

    assert_eq!(branch.name, "crispr/fix-null-ref-12345");

    // Create PR
    let pr = mock_github
        .create_pull_request(
            "acme",
            "webapp",
            "Fix null reference in UserController",
            "This PR fixes a null reference error...",
            "crispr/fix-null-ref-12345",
            "main",
        )
        .await
        .unwrap();

    assert_eq!(pr.title, "Fix null reference in UserController");
    assert_eq!(pr.state, "open");
    assert_eq!(pr.base, "main");

    // Verify PR was recorded
    let prs = mock_github.get_pull_requests().await;
    assert_eq!(prs.len(), 1);
}

#[test]
fn test_pr_branch_naming() {
    // Branch names should follow pattern: crispr/fix-{category}-{short_id}
    let incident_id = Uuid::new_v4();
    let category = "null_reference";

    let branch_name = format!(
        "crispr/fix-{}-{}",
        category.replace('_', "-"),
        &incident_id.to_string()[..8]
    );

    assert!(branch_name.starts_with("crispr/fix-"));
    assert!(branch_name.contains("null-reference"));
}

#[test]
fn test_pr_body_generation() {
    let incident_id = Uuid::new_v4();
    let explanation = "Added null check to prevent TypeError when user is not found";
    let risks: Vec<String> = vec!["Callers may need to handle null case".to_string()];

    let pr_body = generate_pr_body(incident_id, explanation, &risks);

    assert!(pr_body.contains("Auto-Fix"));
    assert!(pr_body.contains(&incident_id.to_string()));
    assert!(pr_body.contains("Risks"));
}

fn generate_pr_body(incident_id: Uuid, explanation: &str, risks: &[String]) -> String {
    format!(
        r#"## CRISPR Auto-Fix

### Summary
{}

### Risks
{}

### Testing
- Run tests to verify the fix

---
*Generated by [CRISPR](https://github.com/acme/crispr)*
Incident: `{}`"#,
        explanation,
        risks
            .iter()
            .map(|r| format!("- {}", r))
            .collect::<Vec<_>>()
            .join("\n"),
        incident_id
    )
}

// ============================================================================
// LLM Request Tracking Tests
// ============================================================================

#[tokio::test]
async fn test_llm_request_tracking() {
    let mock_llm = MockLlmManager::new();

    // Make some requests
    let _ = mock_llm.get_triage_response("Error message").await;
    let _ = mock_llm.get_fix_response("null_reference").await;

    // Verify requests were tracked
    let requests = mock_llm.get_requests().await;
    assert_eq!(requests.len(), 2);

    assert_eq!(requests[0].role, "reader");
    assert_eq!(requests[0].operation, "triage");

    assert_eq!(requests[1].role, "writer");
    assert_eq!(requests[1].operation, "fix");
}

#[tokio::test]
async fn test_llm_request_clearing() {
    let mock_llm = MockLlmManager::new();

    let _ = mock_llm.get_triage_response("Error").await;
    assert_eq!(mock_llm.get_requests().await.len(), 1);

    mock_llm.clear_requests().await;
    assert_eq!(mock_llm.get_requests().await.len(), 0);
}

// ============================================================================
// Test File Path Derivation
// ============================================================================

#[test]
fn test_derive_test_file_path_typescript() {
    let source = "src/handlers/user.ts";
    let test_path = derive_test_path(source, "typescript");
    assert_eq!(test_path, "src/handlers/user.test.ts");
}

#[test]
fn test_derive_test_file_path_python() {
    let source = "src/handlers/user.py";
    let test_path = derive_test_path(source, "python");
    assert_eq!(test_path, "tests/test_user.py");
}

#[test]
fn test_derive_test_file_path_go() {
    let source = "handlers/user.go";
    let test_path = derive_test_path(source, "go");
    assert_eq!(test_path, "handlers/user_test.go");
}

#[test]
fn test_derive_test_file_path_rust() {
    let source = "src/handlers/user.rs";
    let test_path = derive_test_path(source, "rust");
    assert_eq!(test_path, "tests/user_test.rs");
}

#[test]
fn test_derive_test_file_path_java() {
    let source = "src/main/java/User.java";
    let test_path = derive_test_path(source, "java");
    assert_eq!(test_path, "src/test/java/UserTest.java");
}

fn derive_test_path(source_path: &str, language: &str) -> String {
    let path = std::path::Path::new(source_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("test");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    match language {
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
            format!("tests/{}_test.rs", stem)
        }
        "java" => {
            let test_path = parent.replace("src/main/java", "src/test/java");
            format!("{}/{}Test.java", test_path, stem)
        }
        _ => format!("{}.test", stem),
    }
}

// ============================================================================
// End-to-End Pipeline Flow Test
// ============================================================================

#[tokio::test]
async fn test_full_pipeline_flow() {
    // Simulate the full pipeline: ingest -> triage -> fix -> PR

    // 1. Ingest error
    let error = ErrorFixtures::typescript_null_reference();
    let fingerprint = compute_fingerprint(&error);
    assert!(!fingerprint.is_empty());

    // 2. Triage
    let mock_llm = MockLlmManager::new();
    let triage_response = mock_llm
        .get_triage_response(error["message"].as_str().unwrap())
        .await;
    let triage: Value = serde_json::from_str(&triage_response).unwrap();

    assert!(triage["fixable"].as_bool().unwrap());

    // 3. Gather context (mock)
    let mock_github = MockGitHubClient::new();
    mock_github
        .add_file(
            "acme",
            "webapp",
            "src/controllers/user.ts",
            SourceCodeFixtures::typescript_user_handler(),
        )
        .await;

    let file = mock_github
        .get_file("acme", "webapp", "src/controllers/user.ts")
        .await;
    assert!(file.is_some());

    // 4. Generate fix
    let fix_response = mock_llm
        .get_fix_response(triage["category"].as_str().unwrap())
        .await;
    let fix: Value = serde_json::from_str(&fix_response).unwrap();

    assert!(!fix["changes"].as_array().unwrap().is_empty());

    // 5. Create PR
    let incident_id = Uuid::new_v4();
    let branch_name = format!("crispr/fix-{}", &incident_id.to_string()[..8]);

    let pr = mock_github
        .create_pull_request(
            "acme",
            "webapp",
            "Fix null reference in UserController",
            fix["explanation"].as_str().unwrap(),
            &branch_name,
            "main",
        )
        .await
        .unwrap();

    assert_eq!(pr.state, "open");

    // Verify all LLM calls were made
    let llm_requests = mock_llm.get_requests().await;
    assert_eq!(llm_requests.len(), 2); // triage + fix
}

// ============================================================================
// Error Categorization Tests
// ============================================================================

#[test]
fn test_error_categorization() {
    let test_cases = vec![
        ("Cannot read property 'x' of undefined", "null_reference"),
        ("TypeError: undefined is not a function", "type_error"),
        ("Index out of bounds", "bounds_error"),
        ("Cannot find module './user'", "missing_import"),
        ("Connection refused", "external_service"),
        ("Environment variable not set", "configuration"),
    ];

    for (message, expected_category) in test_cases {
        let category = categorize_error(message);
        assert_eq!(
            category, expected_category,
            "Message '{}' should be categorized as '{}'",
            message, expected_category
        );
    }
}

fn categorize_error(message: &str) -> &'static str {
    let lower = message.to_lowercase();

    // Check for type errors first (they may also contain "undefined")
    if lower.starts_with("typeerror") || lower.contains("type mismatch") {
        "type_error"
    } else if lower.contains("undefined")
        || lower.contains("null")
        || lower.contains("cannot read property")
    {
        "null_reference"
    } else if lower.contains("index") || lower.contains("bounds") {
        "bounds_error"
    } else if lower.contains("module") || lower.contains("import") {
        "missing_import"
    } else if lower.contains("connection") || lower.contains("refused") {
        "external_service"
    } else if lower.contains("environment") || lower.contains("config") {
        "configuration"
    } else {
        "other"
    }
}

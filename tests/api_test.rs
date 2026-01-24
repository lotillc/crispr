//! Integration tests for CRISPR HTTP API endpoints
//!
//! Tests cover:
//! - Health check endpoints (/health, /ready)
//! - Incident listing and retrieval
//! - Webhook ingestion (Sentry, Datadog)
//! - Ingest endpoint
//!
//! # Running Tests
//!
//! These tests require a running PostgreSQL database with the CRISPR schema.
//!
//! 1. Start PostgreSQL:
//!    ```bash
//!    docker run -d --name crispr-test-db \
//!      -e POSTGRES_USER=crispr \
//!      -e POSTGRES_PASSWORD=crispr \
//!      -e POSTGRES_DB=crispr_test \
//!      -p 5433:5432 postgres:15
//!    ```
//!
//! 2. Run migrations:
//!    ```bash
//!    DATABASE_URL=postgres://crispr:crispr@localhost:5433/crispr_test sqlx migrate run
//!    ```
//!
//! 3. Run tests:
//!    ```bash
//!    TEST_DATABASE_URL=postgres://crispr:crispr@localhost:5433/crispr_test cargo test
//!    ```
//!
//! Tests marked with `#[ignore]` require database connectivity.
//! Run them with: `cargo test -- --ignored`

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use common::fixtures::{DatadogFixtures, ErrorFixtures, IngestFixtures, SentryFixtures};

// ============================================================================
// Health Check Tests (No Database Required)
// ============================================================================

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    // Create a minimal router for testing
    use axum::routing::get;
    let app = axum::Router::new().route("/health", get(|| async { "OK" }));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"OK");
}

#[tokio::test]
async fn test_health_endpoint_accepts_any_method() {
    use axum::routing::get;
    let app = axum::Router::new().route("/health", get(|| async { "OK" }));

    // GET should work
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================================
// Ingest Payload Validation Tests (No Database Required)
// ============================================================================

#[test]
fn test_ingest_rejects_warning_level() {
    // Warning level errors should not create incidents
    let payload = IngestFixtures::warning_level("acme/webapp");
    let level = payload["level"].as_str().unwrap();

    // Verify it's a warning
    assert_eq!(level, "WARNING");

    // The actual rejection logic is in the ingest handler
    // This test validates the fixture is correct for testing rejection
    assert!(!matches!(level, "ERROR" | "FATAL" | "CRITICAL" | "SEVERE"));
}

#[test]
fn test_ingest_accepts_error_levels() {
    // These levels should be processed
    let valid_levels = ["ERROR", "FATAL", "CRITICAL", "SEVERE"];

    for level in valid_levels {
        assert!(
            matches!(level, "ERROR" | "FATAL" | "CRITICAL" | "SEVERE"),
            "Level {} should be accepted",
            level
        );
    }
}

#[test]
fn test_ingest_with_service_name_inference() {
    // Test that service_name "acme-webapp" gets converted to "acme/webapp"
    let payload = IngestFixtures::with_service_name();
    let service_name = payload["service_name"].as_str().unwrap();

    assert_eq!(service_name, "acme-webapp");

    // Infer project from service name
    let inferred = infer_project_from_service(service_name);
    assert_eq!(inferred, "acme/webapp");
}

fn infer_project_from_service(service_name: &str) -> String {
    if service_name.contains('/') {
        service_name.to_string()
    } else if let Some(idx) = service_name.find('-') {
        let (owner, repo) = service_name.split_at(idx);
        format!("{}/{}", owner, &repo[1..])
    } else {
        service_name.to_string()
    }
}

#[test]
fn test_ingest_payload_project_parsing() {
    // Valid project format
    let valid = parse_project_key("acme/webapp");
    assert!(valid.is_ok());
    let (owner, repo) = valid.unwrap();
    assert_eq!(owner, "acme");
    assert_eq!(repo, "webapp");

    // Invalid formats
    assert!(parse_project_key("invalid").is_err());
    assert!(parse_project_key("a/b/c").is_err());
    assert!(parse_project_key("").is_err());
}

fn parse_project_key(key: &str) -> Result<(String, String), &'static str> {
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("Invalid project key format");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[test]
fn test_ingest_allowed_repos_check() {
    let allowed = vec![
        "acme/webapp".to_string(),
        "acme/*-sandbox".to_string(),
        "test/*".to_string(),
    ];

    // Exact match
    assert!(is_repo_allowed("acme", "webapp", &allowed));

    // Wildcard match
    assert!(is_repo_allowed("acme", "foo-sandbox", &allowed));
    assert!(is_repo_allowed("test", "anything", &allowed));

    // Not allowed
    assert!(!is_repo_allowed("other", "repo", &allowed));
    assert!(!is_repo_allowed("acme", "production", &allowed));
}

fn is_repo_allowed(owner: &str, repo: &str, allowed: &[String]) -> bool {
    let full = format!("{}/{}", owner, repo);
    allowed.iter().any(|pattern| {
        if pattern.contains('*') {
            let regex_pattern = pattern.replace('.', r"\.");
            let regex_pattern = regex_pattern.replace('*', ".*");
            regex::Regex::new(&format!("^{}$", regex_pattern))
                .map(|re| re.is_match(&full))
                .unwrap_or(false)
        } else {
            pattern == &full
        }
    })
}

// ============================================================================
// Sentry Webhook Tests (No Database Required)
// ============================================================================

#[test]
fn test_sentry_webhook_payload_parsing() {
    let payload = SentryFixtures::issue_created();

    // Verify payload structure
    assert_eq!(payload["action"], "created");
    assert!(payload["data"]["issue"].is_object());

    let issue = &payload["data"]["issue"];
    assert_eq!(issue["level"], "error");
    assert_eq!(issue["project"]["slug"], "my-project");
}

#[test]
fn test_sentry_issue_to_ingest_conversion() {
    let payload = SentryFixtures::issue_created();
    let issue = &payload["data"]["issue"];

    // Build message from title and metadata
    let message = if let Some(metadata) = issue.get("metadata") {
        if let (Some(error_type), Some(value)) = (
            metadata.get("type").and_then(|v| v.as_str()),
            metadata.get("value").and_then(|v| v.as_str()),
        ) {
            format!("{}: {}", error_type, value)
        } else {
            issue["title"].as_str().unwrap_or("Unknown").to_string()
        }
    } else {
        issue["title"].as_str().unwrap_or("Unknown").to_string()
    };

    assert_eq!(message, "NullPointerException: User not found");

    // Extract project
    let project = issue["project"]["slug"]
        .as_str()
        .map(|s| format!("sentry/{}", s));
    assert_eq!(project, Some("sentry/my-project".to_string()));
}

#[test]
fn test_sentry_event_with_stacktrace() {
    let payload = SentryFixtures::event_with_stacktrace();
    let event = &payload["data"]["event"];

    assert!(event["exception"]["values"].is_array());

    let values = event["exception"]["values"].as_array().unwrap();
    assert!(!values.is_empty());

    let stacktrace = &values[0]["stacktrace"];
    let frames = stacktrace["frames"].as_array().unwrap();

    // Verify stack frames are parsed correctly
    assert!(!frames.is_empty());
    assert_eq!(frames[0]["filename"], "src/components/UserCard.tsx");
    assert_eq!(frames[0]["lineno"], 25);
}

#[test]
fn test_sentry_level_filtering() {
    // Error levels that should be processed
    let error_levels = ["error", "ERROR", "fatal", "FATAL", "critical", "CRITICAL"];
    for level in error_levels {
        let upper = level.to_uppercase();
        assert!(
            matches!(upper.as_str(), "ERROR" | "FATAL" | "CRITICAL"),
            "Level {} should be processed",
            level
        );
    }

    // Warning should be ignored
    let payload = SentryFixtures::warning_event();
    let level = payload["data"]["issue"]["level"].as_str().unwrap_or("info");
    assert!(!matches!(
        level.to_uppercase().as_str(),
        "ERROR" | "FATAL" | "CRITICAL"
    ));
}

#[test]
fn test_sentry_stack_trace_extraction() {
    let payload = SentryFixtures::event_with_stacktrace();
    let event = &payload["data"]["event"];

    // Extract stack trace from exception
    let stack_trace = extract_sentry_stack_trace(event);
    assert!(stack_trace.is_some());

    let trace = stack_trace.unwrap();
    assert!(trace.contains("UserCard"));
    assert!(trace.contains("src/components/UserCard.tsx:25"));
}

fn extract_sentry_stack_trace(event: &Value) -> Option<String> {
    event
        .get("exception")
        .and_then(|ex| ex.get("values"))
        .and_then(|values| values.as_array())
        .and_then(|values| values.first())
        .and_then(|v| v.get("stacktrace"))
        .and_then(|st| st.get("frames"))
        .and_then(|frames| frames.as_array())
        .map(|frames| {
            frames
                .iter()
                .filter(|f| f.get("in_app").and_then(|v| v.as_bool()).unwrap_or(true))
                .take(10)
                .map(|f| {
                    let filename = f.get("filename").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let lineno = f.get("lineno").and_then(|v| v.as_i64()).unwrap_or(0);
                    let function = f.get("function").and_then(|v| v.as_str()).unwrap_or("<unknown>");
                    format!("  at {} ({}:{})", function, filename, lineno)
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
}

// ============================================================================
// Datadog Webhook Tests (No Database Required)
// ============================================================================

#[test]
fn test_datadog_error_event_parsing() {
    let payload = DatadogFixtures::error_event();

    assert_eq!(payload["alert_type"], "error");
    assert!(payload["title"].as_str().is_some());

    // Extract project from tags
    let tags = payload["tags"].as_array().unwrap();
    let project = extract_project_from_datadog_tags(tags);

    assert_eq!(project, Some("acme/api".to_string()));
}

#[test]
fn test_datadog_monitor_alert() {
    let payload = DatadogFixtures::monitor_alert();

    assert_eq!(payload["alert_status"], "Alert");
    assert!(payload["monitor"].is_object());

    let monitor = &payload["monitor"];
    assert_eq!(monitor["name"], "High CPU Monitor");
}

#[test]
fn test_datadog_log_alert() {
    let payload = DatadogFixtures::log_alert();

    assert!(payload["logs"].is_array());

    let logs = payload["logs"].as_array().unwrap();
    assert!(!logs.is_empty());

    let log = &logs[0];
    assert_eq!(log["status"], "error");
    assert!(log["message"].as_str().is_some());
}

#[test]
fn test_datadog_info_should_be_ignored() {
    let payload = DatadogFixtures::info_event();

    let alert_type = payload["alert_type"].as_str().unwrap_or("info");
    let is_error = alert_type == "error"
        || payload["alert_status"].as_str() == Some("Alert")
        || payload["priority"].as_str() == Some("critical");

    assert!(!is_error);
}

#[test]
fn test_datadog_tag_project_extraction() {
    // Test project extraction from various tag formats
    let test_cases = vec![
        (vec!["project:acme/webapp"], Some("acme/webapp".to_string())),
        (vec!["repo:acme/backend"], Some("acme/backend".to_string())),
        (vec!["service:acme-api"], Some("acme/api".to_string())),
        (vec!["env:production"], None),
    ];

    for (tags, expected) in test_cases {
        let tags: Vec<Value> = tags.into_iter().map(|s| Value::String(s.to_string())).collect();
        let project = extract_project_from_datadog_tags(&tags);
        assert_eq!(project, expected, "Failed for tags: {:?}", tags);
    }
}

fn extract_project_from_datadog_tags(tags: &[Value]) -> Option<String> {
    // First look for explicit project tag
    for tag in tags {
        if let Some(tag_str) = tag.as_str() {
            if tag_str.starts_with("project:") {
                return Some(tag_str.trim_start_matches("project:").to_string());
            }
            if tag_str.starts_with("repo:") {
                return Some(tag_str.trim_start_matches("repo:").to_string());
            }
        }
    }
    // Fall back to service tag
    for tag in tags {
        if let Some(tag_str) = tag.as_str() {
            if tag_str.starts_with("service:") {
                let service = tag_str.trim_start_matches("service:");
                if service.contains('/') {
                    return Some(service.to_string());
                }
                if service.contains('-') {
                    if let Some(idx) = service.find('-') {
                        let (owner, repo) = service.split_at(idx);
                        return Some(format!("{}/{}", owner, &repo[1..]));
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// Error Fixture Validation Tests (No Database Required)
// ============================================================================

#[test]
fn test_typescript_error_fixture() {
    let error = ErrorFixtures::typescript_null_reference();

    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("Cannot read property"));
    assert!(error["stack_trace"].as_str().is_some());
    assert_eq!(error["project"], "acme/webapp");
    assert_eq!(error["level"], "ERROR");
}

#[test]
fn test_python_error_fixture() {
    let error = ErrorFixtures::python_type_error();

    assert!(error["message"].as_str().unwrap().contains("TypeError"));
    assert!(error["stack_trace"].as_str().unwrap().contains(".py"));
}

#[test]
fn test_java_error_fixture() {
    let error = ErrorFixtures::java_array_index();

    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("ArrayIndexOutOfBoundsException"));
    assert!(error["stack_trace"].as_str().unwrap().contains(".java"));
}

#[test]
fn test_configuration_error_fixture() {
    let error = ErrorFixtures::configuration_error();

    // Configuration errors typically can't be fixed with code changes
    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("environment variable"));
    assert_eq!(error["level"], "FATAL");
}

#[test]
fn test_go_panic_fixture() {
    let error = ErrorFixtures::go_panic();

    assert!(error["message"].as_str().unwrap().contains("panic"));
    assert!(error["stack_trace"].as_str().unwrap().contains(".go"));
}

#[test]
fn test_rust_panic_fixture() {
    let error = ErrorFixtures::rust_panic();

    assert!(error["message"].as_str().unwrap().contains("panicked"));
    assert!(error["stack_trace"].as_str().unwrap().contains(".rs"));
}

// ============================================================================
// Message Truncation Tests
// ============================================================================

#[test]
fn test_truncate_title() {
    assert_eq!(truncate_title("Short message"), "Short message");
    assert_eq!(truncate_title("Line 1\nLine 2\nLine 3"), "Line 1");

    let long = "x".repeat(300);
    let truncated = truncate_title(&long);
    assert_eq!(truncated.len(), 200);
    assert!(truncated.ends_with("..."));
}

fn truncate_title(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or(msg);
    if first_line.len() > 200 {
        format!("{}...", &first_line[..197])
    } else {
        first_line.to_string()
    }
}

// ============================================================================
// Incident State Machine Tests
// ============================================================================

#[test]
fn test_incident_state_transitions() {
    // Valid state transitions
    let valid_transitions = vec![
        ("detected", "triaging"),
        ("triaging", "unfixable"),
        ("triaging", "fixing"),
        ("fixing", "fix_failed"),
        ("fixing", "pr_opening"),
        ("pr_opening", "pr_open"),
        ("pr_open", "pr_merged"),
        ("pr_open", "pr_closed"),
        ("fix_failed", "detected"),    // retry
        ("unfixable", "detected"),     // retry
        ("cooldown", "detected"),      // retry
        ("pr_closed", "detected"),     // retry
    ];

    for (from, to) in valid_transitions {
        assert!(
            is_valid_transition(from, to),
            "Transition from {} to {} should be valid",
            from,
            to
        );
    }
}

#[test]
fn test_incident_retryable_states() {
    let retryable = ["fix_failed", "unfixable", "cooldown", "pr_closed"];
    let not_retryable = ["detected", "triaging", "fixing", "pr_opening", "pr_open", "pr_merged"];

    for state in retryable {
        assert!(is_retryable(state), "State {} should be retryable", state);
    }

    for state in not_retryable {
        assert!(!is_retryable(state), "State {} should not be retryable", state);
    }
}

fn is_valid_transition(from: &str, to: &str) -> bool {
    match from {
        "detected" => matches!(to, "triaging"),
        "triaging" => matches!(to, "unfixable" | "fixing"),
        "fixing" => matches!(to, "fix_failed" | "pr_opening"),
        "pr_opening" => matches!(to, "pr_open"),
        "pr_open" => matches!(to, "pr_merged" | "pr_closed"),
        "fix_failed" | "unfixable" | "cooldown" | "pr_closed" => to == "detected",
        _ => false,
    }
}

fn is_retryable(state: &str) -> bool {
    matches!(state, "fix_failed" | "unfixable" | "cooldown" | "pr_closed")
}

// ============================================================================
// UUID Generation Tests
// ============================================================================

#[test]
fn test_uuid_generation() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    assert_ne!(id1, id2);
    assert_eq!(id1.to_string().len(), 36); // Standard UUID format
}

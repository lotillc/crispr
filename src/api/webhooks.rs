//! Webhook handlers for external error tracking services.
//!
//! Supports ingestion from:
//! - Sentry: Issue and error event webhooks
//! - Datadog: Event and error webhooks

use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::ingest::IngestPayload;
use super::{metrics, AppState};
use crate::error::AppError;

// ============================================================================
// Sentry Webhook Types
// ============================================================================

/// Sentry webhook payload wrapper
#[derive(Debug, Deserialize)]
pub struct SentryWebhookPayload {
    /// The action that triggered the webhook
    pub action: String,
    /// The actor who triggered the event (user or system)
    pub actor: Option<SentryActor>,
    /// The data payload containing the issue/event
    pub data: SentryData,
    /// Installation information
    pub installation: Option<SentryInstallation>,
}

#[derive(Debug, Deserialize)]
pub struct SentryActor {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub actor_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SentryData {
    /// The issue object (for issue webhooks)
    pub issue: Option<SentryIssue>,
    /// The event object (for event webhooks)
    pub event: Option<SentryEvent>,
}

#[derive(Debug, Deserialize)]
pub struct SentryIssue {
    pub id: String,
    pub title: String,
    pub culprit: Option<String>,
    pub level: Option<String>,
    pub status: Option<String>,
    pub platform: Option<String>,
    pub project: SentryProject,
    pub metadata: Option<SentryMetadata>,
    #[serde(rename = "firstSeen")]
    pub first_seen: Option<String>,
    #[serde(rename = "lastSeen")]
    pub last_seen: Option<String>,
    pub count: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SentryEvent {
    pub event_id: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
    pub level: Option<String>,
    pub platform: Option<String>,
    pub timestamp: Option<f64>,
    pub tags: Option<Vec<SentryTag>>,
    pub exception: Option<SentryException>,
    pub contexts: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SentryProject {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SentryMetadata {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub value: Option<String>,
    pub filename: Option<String>,
    pub function: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SentryTag {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct SentryException {
    pub values: Option<Vec<SentryExceptionValue>>,
}

#[derive(Debug, Deserialize)]
pub struct SentryExceptionValue {
    #[serde(rename = "type")]
    pub exception_type: Option<String>,
    pub value: Option<String>,
    pub module: Option<String>,
    pub stacktrace: Option<SentryStacktrace>,
}

#[derive(Debug, Deserialize)]
pub struct SentryStacktrace {
    pub frames: Option<Vec<SentryFrame>>,
}

#[derive(Debug, Deserialize)]
pub struct SentryFrame {
    pub filename: Option<String>,
    pub function: Option<String>,
    pub lineno: Option<i64>,
    pub colno: Option<i64>,
    pub abs_path: Option<String>,
    pub context_line: Option<String>,
    pub pre_context: Option<Vec<String>>,
    pub post_context: Option<Vec<String>>,
    pub in_app: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SentryInstallation {
    pub uuid: Option<String>,
}

// ============================================================================
// Datadog Webhook Types
// ============================================================================

/// Datadog webhook payload
#[derive(Debug, Deserialize)]
pub struct DatadogWebhookPayload {
    /// Event ID
    pub id: Option<String>,
    /// Event title
    pub title: Option<String>,
    /// Event text/message
    pub text: Option<String>,
    /// Alert type: error, warning, info, success
    pub alert_type: Option<String>,
    /// Date of the event (Unix timestamp)
    pub date: Option<i64>,
    /// Event priority
    pub priority: Option<String>,
    /// Host that generated the event
    pub host: Option<String>,
    /// Source type name
    pub source_type_name: Option<String>,
    /// Tags associated with the event
    pub tags: Option<Vec<String>>,
    /// Aggregation key for grouping
    pub aggregation_key: Option<String>,
    /// Alert status
    pub alert_status: Option<String>,
    /// Event type (for monitors)
    pub event_type: Option<String>,
    /// Monitor information (for monitor alerts)
    pub monitor: Option<DatadogMonitor>,
    /// Log data (for log-based alerts)
    pub logs: Option<Vec<DatadogLog>>,
    /// Snapshot URL (for graphs)
    pub snapshot_url: Option<String>,
    /// Link back to Datadog
    pub link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DatadogMonitor {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub monitor_type: Option<String>,
    pub query: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DatadogLog {
    pub id: Option<String>,
    pub message: Option<String>,
    pub status: Option<String>,
    pub timestamp: Option<String>,
    pub host: Option<String>,
    pub service: Option<String>,
    pub attributes: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

// ============================================================================
// Webhook Response
// ============================================================================

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub accepted: bool,
    pub message: String,
    pub incidents_created: i32,
    pub incidents_updated: i32,
}

// ============================================================================
// Sentry Webhook Handler
// ============================================================================

/// POST /webhooks/sentry
///
/// Accepts Sentry webhook payloads and converts them to CRISPR's IngestPayload format.
/// Supports issue and event webhooks with level=error or higher.
pub async fn sentry_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SentryWebhookPayload>,
) -> Result<Json<WebhookResponse>, AppError> {
    info!(
        action = %payload.action,
        "Received Sentry webhook"
    );

    // Verify the webhook signature if configured
    if let Some(expected_secret) = &state.config.webhooks.as_ref().and_then(|w| w.sentry_secret.as_ref()) {
        if let Some(signature) = headers.get("sentry-hook-signature") {
            let sig_str = signature.to_str().unwrap_or("");
            if !verify_sentry_signature(sig_str, expected_secret) {
                warn!("Invalid Sentry webhook signature");
                return Err(AppError::Unauthorized("Invalid webhook signature".into()));
            }
        }
    }

    let mut incidents_created = 0;
    let mut incidents_updated = 0;

    // Process based on action type
    match payload.action.as_str() {
        "created" | "triggered" | "resolved" | "assigned" | "ignored" => {
            // Handle issue webhooks
            if let Some(issue) = &payload.data.issue {
                let level = issue.level.as_deref().unwrap_or("error").to_uppercase();

                // Only process error-level issues
                if matches!(level.as_str(), "ERROR" | "FATAL" | "CRITICAL") {
                    let ingest_payload = sentry_issue_to_ingest(issue)?;
                    let response = super::ingest::http_ingest(
                        State(state.clone()),
                        Json(ingest_payload),
                    )
                    .await?;

                    if response.accepted {
                        if response.is_new.unwrap_or(false) {
                            incidents_created += 1;
                        } else {
                            incidents_updated += 1;
                        }
                    }
                }
            }

            // Handle event data if present
            if let Some(event) = &payload.data.event {
                let level = event.level.as_deref().unwrap_or("error").to_uppercase();

                if matches!(level.as_str(), "ERROR" | "FATAL" | "CRITICAL") {
                    if let Some(ingest_payload) = sentry_event_to_ingest(event, &payload.data.issue)? {
                        let response = super::ingest::http_ingest(
                            State(state.clone()),
                            Json(ingest_payload),
                        )
                        .await?;

                        if response.accepted {
                            if response.is_new.unwrap_or(false) {
                                incidents_created += 1;
                            } else {
                                incidents_updated += 1;
                            }
                        }
                    }
                }
            }
        }
        _ => {
            info!(action = %payload.action, "Ignoring Sentry webhook action");
        }
    }

    // Record metrics
    metrics::record_ingest();

    Ok(Json(WebhookResponse {
        accepted: true,
        message: format!(
            "Processed Sentry webhook: {} created, {} updated",
            incidents_created, incidents_updated
        ),
        incidents_created,
        incidents_updated,
    }))
}

/// Convert a Sentry issue to CRISPR IngestPayload
fn sentry_issue_to_ingest(issue: &SentryIssue) -> Result<IngestPayload, AppError> {
    // Build message from title and metadata
    let message = if let Some(metadata) = &issue.metadata {
        if let (Some(error_type), Some(value)) = (&metadata.error_type, &metadata.value) {
            format!("{}: {}", error_type, value)
        } else {
            issue.title.clone()
        }
    } else {
        issue.title.clone()
    };

    // Build stack trace info from metadata
    let stack_trace = issue.metadata.as_ref().and_then(|m| {
        if let (Some(filename), Some(function)) = (&m.filename, &m.function) {
            Some(format!("at {} in {}", function, filename))
        } else {
            None
        }
    });

    // Extract project info - use slug as the project identifier
    // Users should configure project mapping in CRISPR settings
    let project = issue.project.slug.clone().map(|s| format!("sentry/{}", s));

    Ok(IngestPayload {
        project,
        service_name: issue.project.name.clone(),
        kubernetes_namespace: None,
        kubernetes_pod: None,
        level: issue.level.clone().unwrap_or_else(|| "ERROR".to_string()).to_uppercase(),
        message,
        stack_trace,
        timestamp: None, // Sentry provides first_seen/last_seen as strings
        attributes: Some(serde_json::json!({
            "sentry_issue_id": issue.id,
            "sentry_project": issue.project.name,
            "sentry_platform": issue.platform,
            "sentry_culprit": issue.culprit,
            "sentry_count": issue.count,
            "sentry_status": issue.status,
        })),
    })
}

/// Convert a Sentry event to CRISPR IngestPayload
fn sentry_event_to_ingest(
    event: &SentryEvent,
    issue: &Option<SentryIssue>,
) -> Result<Option<IngestPayload>, AppError> {
    // Get message from event or exception
    let message = event.message.clone()
        .or_else(|| event.title.clone())
        .or_else(|| {
            event.exception.as_ref().and_then(|ex| {
                ex.values.as_ref().and_then(|values| {
                    values.first().and_then(|v| {
                        match (&v.exception_type, &v.value) {
                            (Some(t), Some(val)) => Some(format!("{}: {}", t, val)),
                            (Some(t), None) => Some(t.clone()),
                            (None, Some(val)) => Some(val.clone()),
                            _ => None,
                        }
                    })
                })
            })
        });

    let message = match message {
        Some(m) => m,
        None => return Ok(None), // Skip events without a meaningful message
    };

    // Extract stack trace from exception
    let stack_trace = event.exception.as_ref().and_then(|ex| {
        ex.values.as_ref().and_then(|values| {
            values.first().and_then(|v| {
                v.stacktrace.as_ref().and_then(|st| {
                    st.frames.as_ref().map(|frames| {
                        frames
                            .iter()
                            .filter(|f| f.in_app.unwrap_or(true))
                            .take(10) // Limit to top 10 frames
                            .map(|f| {
                                let location = match (&f.filename, &f.lineno) {
                                    (Some(file), Some(line)) => format!("{}:{}", file, line),
                                    (Some(file), None) => file.clone(),
                                    _ => "unknown".to_string(),
                                };
                                let function = f.function.as_deref().unwrap_or("<unknown>");
                                format!("  at {} ({})", function, location)
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                })
            })
        })
    });

    // Get project from issue or tags
    let project = issue.as_ref()
        .and_then(|i| i.project.slug.clone())
        .map(|s| format!("sentry/{}", s))
        .or_else(|| {
            event.tags.as_ref().and_then(|tags| {
                tags.iter()
                    .find(|t| t.key == "project")
                    .map(|t| format!("sentry/{}", t.value))
            })
        });

    // Parse timestamp
    let timestamp = event.timestamp.map(|ts| {
        DateTime::<Utc>::from_timestamp(ts as i64, ((ts % 1.0) * 1_000_000_000.0) as u32)
    }).flatten();

    // Build attributes from tags
    let mut attributes = serde_json::Map::new();
    if let Some(event_id) = &event.event_id {
        attributes.insert("sentry_event_id".to_string(), serde_json::json!(event_id));
    }
    if let Some(platform) = &event.platform {
        attributes.insert("sentry_platform".to_string(), serde_json::json!(platform));
    }
    if let Some(tags) = &event.tags {
        for tag in tags {
            attributes.insert(format!("sentry_tag_{}", tag.key), serde_json::json!(tag.value));
        }
    }

    Ok(Some(IngestPayload {
        project,
        service_name: None,
        kubernetes_namespace: None,
        kubernetes_pod: None,
        level: event.level.clone().unwrap_or_else(|| "ERROR".to_string()).to_uppercase(),
        message,
        stack_trace,
        timestamp,
        attributes: Some(serde_json::Value::Object(attributes)),
    }))
}

/// Verify Sentry webhook signature (placeholder - implement based on Sentry docs)
fn verify_sentry_signature(_signature: &str, _secret: &str) -> bool {
    // TODO: Implement actual signature verification
    // Sentry uses HMAC-SHA256 for webhook signatures
    true
}

// ============================================================================
// Datadog Webhook Handler
// ============================================================================

/// POST /webhooks/datadog
///
/// Accepts Datadog webhook payloads and converts them to CRISPR's IngestPayload format.
/// Supports error events and monitor alerts.
pub async fn datadog_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DatadogWebhookPayload>,
) -> Result<Json<WebhookResponse>, AppError> {
    info!(
        title = ?payload.title,
        alert_type = ?payload.alert_type,
        "Received Datadog webhook"
    );

    // Verify webhook signature if configured
    if let Some(expected_token) = &state.config.webhooks.as_ref().and_then(|w| w.datadog_token.as_ref()) {
        if let Some(token) = headers.get("x-datadog-token") {
            let token_str = token.to_str().unwrap_or("");
            if token_str != expected_token.as_str() {
                warn!("Invalid Datadog webhook token");
                return Err(AppError::Unauthorized("Invalid webhook token".into()));
            }
        }
    }

    let mut incidents_created = 0;
    let mut incidents_updated = 0;

    // Check if this is an error-level event
    let alert_type = payload.alert_type.as_deref().unwrap_or("info");
    let is_error = matches!(alert_type, "error")
        || payload.alert_status.as_deref() == Some("Alert")
        || payload.priority.as_deref() == Some("critical");

    if is_error {
        // Process the main event
        if let Some(ingest_payload) = datadog_event_to_ingest(&payload)? {
            let response = super::ingest::http_ingest(
                State(state.clone()),
                Json(ingest_payload),
            )
            .await?;

            if response.accepted {
                if response.is_new.unwrap_or(false) {
                    incidents_created += 1;
                } else {
                    incidents_updated += 1;
                }
            }
        }

        // Process any attached logs
        if let Some(logs) = &payload.logs {
            for log in logs {
                if log.status.as_deref() == Some("error") || log.status.as_deref() == Some("critical") {
                    if let Some(ingest_payload) = datadog_log_to_ingest(log, &payload)? {
                        let response = super::ingest::http_ingest(
                            State(state.clone()),
                            Json(ingest_payload),
                        )
                        .await?;

                        if response.accepted {
                            if response.is_new.unwrap_or(false) {
                                incidents_created += 1;
                            } else {
                                incidents_updated += 1;
                            }
                        }
                    }
                }
            }
        }
    } else {
        info!(alert_type = %alert_type, "Ignoring non-error Datadog event");
    }

    // Record metrics
    metrics::record_ingest();

    Ok(Json(WebhookResponse {
        accepted: true,
        message: format!(
            "Processed Datadog webhook: {} created, {} updated",
            incidents_created, incidents_updated
        ),
        incidents_created,
        incidents_updated,
    }))
}

/// Convert a Datadog event to CRISPR IngestPayload
fn datadog_event_to_ingest(payload: &DatadogWebhookPayload) -> Result<Option<IngestPayload>, AppError> {
    // Get message from title or text
    let message = payload.title.clone()
        .or_else(|| payload.text.clone())
        .or_else(|| {
            payload.monitor.as_ref().and_then(|m| {
                m.name.clone().or_else(|| m.message.clone())
            })
        });

    let message = match message {
        Some(m) => m,
        None => return Ok(None),
    };

    // Extract project from tags
    let project = extract_project_from_datadog_tags(payload.tags.as_ref());

    // Parse timestamp
    let timestamp = payload.date.map(|ts| {
        DateTime::<Utc>::from_timestamp(ts, 0)
    }).flatten();

    // Determine level
    let level = match payload.alert_type.as_deref() {
        Some("error") => "ERROR",
        Some("warning") => "WARNING",
        _ if payload.priority.as_deref() == Some("critical") => "CRITICAL",
        _ => "ERROR",
    }.to_string();

    // Build attributes
    let mut attributes = serde_json::Map::new();
    if let Some(id) = &payload.id {
        attributes.insert("datadog_event_id".to_string(), serde_json::json!(id));
    }
    if let Some(host) = &payload.host {
        attributes.insert("datadog_host".to_string(), serde_json::json!(host));
    }
    if let Some(source) = &payload.source_type_name {
        attributes.insert("datadog_source".to_string(), serde_json::json!(source));
    }
    if let Some(link) = &payload.link {
        attributes.insert("datadog_link".to_string(), serde_json::json!(link));
    }
    if let Some(agg_key) = &payload.aggregation_key {
        attributes.insert("datadog_aggregation_key".to_string(), serde_json::json!(agg_key));
    }
    if let Some(tags) = &payload.tags {
        attributes.insert("datadog_tags".to_string(), serde_json::json!(tags));
    }
    if let Some(monitor) = &payload.monitor {
        if let Some(id) = monitor.id {
            attributes.insert("datadog_monitor_id".to_string(), serde_json::json!(id));
        }
        if let Some(name) = &monitor.name {
            attributes.insert("datadog_monitor_name".to_string(), serde_json::json!(name));
        }
    }

    Ok(Some(IngestPayload {
        project,
        service_name: payload.host.clone(),
        kubernetes_namespace: None,
        kubernetes_pod: None,
        level,
        message,
        stack_trace: payload.text.clone(), // Text often contains stack traces
        timestamp,
        attributes: Some(serde_json::Value::Object(attributes)),
    }))
}

/// Convert a Datadog log entry to CRISPR IngestPayload
fn datadog_log_to_ingest(
    log: &DatadogLog,
    parent: &DatadogWebhookPayload,
) -> Result<Option<IngestPayload>, AppError> {
    let message = match &log.message {
        Some(m) => m.clone(),
        None => return Ok(None),
    };

    // Extract project from log tags or parent tags
    let project = extract_project_from_datadog_tags(log.tags.as_ref())
        .or_else(|| extract_project_from_datadog_tags(parent.tags.as_ref()));

    // Parse timestamp
    let timestamp = log.timestamp.as_ref().and_then(|ts| {
        DateTime::parse_from_rfc3339(ts).ok().map(|dt| dt.with_timezone(&Utc))
    });

    // Build attributes
    let mut attributes = serde_json::Map::new();
    if let Some(id) = &log.id {
        attributes.insert("datadog_log_id".to_string(), serde_json::json!(id));
    }
    if let Some(host) = &log.host {
        attributes.insert("datadog_host".to_string(), serde_json::json!(host));
    }
    if let Some(service) = &log.service {
        attributes.insert("datadog_service".to_string(), serde_json::json!(service));
    }
    if let Some(log_attrs) = &log.attributes {
        attributes.insert("datadog_attributes".to_string(), log_attrs.clone());
    }
    if let Some(tags) = &log.tags {
        attributes.insert("datadog_tags".to_string(), serde_json::json!(tags));
    }

    let level = match log.status.as_deref() {
        Some("critical") => "CRITICAL",
        Some("error") => "ERROR",
        Some("warn") | Some("warning") => "WARNING",
        _ => "ERROR",
    }.to_string();

    Ok(Some(IngestPayload {
        project,
        service_name: log.service.clone().or_else(|| log.host.clone()),
        kubernetes_namespace: None,
        kubernetes_pod: None,
        level,
        message,
        stack_trace: None,
        timestamp,
        attributes: Some(serde_json::Value::Object(attributes)),
    }))
}

/// Extract project identifier from Datadog tags
/// Looks for tags like "project:owner/repo", "repo:owner/repo", "service:name"
fn extract_project_from_datadog_tags(tags: Option<&Vec<String>>) -> Option<String> {
    tags.and_then(|tags| {
        // First look for explicit project tag
        for tag in tags {
            if tag.starts_with("project:") {
                return Some(tag.trim_start_matches("project:").to_string());
            }
            if tag.starts_with("repo:") {
                return Some(tag.trim_start_matches("repo:").to_string());
            }
        }
        // Fall back to service tag
        for tag in tags {
            if tag.starts_with("service:") {
                let service = tag.trim_start_matches("service:");
                // Try to convert service name to project format
                if service.contains('/') {
                    return Some(service.to_string());
                }
                if service.contains('-') {
                    // Convert first hyphen to slash (common pattern)
                    if let Some(idx) = service.find('-') {
                        let (owner, repo) = service.split_at(idx);
                        return Some(format!("{}/{}", owner, &repo[1..]));
                    }
                }
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_project_from_datadog_tags() {
        let tags = vec![
            "env:production".to_string(),
            "project:acme/webapp".to_string(),
            "version:1.0".to_string(),
        ];
        assert_eq!(
            extract_project_from_datadog_tags(Some(&tags)),
            Some("acme/webapp".to_string())
        );

        let tags2 = vec![
            "env:production".to_string(),
            "service:acme-webapp".to_string(),
        ];
        assert_eq!(
            extract_project_from_datadog_tags(Some(&tags2)),
            Some("acme/webapp".to_string())
        );

        let tags3 = vec!["env:production".to_string()];
        assert_eq!(extract_project_from_datadog_tags(Some(&tags3)), None);

        assert_eq!(extract_project_from_datadog_tags(None), None);
    }

    #[test]
    fn test_sentry_issue_to_ingest() {
        let issue = SentryIssue {
            id: "123".to_string(),
            title: "NullPointerException in UserService".to_string(),
            culprit: Some("com.example.UserService.getUser".to_string()),
            level: Some("error".to_string()),
            status: Some("unresolved".to_string()),
            platform: Some("java".to_string()),
            project: SentryProject {
                id: Some("1".to_string()),
                name: Some("My Project".to_string()),
                slug: Some("my-project".to_string()),
            },
            metadata: Some(SentryMetadata {
                error_type: Some("NullPointerException".to_string()),
                value: Some("User not found".to_string()),
                filename: Some("UserService.java".to_string()),
                function: Some("getUser".to_string()),
            }),
            first_seen: None,
            last_seen: None,
            count: Some(5),
        };

        let payload = sentry_issue_to_ingest(&issue).unwrap();
        assert_eq!(payload.level, "ERROR");
        assert_eq!(payload.message, "NullPointerException: User not found");
        assert_eq!(payload.project, Some("sentry/my-project".to_string()));
        assert!(payload.stack_trace.is_some());
    }

    #[test]
    fn test_datadog_event_to_ingest() {
        let payload = DatadogWebhookPayload {
            id: Some("event-123".to_string()),
            title: Some("High error rate on API".to_string()),
            text: Some("Error rate exceeded 5%".to_string()),
            alert_type: Some("error".to_string()),
            date: Some(1700000000),
            priority: None,
            host: Some("api-server-1".to_string()),
            source_type_name: Some("monitor".to_string()),
            tags: Some(vec!["project:acme/api".to_string(), "env:prod".to_string()]),
            aggregation_key: None,
            alert_status: None,
            event_type: None,
            monitor: None,
            logs: None,
            snapshot_url: None,
            link: Some("https://app.datadoghq.com/event/123".to_string()),
        };

        let ingest = datadog_event_to_ingest(&payload).unwrap().unwrap();
        assert_eq!(ingest.level, "ERROR");
        assert_eq!(ingest.message, "High error rate on API");
        assert_eq!(ingest.project, Some("acme/api".to_string()));
        assert!(ingest.attributes.is_some());
    }
}

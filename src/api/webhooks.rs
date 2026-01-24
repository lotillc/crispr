//! Webhook handlers for external error tracking services.
//!
//! Supports ingestion from:
//! - Sentry: Issue and error event webhooks
//! - Datadog: Event and error webhooks
//! - Grafana: Alert notifications from Grafana Alerting

use axum::{
    body::Bytes,
    extract::State,
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{info, warn};

use super::ingest::IngestPayload;
use super::{metrics, AppState};
use crate::error::AppError;

/// Cached webhook settings from database
#[derive(Debug, Clone, Default)]
struct WebhookSecrets {
    sentry_enabled: bool,
    sentry_secret: Option<String>,
    datadog_enabled: bool,
    datadog_token: Option<String>,
    grafana_enabled: bool,
    grafana_token: Option<String>,
}

/// Row type for webhook settings queries (runtime checking)
#[derive(sqlx::FromRow)]
struct WebhookSettingsRow {
    sentry_enabled: Option<bool>,
    sentry_secret: Option<String>,
    datadog_enabled: Option<bool>,
    datadog_token: Option<String>,
    grafana_enabled: Option<bool>,
    grafana_token: Option<String>,
}

/// Get webhook secrets from database (with fallback to config)
async fn get_webhook_secrets(state: &AppState) -> WebhookSecrets {
    // Try to get from database first
    let result: Result<Option<WebhookSettingsRow>, _> = sqlx::query_as(
        r#"
        SELECT sentry_enabled, sentry_secret,
               datadog_enabled, datadog_token,
               grafana_enabled, grafana_token
        FROM webhook_settings
        LIMIT 1
        "#
    )
    .fetch_optional(&state.db)
    .await;

    if let Ok(Some(row)) = result {
        return WebhookSecrets {
            sentry_enabled: row.sentry_enabled.unwrap_or(false),
            sentry_secret: row.sentry_secret,
            datadog_enabled: row.datadog_enabled.unwrap_or(false),
            datadog_token: row.datadog_token,
            grafana_enabled: row.grafana_enabled.unwrap_or(false),
            grafana_token: row.grafana_token,
        };
    }

    // Fall back to config file
    if let Some(webhooks) = &state.config.webhooks {
        WebhookSecrets {
            sentry_enabled: webhooks.sentry_secret.is_some(),
            sentry_secret: webhooks.sentry_secret.clone(),
            datadog_enabled: webhooks.datadog_token.is_some(),
            datadog_token: webhooks.datadog_token.clone(),
            grafana_enabled: webhooks.grafana_token.is_some(),
            grafana_token: webhooks.grafana_token.clone(),
        }
    } else {
        WebhookSecrets::default()
    }
}

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
    body: Bytes,
) -> Result<Json<WebhookResponse>, AppError> {
    // Get webhook settings from database (with fallback to config)
    let secrets = get_webhook_secrets(&state).await;

    // Verify the webhook signature if a secret is configured
    if let Some(secret) = &secrets.sentry_secret {
        let signature = headers
            .get("sentry-hook-signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                warn!("Missing Sentry webhook signature header");
                AppError::Unauthorized("Missing webhook signature".into())
            })?;

        if !verify_sentry_signature(&body, signature, secret) {
            warn!("Invalid Sentry webhook signature");
            return Err(AppError::Unauthorized("Invalid webhook signature".into()));
        }
    }

    // Parse the JSON payload after signature verification
    let payload: SentryWebhookPayload = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON payload: {}", e)))?;

    info!(
        action = %payload.action,
        "Received Sentry webhook"
    );

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

/// Verify Sentry webhook signature using HMAC-SHA256.
///
/// Sentry sends the signature in the `Sentry-Hook-Signature` header.
/// The signature is computed as HMAC-SHA256(client_secret, request_body)
/// and sent as a hex-encoded string.
fn verify_sentry_signature(body: &[u8], signature: &str, secret: &str) -> bool {
    // Create HMAC-SHA256 instance with the secret
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => {
            warn!("Failed to create HMAC instance - invalid key length");
            return false;
        }
    };

    // Update with the request body
    mac.update(body);

    // Decode the expected signature from hex
    let expected_bytes = match hex::decode(signature) {
        Ok(bytes) => bytes,
        Err(_) => {
            warn!("Failed to decode signature as hex");
            return false;
        }
    };

    // Verify using constant-time comparison to prevent timing attacks
    mac.verify_slice(&expected_bytes).is_ok()
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

    // Get webhook settings from database (with fallback to config)
    let secrets = get_webhook_secrets(&state).await;

    // Verify webhook token if configured
    if let Some(expected_token) = &secrets.datadog_token {
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

// ============================================================================
// Grafana Webhook Types
// ============================================================================

/// Grafana alert notification payload (Grafana Alerting / Unified Alerting)
#[derive(Debug, Deserialize)]
pub struct GrafanaWebhookPayload {
    /// Alert receiver name
    pub receiver: Option<String>,
    /// Status: "firing" or "resolved"
    pub status: Option<String>,
    /// List of alerts
    pub alerts: Vec<GrafanaAlert>,
    /// Group labels
    #[serde(rename = "groupLabels")]
    pub group_labels: Option<serde_json::Value>,
    /// Common labels shared by all alerts
    #[serde(rename = "commonLabels")]
    pub common_labels: Option<serde_json::Value>,
    /// Common annotations shared by all alerts
    #[serde(rename = "commonAnnotations")]
    pub common_annotations: Option<GrafanaAnnotations>,
    /// External URL to Grafana
    #[serde(rename = "externalURL")]
    pub external_url: Option<String>,
    /// Version of the webhook payload
    pub version: Option<String>,
    /// Group key for deduplication
    #[serde(rename = "groupKey")]
    pub group_key: Option<String>,
    /// Truncated alerts count
    #[serde(rename = "truncatedAlerts")]
    pub truncated_alerts: Option<i32>,
    /// Organization ID
    #[serde(rename = "orgId")]
    pub org_id: Option<i64>,
    /// Title (legacy format)
    pub title: Option<String>,
    /// State (legacy format)
    pub state: Option<String>,
    /// Message (legacy format)
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GrafanaAlert {
    /// Status: "firing" or "resolved"
    pub status: Option<String>,
    /// Alert labels (includes alertname, severity, etc.)
    pub labels: Option<serde_json::Value>,
    /// Alert annotations (includes summary, description, etc.)
    pub annotations: Option<GrafanaAnnotations>,
    /// Start time of the alert
    #[serde(rename = "startsAt")]
    pub starts_at: Option<String>,
    /// End time of the alert (for resolved)
    #[serde(rename = "endsAt")]
    pub ends_at: Option<String>,
    /// URL to the alert in Grafana
    #[serde(rename = "generatorURL")]
    pub generator_url: Option<String>,
    /// Silence URL
    #[serde(rename = "silenceURL")]
    pub silence_url: Option<String>,
    /// Dashboard URL
    #[serde(rename = "dashboardURL")]
    pub dashboard_url: Option<String>,
    /// Panel URL
    #[serde(rename = "panelURL")]
    pub panel_url: Option<String>,
    /// Fingerprint for deduplication
    pub fingerprint: Option<String>,
    /// Values that triggered the alert
    pub values: Option<serde_json::Value>,
    /// Value string representation
    #[serde(rename = "valueString")]
    pub value_string: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GrafanaAnnotations {
    /// Alert summary
    pub summary: Option<String>,
    /// Alert description
    pub description: Option<String>,
    /// Runbook URL
    pub runbook_url: Option<String>,
}

// ============================================================================
// Grafana Webhook Handler
// ============================================================================

/// POST /webhooks/grafana
///
/// Accepts Grafana alert notification webhooks and converts them to CRISPR incidents.
/// Supports both legacy Grafana alerting and unified alerting formats.
pub async fn grafana_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GrafanaWebhookPayload>,
) -> Result<Json<WebhookResponse>, AppError> {
    info!(
        status = ?payload.status,
        alerts_count = payload.alerts.len(),
        "Received Grafana webhook"
    );

    // Get webhook settings from database (with fallback to config)
    let secrets = get_webhook_secrets(&state).await;

    // Verify webhook token if configured
    if let Some(expected_token) = &secrets.grafana_token {
        // Check Authorization header (Bearer token)
        let auth_valid = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|auth| {
                auth.strip_prefix("Bearer ")
                    .map(|token| token == expected_token.as_str())
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        // Also check X-Grafana-Token header as fallback
        let token_valid = headers
            .get("x-grafana-token")
            .and_then(|v| v.to_str().ok())
            .map(|token| token == expected_token.as_str())
            .unwrap_or(false);

        if !auth_valid && !token_valid {
            warn!("Invalid or missing Grafana webhook token");
            return Err(AppError::Unauthorized("Invalid webhook token".into()));
        }
    }

    let mut incidents_created = 0;
    let mut incidents_updated = 0;

    // Process each alert
    for alert in &payload.alerts {
        // Only process firing alerts (not resolved)
        let status = alert.status.as_deref().unwrap_or("firing");
        if status != "firing" {
            continue;
        }

        // Check severity - only process error/critical/warning
        let severity = alert.labels.as_ref()
            .and_then(|l| l.get("severity"))
            .and_then(|v| v.as_str())
            .unwrap_or("error");

        if !matches!(severity, "error" | "critical" | "warning" | "high") {
            continue;
        }

        if let Some(ingest_payload) = grafana_alert_to_ingest(alert, &payload)? {
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

    // Record metrics
    metrics::record_ingest();

    Ok(Json(WebhookResponse {
        accepted: true,
        message: format!(
            "Processed Grafana webhook: {} created, {} updated",
            incidents_created, incidents_updated
        ),
        incidents_created,
        incidents_updated,
    }))
}

/// Convert a Grafana alert to CRISPR IngestPayload
fn grafana_alert_to_ingest(
    alert: &GrafanaAlert,
    parent: &GrafanaWebhookPayload,
) -> Result<Option<IngestPayload>, AppError> {
    // Get alert name from labels
    let alertname = alert.labels.as_ref()
        .and_then(|l| l.get("alertname"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Alert");

    // Get message from annotations (summary > description) or alert name
    let message = alert.annotations.as_ref()
        .and_then(|a| a.summary.clone().or_else(|| a.description.clone()))
        .or_else(|| parent.common_annotations.as_ref()
            .and_then(|a| a.summary.clone().or_else(|| a.description.clone())))
        .or_else(|| parent.message.clone())
        .unwrap_or_else(|| alertname.to_string());

    // Extract project from labels
    let project = extract_project_from_grafana_labels(alert.labels.as_ref())
        .or_else(|| extract_project_from_grafana_labels(parent.common_labels.as_ref()));

    // Parse timestamp
    let timestamp = alert.starts_at.as_ref().and_then(|ts| {
        DateTime::parse_from_rfc3339(ts).ok().map(|dt| dt.with_timezone(&Utc))
    });

    // Determine level from severity label
    let level = alert.labels.as_ref()
        .and_then(|l| l.get("severity"))
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "critical" | "high" => "CRITICAL",
            "error" => "ERROR",
            "warning" | "warn" => "WARNING",
            _ => "ERROR",
        })
        .unwrap_or("ERROR")
        .to_string();

    // Build attributes
    let mut attributes = serde_json::Map::new();
    attributes.insert("grafana_alertname".to_string(), serde_json::json!(alertname));

    if let Some(fingerprint) = &alert.fingerprint {
        attributes.insert("grafana_fingerprint".to_string(), serde_json::json!(fingerprint));
    }
    if let Some(url) = &alert.generator_url {
        attributes.insert("grafana_url".to_string(), serde_json::json!(url));
    }
    if let Some(url) = &alert.dashboard_url {
        attributes.insert("grafana_dashboard_url".to_string(), serde_json::json!(url));
    }
    if let Some(url) = &alert.panel_url {
        attributes.insert("grafana_panel_url".to_string(), serde_json::json!(url));
    }
    if let Some(values) = &alert.values {
        attributes.insert("grafana_values".to_string(), values.clone());
    }
    if let Some(value_string) = &alert.value_string {
        attributes.insert("grafana_value_string".to_string(), serde_json::json!(value_string));
    }
    if let Some(labels) = &alert.labels {
        attributes.insert("grafana_labels".to_string(), labels.clone());
    }
    if let Some(receiver) = &parent.receiver {
        attributes.insert("grafana_receiver".to_string(), serde_json::json!(receiver));
    }

    // Use value_string as additional context in the message if available
    let full_message = if let Some(value_string) = &alert.value_string {
        format!("{}\n\nValues: {}", message, value_string)
    } else {
        message
    };

    // Get service name from labels
    let service_name = alert.labels.as_ref()
        .and_then(|l| l.get("service").or_else(|| l.get("job")).or_else(|| l.get("instance")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(Some(IngestPayload {
        project,
        service_name,
        kubernetes_namespace: alert.labels.as_ref()
            .and_then(|l| l.get("namespace"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        kubernetes_pod: alert.labels.as_ref()
            .and_then(|l| l.get("pod"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        level,
        message: full_message,
        stack_trace: None, // Grafana alerts typically don't include stack traces
        timestamp,
        attributes: Some(serde_json::Value::Object(attributes)),
    }))
}

/// Extract project identifier from Grafana labels
/// Looks for labels like "project", "repo", "repository", or "service"
fn extract_project_from_grafana_labels(labels: Option<&serde_json::Value>) -> Option<String> {
    labels.and_then(|l| {
        // Check explicit project labels first
        if let Some(project) = l.get("project").and_then(|v| v.as_str()) {
            return Some(project.to_string());
        }
        if let Some(repo) = l.get("repo").and_then(|v| v.as_str()) {
            return Some(repo.to_string());
        }
        if let Some(repo) = l.get("repository").and_then(|v| v.as_str()) {
            return Some(repo.to_string());
        }
        // Try to construct from namespace/service
        if let (Some(namespace), Some(service)) = (
            l.get("namespace").and_then(|v| v.as_str()),
            l.get("service").and_then(|v| v.as_str())
        ) {
            return Some(format!("{}/{}", namespace, service));
        }
        // Fall back to service with hyphen conversion
        if let Some(service) = l.get("service").and_then(|v| v.as_str()) {
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

    #[test]
    fn test_verify_sentry_signature_valid() {
        // Test with a known HMAC-SHA256 value
        let secret = "test-secret-key";
        let body = b"test webhook body";

        // Compute expected signature: HMAC-SHA256(secret, body)
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let expected_signature = hex::encode(mac.finalize().into_bytes());

        assert!(verify_sentry_signature(body, &expected_signature, secret));
    }

    #[test]
    fn test_verify_sentry_signature_invalid() {
        let secret = "test-secret-key";
        let body = b"test webhook body";
        let wrong_signature = "0000000000000000000000000000000000000000000000000000000000000000";

        assert!(!verify_sentry_signature(body, wrong_signature, secret));
    }

    #[test]
    fn test_verify_sentry_signature_wrong_secret() {
        let secret = "correct-secret";
        let wrong_secret = "wrong-secret";
        let body = b"test webhook body";

        // Compute signature with correct secret
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        // Verify with wrong secret should fail
        assert!(!verify_sentry_signature(body, &signature, wrong_secret));
    }

    #[test]
    fn test_verify_sentry_signature_invalid_hex() {
        let secret = "test-secret-key";
        let body = b"test webhook body";
        let invalid_hex = "not-valid-hex-string";

        assert!(!verify_sentry_signature(body, invalid_hex, secret));
    }

    #[test]
    fn test_verify_sentry_signature_tampered_body() {
        let secret = "test-secret-key";
        let body = b"original body";
        let tampered_body = b"tampered body";

        // Compute signature for original body
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());

        // Verify with tampered body should fail
        assert!(!verify_sentry_signature(tampered_body, &signature, secret));
    }

    #[test]
    fn test_extract_project_from_grafana_labels() {
        // Explicit project label
        let labels = serde_json::json!({
            "project": "acme/webapp",
            "severity": "error"
        });
        assert_eq!(
            extract_project_from_grafana_labels(Some(&labels)),
            Some("acme/webapp".to_string())
        );

        // Namespace + service
        let labels2 = serde_json::json!({
            "namespace": "production",
            "service": "api-gateway",
            "severity": "critical"
        });
        assert_eq!(
            extract_project_from_grafana_labels(Some(&labels2)),
            Some("production/api-gateway".to_string())
        );

        // Service with hyphen conversion
        let labels3 = serde_json::json!({
            "service": "acme-webapp",
            "severity": "error"
        });
        assert_eq!(
            extract_project_from_grafana_labels(Some(&labels3)),
            Some("acme/webapp".to_string())
        );

        // No project info
        let labels4 = serde_json::json!({
            "severity": "error",
            "alertname": "HighErrorRate"
        });
        assert_eq!(extract_project_from_grafana_labels(Some(&labels4)), None);

        assert_eq!(extract_project_from_grafana_labels(None), None);
    }

    #[test]
    fn test_grafana_alert_to_ingest() {
        let alert = GrafanaAlert {
            status: Some("firing".to_string()),
            labels: Some(serde_json::json!({
                "alertname": "HighErrorRate",
                "severity": "critical",
                "service": "api-gateway",
                "namespace": "production"
            })),
            annotations: Some(GrafanaAnnotations {
                summary: Some("Error rate exceeded threshold".to_string()),
                description: Some("The API gateway error rate is above 5%".to_string()),
                runbook_url: None,
            }),
            starts_at: Some("2024-01-15T10:00:00Z".to_string()),
            ends_at: None,
            generator_url: Some("https://grafana.example.com/alerting/1".to_string()),
            silence_url: None,
            dashboard_url: Some("https://grafana.example.com/d/abc123".to_string()),
            panel_url: None,
            fingerprint: Some("abc123def456".to_string()),
            values: Some(serde_json::json!({"error_rate": 0.08})),
            value_string: Some("error_rate = 8%".to_string()),
        };

        let parent = GrafanaWebhookPayload {
            receiver: Some("crispr".to_string()),
            status: Some("firing".to_string()),
            alerts: vec![],
            group_labels: None,
            common_labels: None,
            common_annotations: None,
            external_url: None,
            version: None,
            group_key: None,
            truncated_alerts: None,
            org_id: None,
            title: None,
            state: None,
            message: None,
        };

        let ingest = grafana_alert_to_ingest(&alert, &parent).unwrap().unwrap();
        assert_eq!(ingest.level, "CRITICAL");
        assert!(ingest.message.contains("Error rate exceeded threshold"));
        assert!(ingest.message.contains("error_rate = 8%"));
        assert_eq!(ingest.project, Some("production/api-gateway".to_string()));
        assert!(ingest.attributes.is_some());

        let attrs = ingest.attributes.unwrap();
        assert_eq!(attrs.get("grafana_alertname").unwrap(), "HighErrorRate");
        assert_eq!(attrs.get("grafana_fingerprint").unwrap(), "abc123def456");
    }
}

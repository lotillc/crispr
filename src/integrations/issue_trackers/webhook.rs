//! Generic Webhook Integration
//!
//! Sends webhook notifications on incident state changes.
//! Allows integration with any external system (Linear, Asana, custom tools).

use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use serde::Serialize;
use sqlx::PgPool;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{IncidentStateChange, ProjectIssueTracker};

/// Webhook event types
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    IncidentDetected,
    IncidentTriaging,
    IncidentFixing,
    IncidentPrOpen,
    IncidentPrMerged,
    IncidentVerifying,
    IncidentVerified,
    IncidentFailed,
    IncidentStateChanged,
}

impl WebhookEventType {
    fn from_state(state: &str) -> Self {
        match state {
            "detected" => Self::IncidentDetected,
            "triaging" => Self::IncidentTriaging,
            "pending_fix" | "fixing" | "pr_opening" => Self::IncidentFixing,
            "pr_open" => Self::IncidentPrOpen,
            "pr_merged" => Self::IncidentPrMerged,
            "verifying" | "pending_verification" => Self::IncidentVerifying,
            "verified" => Self::IncidentVerified,
            "unfixable" | "fix_failed" | "pr_closed" | "max_attempts" => Self::IncidentFailed,
            _ => Self::IncidentStateChanged,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::IncidentDetected => "incident.detected",
            Self::IncidentTriaging => "incident.triaging",
            Self::IncidentFixing => "incident.fixing",
            Self::IncidentPrOpen => "incident.pr_open",
            Self::IncidentPrMerged => "incident.pr_merged",
            Self::IncidentVerifying => "incident.verifying",
            Self::IncidentVerified => "incident.verified",
            Self::IncidentFailed => "incident.failed",
            Self::IncidentStateChanged => "incident.state_changed",
        }
    }
}

/// Webhook payload sent to external systems
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    /// Event type (e.g., "incident.detected", "incident.verified")
    pub event: String,

    /// Timestamp of the event
    pub timestamp: String,

    /// Incident details
    pub incident: WebhookIncident,

    /// Repository information
    pub repository: WebhookRepository,

    /// CRISPR instance identifier (for multi-instance setups)
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookIncident {
    pub id: String,
    pub fingerprint: String,
    pub title: Option<String>,
    pub state: String,
    pub previous_state: Option<String>,
    pub fix_attempts: i32,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub verification_duration_hours: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct WebhookRepository {
    pub owner: String,
    pub name: String,
    pub full_name: String,
}

impl WebhookPayload {
    fn from_event(event: &IncidentStateChange) -> Self {
        let event_type = WebhookEventType::from_state(&event.new_state);

        Self {
            event: event_type.as_str().to_string(),
            timestamp: event.occurred_at.to_rfc3339(),
            incident: WebhookIncident {
                id: event.incident_id.to_string(),
                fingerprint: event.fingerprint.clone(),
                title: event.title.clone(),
                state: event.new_state.clone(),
                previous_state: event.old_state.clone(),
                fix_attempts: event.fix_attempts,
                pr_url: event.pr_url.clone(),
                pr_number: event.pr_number,
                verification_duration_hours: event.verification_duration_hours,
            },
            repository: WebhookRepository {
                owner: event.github_owner.clone(),
                name: event.github_repo.clone(),
                full_name: format!("{}/{}", event.github_owner, event.github_repo),
            },
            source: "crispr".to_string(),
        }
    }
}

/// Check if we should send webhook for this event
fn should_send(config: &ProjectIssueTracker, state: &str) -> bool {
    let events = config.webhook_events.as_ref();

    match events {
        None => true, // No filter = send all
        Some(events) => {
            if events.contains(&"all".to_string()) {
                return true;
            }

            let event_type = WebhookEventType::from_state(state);
            events.contains(&event_type.as_str().to_string())
                || events.contains(&state.to_string())
        }
    }
}

/// Record webhook delivery attempt
async fn record_delivery(
    db: &PgPool,
    project_id: Uuid,
    incident_id: Uuid,
    webhook_url: &str,
    event_type: &str,
    payload: &WebhookPayload,
    response_status: Option<i32>,
    response_body: Option<&str>,
    error: Option<&str>,
) -> Result<Uuid> {
    let status = if error.is_some() {
        "failed"
    } else if response_status.map(|s| s >= 200 && s < 300).unwrap_or(false) {
        "success"
    } else {
        "failed"
    };

    let id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO webhook_deliveries
            (id, project_id, incident_id, webhook_url, event_type, payload,
             response_status, response_body, status, error_message, delivered_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
        id,
        project_id,
        incident_id,
        webhook_url,
        event_type,
        serde_json::to_value(payload)?,
        response_status,
        response_body,
        status,
        error,
        if status == "success" { Some(Utc::now()) } else { None }
    )
    .execute(db)
    .await?;

    Ok(id)
}

/// Send webhook notification
pub async fn send_webhook(
    db: &PgPool,
    config: &ProjectIssueTracker,
    event: &IncidentStateChange,
) -> Result<()> {
    let webhook_url = config
        .webhook_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Webhook URL not configured"))?;

    // Check if we should send for this event type
    if !should_send(config, &event.new_state) {
        debug!(
            state = %event.new_state,
            "Skipping webhook - event not in filter"
        );
        return Ok(());
    }

    let payload = WebhookPayload::from_event(event);
    let event_type = WebhookEventType::from_state(&event.new_state).as_str();

    let client = Client::builder()
        .user_agent("CRISPR-Webhook/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut request = client
        .post(webhook_url)
        .header("Content-Type", "application/json")
        .header("X-CRISPR-Event", event_type)
        .header("X-CRISPR-Delivery", Uuid::new_v4().to_string());

    // Add custom headers from config
    if let Some(headers) = &config.webhook_headers {
        if let Some(obj) = headers.as_object() {
            for (key, value) in obj {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }
    }

    let response = request.json(&payload).send().await;

    match response {
        Ok(resp) => {
            let status = resp.status().as_u16() as i32;
            let body = resp.text().await.unwrap_or_default();

            if status >= 200 && status < 300 {
                info!(
                    incident_id = %event.incident_id,
                    webhook_url = %webhook_url,
                    status = status,
                    "Webhook delivered successfully"
                );

                record_delivery(
                    db,
                    event.project_id,
                    event.incident_id,
                    webhook_url,
                    event_type,
                    &payload,
                    Some(status),
                    Some(&body),
                    None,
                )
                .await?;
            } else {
                warn!(
                    incident_id = %event.incident_id,
                    webhook_url = %webhook_url,
                    status = status,
                    body = %body,
                    "Webhook returned non-success status"
                );

                record_delivery(
                    db,
                    event.project_id,
                    event.incident_id,
                    webhook_url,
                    event_type,
                    &payload,
                    Some(status),
                    Some(&body),
                    Some(&format!("HTTP {}", status)),
                )
                .await?;
            }
        }
        Err(e) => {
            error!(
                incident_id = %event.incident_id,
                webhook_url = %webhook_url,
                error = %e,
                "Webhook delivery failed"
            );

            record_delivery(
                db,
                event.project_id,
                event.incident_id,
                webhook_url,
                event_type,
                &payload,
                None,
                None,
                Some(&e.to_string()),
            )
            .await?;

            return Err(e.into());
        }
    }

    Ok(())
}

/// Retry failed webhook deliveries
pub async fn retry_failed_webhooks(db: &PgPool) -> Result<()> {
    // Find deliveries that need retry (failed within last hour, less than 3 attempts)
    let failed = sqlx::query!(
        r#"
        SELECT id, project_id, incident_id, webhook_url, event_type, payload, attempt_number
        FROM webhook_deliveries
        WHERE status = 'failed'
        AND attempt_number < 3
        AND created_at > NOW() - INTERVAL '1 hour'
        ORDER BY created_at ASC
        LIMIT 100
        "#
    )
    .fetch_all(db)
    .await?;

    if failed.is_empty() {
        return Ok(());
    }

    info!(count = failed.len(), "Retrying failed webhook deliveries");

    let client = Client::builder()
        .user_agent("CRISPR-Webhook/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    for delivery in failed {
        let response = client
            .post(&delivery.webhook_url)
            .header("Content-Type", "application/json")
            .header("X-CRISPR-Event", &delivery.event_type)
            .header("X-CRISPR-Delivery", delivery.id.to_string())
            .header("X-CRISPR-Retry", delivery.attempt_number.unwrap_or(1).to_string())
            .json(&delivery.payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let body = resp.text().await.unwrap_or_default();

                if status >= 200 && status < 300 {
                    sqlx::query!(
                        r#"
                        UPDATE webhook_deliveries
                        SET status = 'success',
                            response_status = $1,
                            response_body = $2,
                            delivered_at = NOW(),
                            attempt_number = attempt_number + 1
                        WHERE id = $3
                        "#,
                        status,
                        body,
                        delivery.id
                    )
                    .execute(db)
                    .await?;

                    info!(
                        delivery_id = %delivery.id,
                        attempt = delivery.attempt_number.unwrap_or(1) + 1,
                        "Webhook retry succeeded"
                    );
                } else {
                    sqlx::query!(
                        r#"
                        UPDATE webhook_deliveries
                        SET response_status = $1,
                            response_body = $2,
                            error_message = $3,
                            attempt_number = attempt_number + 1
                        WHERE id = $4
                        "#,
                        status,
                        body,
                        format!("HTTP {}", status),
                        delivery.id
                    )
                    .execute(db)
                    .await?;
                }
            }
            Err(e) => {
                sqlx::query!(
                    r#"
                    UPDATE webhook_deliveries
                    SET error_message = $1, attempt_number = attempt_number + 1
                    WHERE id = $2
                    "#,
                    e.to_string(),
                    delivery.id
                )
                .execute(db)
                .await?;
            }
        }
    }

    Ok(())
}

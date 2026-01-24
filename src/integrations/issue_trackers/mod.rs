//! Issue Tracker Integrations
//!
//! Provides integration with external issue trackers to sync incident state:
//! - GitHub Issues: Create/update issues in the same repo
//! - Jira: Create/transition issues with configurable workflows
//! - Linear: Modern issue tracking with API
//! - Webhooks: Generic integration for any system

pub mod github_issues;
pub mod jira;
pub mod linear;
pub mod webhook;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Issue tracker provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueTrackerProvider {
    Github,
    Jira,
    Linear,
    Webhook,
}

impl std::fmt::Display for IssueTrackerProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueTrackerProvider::Github => write!(f, "github"),
            IssueTrackerProvider::Jira => write!(f, "jira"),
            IssueTrackerProvider::Linear => write!(f, "linear"),
            IssueTrackerProvider::Webhook => write!(f, "webhook"),
        }
    }
}

impl std::str::FromStr for IssueTrackerProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "github" => Ok(IssueTrackerProvider::Github),
            "jira" => Ok(IssueTrackerProvider::Jira),
            "linear" => Ok(IssueTrackerProvider::Linear),
            "webhook" => Ok(IssueTrackerProvider::Webhook),
            _ => Err(anyhow::anyhow!("Unknown issue tracker provider: {}", s)),
        }
    }
}

/// Issue tracker configuration for a project
#[derive(Debug, Clone, FromRow)]
pub struct ProjectIssueTracker {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub enabled: bool,

    // GitHub config
    pub github_labels: Option<Vec<String>>,
    pub github_assignees: Option<Vec<String>>,

    // Jira config
    pub jira_site_url: Option<String>,
    pub jira_project_key: Option<String>,
    pub jira_issue_type: Option<String>,
    pub jira_priority: Option<String>,

    // Status mapping
    pub status_mapping: Option<serde_json::Value>,

    // Webhook config
    pub webhook_url: Option<String>,
    pub webhook_headers: Option<serde_json::Value>,
    pub webhook_events: Option<Vec<String>>,

    // Linear config
    pub linear_team_id: Option<String>,
    pub linear_project_id: Option<String>,

    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// API response for issue tracker config
#[derive(Debug, Serialize)]
pub struct IssueTrackerConfigResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_assignees: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_site_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_project_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_issue_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_mapping: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_events: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_team_id: Option<String>,

    pub created_at: DateTime<Utc>,
}

impl From<ProjectIssueTracker> for IssueTrackerConfigResponse {
    fn from(t: ProjectIssueTracker) -> Self {
        Self {
            id: t.id,
            project_id: t.project_id,
            provider: t.provider,
            enabled: t.enabled,
            github_labels: t.github_labels,
            github_assignees: t.github_assignees,
            jira_site_url: t.jira_site_url,
            jira_project_key: t.jira_project_key,
            jira_issue_type: t.jira_issue_type,
            status_mapping: t.status_mapping,
            webhook_url: t.webhook_url,
            webhook_events: t.webhook_events,
            linear_team_id: t.linear_team_id,
            created_at: t.created_at.unwrap_or_else(Utc::now),
        }
    }
}

/// External issue linked to an incident
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct IncidentExternalIssue {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub provider: String,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    pub external_status: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub sync_error: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Incident state change event for issue tracker sync
#[derive(Debug, Clone, Serialize)]
pub struct IncidentStateChange {
    pub incident_id: Uuid,
    pub project_id: Uuid,
    pub fingerprint: String,
    pub title: Option<String>,
    pub old_state: Option<String>,
    pub new_state: String,
    pub message: Option<String>,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub fix_attempts: i32,
    pub verification_duration_hours: Option<i32>,
    pub occurred_at: DateTime<Utc>,

    // Project info for context
    pub github_owner: String,
    pub github_repo: String,
}

/// Get all enabled issue trackers for a project
pub async fn get_project_issue_trackers(
    db: &PgPool,
    project_id: Uuid,
) -> Result<Vec<ProjectIssueTracker>> {
    let trackers = sqlx::query_as!(
        ProjectIssueTracker,
        r#"
        SELECT id, project_id, provider, enabled,
               github_labels, github_assignees,
               jira_site_url, jira_project_key, jira_issue_type, jira_priority,
               status_mapping, webhook_url, webhook_headers, webhook_events,
               linear_team_id, linear_project_id,
               created_at, updated_at
        FROM project_issue_trackers
        WHERE project_id = $1 AND enabled = true
        "#,
        project_id
    )
    .fetch_all(db)
    .await?;

    Ok(trackers)
}

/// Get external issue for an incident
pub async fn get_external_issue(
    db: &PgPool,
    incident_id: Uuid,
    provider: &str,
) -> Result<Option<IncidentExternalIssue>> {
    let issue = sqlx::query_as!(
        IncidentExternalIssue,
        r#"
        SELECT id, incident_id, provider, external_id, external_url,
               external_status, last_synced_at, sync_error, created_at, updated_at
        FROM incident_external_issues
        WHERE incident_id = $1 AND provider = $2
        "#,
        incident_id,
        provider
    )
    .fetch_optional(db)
    .await?;

    Ok(issue)
}

/// Create or update external issue link
pub async fn upsert_external_issue(
    db: &PgPool,
    incident_id: Uuid,
    provider: &str,
    external_id: &str,
    external_url: &str,
    external_status: Option<&str>,
) -> Result<IncidentExternalIssue> {
    let issue = sqlx::query_as!(
        IncidentExternalIssue,
        r#"
        INSERT INTO incident_external_issues
            (incident_id, provider, external_id, external_url, external_status, last_synced_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (incident_id, provider) DO UPDATE SET
            external_id = EXCLUDED.external_id,
            external_url = EXCLUDED.external_url,
            external_status = EXCLUDED.external_status,
            last_synced_at = NOW(),
            sync_error = NULL,
            updated_at = NOW()
        RETURNING id, incident_id, provider, external_id, external_url,
                  external_status, last_synced_at, sync_error, created_at, updated_at
        "#,
        incident_id,
        provider,
        external_id,
        external_url,
        external_status
    )
    .fetch_one(db)
    .await?;

    Ok(issue)
}

/// Record sync error for external issue
pub async fn record_sync_error(
    db: &PgPool,
    incident_id: Uuid,
    provider: &str,
    error: &str,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE incident_external_issues
        SET sync_error = $1, updated_at = NOW()
        WHERE incident_id = $2 AND provider = $3
        "#,
        error,
        incident_id,
        provider
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Get all external issues for an incident
pub async fn get_incident_external_issues(
    db: &PgPool,
    incident_id: Uuid,
) -> Result<Vec<IncidentExternalIssue>> {
    let issues = sqlx::query_as!(
        IncidentExternalIssue,
        r#"
        SELECT id, incident_id, provider, external_id, external_url,
               external_status, last_synced_at, sync_error, created_at, updated_at
        FROM incident_external_issues
        WHERE incident_id = $1
        ORDER BY created_at ASC
        "#,
        incident_id
    )
    .fetch_all(db)
    .await?;

    Ok(issues)
}

/// Sync incident state to all configured issue trackers
pub async fn sync_incident_state(
    db: &PgPool,
    github_client: &crate::integrations::github::GitHubClient,
    event: &IncidentStateChange,
) -> Result<()> {
    let trackers = get_project_issue_trackers(db, event.project_id).await?;

    for tracker in trackers {
        let result = match tracker.provider.as_str() {
            "github" => {
                github_issues::sync_to_github(db, github_client, &tracker, event).await
            }
            "jira" => {
                jira::sync_to_jira(db, &tracker, event).await
            }
            "linear" => {
                linear::sync_to_linear(db, &tracker, event).await
            }
            "webhook" => {
                webhook::send_webhook(db, &tracker, event).await
            }
            _ => {
                tracing::warn!(provider = %tracker.provider, "Unknown issue tracker provider");
                Ok(())
            }
        };

        if let Err(e) = result {
            tracing::error!(
                provider = %tracker.provider,
                incident_id = %event.incident_id,
                error = %e,
                "Failed to sync to issue tracker"
            );
            // Record error but don't fail the whole sync
            let _ = record_sync_error(db, event.incident_id, &tracker.provider, &e.to_string()).await;
        }
    }

    Ok(())
}

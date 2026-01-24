use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use super::metrics;
use super::AppState;
use crate::error::AppError;
use crate::pipeline::fingerprint;
use crate::store::models::Project;

/// Ingest payload - the error log to process
#[derive(Debug, Deserialize)]
pub struct IngestPayload {
    /// Project identification: "owner/repo" format
    pub project: Option<String>,
    /// Alternative: service name (can be mapped to project)
    pub service_name: Option<String>,
    /// Kubernetes namespace (for k8s deployments)
    pub kubernetes_namespace: Option<String>,
    /// Kubernetes pod name
    pub kubernetes_pod: Option<String>,

    /// Log level: ERROR, FATAL, CRITICAL
    pub level: String,
    /// Error message
    pub message: String,
    /// Stack trace (optional but recommended)
    pub stack_trace: Option<String>,
    /// Timestamp of the error
    pub timestamp: Option<DateTime<Utc>>,

    /// Additional attributes/metadata
    pub attributes: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub accepted: bool,
    pub incident_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub message: String,
    pub is_new: Option<bool>,
    pub occurrence_count: Option<i32>,
}

/// HTTP POST /ingest endpoint
pub async fn http_ingest(
    State(state): State<AppState>,
    Json(payload): Json<IngestPayload>,
) -> Result<Json<IngestResponse>, AppError> {
    // Record ingest request metric
    metrics::record_ingest();

    // Only process errors
    let level = payload.level.to_uppercase();
    if !matches!(level.as_str(), "ERROR" | "FATAL" | "CRITICAL" | "SEVERE") {
        return Ok(Json(IngestResponse {
            accepted: false,
            incident_id: None,
            project_id: None,
            message: "Only ERROR/FATAL/CRITICAL/SEVERE logs are processed".into(),
            is_new: None,
            occurrence_count: None,
        }));
    }

    // Identify project
    let project_key = payload
        .project
        .clone()
        .or_else(|| infer_project(&payload))
        .ok_or_else(|| AppError::BadRequest("Cannot identify project from payload".into()))?;

    let (owner, repo) = parse_project_key(&project_key)?;

    // Check safety rails
    if let Some(allowed) = &state.config.safety.allowed_repos {
        if !is_repo_allowed(&owner, &repo, allowed) {
            return Ok(Json(IngestResponse {
                accepted: false,
                incident_id: None,
                project_id: None,
                message: format!("Repository {}/{} not in allowed list", owner, repo),
                is_new: None,
                occurrence_count: None,
            }));
        }
    }

    // Auto-register or fetch project
    let project = sqlx::query_as!(
        Project,
        r#"
        INSERT INTO projects (id, github_owner, github_repo, auto_registered)
        VALUES ($1, $2, $3, true)
        ON CONFLICT (github_owner, github_repo)
        DO UPDATE SET updated_at = NOW()
        RETURNING id, github_owner, github_repo, default_branch, language,
                  enabled, auto_registered, settings, created_at, updated_at
        "#,
        Uuid::new_v4(),
        owner,
        repo
    )
    .fetch_one(&state.db)
    .await?;

    if !project.enabled.unwrap_or(true) {
        return Ok(Json(IngestResponse {
            accepted: false,
            incident_id: None,
            project_id: Some(project.id),
            message: "Project is disabled".into(),
            is_new: None,
            occurrence_count: None,
        }));
    }

    // Build payload for fingerprinting
    let payload_json = serde_json::json!({
        "message": payload.message,
        "stack_trace": payload.stack_trace,
        "project": project_key,
        "level": level,
        "attributes": payload.attributes,
    });

    // Fingerprint the error
    let fp = fingerprint::compute(&payload_json);

    // Upsert incident
    let incident = sqlx::query!(
        r#"
        INSERT INTO incidents (id, project_id, fingerprint, title, state, first_seen, last_seen, occurrence_count)
        VALUES ($1, $2, $3, $4, 'detected', NOW(), NOW(), 1)
        ON CONFLICT (project_id, fingerprint)
        DO UPDATE SET
            occurrence_count = incidents.occurrence_count + 1,
            last_seen = NOW(),
            state = CASE
                WHEN incidents.state = 'cooldown' AND incidents.cooldown_until < NOW() THEN 'detected'
                WHEN incidents.state = 'pr_merged' THEN 'detected'
                WHEN incidents.state = 'pr_closed' THEN 'detected'
                ELSE incidents.state
            END,
            updated_at = NOW()
        RETURNING id, occurrence_count,
                  (xmax = 0) as "is_new!"
        "#,
        Uuid::new_v4(),
        project.id,
        fp,
        truncate_title(&payload.message),
    )
    .fetch_one(&state.db)
    .await?;

    let is_new = incident.is_new;
    let occurrence_count = incident.occurrence_count.unwrap_or(1);

    // Store sample if this is a new incident or we need more samples (first 3)
    if occurrence_count <= 3 {
        let sample_data = serde_json::to_vec(&payload_json)?;
        state.storage.put(incident.id, &sample_data).await?;

        info!(
            incident_id = %incident.id,
            project = %project_key,
            is_new = is_new,
            "Sample stored"
        );
    }

    // Log activity
    sqlx::query!(
        r#"
        INSERT INTO activity_log (id, incident_id, project_id, action, details)
        VALUES ($1, $2, $3, 'ingested', $4)
        "#,
        Uuid::new_v4(),
        incident.id,
        project.id,
        serde_json::json!({
            "occurrence": occurrence_count,
            "level": level,
            "is_new": is_new,
        }),
    )
    .execute(&state.db)
    .await?;

    if is_new {
        info!(
            incident_id = %incident.id,
            project = %project_key,
            fingerprint = %fp,
            "New incident created"
        );

        // Broadcast incident:created event via SSE
        state.events.incident_created(
            incident.id,
            project.id,
            &truncate_title(&payload.message),
            "detected",
        );

        // Broadcast updated stats
        super::events::broadcast_stats(&state).await;
    } else {
        info!(
            incident_id = %incident.id,
            project = %project_key,
            occurrence_count = occurrence_count,
            "Incident occurrence recorded"
        );
    }

    Ok(Json(IngestResponse {
        accepted: true,
        incident_id: Some(incident.id),
        project_id: Some(project.id),
        message: if is_new {
            "New incident created".into()
        } else {
            format!("Occurrence #{} recorded", occurrence_count)
        },
        is_new: Some(is_new),
        occurrence_count: Some(occurrence_count),
    }))
}

/// Try to infer project from alternative fields
fn infer_project(payload: &IngestPayload) -> Option<String> {
    // Try service_name first (common pattern: "owner-repo" or "owner/repo")
    if let Some(service) = &payload.service_name {
        // If it contains a slash, use as-is
        if service.contains('/') {
            return Some(service.clone());
        }
        // If it contains a hyphen, try converting first hyphen to slash
        if let Some(idx) = service.find('-') {
            let (owner, repo) = service.split_at(idx);
            return Some(format!("{}/{}", owner, &repo[1..]));
        }
    }

    // Could add more inference logic here:
    // - Parse from kubernetes_namespace/kubernetes_pod
    // - Look up service_name in a mapping table
    // - etc.

    None
}

/// Parse "owner/repo" format
fn parse_project_key(key: &str) -> Result<(String, String), AppError> {
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest(format!(
            "Invalid project key '{}'. Expected 'owner/repo' format",
            key
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Check if repo is in allowed list (supports wildcards)
fn is_repo_allowed(owner: &str, repo: &str, allowed: &[String]) -> bool {
    let full = format!("{}/{}", owner, repo);
    allowed.iter().any(|pattern| {
        if pattern.contains('*') {
            // Convert glob pattern to regex
            let pattern = pattern.replace('.', r"\.");
            let pattern = pattern.replace('*', ".*");
            regex::Regex::new(&format!("^{}$", pattern))
                .map(|re| re.is_match(&full))
                .unwrap_or(false)
        } else {
            pattern == &full
        }
    })
}

/// Truncate message to create a title (max 200 chars)
fn truncate_title(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or(msg);
    if first_line.len() > 200 {
        format!("{}...", &first_line[..197])
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_project_key() {
        let (owner, repo) = parse_project_key("acme/webapp").unwrap();
        assert_eq!(owner, "acme");
        assert_eq!(repo, "webapp");

        assert!(parse_project_key("invalid").is_err());
        assert!(parse_project_key("a/b/c").is_err());
    }

    #[test]
    fn test_is_repo_allowed() {
        let allowed = vec![
            "acme/webapp".to_string(),
            "acme/*-sandbox".to_string(),
            "test/*".to_string(),
        ];

        assert!(is_repo_allowed("acme", "webapp", &allowed));
        assert!(is_repo_allowed("acme", "foo-sandbox", &allowed));
        assert!(is_repo_allowed("test", "anything", &allowed));
        assert!(!is_repo_allowed("other", "repo", &allowed));
    }

    #[test]
    fn test_truncate_title() {
        assert_eq!(truncate_title("Short message"), "Short message");
        assert_eq!(
            truncate_title("Line 1\nLine 2\nLine 3"),
            "Line 1"
        );

        let long = "x".repeat(300);
        let truncated = truncate_title(&long);
        assert_eq!(truncated.len(), 200);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_infer_project() {
        let payload = IngestPayload {
            project: None,
            service_name: Some("acme/webapp".to_string()),
            kubernetes_namespace: None,
            kubernetes_pod: None,
            level: "ERROR".to_string(),
            message: "test".to_string(),
            stack_trace: None,
            timestamp: None,
            attributes: None,
        };
        assert_eq!(infer_project(&payload), Some("acme/webapp".to_string()));

        let payload2 = IngestPayload {
            service_name: Some("acme-webapp".to_string()),
            ..payload
        };
        assert_eq!(infer_project(&payload2), Some("acme/webapp".to_string()));
    }
}

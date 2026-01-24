//! Jira Integration
//!
//! Creates and transitions Jira issues to track incidents.
//! Supports both OAuth 2.0 and API token authentication.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, info, warn};

use super::{get_external_issue, upsert_external_issue, IncidentStateChange, ProjectIssueTracker};

/// Jira API client
pub struct JiraClient {
    client: Client,
    site_url: String,
    auth_header: String,
}

impl JiraClient {
    /// Create a new Jira client with API token auth
    pub fn with_api_token(site_url: &str, email: &str, api_token: &str) -> Self {
        let client = Client::builder()
            .user_agent("CRISPR/1.0")
            .build()
            .expect("Failed to create HTTP client");

        // Basic auth with email:api_token
        let credentials = format!("{}:{}", email, api_token);
        let auth_header = format!(
            "Basic {}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, credentials)
        );

        Self {
            client,
            site_url: site_url.trim_end_matches('/').to_string(),
            auth_header,
        }
    }

    /// Create a new Jira client with OAuth access token
    pub fn with_oauth(site_url: &str, access_token: &str) -> Self {
        let client = Client::builder()
            .user_agent("CRISPR/1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            site_url: site_url.trim_end_matches('/').to_string(),
            auth_header: format!("Bearer {}", access_token),
        }
    }

    /// Create a Jira issue
    pub async fn create_issue(&self, request: &CreateIssueRequest) -> Result<JiraIssue> {
        let url = format!("{}/rest/api/3/issue", self.site_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jira API error {}: {}", status, error));
        }

        Ok(response.json().await?)
    }

    /// Update a Jira issue
    pub async fn update_issue(&self, issue_key: &str, request: &UpdateIssueRequest) -> Result<()> {
        let url = format!("{}/rest/api/3/issue/{}", self.site_url, issue_key);

        let response = self
            .client
            .put(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jira API error {}: {}", status, error));
        }

        Ok(())
    }

    /// Transition a Jira issue to a new status
    pub async fn transition_issue(&self, issue_key: &str, transition_id: &str) -> Result<()> {
        let url = format!("{}/rest/api/3/issue/{}/transitions", self.site_url, issue_key);

        let body = serde_json::json!({
            "transition": {
                "id": transition_id
            }
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jira API error {}: {}", status, error));
        }

        Ok(())
    }

    /// Get available transitions for an issue
    pub async fn get_transitions(&self, issue_key: &str) -> Result<Vec<JiraTransition>> {
        let url = format!("{}/rest/api/3/issue/{}/transitions", self.site_url, issue_key);

        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jira API error {}: {}", status, error));
        }

        let result: TransitionsResponse = response.json().await?;
        Ok(result.transitions)
    }

    /// Add a comment to an issue
    pub async fn add_comment(&self, issue_key: &str, body: &str) -> Result<()> {
        let url = format!("{}/rest/api/3/issue/{}/comment", self.site_url, issue_key);

        let comment = serde_json::json!({
            "body": {
                "type": "doc",
                "version": 1,
                "content": [
                    {
                        "type": "paragraph",
                        "content": [
                            {
                                "type": "text",
                                "text": body
                            }
                        ]
                    }
                ]
            }
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&comment)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jira API error {}: {}", status, error));
        }

        Ok(())
    }

    /// Test the connection
    pub async fn test_connection(&self) -> Result<JiraUser> {
        let url = format!("{}/rest/api/3/myself", self.site_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jira API error {}: {}", status, error));
        }

        Ok(response.json().await?)
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct CreateIssueRequest {
    pub fields: CreateIssueFields,
}

#[derive(Debug, Serialize)]
pub struct CreateIssueFields {
    pub project: ProjectKey,
    pub summary: String,
    pub description: JiraDoc,
    pub issuetype: IssueType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ProjectKey {
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct IssueType {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Priority {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct JiraDoc {
    #[serde(rename = "type")]
    pub doc_type: String,
    pub version: i32,
    pub content: Vec<JiraDocContent>,
}

#[derive(Debug, Serialize)]
pub struct JiraDocContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub content: Vec<JiraTextContent>,
}

#[derive(Debug, Serialize)]
pub struct JiraTextContent {
    #[serde(rename = "type")]
    pub text_type: String,
    pub text: String,
}

impl JiraDoc {
    pub fn from_text(text: &str) -> Self {
        Self {
            doc_type: "doc".to_string(),
            version: 1,
            content: vec![JiraDocContent {
                content_type: "paragraph".to_string(),
                content: vec![JiraTextContent {
                    text_type: "text".to_string(),
                    text: text.to_string(),
                }],
            }],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UpdateIssueRequest {
    pub fields: UpdateIssueFields,
}

#[derive(Debug, Serialize, Default)]
pub struct UpdateIssueFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<JiraDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssue {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

impl JiraIssue {
    pub fn browse_url(&self, site_url: &str) -> String {
        format!("{}/browse/{}", site_url.trim_end_matches('/'), self.key)
    }
}

#[derive(Debug, Deserialize)]
pub struct TransitionsResponse {
    pub transitions: Vec<JiraTransition>,
}

#[derive(Debug, Deserialize)]
pub struct JiraTransition {
    pub id: String,
    pub name: String,
    pub to: JiraStatus,
}

#[derive(Debug, Deserialize)]
pub struct JiraStatus {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraUser {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>,
}

// ============================================================================
// Sync Logic
// ============================================================================

/// Build Jira issue description from incident
fn build_description(event: &IncidentStateChange) -> String {
    let mut desc = String::new();

    desc.push_str("h2. CRISPR Incident\n\n");

    if let Some(title) = &event.title {
        desc.push_str(&format!("*Error:* {}\n\n", title));
    }

    desc.push_str(&format!("*Status:* {}\n", event.new_state));
    desc.push_str(&format!("*Fingerprint:* {}\n", event.fingerprint));
    desc.push_str(&format!("*Repository:* {}/{}\n", event.github_owner, event.github_repo));
    desc.push_str(&format!("*Fix Attempts:* {}\n", event.fix_attempts));

    if let Some(duration) = event.verification_duration_hours {
        desc.push_str(&format!("*Verification Period:* {} hours\n", duration));
    }

    if let Some(pr_url) = &event.pr_url {
        desc.push_str(&format!("\nh3. Pull Request\n\n{}\n", pr_url));
    }

    desc.push_str("\n----\n");
    desc.push_str("_This issue is managed by CRISPR. Do not close manually._\n");

    desc
}

/// Build issue summary from incident
fn build_summary(event: &IncidentStateChange) -> String {
    let prefix = "[CRISPR]";
    if let Some(title) = &event.title {
        let max_len = 200;
        if title.len() > max_len {
            format!("{} {}...", prefix, &title[..max_len])
        } else {
            format!("{} {}", prefix, title)
        }
    } else {
        format!("{} Incident {}", prefix, &event.fingerprint[..8])
    }
}

/// Get Jira client from stored credentials
async fn get_jira_client(db: &PgPool, site_url: &str) -> Result<JiraClient> {
    // Try to find credentials for this site
    let conn = sqlx::query!(
        r#"
        SELECT access_token, refresh_token, api_token, user_email
        FROM jira_connections
        WHERE site_url = $1
        "#,
        site_url
    )
    .fetch_optional(db)
    .await?;

    let conn = conn.ok_or_else(|| anyhow::anyhow!("No Jira credentials found for {}", site_url))?;

    // Prefer OAuth token if available
    if let Some(access_token) = conn.access_token {
        return Ok(JiraClient::with_oauth(site_url, &access_token));
    }

    // Fall back to API token
    if let (Some(api_token), Some(email)) = (conn.api_token, conn.user_email) {
        return Ok(JiraClient::with_api_token(site_url, &email, &api_token));
    }

    Err(anyhow::anyhow!("No valid Jira credentials found"))
}

/// Find transition ID for target status
async fn find_transition(client: &JiraClient, issue_key: &str, target_status: &str) -> Result<Option<String>> {
    let transitions = client.get_transitions(issue_key).await?;

    // Find transition that leads to the target status
    for t in transitions {
        if t.to.name.eq_ignore_ascii_case(target_status) || t.name.eq_ignore_ascii_case(target_status) {
            return Ok(Some(t.id));
        }
    }

    Ok(None)
}

/// Sync incident to Jira
pub async fn sync_to_jira(
    db: &PgPool,
    config: &ProjectIssueTracker,
    event: &IncidentStateChange,
) -> Result<()> {
    let site_url = config
        .jira_site_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Jira site URL not configured"))?;

    let project_key = config
        .jira_project_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Jira project key not configured"))?;

    let client = get_jira_client(db, site_url).await?;

    // Check if we already have an issue
    let existing = get_external_issue(db, event.incident_id, "jira").await?;

    let summary = build_summary(event);
    let description = build_description(event);

    if let Some(existing_issue) = existing {
        // Update existing issue
        if let Some(issue_key) = &existing_issue.external_id {
            // Update description
            let update = UpdateIssueRequest {
                fields: UpdateIssueFields {
                    summary: Some(summary),
                    description: Some(JiraDoc::from_text(&description)),
                    labels: Some(vec!["crispr".to_string(), format!("crispr-{}", event.new_state)]),
                },
            };
            client.update_issue(issue_key, &update).await?;

            // Try to transition to mapped status
            if let Some(mapping) = &config.status_mapping {
                if let Some(target_status) = mapping.get(&event.new_state).and_then(|v| v.as_str()) {
                    if let Some(transition_id) = find_transition(&client, issue_key, target_status).await? {
                        client.transition_issue(issue_key, &transition_id).await?;
                        info!(
                            incident_id = %event.incident_id,
                            issue_key = %issue_key,
                            target_status = %target_status,
                            "Transitioned Jira issue"
                        );
                    } else {
                        debug!(
                            issue_key = %issue_key,
                            target_status = %target_status,
                            "No transition found for target status"
                        );
                    }
                }
            }

            // Update our record
            upsert_external_issue(
                db,
                event.incident_id,
                "jira",
                issue_key,
                &format!("{}/browse/{}", site_url, issue_key),
                Some(&event.new_state),
            )
            .await?;

            info!(
                incident_id = %event.incident_id,
                issue_key = %issue_key,
                state = %event.new_state,
                "Updated Jira issue"
            );
        }
    } else {
        // Create new issue
        let issue_type = config
            .jira_issue_type
            .clone()
            .unwrap_or_else(|| "Bug".to_string());

        let request = CreateIssueRequest {
            fields: CreateIssueFields {
                project: ProjectKey {
                    key: project_key.clone(),
                },
                summary,
                description: JiraDoc::from_text(&description),
                issuetype: IssueType { name: issue_type },
                priority: config.jira_priority.as_ref().map(|p| Priority { name: p.clone() }),
                labels: Some(vec!["crispr".to_string(), format!("crispr-{}", event.new_state)]),
            },
        };

        let issue = client.create_issue(&request).await?;

        // Record the external issue
        upsert_external_issue(
            db,
            event.incident_id,
            "jira",
            &issue.key,
            &issue.browse_url(site_url),
            Some(&event.new_state),
        )
        .await?;

        info!(
            incident_id = %event.incident_id,
            issue_key = %issue.key,
            "Created Jira issue"
        );
    }

    Ok(())
}

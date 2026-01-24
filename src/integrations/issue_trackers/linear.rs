//! Linear Integration
//!
//! Creates and updates Linear issues to track incidents.
//! Uses Linear's GraphQL API.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::info;

use super::{get_external_issue, upsert_external_issue, IncidentStateChange, ProjectIssueTracker};

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

/// Linear API client
pub struct LinearClient {
    client: Client,
    api_key: String,
}

impl LinearClient {
    pub fn new(api_key: &str) -> Self {
        let client = Client::builder()
            .user_agent("CRISPR/1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: api_key.to_string(),
        }
    }

    async fn graphql<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables
        });

        let response = self
            .client
            .post(LINEAR_API_URL)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Linear API error {}: {}", status, error));
        }

        let result: GraphQLResponse<T> = response.json().await?;

        if let Some(errors) = result.errors {
            let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(anyhow::anyhow!("Linear GraphQL errors: {}", error_msgs.join(", ")));
        }

        result
            .data
            .ok_or_else(|| anyhow::anyhow!("No data in Linear response"))
    }

    /// Create an issue
    pub async fn create_issue(
        &self,
        team_id: &str,
        title: &str,
        description: &str,
        project_id: Option<&str>,
    ) -> Result<LinearIssue> {
        let query = r#"
            mutation CreateIssue($input: IssueCreateInput!) {
                issueCreate(input: $input) {
                    success
                    issue {
                        id
                        identifier
                        title
                        url
                    }
                }
            }
        "#;

        let mut input = serde_json::json!({
            "teamId": team_id,
            "title": title,
            "description": description
        });

        if let Some(pid) = project_id {
            input["projectId"] = serde_json::json!(pid);
        }

        let variables = serde_json::json!({ "input": input });

        let result: CreateIssueResponse = self.graphql(query, variables).await?;

        if !result.issue_create.success {
            return Err(anyhow::anyhow!("Failed to create Linear issue"));
        }

        result
            .issue_create
            .issue
            .ok_or_else(|| anyhow::anyhow!("No issue returned"))
    }

    /// Update an issue
    pub async fn update_issue(
        &self,
        issue_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        state_id: Option<&str>,
    ) -> Result<LinearIssue> {
        let query = r#"
            mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
                issueUpdate(id: $id, input: $input) {
                    success
                    issue {
                        id
                        identifier
                        title
                        url
                    }
                }
            }
        "#;

        let mut input = serde_json::json!({});

        if let Some(t) = title {
            input["title"] = serde_json::json!(t);
        }

        if let Some(d) = description {
            input["description"] = serde_json::json!(d);
        }

        if let Some(s) = state_id {
            input["stateId"] = serde_json::json!(s);
        }

        let variables = serde_json::json!({
            "id": issue_id,
            "input": input
        });

        let result: UpdateIssueResponse = self.graphql(query, variables).await?;

        if !result.issue_update.success {
            return Err(anyhow::anyhow!("Failed to update Linear issue"));
        }

        result
            .issue_update
            .issue
            .ok_or_else(|| anyhow::anyhow!("No issue returned"))
    }

    /// Get workflow states for a team
    pub async fn get_workflow_states(&self, team_id: &str) -> Result<Vec<WorkflowState>> {
        let query = r#"
            query GetStates($teamId: String!) {
                team(id: $teamId) {
                    states {
                        nodes {
                            id
                            name
                            type
                        }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({ "teamId": team_id });

        let result: TeamStatesResponse = self.graphql(query, variables).await?;

        Ok(result.team.states.nodes)
    }

    /// Test connection and get viewer info
    pub async fn test_connection(&self) -> Result<LinearViewer> {
        let query = r#"
            query Viewer {
                viewer {
                    id
                    name
                    email
                }
            }
        "#;

        let result: ViewerResponse = self.graphql(query, serde_json::json!({})).await?;

        Ok(result.viewer)
    }
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueResponse {
    #[serde(rename = "issueCreate")]
    pub issue_create: IssueCreateResult,
}

#[derive(Debug, Deserialize)]
pub struct IssueCreateResult {
    pub success: bool,
    pub issue: Option<LinearIssue>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIssueResponse {
    #[serde(rename = "issueUpdate")]
    pub issue_update: IssueUpdateResult,
}

#[derive(Debug, Deserialize)]
pub struct IssueUpdateResult {
    pub success: bool,
    pub issue: Option<LinearIssue>,
}

#[derive(Debug, Deserialize)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct TeamStatesResponse {
    pub team: TeamWithStates,
}

#[derive(Debug, Deserialize)]
pub struct TeamWithStates {
    pub states: StatesConnection,
}

#[derive(Debug, Deserialize)]
pub struct StatesConnection {
    pub nodes: Vec<WorkflowState>,
}

#[derive(Debug, Deserialize)]
pub struct WorkflowState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub state_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ViewerResponse {
    pub viewer: LinearViewer,
}

#[derive(Debug, Deserialize)]
pub struct LinearViewer {
    pub id: String,
    pub name: String,
    pub email: String,
}

// ============================================================================
// Sync Logic
// ============================================================================

/// Build Linear issue description from incident
fn build_description(event: &IncidentStateChange) -> String {
    let mut desc = String::new();

    desc.push_str("## CRISPR Incident\n\n");

    if let Some(title) = &event.title {
        desc.push_str(&format!("**Error:** {}\n\n", title));
    }

    desc.push_str(&format!("**Status:** `{}`\n", event.new_state));
    desc.push_str(&format!("**Fingerprint:** `{}`\n", event.fingerprint));
    desc.push_str(&format!("**Repository:** {}/{}\n", event.github_owner, event.github_repo));
    desc.push_str(&format!("**Fix Attempts:** {}\n", event.fix_attempts));

    if let Some(duration) = event.verification_duration_hours {
        desc.push_str(&format!("**Verification Period:** {} hours\n", duration));
    }

    if let Some(pr_url) = &event.pr_url {
        desc.push_str(&format!("\n### Pull Request\n\n{}\n", pr_url));
    }

    desc.push_str("\n---\n");
    desc.push_str("*This issue is managed by CRISPR. Do not close manually.*\n");

    desc
}

/// Build issue title from incident
fn build_title(event: &IncidentStateChange) -> String {
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

/// Get Linear client from stored credentials
async fn get_linear_client(db: &PgPool) -> Result<LinearClient> {
    let conn = sqlx::query!(
        "SELECT access_token FROM linear_connections ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(db)
    .await?;

    let conn = conn.ok_or_else(|| anyhow::anyhow!("No Linear credentials found"))?;
    let token = conn
        .access_token
        .ok_or_else(|| anyhow::anyhow!("No Linear access token"))?;

    Ok(LinearClient::new(&token))
}

/// Map CRISPR state to Linear state type
fn map_state_to_linear_type(state: &str) -> &'static str {
    match state {
        "detected" | "triaging" => "backlog",
        "pending_fix" | "fixing" | "pr_opening" => "started",
        "pr_open" | "pr_merged" | "verifying" => "started",
        "verified" => "completed",
        "unfixable" | "fix_failed" | "pr_closed" | "max_attempts" | "ignored" => "canceled",
        _ => "backlog",
    }
}

/// Sync incident to Linear
pub async fn sync_to_linear(
    db: &PgPool,
    config: &ProjectIssueTracker,
    event: &IncidentStateChange,
) -> Result<()> {
    let team_id = config
        .linear_team_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Linear team ID not configured"))?;

    let client = get_linear_client(db).await?;

    // Check if we already have an issue
    let existing = get_external_issue(db, event.incident_id, "linear").await?;

    let title = build_title(event);
    let description = build_description(event);

    if let Some(existing_issue) = existing {
        // Update existing issue
        if let Some(issue_id) = &existing_issue.external_id {
            // Get workflow states to find the right one
            let states = client.get_workflow_states(team_id).await?;
            let target_type = map_state_to_linear_type(&event.new_state);

            let state_id = states.iter().find(|s| s.state_type == target_type).map(|s| s.id.as_str());

            let issue = client
                .update_issue(issue_id, Some(&title), Some(&description), state_id)
                .await?;

            // Update our record
            upsert_external_issue(
                db,
                event.incident_id,
                "linear",
                &issue.id,
                &issue.url,
                Some(&event.new_state),
            )
            .await?;

            info!(
                incident_id = %event.incident_id,
                issue_id = %issue.identifier,
                state = %event.new_state,
                "Updated Linear issue"
            );
        }
    } else {
        // Create new issue
        let issue = client
            .create_issue(team_id, &title, &description, config.linear_project_id.as_deref())
            .await?;

        // Record the external issue
        upsert_external_issue(
            db,
            event.incident_id,
            "linear",
            &issue.id,
            &issue.url,
            Some(&event.new_state),
        )
        .await?;

        info!(
            incident_id = %event.incident_id,
            issue_id = %issue.identifier,
            "Created Linear issue"
        );
    }

    Ok(())
}

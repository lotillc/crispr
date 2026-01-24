//! GitHub Integration
//!
//! Supports two authentication modes:
//! - PAT (Personal Access Token): Simple, good for development
//! - OAuth: Full app integration for production

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::GitHubConfig;

const GITHUB_API_URL: &str = "https://api.github.com";

/// GitHub client for repository operations
pub struct GitHubClient {
    client: Client,
    config: GitHubConfig,
    db: PgPool,
    token: RwLock<Option<String>>,
}

impl GitHubClient {
    pub fn new(config: GitHubConfig, db: PgPool) -> Self {
        let client = Client::builder()
            .user_agent("CRISPR/1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            db,
            token: RwLock::new(None),
        }
    }

    /// Initialize the client, loading token from config or database
    pub async fn init(&self) -> Result<()> {
        match self.config.mode.as_str() {
            "pat" => {
                if let Some(pat) = &self.config.personal_access_token {
                    *self.token.write().await = Some(pat.clone());
                    info!("GitHub PAT mode configured");
                }
            }
            "oauth" => {
                // Load token from database
                let conn = sqlx::query!(
                    "SELECT access_token FROM github_connections ORDER BY created_at DESC LIMIT 1"
                )
                .fetch_optional(&self.db)
                .await?;

                if let Some(c) = conn {
                    *self.token.write().await = Some(c.access_token);
                    info!("GitHub OAuth token loaded from database");
                } else {
                    warn!("GitHub OAuth mode but no token in database");
                }
            }
            _ => {
                warn!(mode = %self.config.mode, "Unknown GitHub mode");
            }
        }

        Ok(())
    }

    /// Check if GitHub is connected
    pub async fn is_connected(&self) -> bool {
        self.token.read().await.is_some()
    }

    /// Get the current authenticated user
    pub async fn get_user(&self) -> Result<GitHubUser> {
        let response = self.get("/user").await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Get repository information
    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<GitHubRepo> {
        let response = self.get(&format!("/repos/{}/{}", owner, repo)).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Get file contents from a repository
    pub async fn get_file(&self, owner: &str, repo: &str, path: &str, ref_: Option<&str>) -> Result<FileContent> {
        let mut url = format!("/repos/{}/{}/contents/{}", owner, repo, path);
        if let Some(r) = ref_ {
            url = format!("{}?ref={}", url, r);
        }
        let response = self.get(&url).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Get directory contents
    pub async fn list_directory(&self, owner: &str, repo: &str, path: &str, ref_: Option<&str>) -> Result<Vec<FileEntry>> {
        let mut url = format!("/repos/{}/{}/contents/{}", owner, repo, path);
        if let Some(r) = ref_ {
            url = format!("{}?ref={}", url, r);
        }
        let response = self.get(&url).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Get the default branch for a repository
    pub async fn get_default_branch(&self, owner: &str, repo: &str) -> Result<String> {
        let repo_info = self.get_repo(owner, repo).await?;
        Ok(repo_info.default_branch)
    }

    /// Get a branch reference
    pub async fn get_branch(&self, owner: &str, repo: &str, branch: &str) -> Result<GitHubRef> {
        let response = self.get(&format!("/repos/{}/{}/git/refs/heads/{}", owner, repo, branch)).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Create a new branch from a ref
    pub async fn create_branch(&self, owner: &str, repo: &str, branch: &str, from_sha: &str) -> Result<GitHubRef> {
        let response = self.post(
            &format!("/repos/{}/{}/git/refs", owner, repo),
            &serde_json::json!({
                "ref": format!("refs/heads/{}", branch),
                "sha": from_sha
            })
        ).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Create or update a file in a repository
    pub async fn create_or_update_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &str,
        message: &str,
        branch: &str,
        sha: Option<&str>,
    ) -> Result<CommitResponse> {
        let mut body = serde_json::json!({
            "message": message,
            "content": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, content),
            "branch": branch
        });

        if let Some(s) = sha {
            body["sha"] = serde_json::json!(s);
        }

        let response = self.put(&format!("/repos/{}/{}/contents/{}", owner, repo, path), &body).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Create a pull request
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequest> {
        let response = self.post(
            &format!("/repos/{}/{}/pulls", owner, repo),
            &serde_json::json!({
                "title": title,
                "body": body,
                "head": head,
                "base": base
            })
        ).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Create a GitHub issue
    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        labels: Option<Vec<String>>,
    ) -> Result<GitHubIssue> {
        let mut request = serde_json::json!({
            "title": title,
            "body": body
        });

        if let Some(l) = labels {
            request["labels"] = serde_json::json!(l);
        }

        let response = self.post(&format!("/repos/{}/{}/issues", owner, repo), &request).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Search for code in a repository
    pub async fn search_code(&self, owner: &str, repo: &str, query: &str) -> Result<CodeSearchResult> {
        let q = format!("{}+repo:{}/{}", query, owner, repo);
        let response = self.get(&format!("/search/code?q={}", urlencoding::encode(&q))).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Create a webhook for a repository
    pub async fn create_webhook(
        &self,
        owner: &str,
        repo: &str,
        webhook_url: &str,
        secret: Option<&str>,
        events: Vec<&str>,
    ) -> Result<GitHubWebhook> {
        let mut config = serde_json::json!({
            "url": webhook_url,
            "content_type": "json",
            "insecure_ssl": "0"
        });

        if let Some(s) = secret {
            config["secret"] = serde_json::json!(s);
        }

        let response = self.post(
            &format!("/repos/{}/{}/hooks", owner, repo),
            &serde_json::json!({
                "name": "web",
                "active": true,
                "events": events,
                "config": config
            })
        ).await?;

        Ok(serde_json::from_value(response)?)
    }

    /// List webhooks for a repository
    pub async fn list_webhooks(&self, owner: &str, repo: &str) -> Result<Vec<GitHubWebhook>> {
        let response = self.get(&format!("/repos/{}/{}/hooks", owner, repo)).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Delete a webhook
    pub async fn delete_webhook(&self, owner: &str, repo: &str, hook_id: i64) -> Result<()> {
        self.delete(&format!("/repos/{}/{}/hooks/{}", owner, repo, hook_id)).await?;
        Ok(())
    }

    /// Check if a CRISPR webhook already exists for a repo
    pub async fn find_crispr_webhook(&self, owner: &str, repo: &str, webhook_url: &str) -> Result<Option<GitHubWebhook>> {
        let webhooks = self.list_webhooks(owner, repo).await?;
        Ok(webhooks.into_iter().find(|w| {
            w.config.url.as_deref() == Some(webhook_url)
        }))
    }

    // HTTP helpers

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let token = self.token.read().await;
        let token = token.as_ref().ok_or_else(|| anyhow::anyhow!("GitHub not connected"))?;

        let response = self.client
            .get(format!("{}{}", GITHUB_API_URL, path))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("GitHub API error {}: {}", status, error_text));
        }

        Ok(response.json().await?)
    }

    async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let token = self.token.read().await;
        let token = token.as_ref().ok_or_else(|| anyhow::anyhow!("GitHub not connected"))?;

        let response = self.client
            .post(format!("{}{}", GITHUB_API_URL, path))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("GitHub API error {}: {}", status, error_text));
        }

        Ok(response.json().await?)
    }

    async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let token = self.token.read().await;
        let token = token.as_ref().ok_or_else(|| anyhow::anyhow!("GitHub not connected"))?;

        let response = self.client
            .put(format!("{}{}", GITHUB_API_URL, path))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("GitHub API error {}: {}", status, error_text));
        }

        Ok(response.json().await?)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let token = self.token.read().await;
        let token = token.as_ref().ok_or_else(|| anyhow::anyhow!("GitHub not connected"))?;

        let response = self.client
            .delete(format!("{}{}", GITHUB_API_URL, path))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("GitHub API error {}: {}", status, error_text));
        }

        Ok(())
    }
}

/// Initialize GitHub client
pub async fn init(config: &GitHubConfig, db: &PgPool) -> Result<Arc<GitHubClient>> {
    let client = GitHubClient::new(config.clone(), db.clone());
    client.init().await?;
    Ok(Arc::new(client))
}

// ============================================================================
// API Response Types
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: i64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub default_branch: String,
    pub language: Option<String>,
    pub private: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub content: Option<String>,
    pub encoding: Option<String>,
    #[serde(rename = "type")]
    pub file_type: String,
}

impl FileContent {
    /// Decode base64 content
    pub fn decoded_content(&self) -> Result<String> {
        let content = self.content.as_ref().ok_or_else(|| anyhow::anyhow!("No content"))?;
        let cleaned = content.replace('\n', "");
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cleaned)?;
        Ok(String::from_utf8(bytes)?)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub sha: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitHubRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub object: RefObject,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RefObject {
    pub sha: String,
    #[serde(rename = "type")]
    pub object_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommitResponse {
    pub content: FileContent,
    pub commit: CommitInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequest {
    pub id: i64,
    pub number: i32,
    pub title: String,
    pub html_url: String,
    pub state: String,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitHubIssue {
    pub id: i64,
    pub number: i32,
    pub title: String,
    pub html_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CodeSearchResult {
    pub total_count: i32,
    pub items: Vec<CodeSearchItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CodeSearchItem {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubWebhook {
    pub id: i64,
    pub name: String,
    pub active: bool,
    pub events: Vec<String>,
    pub config: WebhookConfig,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookConfig {
    pub url: Option<String>,
    pub content_type: Option<String>,
    pub insecure_ssl: Option<String>,
}

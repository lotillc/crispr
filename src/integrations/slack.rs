//! Slack Integration
//!
//! Sends notifications to Slack when PRs are opened, merged, etc.
//! Supports OAuth2 connection and channel selection.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::SlackConfig;

/// Slack client for sending notifications
pub struct SlackClient {
    config: SlackConfig,
    db: PgPool,
    http: reqwest::Client,
}

/// Slack channel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannel {
    pub id: String,
    pub name: String,
    pub is_private: bool,
    pub is_member: bool,
}

/// Slack connection info from database
#[derive(Debug, Clone)]
pub struct SlackConnection {
    pub id: uuid::Uuid,
    pub access_token: String,
    pub team_id: String,
    pub team_name: Option<String>,
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub bot_user_id: Option<String>,
}

/// Notification type for different events
#[derive(Debug, Clone)]
pub enum NotificationType {
    PrOpened {
        incident_id: uuid::Uuid,
        pr_url: String,
        pr_title: String,
        pr_number: i32,
        project: String,
        error_message: String,
        confidence: f32,
    },
    PrMerged {
        incident_id: uuid::Uuid,
        pr_url: String,
        pr_number: i32,
        project: String,
    },
    IncidentDetected {
        incident_id: uuid::Uuid,
        project: String,
        error_message: String,
        level: String,
        sample_count: i32,
    },
    FixFailed {
        incident_id: uuid::Uuid,
        project: String,
        error_message: String,
        reason: String,
    },
}

impl SlackClient {
    pub fn new(config: SlackConfig, db: PgPool) -> Self {
        Self {
            config,
            db,
            http: reqwest::Client::new(),
        }
    }

    /// Check if Slack is connected and has a channel configured
    pub async fn is_connected(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        self.get_connection().await.is_some()
    }

    /// Get current Slack connection
    pub async fn get_connection(&self) -> Option<SlackConnection> {
        sqlx::query_as!(
            SlackConnection,
            r#"
            SELECT id, access_token, team_id, team_name, channel_id, channel_name, bot_user_id
            FROM slack_connections
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
    }

    /// List available channels the bot can post to
    pub async fn list_channels(&self) -> Result<Vec<SlackChannel>> {
        let conn = self.get_connection().await.ok_or_else(|| {
            anyhow::anyhow!("Slack not connected")
        })?;

        // Get public channels the bot is a member of or can join
        let response = self
            .http
            .get("https://slack.com/api/conversations.list")
            .header("Authorization", format!("Bearer {}", conn.access_token))
            .query(&[
                ("types", "public_channel,private_channel"),
                ("exclude_archived", "true"),
                ("limit", "200"),
            ])
            .send()
            .await?;

        #[derive(Deserialize)]
        struct ChannelResponse {
            ok: bool,
            channels: Option<Vec<SlackChannelApi>>,
            error: Option<String>,
        }

        #[derive(Deserialize)]
        struct SlackChannelApi {
            id: String,
            name: String,
            is_private: bool,
            is_member: bool,
        }

        let data: ChannelResponse = response.json().await?;

        if !data.ok {
            return Err(anyhow::anyhow!(
                "Slack API error: {}",
                data.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        let channels = data
            .channels
            .unwrap_or_default()
            .into_iter()
            .map(|c| SlackChannel {
                id: c.id,
                name: c.name,
                is_private: c.is_private,
                is_member: c.is_member,
            })
            .collect();

        Ok(channels)
    }

    /// Set the channel for notifications
    pub async fn set_channel(&self, channel_id: &str, channel_name: &str) -> Result<()> {
        let conn = self.get_connection().await.ok_or_else(|| {
            anyhow::anyhow!("Slack not connected")
        })?;

        // Try to join the channel first (in case we're not a member)
        let join_response = self
            .http
            .post("https://slack.com/api/conversations.join")
            .header("Authorization", format!("Bearer {}", conn.access_token))
            .json(&serde_json::json!({
                "channel": channel_id
            }))
            .send()
            .await?;

        #[derive(Deserialize)]
        struct JoinResponse {
            ok: bool,
            error: Option<String>,
        }

        let join_result: JoinResponse = join_response.json().await?;

        // "already_in_channel" is not an error
        if !join_result.ok && join_result.error.as_deref() != Some("already_in_channel") {
            // Log but don't fail - might be a private channel we're already in
            warn!(
                "Could not join channel: {}",
                join_result.error.unwrap_or_default()
            );
        }

        // Update the channel in database
        sqlx::query!(
            r#"
            UPDATE slack_connections
            SET channel_id = $1, channel_name = $2, updated_at = NOW()
            WHERE id = $3
            "#,
            channel_id,
            channel_name,
            conn.id
        )
        .execute(&self.db)
        .await?;

        info!(channel = %channel_name, "Slack notification channel set");

        Ok(())
    }

    /// Send a notification
    pub async fn notify(&self, notification: NotificationType) -> Result<()> {
        let conn = match self.get_connection().await {
            Some(c) => c,
            None => return Ok(()), // Silently skip if not connected
        };

        let channel_id = match conn.channel_id {
            Some(id) => id,
            None => return Ok(()), // Silently skip if no channel set
        };

        let (text, blocks) = self.build_message(notification);

        let response = self
            .http
            .post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", conn.access_token))
            .json(&serde_json::json!({
                "channel": channel_id,
                "text": text,
                "blocks": blocks,
                "unfurl_links": false,
                "unfurl_media": false,
            }))
            .send()
            .await?;

        #[derive(Deserialize)]
        struct PostResponse {
            ok: bool,
            error: Option<String>,
        }

        let result: PostResponse = response.json().await?;

        if !result.ok {
            warn!(
                error = ?result.error,
                "Failed to send Slack notification"
            );
        }

        Ok(())
    }

    /// Build message text and blocks for a notification
    fn build_message(&self, notification: NotificationType) -> (String, serde_json::Value) {
        match notification {
            NotificationType::PrOpened {
                incident_id,
                pr_url,
                pr_title,
                pr_number,
                project,
                error_message,
                confidence,
            } => {
                let confidence_emoji = if confidence >= 0.9 {
                    ":white_check_mark:"
                } else if confidence >= 0.7 {
                    ":large_yellow_circle:"
                } else {
                    ":warning:"
                };

                let text = format!(
                    "CRISPR generated a fix for {} - PR #{}",
                    project, pr_number
                );

                let blocks = serde_json::json!([
                    {
                        "type": "header",
                        "text": {
                            "type": "plain_text",
                            "text": ":adhesive_bandage: Fix Generated",
                            "emoji": true
                        }
                    },
                    {
                        "type": "section",
                        "fields": [
                            {
                                "type": "mrkdwn",
                                "text": format!("*Project:*\n{}", project)
                            },
                            {
                                "type": "mrkdwn",
                                "text": format!("*Confidence:*\n{} {}%", confidence_emoji, (confidence * 100.0) as i32)
                            }
                        ]
                    },
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!("*Error:*\n```{}```", truncate(&error_message, 200))
                        }
                    },
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!("<{}|*{}*>\nPR #{} • Incident `{}`", pr_url, pr_title, pr_number, &incident_id.to_string()[..8])
                        }
                    },
                    {
                        "type": "actions",
                        "elements": [
                            {
                                "type": "button",
                                "text": {
                                    "type": "plain_text",
                                    "text": "Review PR",
                                    "emoji": true
                                },
                                "url": pr_url,
                                "style": "primary"
                            }
                        ]
                    },
                    {
                        "type": "divider"
                    }
                ]);

                (text, blocks)
            }

            NotificationType::PrMerged {
                incident_id,
                pr_url,
                pr_number,
                project,
            } => {
                let text = format!("PR #{} merged for {} - Patient discharged!", pr_number, project);

                let blocks = serde_json::json!([
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!(
                                ":white_check_mark: *Patient Discharged*\n\n<{}|PR #{}> for *{}* has been merged.\nIncident `{}` resolved.",
                                pr_url, pr_number, project, &incident_id.to_string()[..8]
                            )
                        }
                    },
                    {
                        "type": "divider"
                    }
                ]);

                (text, blocks)
            }

            NotificationType::IncidentDetected {
                incident_id,
                project,
                error_message,
                level,
                sample_count,
            } => {
                let level_emoji = match level.to_uppercase().as_str() {
                    "ERROR" => ":red_circle:",
                    "WARN" | "WARNING" => ":large_yellow_circle:",
                    _ => ":large_blue_circle:",
                };

                let text = format!("New {} in {}", level, project);

                let blocks = serde_json::json!([
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!(
                                "{} *New {} Detected*\n\n*Project:* {}\n*Occurrences:* {}\n*Incident:* `{}`\n\n```{}```",
                                level_emoji, level, project, sample_count, &incident_id.to_string()[..8], truncate(&error_message, 200)
                            )
                        }
                    },
                    {
                        "type": "context",
                        "elements": [
                            {
                                "type": "mrkdwn",
                                "text": "CRISPR is analyzing this error..."
                            }
                        ]
                    },
                    {
                        "type": "divider"
                    }
                ]);

                (text, blocks)
            }

            NotificationType::FixFailed {
                incident_id,
                project,
                error_message,
                reason,
            } => {
                let text = format!("Fix failed for {} - manual review needed", project);

                let blocks = serde_json::json!([
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!(
                                ":x: *Fix Could Not Be Generated*\n\n*Project:* {}\n*Incident:* `{}`\n*Reason:* {}",
                                project, &incident_id.to_string()[..8], reason
                            )
                        }
                    },
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!("*Error:*\n```{}```", truncate(&error_message, 200))
                        }
                    },
                    {
                        "type": "context",
                        "elements": [
                            {
                                "type": "mrkdwn",
                                "text": "This error requires manual attention."
                            }
                        ]
                    },
                    {
                        "type": "divider"
                    }
                ]);

                (text, blocks)
            }
        }
    }

    // Convenience methods for common notifications

    /// Send a notification that a PR was opened
    pub async fn notify_pr_opened(
        &self,
        incident_id: &uuid::Uuid,
        pr_url: &str,
        pr_title: &str,
        pr_number: i32,
        project: &str,
        error_message: &str,
        confidence: f32,
    ) -> Result<()> {
        self.notify(NotificationType::PrOpened {
            incident_id: *incident_id,
            pr_url: pr_url.to_string(),
            pr_title: pr_title.to_string(),
            pr_number,
            project: project.to_string(),
            error_message: error_message.to_string(),
            confidence,
        })
        .await
    }

    /// Send a notification that a PR was merged
    pub async fn notify_pr_merged(
        &self,
        incident_id: &uuid::Uuid,
        pr_url: &str,
        pr_number: i32,
        project: &str,
    ) -> Result<()> {
        self.notify(NotificationType::PrMerged {
            incident_id: *incident_id,
            pr_url: pr_url.to_string(),
            pr_number,
            project: project.to_string(),
        })
        .await
    }

    /// Send a notification that a new incident was detected
    pub async fn notify_incident_detected(
        &self,
        incident_id: &uuid::Uuid,
        project: &str,
        error_message: &str,
        level: &str,
        sample_count: i32,
    ) -> Result<()> {
        self.notify(NotificationType::IncidentDetected {
            incident_id: *incident_id,
            project: project.to_string(),
            error_message: error_message.to_string(),
            level: level.to_string(),
            sample_count,
        })
        .await
    }

    /// Send a notification that fix generation failed
    pub async fn notify_fix_failed(
        &self,
        incident_id: &uuid::Uuid,
        project: &str,
        error_message: &str,
        reason: &str,
    ) -> Result<()> {
        self.notify(NotificationType::FixFailed {
            incident_id: *incident_id,
            project: project.to_string(),
            error_message: error_message.to_string(),
            reason: reason.to_string(),
        })
        .await
    }

    /// Disconnect Slack (remove connection from database)
    pub async fn disconnect(&self) -> Result<()> {
        sqlx::query!("DELETE FROM slack_connections")
            .execute(&self.db)
            .await?;

        info!("Slack disconnected");
        Ok(())
    }

    /// Test the connection by sending a test message
    pub async fn test_connection(&self) -> Result<()> {
        let conn = self.get_connection().await.ok_or_else(|| {
            anyhow::anyhow!("Slack not connected")
        })?;

        let channel_id = conn.channel_id.ok_or_else(|| {
            anyhow::anyhow!("No channel selected")
        })?;

        let response = self
            .http
            .post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", conn.access_token))
            .json(&serde_json::json!({
                "channel": channel_id,
                "text": ":wave: CRISPR is connected and ready to send notifications!",
                "unfurl_links": false,
            }))
            .send()
            .await?;

        #[derive(Deserialize)]
        struct PostResponse {
            ok: bool,
            error: Option<String>,
        }

        let result: PostResponse = response.json().await?;

        if !result.ok {
            return Err(anyhow::anyhow!(
                "Test message failed: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        Ok(())
    }
}

/// Truncate a string to a maximum length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Initialize Slack client
pub async fn init(config: &SlackConfig, db: &PgPool) -> Result<Arc<SlackClient>> {
    let client = SlackClient::new(config.clone(), db.clone());

    if config.enabled {
        if let Some(conn) = client.get_connection().await {
            if conn.channel_id.is_some() {
                info!(
                    team = ?conn.team_name,
                    channel = ?conn.channel_name,
                    "Slack connected"
                );
            } else {
                info!(team = ?conn.team_name, "Slack connected but no channel selected");
            }
        } else {
            info!("Slack enabled but not connected yet");
        }
    }

    Ok(Arc::new(client))
}

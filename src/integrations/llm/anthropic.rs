//! Anthropic Claude API integration

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmProvider, LlmResponse, Model};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<LlmResponse> {
        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system.map(|s| s.to_string()),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/messages", ANTHROPIC_API_URL))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Anthropic API error {}: {}",
                status,
                error_text
            ));
        }

        let response: MessagesResponse = response.json().await?;

        let content = response
            .content
            .into_iter()
            .filter_map(|c| {
                if c.content_type == "text" {
                    Some(c.text)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(LlmResponse {
            content,
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            model: response.model,
            stop_reason: response.stop_reason,
        })
    }

    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// List available Anthropic models
pub async fn list_models(api_key: &str) -> Result<Vec<Model>> {
    // Anthropic doesn't have a models list endpoint, so we return known models
    // These are current as of 2024
    let models = vec![
        Model {
            id: "claude-3-5-haiku-20241022".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
            context_window: Some(200000),
            supports_vision: Some(true),
        },
        Model {
            id: "claude-3-5-sonnet-20241022".to_string(),
            name: "Claude 3.5 Sonnet".to_string(),
            context_window: Some(200000),
            supports_vision: Some(true),
        },
        Model {
            id: "claude-sonnet-4-20250514".to_string(),
            name: "Claude Sonnet 4".to_string(),
            context_window: Some(200000),
            supports_vision: Some(true),
        },
        Model {
            id: "claude-opus-4-20250514".to_string(),
            name: "Claude Opus 4".to_string(),
            context_window: Some(200000),
            supports_vision: Some(true),
        },
        Model {
            id: "claude-3-haiku-20240307".to_string(),
            name: "Claude 3 Haiku".to_string(),
            context_window: Some(200000),
            supports_vision: Some(true),
        },
    ];

    // Verify the API key works by making a minimal request
    if !api_key.is_empty() {
        let client = Client::new();
        let response = client
            .post(format!("{}/messages", ANTHROPIC_API_URL))
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&MessagesRequest {
                model: "claude-3-haiku-20240307".to_string(),
                max_tokens: 1,
                system: None,
                messages: vec![Message {
                    role: "user".to_string(),
                    content: "hi".to_string(),
                }],
            })
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Anthropic API key validation failed {}: {}",
                status,
                error_text
            ));
        }
    }

    Ok(models)
}

// API request/response types

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: i32,
    output_tokens: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_list() {
        // Just verify we have models defined
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let models = list_models("").await.unwrap();
            assert!(!models.is_empty());
            assert!(models.iter().any(|m| m.id.contains("haiku")));
        });
    }
}

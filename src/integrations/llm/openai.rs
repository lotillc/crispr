//! OpenAI GPT API integration

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmProvider, LlmResponse, Model};

const OPENAI_API_URL: &str = "https://api.openai.com/v1";

/// OpenAI API provider
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: &str, model: &str, base_url: Option<&str>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: base_url.unwrap_or(OPENAI_API_URL).to_string(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<LlmResponse> {
        let mut messages = Vec::new();

        if let Some(sys) = system {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: Some(4096),
            temperature: Some(0.0),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "OpenAI API error {}: {}",
                status,
                error_text
            ));
        }

        let response: ChatCompletionResponse = response.json().await?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No response choices returned"))?;

        Ok(LlmResponse {
            content: choice.message.content,
            input_tokens: response.usage.prompt_tokens,
            output_tokens: response.usage.completion_tokens,
            model: response.model,
            stop_reason: choice.finish_reason,
        })
    }

    fn provider_name(&self) -> &str {
        "openai"
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// List available OpenAI models
pub async fn list_models(api_key: &str, base_url: Option<&str>) -> Result<Vec<Model>> {
    let base = base_url.unwrap_or(OPENAI_API_URL);
    let client = Client::new();

    let response = client
        .get(format!("{}/models", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OpenAI API error {}: {}",
            status,
            error_text
        ));
    }

    let response: ModelsResponse = response.json().await?;

    // Filter to chat models only
    let models: Vec<Model> = response
        .data
        .into_iter()
        .filter(|m| {
            m.id.starts_with("gpt-")
                || m.id.starts_with("o1")
                || m.id.starts_with("o3")
                || m.id.contains("turbo")
        })
        .map(|m| {
            let context_window = estimate_context_window(&m.id);
            Model {
                id: m.id.clone(),
                name: format_model_name(&m.id),
                context_window: Some(context_window),
                supports_vision: Some(m.id.contains("vision") || m.id.contains("gpt-4")),
            }
        })
        .collect();

    Ok(models)
}

fn estimate_context_window(model_id: &str) -> i32 {
    if model_id.contains("32k") {
        32768
    } else if model_id.contains("16k") {
        16384
    } else if model_id.starts_with("gpt-4o") {
        128000
    } else if model_id.starts_with("gpt-4-turbo") {
        128000
    } else if model_id.starts_with("gpt-4") {
        8192
    } else if model_id.starts_with("o1") || model_id.starts_with("o3") {
        128000
    } else {
        4096
    }
}

fn format_model_name(model_id: &str) -> String {
    // Convert model IDs to friendly names
    match model_id {
        "gpt-4o" => "GPT-4o".to_string(),
        "gpt-4o-mini" => "GPT-4o Mini".to_string(),
        "gpt-4-turbo" => "GPT-4 Turbo".to_string(),
        "gpt-4" => "GPT-4".to_string(),
        "gpt-3.5-turbo" => "GPT-3.5 Turbo".to_string(),
        "o1" => "o1".to_string(),
        "o1-mini" => "o1 Mini".to_string(),
        "o1-preview" => "o1 Preview".to_string(),
        other => other.to_string(),
    }
}

// API request/response types

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    model: String,
    choices: Vec<Choice>,
    usage: ChatUsage,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
}

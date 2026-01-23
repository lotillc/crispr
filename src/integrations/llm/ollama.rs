//! Ollama local model integration
//!
//! Ollama runs models locally, so there's no API key needed.
//! Default endpoint: http://localhost:11434

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmProvider, LlmResponse, Model};

/// Ollama local provider
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<LlmResponse> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            system: system.map(|s| s.to_string()),
            stream: Some(false),
            options: Some(GenerateOptions {
                temperature: Some(0.0),
                num_predict: Some(4096),
            }),
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Ollama API error {}: {}",
                status,
                error_text
            ));
        }

        let response: GenerateResponse = response.json().await?;

        // Ollama doesn't return token counts directly in all cases
        // We estimate based on response length if not provided
        let input_tokens = response.prompt_eval_count.unwrap_or_else(|| {
            // Rough estimate: ~4 chars per token
            (prompt.len() / 4) as i32
        });
        let output_tokens = response.eval_count.unwrap_or_else(|| {
            (response.response.len() / 4) as i32
        });

        Ok(LlmResponse {
            content: response.response,
            input_tokens,
            output_tokens,
            model: response.model,
            stop_reason: Some(if response.done { "stop" } else { "length" }.to_string()),
        })
    }

    fn provider_name(&self) -> &str {
        "local"
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// List available Ollama models
pub async fn list_models(base_url: &str) -> Result<Vec<Model>> {
    let client = Client::new();
    let base = base_url.trim_end_matches('/');

    let response = client
        .get(format!("{}/api/tags", base))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Ollama API error {}: {}",
            status,
            error_text
        ));
    }

    let response: TagsResponse = response.json().await?;

    let models = response
        .models
        .into_iter()
        .map(|m| {
            let context_window = estimate_context_window(&m.name);
            Model {
                id: m.name.clone(),
                name: format_model_name(&m.name),
                context_window: Some(context_window),
                supports_vision: Some(m.name.contains("llava") || m.name.contains("vision")),
            }
        })
        .collect();

    Ok(models)
}

fn estimate_context_window(model_name: &str) -> i32 {
    // Common context windows for popular models
    if model_name.contains("llama3") || model_name.contains("llama-3") {
        8192
    } else if model_name.contains("llama2") || model_name.contains("llama-2") {
        4096
    } else if model_name.contains("mistral") {
        8192
    } else if model_name.contains("mixtral") {
        32768
    } else if model_name.contains("codellama") {
        16384
    } else if model_name.contains("qwen") {
        32768
    } else if model_name.contains("deepseek") {
        16384
    } else {
        4096
    }
}

fn format_model_name(model_name: &str) -> String {
    // Clean up model names for display
    let name = model_name
        .replace(":latest", "")
        .replace("-", " ")
        .replace("_", " ");

    // Capitalize first letter of each word
    name.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// API request/response types

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenerateOptions>,
}

#[derive(Serialize)]
struct GenerateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    model: String,
    response: String,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<i32>,
    #[serde(default)]
    eval_count: Option<i32>,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_model_name() {
        assert_eq!(format_model_name("llama3:latest"), "Llama3");
        assert_eq!(format_model_name("codellama-7b"), "Codellama 7b");
        assert_eq!(format_model_name("mistral_instruct"), "Mistral Instruct");
    }
}

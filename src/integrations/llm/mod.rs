//! LLM Integration Layer
//!
//! Supports three providers:
//! - Anthropic (Claude models)
//! - OpenAI (GPT models)
//! - Ollama (local models)
//!
//! Uses a reader/writer/internist split to optimize for different tasks:
//! - Reader (cheap): triage, context gathering, fingerprinting
//! - Writer (expensive): fix generation, PR descriptions
//! - Internist (high reasoning): pattern analysis, architectural recommendations

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use crate::api::metrics;
use crate::config::LlmConfigFile;

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod pricing;

/// Model information returned from providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub context_window: Option<i32>,
    pub supports_vision: Option<bool>,
}

/// Response from an LLM completion
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub model: String,
    pub stop_reason: Option<String>,
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a completion request
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<LlmResponse>;

    /// Get the provider name (anthropic, openai, local)
    fn provider_name(&self) -> &str;

    /// Get the model name
    fn model_name(&self) -> &str;
}

/// Role of the LLM (reader = cheap, writer = expensive, internist = high reasoning)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmRole {
    Reader,
    Writer,
    Internist,
}

impl LlmRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Writer => "writer",
            Self::Internist => "internist",
        }
    }
}

/// LLM Manager - holds configured providers and handles usage tracking
pub struct LlmManager {
    db: PgPool,
    reader: RwLock<Option<Arc<dyn LlmProvider>>>,
    writer: RwLock<Option<Arc<dyn LlmProvider>>>,
    internist: RwLock<Option<Arc<dyn LlmProvider>>>,
    pricing: pricing::PricingConfig,
}

impl LlmManager {
    /// Create a new LLM manager and load config from database
    pub async fn new(db: PgPool) -> Result<Self> {
        let pricing = pricing::load_pricing()?;

        let manager = Self {
            db: db.clone(),
            reader: RwLock::new(None),
            writer: RwLock::new(None),
            internist: RwLock::new(None),
            pricing,
        };

        // Try to load existing config from database
        if let Err(e) = manager.reload().await {
            warn!(error = %e, "No LLM config in database yet");
        }

        Ok(manager)
    }

    /// Reload configuration from database
    pub async fn reload(&self) -> Result<()> {
        let config = sqlx::query!(
            r#"
            SELECT reader_provider, reader_model, reader_api_key, reader_base_url,
                   writer_provider, writer_model, writer_api_key, writer_base_url,
                   internist_provider, internist_model, internist_api_key, internist_base_url
            FROM llm_config
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(c) = config {
            // Create reader provider
            let reader = create_provider(
                &c.reader_provider,
                &c.reader_model,
                c.reader_api_key.as_deref(),
                c.reader_base_url.as_deref(),
            )?;
            *self.reader.write().await = Some(reader);

            // Create writer provider
            let writer = create_provider(
                &c.writer_provider,
                &c.writer_model,
                c.writer_api_key.as_deref(),
                c.writer_base_url.as_deref(),
            )?;
            *self.writer.write().await = Some(writer);

            // Create internist provider (optional)
            if let (Some(provider), Some(model)) = (&c.internist_provider, &c.internist_model) {
                match create_provider(
                    provider,
                    model,
                    c.internist_api_key.as_deref(),
                    c.internist_base_url.as_deref(),
                ) {
                    Ok(internist) => {
                        *self.internist.write().await = Some(internist);
                        info!(
                            reader = %c.reader_model,
                            writer = %c.writer_model,
                            internist = %model,
                            "LLM providers configured"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to configure internist LLM");
                        info!(
                            reader = %c.reader_model,
                            writer = %c.writer_model,
                            "LLM providers configured (internist disabled)"
                        );
                    }
                }
            } else {
                info!(
                    reader = %c.reader_model,
                    writer = %c.writer_model,
                    "LLM providers configured (internist not configured)"
                );
            }
        }

        Ok(())
    }

    /// Get the reader LLM (cheap, for triage/context)
    pub async fn reader(&self) -> Option<TrackedLlm> {
        self.reader.read().await.as_ref().map(|p| TrackedLlm {
            inner: p.clone(),
            db: self.db.clone(),
            role: LlmRole::Reader,
            pricing: self.pricing.clone(),
        })
    }

    /// Get the writer LLM (expensive, for fixes)
    pub async fn writer(&self) -> Option<TrackedLlm> {
        self.writer.read().await.as_ref().map(|p| TrackedLlm {
            inner: p.clone(),
            db: self.db.clone(),
            role: LlmRole::Writer,
            pricing: self.pricing.clone(),
        })
    }

    /// Get the internist LLM (high reasoning, for pattern analysis)
    pub async fn internist(&self) -> Option<TrackedLlm> {
        self.internist.read().await.as_ref().map(|p| TrackedLlm {
            inner: p.clone(),
            db: self.db.clone(),
            role: LlmRole::Internist,
            pricing: self.pricing.clone(),
        })
    }

    /// Check if LLMs are configured
    pub async fn is_configured(&self) -> bool {
        self.reader.read().await.is_some() && self.writer.read().await.is_some()
    }

    /// Check if internist LLM is configured
    pub async fn is_internist_configured(&self) -> bool {
        self.internist.read().await.is_some()
    }
}

/// Tracked LLM - wraps a provider and records usage
pub struct TrackedLlm {
    inner: Arc<dyn LlmProvider>,
    db: PgPool,
    role: LlmRole,
    pricing: pricing::PricingConfig,
}

impl TrackedLlm {
    /// Complete a prompt and track usage
    pub async fn complete(
        &self,
        prompt: &str,
        system: Option<&str>,
        operation: &str,
        incident_id: Option<Uuid>,
        project_id: Option<Uuid>,
    ) -> Result<LlmResponse> {
        let response = self.inner.complete(prompt, system).await?;

        // Record metrics
        metrics::record_llm_request(self.inner.provider_name(), operation);
        metrics::record_llm_tokens(
            self.inner.provider_name(),
            response.input_tokens,
            response.output_tokens,
        );

        // Calculate cost
        let cost = self.pricing.calculate_cost(
            self.inner.provider_name(),
            self.inner.model_name(),
            response.input_tokens,
            response.output_tokens,
        );

        // Record usage
        sqlx::query!(
            r#"
            INSERT INTO llm_usage
            (id, incident_id, project_id, role, provider, model, operation, input_tokens, output_tokens, cost_usd)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            Uuid::new_v4(),
            incident_id,
            project_id,
            self.role.as_str(),
            self.inner.provider_name(),
            self.inner.model_name(),
            operation,
            response.input_tokens,
            response.output_tokens,
            cost,
        )
        .execute(&self.db)
        .await?;

        info!(
            role = %self.role.as_str(),
            provider = %self.inner.provider_name(),
            model = %self.inner.model_name(),
            operation = %operation,
            input_tokens = response.input_tokens,
            output_tokens = response.output_tokens,
            cost_usd = %cost,
            "LLM request completed"
        );

        Ok(response)
    }

    /// Get the underlying provider name
    pub fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    /// Get the underlying model name
    pub fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

/// Create a provider from configuration
fn create_provider(
    provider: &str,
    model: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<Arc<dyn LlmProvider>> {
    match provider {
        "anthropic" => {
            let key = api_key.ok_or_else(|| anyhow::anyhow!("Anthropic requires API key"))?;
            Ok(Arc::new(anthropic::AnthropicProvider::new(key, model)))
        }
        "openai" => {
            let key = api_key.ok_or_else(|| anyhow::anyhow!("OpenAI requires API key"))?;
            Ok(Arc::new(openai::OpenAiProvider::new(key, model, base_url)))
        }
        "local" | "ollama" => {
            let url = base_url.unwrap_or("http://localhost:11434");
            Ok(Arc::new(ollama::OllamaProvider::new(url, model)))
        }
        _ => Err(anyhow::anyhow!("Unknown provider: {}", provider)),
    }
}

/// List available models for a provider
pub async fn list_models(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<Vec<Model>> {
    match provider {
        "anthropic" => anthropic::list_models(api_key.unwrap_or("")).await,
        "openai" => openai::list_models(api_key.unwrap_or(""), base_url).await,
        "local" | "ollama" => {
            ollama::list_models(base_url.unwrap_or("http://localhost:11434")).await
        }
        _ => Err(anyhow::anyhow!("Unknown provider: {}", provider)),
    }
}

/// Verify that credentials work for a provider
pub async fn verify_credentials(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<bool> {
    match list_models(provider, api_key, base_url).await {
        Ok(models) => Ok(!models.is_empty()),
        Err(_) => Ok(false),
    }
}

/// Initialize LLM manager from config
pub async fn init(config: &Option<LlmConfigFile>, db: &PgPool) -> Result<Arc<LlmManager>> {
    let manager = LlmManager::new(db.clone()).await?;

    // If config file has LLM settings, save them to database
    if let Some(llm_config) = config {
        if let (Some(reader), Some(writer)) = (&llm_config.reader, &llm_config.writer) {
            // Check if config already exists
            let existing = sqlx::query!("SELECT id FROM llm_config LIMIT 1")
                .fetch_optional(db)
                .await?;

            if existing.is_none() {
                info!("Saving LLM config from file to database");
                sqlx::query!(
                    r#"
                    INSERT INTO llm_config
                    (id, reader_provider, reader_model, reader_api_key, reader_base_url,
                     writer_provider, writer_model, writer_api_key, writer_base_url)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    "#,
                    Uuid::new_v4(),
                    reader.provider,
                    reader.model,
                    reader.api_key,
                    reader.base_url,
                    writer.provider,
                    writer.model,
                    writer.api_key,
                    writer.base_url,
                )
                .execute(db)
                .await?;

                manager.reload().await?;
            }
        }
    }

    Ok(Arc::new(manager))
}

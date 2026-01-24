use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub environment: String,
    pub display_name: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub github: GitHubConfig,
    pub slack: SlackConfig,
    pub llm: Option<LlmConfigFile>,
    pub safety: SafetyConfig,
    pub pipeline: PipelineConfig,
    #[serde(default)]
    pub worker: WorkerConfig,
    #[serde(default)]
    pub ingest: IngestConfig,
    #[serde(default)]
    pub webhooks: Option<WebhooksConfig>,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub http_port: u16,
    pub grpc_port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub mode: String, // "postgres" | "object_store"
    pub object_store: Option<ObjectStoreConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectStoreConfig {
    pub provider: String, // "s3" | "azure" | "gcp" | "minio" | "r2"
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>, // for minio/r2
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubConfig {
    pub mode: String, // "pat" | "oauth"
    pub personal_access_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackConfig {
    pub enabled: bool,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfigFile {
    pub reader: Option<LlmProviderConfig>,
    pub writer: Option<LlmProviderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmProviderConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SafetyConfig {
    pub allowed_repos: Option<Vec<String>>,
    pub dry_run: bool,
    pub pr_branch_prefix: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineConfig {
    pub cooldown_hours: u32,
    pub max_fix_attempts: u32,
    pub auto_approve: bool,
    #[serde(default = "default_repos_dir")]
    pub repos_dir: String,
}

fn default_repos_dir() -> String {
    "./repos".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default = "generate_worker_id")]
    pub id: String,
    #[serde(default = "default_claim_timeout")]
    pub claim_timeout_minutes: u32,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_seconds: u32,
}

fn generate_worker_id() -> String {
    use std::process;
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    format!("{}-{}", hostname, process::id())
}

fn default_claim_timeout() -> u32 {
    15 // minutes
}

fn default_heartbeat_interval() -> u32 {
    30 // seconds
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            id: generate_worker_id(),
            claim_timeout_minutes: default_claim_timeout(),
            heartbeat_interval_seconds: default_heartbeat_interval(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestConfig {
    #[serde(default = "default_rate_limit_per_second")]
    pub rate_limit_per_second: u32,
}

fn default_rate_limit_per_second() -> u32 {
    100
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            rate_limit_per_second: default_rate_limit_per_second(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WebhooksConfig {
    /// Secret for verifying Sentry webhook signatures
    pub sentry_secret: Option<String>,
    /// Token for verifying Datadog webhook requests
    pub datadog_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

pub fn load() -> Result<Config> {
    let config_path = std::env::var("CRISPR_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/config.dev.yaml"));

    let settings = config::Config::builder()
        .add_source(config::File::from(config_path))
        .add_source(config::Environment::with_prefix("CRISPR").separator("__"))
        .build()?;

    let config: Config = settings.try_deserialize()?;
    Ok(config)
}

pub fn validate(config: &Config) -> Result<()> {
    // Dev environment must have safety rails
    if config.environment == "development" {
        if config.safety.allowed_repos.is_none() && !config.safety.dry_run {
            return Err(anyhow!(
                "Development environment must set allowed_repos or dry_run=true"
            ));
        }
    }

    // Validate GitHub config
    match config.github.mode.as_str() {
        "pat" => {
            if config.github.personal_access_token.is_none() {
                return Err(anyhow!("GitHub PAT mode requires personal_access_token"));
            }
        }
        "oauth" => {
            if config.github.client_id.is_none() || config.github.client_secret.is_none() {
                return Err(anyhow!(
                    "GitHub OAuth mode requires client_id and client_secret"
                ));
            }
        }
        _ => return Err(anyhow!("Invalid GitHub mode: {}", config.github.mode)),
    }

    // Validate Slack config if enabled
    if config.slack.enabled {
        if config.slack.client_id.is_none() || config.slack.client_secret.is_none() {
            return Err(anyhow!(
                "Slack enabled but missing client_id or client_secret"
            ));
        }
    }

    Ok(())
}

pub fn init_logging(config: &Config) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    match config.logging.format.as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().json())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().pretty())
                .init();
        }
    }
}

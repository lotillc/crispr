# CRISPR - Continuous Repair and Incident Self-Patching Runtime

## Overview

CRISPR is a self-hosted, vendor-agnostic production error monitoring system that automatically generates fixes and opens pull requests. It sits downstream of your existing logging infrastructure, receives errors via HTTP or OTLP, and uses LLMs to diagnose and fix issues.

**Key differentiators:**
- Works with any log collector (Fluent Bit, Vector, OTel Collector, Datadog, etc.)
- Self-hosted, no data leaves your infrastructure
- Bring your own LLM (Anthropic, OpenAI, or local via Ollama)
- Transparent cost tracking
- Progressive scaling (Postgres-only → Postgres + S3)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CRISPR Server                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────────┐    │
│  │     Web UI       │  │    REST API      │  │     Ingest API         │    │
│  │    (Svelte)      │  │   /api/v1/*      │  │  POST /ingest (HTTP)   │    │
│  │                  │  │                  │  │  :4317 (OTLP gRPC)     │    │
│  └────────┬─────────┘  └────────┬─────────┘  └───────────┬────────────┘    │
│           │                     │                        │                  │
│           └─────────────────────┼────────────────────────┘                  │
│                                 │                                           │
│  ┌──────────────────────────────┴──────────────────────────────────────┐   │
│  │                         Pipeline Engine                              │   │
│  │                                                                      │   │
│  │  ┌────────────┐  ┌────────┐  ┌─────────┐  ┌─────┐  ┌────────────┐   │   │
│  │  │ Fingerprint│─▶│ Triage │─▶│ Context │─▶│ Fix │─▶│ PR/Notify  │   │   │
│  │  │  (reader)  │  │(reader)│  │(reader) │  │(wrt)│  │  (writer)  │   │   │
│  │  └────────────┘  └────────┘  └─────────┘  └─────┘  └────────────┘   │   │
│  │                                                                      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                 │                                           │
│           ┌─────────────────────┼─────────────────────┐                     │
│           │                     │                     │                     │
│  ┌────────┴────────┐  ┌─────────┴─────────┐  ┌───────┴────────┐            │
│  │    Postgres     │  │   Object Store    │  │  Integrations  │            │
│  │    (state)      │  │   (samples)       │  │ GitHub, Slack  │            │
│  └─────────────────┘  └───────────────────┘  └────────────────┘            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
crispr/
├── Cargo.toml
├── Cargo.lock
├── config/
│   ├── config.dev.yaml
│   ├── config.prod.yaml
│   └── pricing.toml
├── migrations/
│   ├── 001_initial.sql
│   └── 002_llm_usage.sql
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── error.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── router.rs
│   │   ├── auth.rs
│   │   ├── projects.rs
│   │   ├── incidents.rs
│   │   ├── settings.rs
│   │   └── ingest.rs
│   ├── pipeline/
│   │   ├── mod.rs
│   │   ├── fingerprint.rs
│   │   ├── triage.rs
│   │   ├── context.rs
│   │   ├── fix.rs
│   │   └── pr.rs
│   ├── integrations/
│   │   ├── mod.rs
│   │   ├── github.rs
│   │   ├── slack.rs
│   │   └── llm/
│   │       ├── mod.rs
│   │       ├── provider.rs
│   │       ├── anthropic.rs
│   │       ├── openai.rs
│   │       ├── ollama.rs
│   │       └── pricing.rs
│   ├── store/
│   │   ├── mod.rs
│   │   ├── postgres.rs
│   │   ├── object.rs
│   │   └── models.rs
│   └── worker.rs
├── ui/
│   ├── package.json
│   ├── svelte.config.js
│   ├── vite.config.js
│   ├── src/
│   │   ├── app.html
│   │   ├── app.css
│   │   ├── lib/
│   │   │   ├── api.ts
│   │   │   ├── stores.ts
│   │   │   └── components/
│   │   │       ├── EnvBanner.svelte
│   │   │       ├── LlmConfig.svelte
│   │   │       ├── UsageChart.svelte
│   │   │       └── IncidentList.svelte
│   │   └── routes/
│   │       ├── +layout.svelte
│   │       ├── +page.svelte
│   │       ├── connect/
│   │       ├── projects/
│   │       ├── incidents/
│   │       ├── settings/
│   │       └── usage/
│   └── static/
├── Dockerfile
├── docker-compose.yaml
├── docker-compose.dev.yaml
└── README.md
```

## Database Schema

```sql
-- migrations/001_initial.sql

-- Environment and instance tracking
CREATE TABLE instance_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    environment TEXT NOT NULL,  -- 'development' | 'staging' | 'production'
    display_name TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- GitHub connection
CREATE TABLE github_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mode TEXT NOT NULL,  -- 'pat' | 'oauth'
    access_token TEXT NOT NULL,  -- encrypted
    refresh_token TEXT,
    token_expires_at TIMESTAMPTZ,
    user_login TEXT,
    user_id BIGINT,
    scopes TEXT[],
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Slack connection
CREATE TABLE slack_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    access_token TEXT NOT NULL,  -- encrypted
    team_id TEXT NOT NULL,
    team_name TEXT,
    channel_id TEXT,
    channel_name TEXT,
    bot_user_id TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Projects (repos we watch)
CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    github_owner TEXT NOT NULL,
    github_repo TEXT NOT NULL,
    default_branch TEXT DEFAULT 'main',
    language TEXT,  -- detected or configured
    enabled BOOLEAN DEFAULT true,
    auto_registered BOOLEAN DEFAULT false,
    settings JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(github_owner, github_repo)
);

CREATE INDEX idx_projects_enabled ON projects(enabled) WHERE enabled = true;

-- Incidents (unique errors)
CREATE TABLE incidents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    title TEXT,
    state TEXT DEFAULT 'detected',
    severity TEXT,  -- 'low' | 'medium' | 'high' | 'critical'
    occurrence_count INT DEFAULT 1,
    first_seen TIMESTAMPTZ DEFAULT NOW(),
    last_seen TIMESTAMPTZ DEFAULT NOW(),
    cooldown_until TIMESTAMPTZ,
    sample_location TEXT,  -- 'pg:{uuid}' or 'obj:{key}'
    fix_attempts INT DEFAULT 0,
    pr_url TEXT,
    pr_number INT,
    pr_state TEXT,  -- 'open' | 'merged' | 'closed'
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(project_id, fingerprint)
);

CREATE INDEX idx_incidents_project_state ON incidents(project_id, state);
CREATE INDEX idx_incidents_last_seen ON incidents(last_seen DESC);
CREATE INDEX idx_incidents_cooldown ON incidents(cooldown_until) WHERE cooldown_until IS NOT NULL;

-- Incident state enum for reference:
-- 'detected'    - error received, not yet processed
-- 'triaging'    - LLM analyzing
-- 'unfixable'   - triage determined not auto-fixable
-- 'fixing'      - generating fix
-- 'fix_failed'  - fix generation failed
-- 'pr_opening'  - creating PR
-- 'pr_open'     - PR created, awaiting review
-- 'pr_merged'   - PR was merged
-- 'pr_closed'   - PR was closed without merge
-- 'cooldown'    - waiting before retry

-- Samples (log payloads, stored in Postgres or Object Store)
CREATE TABLE samples (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    payload BYTEA NOT NULL,  -- zstd compressed
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_samples_incident ON samples(incident_id);

-- Activity log (audit trail)
CREATE TABLE activity_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id UUID REFERENCES incidents(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    details JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_activity_incident ON activity_log(incident_id, created_at DESC);
CREATE INDEX idx_activity_project ON activity_log(project_id, created_at DESC);


-- migrations/002_llm_usage.sql

-- LLM configuration
CREATE TABLE llm_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    reader_provider TEXT NOT NULL,
    reader_model TEXT NOT NULL,
    reader_api_key TEXT,  -- encrypted, null for local
    reader_base_url TEXT,
    
    writer_provider TEXT NOT NULL,
    writer_model TEXT NOT NULL,
    writer_api_key TEXT,  -- encrypted
    writer_base_url TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- LLM usage tracking
CREATE TABLE llm_usage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id UUID REFERENCES incidents(id) ON DELETE SET NULL,
    project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
    
    role TEXT NOT NULL,  -- 'reader' | 'writer'
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    operation TEXT NOT NULL,  -- 'fingerprint' | 'triage' | 'context' | 'fix' | 'pr_description'
    
    input_tokens INT NOT NULL,
    output_tokens INT NOT NULL,
    cost_usd DECIMAL(10, 6),
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_llm_usage_project ON llm_usage(project_id, created_at DESC);
CREATE INDEX idx_llm_usage_created ON llm_usage(created_at DESC);
CREATE INDEX idx_llm_usage_role ON llm_usage(role, created_at DESC);

-- Custom pricing overrides (for new models or fine-tunes)
CREATE TABLE llm_pricing_overrides (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_per_mtok DECIMAL(10, 4) NOT NULL,
    output_per_mtok DECIMAL(10, 4) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(provider, model)
);
```

## Configuration

```yaml
# config/config.dev.yaml

environment: development
display_name: "CRISPR (local dev)"

server:
  host: "127.0.0.1"
  http_port: 8080
  grpc_port: 4317

database:
  url: "postgres://crispr:crispr@localhost:5432/crispr_dev"
  max_connections: 5

storage:
  mode: postgres  # samples stored in postgres

github:
  mode: pat
  personal_access_token: ${GITHUB_PAT}

slack:
  enabled: false

llm:
  # Can be pre-configured or set via UI
  # reader:
  #   provider: anthropic
  #   api_key: ${ANTHROPIC_API_KEY}
  #   model: claude-3-haiku-20240307
  # writer:
  #   provider: anthropic
  #   api_key: ${ANTHROPIC_API_KEY}
  #   model: claude-sonnet-4-20250514

safety:
  allowed_repos:
    - "youruser/test-repo"
    - "youruser/*-sandbox"
  dry_run: false
  pr_branch_prefix: "crispr-dev/"

pipeline:
  cooldown_hours: 24
  max_fix_attempts: 3
  auto_approve: false  # require human approval before PR

logging:
  level: debug
  format: pretty  # pretty | json
```

```yaml
# config/config.prod.yaml

environment: production
display_name: "CRISPR"

server:
  host: "0.0.0.0"
  http_port: 8080
  grpc_port: 4317

database:
  url: ${DATABASE_URL}
  max_connections: 20

storage:
  mode: object_store
  object_store:
    provider: s3  # s3 | azure | gcp | r2 | minio
    bucket: ${S3_BUCKET}
    region: ${AWS_REGION}
    # credentials from env: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY

github:
  mode: oauth
  client_id: ${GITHUB_CLIENT_ID}
  client_secret: ${GITHUB_CLIENT_SECRET}

slack:
  enabled: true
  client_id: ${SLACK_CLIENT_ID}
  client_secret: ${SLACK_CLIENT_SECRET}

safety:
  # No restrictions in prod
  dry_run: false
  pr_branch_prefix: "crispr/"

pipeline:
  cooldown_hours: 48
  max_fix_attempts: 3
  auto_approve: false

logging:
  level: info
  format: json
```

```toml
# config/pricing.toml
# Default pricing, can be overridden in DB

[anthropic]
"claude-3-haiku-20240307" = { input = 0.25, output = 1.25 }
"claude-3-5-haiku-20241022" = { input = 0.80, output = 4.00 }
"claude-3-5-sonnet-20241022" = { input = 3.00, output = 15.00 }
"claude-sonnet-4-20250514" = { input = 3.00, output = 15.00 }
"claude-opus-4-20250514" = { input = 15.00, output = 75.00 }

[openai]
"gpt-4o-mini" = { input = 0.15, output = 0.60 }
"gpt-4o" = { input = 2.50, output = 10.00 }
"gpt-4-turbo" = { input = 10.00, output = 30.00 }
"o1" = { input = 15.00, output = 60.00 }
"o1-mini" = { input = 3.00, output = 12.00 }

[local]
# All local models are free
"*" = { input = 0.0, output = 0.0 }
```

## Rust Implementation

### Cargo.toml

```toml
[package]
name = "crispr"
version = "0.1.0"
edition = "2021"
description = "Continuous Repair and Incident Self-Patching Runtime"
license = "MIT"

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Web framework
axum = { version = "0.7", features = ["macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors", "trace"] }

# gRPC for OTLP
tonic = "0.11"
prost = "0.12"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "uuid", "chrono", "rust_decimal"] }

# Object storage
object_store = { version = "0.9", features = ["aws", "azure", "gcp"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"

# HTTP client (for LLM APIs, GitHub, Slack)
reqwest = { version = "0.11", features = ["json", "rustls-tls"], default-features = false }

# Compression
zstd = "0.13"

# Crypto
sha2 = "0.10"
hex = "0.4"

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1", features = ["serde"] }
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
config = "0.14"
dotenvy = "0.15"
async-trait = "0.1"

# Embed UI assets
rust-embed = "8"

[build-dependencies]
tonic-build = "0.11"

[profile.release]
lto = true
codegen-units = 1
strip = true
```

### src/main.rs

```rust
use anyhow::Result;
use tracing::info;

mod api;
mod config;
mod error;
mod integrations;
mod pipeline;
mod store;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    
    let config = config::load()?;
    
    // Initialize logging
    config::init_logging(&config);
    
    info!(
        environment = %config.environment,
        display_name = %config.display_name,
        "Starting CRISPR"
    );
    
    // Validate configuration
    config::validate(&config)?;
    
    // Connect to database
    let db = store::postgres::connect(&config.database).await?;
    
    // Run migrations
    sqlx::migrate!("./migrations").run(&db).await?;
    
    // Initialize storage
    let storage = store::object::init(&config.storage, db.clone()).await?;
    
    // Initialize integrations
    let github = integrations::github::init(&config.github, &db).await?;
    let slack = integrations::slack::init(&config.slack, &db).await?;
    let llm = integrations::llm::init(&config.llm, &db).await?;
    
    // Build application state
    let state = api::AppState {
        config: config.clone(),
        db,
        storage,
        github,
        slack,
        llm,
    };
    
    // Start background worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::run(worker_state).await;
    });
    
    // Start gRPC server (OTLP)
    let grpc_state = state.clone();
    let grpc_addr = format!("{}:{}", config.server.host, config.server.grpc_port);
    tokio::spawn(async move {
        api::ingest::serve_grpc(grpc_state, &grpc_addr).await.unwrap();
    });
    
    // Start HTTP server
    let http_addr = format!("{}:{}", config.server.host, config.server.http_port);
    info!(addr = %http_addr, "HTTP server starting");
    
    let app = api::router::build(state);
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

### src/config.rs

```rust
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
    pub mode: String,  // "postgres" | "object_store"
    pub object_store: Option<ObjectStoreConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectStoreConfig {
    pub provider: String,  // "s3" | "azure" | "gcp" | "minio" | "r2"
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,  // for minio/r2
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubConfig {
    pub mode: String,  // "pat" | "oauth"
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

pub fn load() -> Result<Config> {
    let config_path = std::env::var("CRISPR_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config.yaml"));
    
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
                return Err(anyhow!("GitHub OAuth mode requires client_id and client_secret"));
            }
        }
        _ => return Err(anyhow!("Invalid GitHub mode: {}", config.github.mode)),
    }
    
    // Validate Slack config if enabled
    if config.slack.enabled {
        if config.slack.client_id.is_none() || config.slack.client_secret.is_none() {
            return Err(anyhow!("Slack enabled but missing client_id or client_secret"));
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
```

### src/api/mod.rs

```rust
use crate::config::Config;
use crate::integrations::{github::GitHubClient, llm::LlmManager, slack::SlackClient};
use crate::store::{object::SampleStore, postgres::DbPool};
use std::sync::Arc;

pub mod auth;
pub mod incidents;
pub mod ingest;
pub mod projects;
pub mod router;
pub mod settings;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub storage: SampleStore,
    pub github: Arc<GitHubClient>,
    pub slack: Arc<SlackClient>,
    pub llm: Arc<LlmManager>,
}
```

### src/api/router.rs

```rust
use axum::{
    routing::{get, post, patch, delete},
    Router,
};
use rust_embed::RustEmbed;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use super::*;

#[derive(RustEmbed)]
#[folder = "ui/build"]
struct UiAssets;

pub fn build(state: AppState) -> Router {
    let api = Router::new()
        // Auth
        .route("/api/v1/auth/status", get(auth::status))
        .route("/api/v1/auth/github", get(auth::github_redirect))
        .route("/api/v1/auth/github/callback", get(auth::github_callback))
        .route("/api/v1/auth/slack", get(auth::slack_redirect))
        .route("/api/v1/auth/slack/callback", get(auth::slack_callback))
        
        // Projects
        .route("/api/v1/projects", get(projects::list).post(projects::create))
        .route("/api/v1/projects/:id", get(projects::get).patch(projects::update).delete(projects::delete))
        
        // Incidents
        .route("/api/v1/incidents", get(incidents::list))
        .route("/api/v1/incidents/:id", get(incidents::get))
        .route("/api/v1/incidents/:id/retry", post(incidents::retry))
        .route("/api/v1/incidents/:id/ignore", post(incidents::ignore))
        
        // Settings
        .route("/api/v1/settings/llm", get(settings::get_llm).put(settings::update_llm))
        .route("/api/v1/settings/llm/verify", post(settings::verify_llm))
        .route("/api/v1/settings/llm/models", post(settings::list_models))
        
        // Usage
        .route("/api/v1/usage/summary", get(settings::usage_summary))
        .route("/api/v1/usage/details", get(settings::usage_details))
        
        // Ingest
        .route("/ingest", post(ingest::http_ingest))
        
        .with_state(state.clone());
    
    // Static UI assets
    let ui = Router::new()
        .fallback(serve_ui);
    
    Router::new()
        .merge(api)
        .merge(ui)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

async fn serve_ui(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    
    match UiAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback
            match UiAssets::get("index.html") {
                Some(content) => (
                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                    content.data.into_owned(),
                )
                    .into_response(),
                None => axum::http::StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}
```

### src/api/ingest.rs

```rust
use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::AppError;

#[derive(Debug, Deserialize)]
pub struct IngestPayload {
    // Project identification (at least one required)
    pub project: Option<String>,  // "owner/repo"
    pub service_name: Option<String>,
    pub kubernetes_namespace: Option<String>,
    pub kubernetes_pod: Option<String>,
    
    // Log data
    pub level: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    
    // Optional metadata
    pub attributes: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub accepted: bool,
    pub incident_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub message: String,
}

pub async fn http_ingest(
    State(state): State<AppState>,
    Json(payload): Json<IngestPayload>,
) -> Result<Json<IngestResponse>, AppError> {
    // Only process errors
    let level = payload.level.to_uppercase();
    if !matches!(level.as_str(), "ERROR" | "FATAL" | "CRITICAL") {
        return Ok(Json(IngestResponse {
            accepted: false,
            incident_id: None,
            project_id: None,
            message: "Only ERROR/FATAL/CRITICAL logs are processed".into(),
        }));
    }
    
    // Identify project
    let project_key = payload.project
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
        RETURNING *
        "#,
        Uuid::new_v4(),
        owner,
        repo
    )
    .fetch_one(&state.db)
    .await?;
    
    if !project.enabled {
        return Ok(Json(IngestResponse {
            accepted: false,
            incident_id: None,
            project_id: Some(project.id),
            message: "Project is disabled".into(),
        }));
    }
    
    // Fingerprint the error
    let fingerprint = crate::pipeline::fingerprint::compute(&payload);
    
    // Upsert incident
    let incident = sqlx::query_as!(
        Incident,
        r#"
        INSERT INTO incidents (id, project_id, fingerprint, title, state, first_seen, last_seen)
        VALUES ($1, $2, $3, $4, 'detected', NOW(), NOW())
        ON CONFLICT (project_id, fingerprint) 
        DO UPDATE SET 
            occurrence_count = incidents.occurrence_count + 1,
            last_seen = NOW(),
            state = CASE 
                WHEN incidents.state = 'cooldown' AND incidents.cooldown_until < NOW() THEN 'detected'
                ELSE incidents.state
            END
        RETURNING *
        "#,
        Uuid::new_v4(),
        project.id,
        fingerprint,
        truncate_title(&payload.message),
    )
    .fetch_one(&state.db)
    .await?;
    
    // Store sample if this is a new incident or we need more samples
    if incident.occurrence_count <= 3 {
        let sample_data = serde_json::to_vec(&payload)?;
        state.storage.put(incident.id, &sample_data).await?;
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
        serde_json::json!({ "occurrence": incident.occurrence_count }),
    )
    .execute(&state.db)
    .await?;
    
    Ok(Json(IngestResponse {
        accepted: true,
        incident_id: Some(incident.id),
        project_id: Some(project.id),
        message: "Ingested".into(),
    }))
}

fn infer_project(payload: &IngestPayload) -> Option<String> {
    // Try to infer from kubernetes metadata or service name
    // This is customizable per deployment
    payload.service_name.clone()
}

fn parse_project_key(key: &str) -> Result<(String, String), AppError> {
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest(format!("Invalid project key: {}", key)));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn is_repo_allowed(owner: &str, repo: &str, allowed: &[String]) -> bool {
    let full = format!("{}/{}", owner, repo);
    allowed.iter().any(|pattern| {
        if pattern.contains('*') {
            let pattern = pattern.replace("*", ".*");
            regex::Regex::new(&format!("^{}$", pattern))
                .map(|re| re.is_match(&full))
                .unwrap_or(false)
        } else {
            pattern == &full
        }
    })
}

fn truncate_title(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or(msg);
    if first_line.len() > 200 {
        format!("{}...", &first_line[..197])
    } else {
        first_line.to_string()
    }
}

// OTLP gRPC server
pub async fn serve_grpc(state: AppState, addr: &str) -> Result<(), anyhow::Error> {
    use tonic::transport::Server;
    
    // OTLP log service implementation
    let log_service = OtlpLogService { state };
    
    tracing::info!(addr = %addr, "gRPC server starting");
    
    Server::builder()
        .add_service(log_service.into_service())
        .serve(addr.parse()?)
        .await?;
    
    Ok(())
}
```

### src/api/settings.rs

```rust
use axum::{extract::State, Json};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::AppState;
use crate::error::AppError;
use crate::integrations::llm;

#[derive(Serialize)]
pub struct LlmConfigResponse {
    pub configured: bool,
    pub reader: Option<LlmProviderInfo>,
    pub writer: Option<LlmProviderInfo>,
}

#[derive(Serialize)]
pub struct LlmProviderInfo {
    pub provider: String,
    pub model: String,
    pub has_api_key: bool,
    pub base_url: Option<String>,
}

pub async fn get_llm(State(state): State<AppState>) -> Result<Json<LlmConfigResponse>, AppError> {
    let config = sqlx::query_as!(
        LlmConfigRow,
        "SELECT * FROM llm_config ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await?;
    
    match config {
        Some(c) => Ok(Json(LlmConfigResponse {
            configured: true,
            reader: Some(LlmProviderInfo {
                provider: c.reader_provider,
                model: c.reader_model,
                has_api_key: c.reader_api_key.is_some(),
                base_url: c.reader_base_url,
            }),
            writer: Some(LlmProviderInfo {
                provider: c.writer_provider,
                model: c.writer_model,
                has_api_key: c.writer_api_key.is_some(),
                base_url: c.writer_base_url,
            }),
        })),
        None => Ok(Json(LlmConfigResponse {
            configured: false,
            reader: None,
            writer: None,
        })),
    }
}

#[derive(Deserialize)]
pub struct UpdateLlmRequest {
    pub reader_provider: String,
    pub reader_model: String,
    pub reader_api_key: Option<String>,
    pub reader_base_url: Option<String>,
    
    pub writer_provider: String,
    pub writer_model: String,
    pub writer_api_key: Option<String>,
    pub writer_base_url: Option<String>,
}

pub async fn update_llm(
    State(state): State<AppState>,
    Json(req): Json<UpdateLlmRequest>,
) -> Result<Json<LlmConfigResponse>, AppError> {
    // Upsert config (only one row)
    sqlx::query!(
        r#"
        INSERT INTO llm_config (id, reader_provider, reader_model, reader_api_key, reader_base_url,
                                writer_provider, writer_model, writer_api_key, writer_base_url)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (id) DO UPDATE SET
            reader_provider = $2, reader_model = $3, reader_api_key = COALESCE($4, llm_config.reader_api_key),
            reader_base_url = $5, writer_provider = $6, writer_model = $7,
            writer_api_key = COALESCE($8, llm_config.writer_api_key), writer_base_url = $9,
            updated_at = NOW()
        "#,
        uuid::Uuid::nil(),  // singleton row
        req.reader_provider,
        req.reader_model,
        req.reader_api_key,
        req.reader_base_url,
        req.writer_provider,
        req.writer_model,
        req.writer_api_key,
        req.writer_base_url,
    )
    .execute(&state.db)
    .await?;
    
    // Reload LLM manager
    state.llm.reload(&state.db).await?;
    
    get_llm(State(state)).await
}

#[derive(Deserialize)]
pub struct VerifyLlmRequest {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Serialize)]
pub struct VerifyLlmResponse {
    pub valid: bool,
    pub error: Option<String>,
    pub models: Vec<llm::Model>,
}

pub async fn verify_llm(Json(req): Json<VerifyLlmRequest>) -> Json<VerifyLlmResponse> {
    match llm::list_models(&req.provider, req.api_key.as_deref(), req.base_url.as_deref()).await {
        Ok(models) => Json(VerifyLlmResponse {
            valid: true,
            error: None,
            models,
        }),
        Err(e) => Json(VerifyLlmResponse {
            valid: false,
            error: Some(e.to_string()),
            models: vec![],
        }),
    }
}

#[derive(Deserialize)]
pub struct ListModelsRequest {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

pub async fn list_models(Json(req): Json<ListModelsRequest>) -> Result<Json<Vec<llm::Model>>, AppError> {
    let models = llm::list_models(&req.provider, req.api_key.as_deref(), req.base_url.as_deref()).await?;
    Ok(Json(models))
}

#[derive(Serialize)]
pub struct UsageSummary {
    pub period: String,
    pub reader: UsageStats,
    pub writer: UsageStats,
    pub total: UsageStats,
    pub by_project: Vec<ProjectUsage>,
    pub by_day: Vec<DailyUsage>,
}

#[derive(Serialize, Default)]
pub struct UsageStats {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: Decimal,
}

#[derive(Serialize)]
pub struct ProjectUsage {
    pub project_id: uuid::Uuid,
    pub owner: String,
    pub repo: String,
    pub cost_usd: Decimal,
}

#[derive(Serialize)]
pub struct DailyUsage {
    pub date: chrono::NaiveDate,
    pub reader_cost: Decimal,
    pub writer_cost: Decimal,
}

pub async fn usage_summary(State(state): State<AppState>) -> Result<Json<UsageSummary>, AppError> {
    // Last 7 days by default
    let since = chrono::Utc::now() - chrono::Duration::days(7);
    
    let reader = sqlx::query_as!(
        UsageStats,
        r#"
        SELECT 
            COUNT(*) as "requests!",
            COALESCE(SUM(input_tokens), 0) as "input_tokens!",
            COALESCE(SUM(output_tokens), 0) as "output_tokens!",
            COALESCE(SUM(cost_usd), 0) as "cost_usd!"
        FROM llm_usage 
        WHERE role = 'reader' AND created_at > $1
        "#,
        since
    )
    .fetch_one(&state.db)
    .await?;
    
    let writer = sqlx::query_as!(
        UsageStats,
        r#"
        SELECT 
            COUNT(*) as "requests!",
            COALESCE(SUM(input_tokens), 0) as "input_tokens!",
            COALESCE(SUM(output_tokens), 0) as "output_tokens!",
            COALESCE(SUM(cost_usd), 0) as "cost_usd!"
        FROM llm_usage 
        WHERE role = 'writer' AND created_at > $1
        "#,
        since
    )
    .fetch_one(&state.db)
    .await?;
    
    let total = UsageStats {
        requests: reader.requests + writer.requests,
        input_tokens: reader.input_tokens + writer.input_tokens,
        output_tokens: reader.output_tokens + writer.output_tokens,
        cost_usd: reader.cost_usd + writer.cost_usd,
    };
    
    let by_project = sqlx::query_as!(
        ProjectUsage,
        r#"
        SELECT 
            p.id as "project_id!",
            p.github_owner as "owner!",
            p.github_repo as "repo!",
            COALESCE(SUM(u.cost_usd), 0) as "cost_usd!"
        FROM projects p
        LEFT JOIN llm_usage u ON u.project_id = p.id AND u.created_at > $1
        GROUP BY p.id, p.github_owner, p.github_repo
        ORDER BY "cost_usd!" DESC
        LIMIT 10
        "#,
        since
    )
    .fetch_all(&state.db)
    .await?;
    
    let by_day = sqlx::query_as!(
        DailyUsage,
        r#"
        SELECT 
            DATE(created_at) as "date!",
            COALESCE(SUM(CASE WHEN role = 'reader' THEN cost_usd ELSE 0 END), 0) as "reader_cost!",
            COALESCE(SUM(CASE WHEN role = 'writer' THEN cost_usd ELSE 0 END), 0) as "writer_cost!"
        FROM llm_usage
        WHERE created_at > $1
        GROUP BY DATE(created_at)
        ORDER BY "date!" ASC
        "#,
        since
    )
    .fetch_all(&state.db)
    .await?;
    
    Ok(Json(UsageSummary {
        period: "last_7_days".into(),
        reader,
        writer,
        total,
        by_project,
        by_day,
    }))
}
```

### src/integrations/llm/mod.rs

```rust
use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod pricing;

#[derive(Debug, Clone, Serialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub context_window: Option<i32>,
}

#[derive(Clone)]
pub struct LlmResponse {
    pub content: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<LlmResponse>;
    fn provider_name(&self) -> &str;
    fn model_name(&self) -> &str;
}

pub struct LlmManager {
    db: PgPool,
    reader: RwLock<Option<Arc<dyn LlmProvider>>>,
    writer: RwLock<Option<Arc<dyn LlmProvider>>>,
}

impl LlmManager {
    pub async fn new(db: PgPool) -> Result<Self> {
        let manager = Self {
            db: db.clone(),
            reader: RwLock::new(None),
            writer: RwLock::new(None),
        };
        manager.reload(&db).await?;
        Ok(manager)
    }
    
    pub async fn reload(&self, db: &PgPool) -> Result<()> {
        let config = sqlx::query!(
            "SELECT * FROM llm_config ORDER BY created_at DESC LIMIT 1"
        )
        .fetch_optional(db)
        .await?;
        
        if let Some(c) = config {
            let reader = create_provider(
                &c.reader_provider,
                &c.reader_model,
                c.reader_api_key.as_deref(),
                c.reader_base_url.as_deref(),
            )?;
            *self.reader.write().await = Some(reader);
            
            let writer = create_provider(
                &c.writer_provider,
                &c.writer_model,
                c.writer_api_key.as_deref(),
                c.writer_base_url.as_deref(),
            )?;
            *self.writer.write().await = Some(writer);
        }
        
        Ok(())
    }
    
    pub async fn reader(&self) -> Option<TrackedLlm> {
        self.reader.read().await.as_ref().map(|p| TrackedLlm {
            inner: p.clone(),
            db: self.db.clone(),
            role: LlmRole::Reader,
        })
    }
    
    pub async fn writer(&self) -> Option<TrackedLlm> {
        self.writer.read().await.as_ref().map(|p| TrackedLlm {
            inner: p.clone(),
            db: self.db.clone(),
            role: LlmRole::Writer,
        })
    }
}

#[derive(Clone, Copy)]
pub enum LlmRole {
    Reader,
    Writer,
}

impl LlmRole {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Writer => "writer",
        }
    }
}

pub struct TrackedLlm {
    inner: Arc<dyn LlmProvider>,
    db: PgPool,
    role: LlmRole,
}

impl TrackedLlm {
    pub async fn complete(
        &self,
        prompt: &str,
        system: Option<&str>,
        operation: &str,
        incident_id: Option<Uuid>,
        project_id: Option<Uuid>,
    ) -> Result<LlmResponse> {
        let response = self.inner.complete(prompt, system).await?;
        
        let cost = pricing::calculate_cost(
            self.inner.provider_name(),
            self.inner.model_name(),
            response.input_tokens,
            response.output_tokens,
            &self.db,
        ).await;
        
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
        
        Ok(response)
    }
}

fn create_provider(
    provider: &str,
    model: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<Arc<dyn LlmProvider>> {
    match provider {
        "anthropic" => Ok(Arc::new(anthropic::AnthropicProvider::new(
            api_key.ok_or_else(|| anyhow::anyhow!("Anthropic requires API key"))?,
            model,
        ))),
        "openai" => Ok(Arc::new(openai::OpenAiProvider::new(
            api_key.ok_or_else(|| anyhow::anyhow!("OpenAI requires API key"))?,
            model,
        ))),
        "local" => Ok(Arc::new(ollama::OllamaProvider::new(
            base_url.unwrap_or("http://localhost:11434"),
            model,
        ))),
        _ => Err(anyhow::anyhow!("Unknown provider: {}", provider)),
    }
}

pub async fn list_models(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<Vec<Model>> {
    match provider {
        "anthropic" => anthropic::list_models(api_key.unwrap_or("")).await,
        "openai" => openai::list_models(api_key.unwrap_or("")).await,
        "local" => ollama::list_models(base_url.unwrap_or("http://localhost:11434")).await,
        _ => Err(anyhow::anyhow!("Unknown provider: {}", provider)),
    }
}
```

### src/worker.rs

```rust
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api::AppState;
use crate::pipeline;

pub async fn run(state: AppState) {
    info!("Background worker started");
    
    let mut ticker = interval(Duration::from_secs(5));
    
    loop {
        ticker.tick().await;
        
        if let Err(e) = process_pending(&state).await {
            error!(error = %e, "Worker error");
        }
    }
}

async fn process_pending(state: &AppState) -> anyhow::Result<()> {
    // Fetch incidents ready for processing
    let incidents = sqlx::query_as!(
        Incident,
        r#"
        SELECT * FROM incidents
        WHERE state = 'detected'
        AND (cooldown_until IS NULL OR cooldown_until < NOW())
        ORDER BY last_seen DESC
        LIMIT 10
        FOR UPDATE SKIP LOCKED
        "#
    )
    .fetch_all(&state.db)
    .await?;
    
    for incident in incidents {
        if let Err(e) = process_incident(state, &incident).await {
            error!(incident_id = %incident.id, error = %e, "Failed to process incident");
            
            // Mark as failed, increment attempts
            sqlx::query!(
                r#"
                UPDATE incidents 
                SET state = 'fix_failed', fix_attempts = fix_attempts + 1, updated_at = NOW()
                WHERE id = $1
                "#,
                incident.id
            )
            .execute(&state.db)
            .await?;
        }
    }
    
    Ok(())
}

async fn process_incident(state: &AppState, incident: &Incident) -> anyhow::Result<()> {
    let project = sqlx::query_as!(
        Project,
        "SELECT * FROM projects WHERE id = $1",
        incident.project_id
    )
    .fetch_one(&state.db)
    .await?;
    
    // Check max attempts
    if incident.fix_attempts >= state.config.pipeline.max_fix_attempts as i32 {
        warn!(incident_id = %incident.id, "Max fix attempts reached, entering cooldown");
        set_cooldown(state, incident).await?;
        return Ok(());
    }
    
    // Check dry run
    if state.config.safety.dry_run {
        info!(incident_id = %incident.id, "Dry run mode, skipping fix");
        return Ok(());
    }
    
    info!(incident_id = %incident.id, project = %format!("{}/{}", project.github_owner, project.github_repo), "Processing incident");
    
    // Update state
    set_state(state, incident.id, "triaging").await?;
    
    // Get LLM clients
    let reader = state.llm.reader().await
        .ok_or_else(|| anyhow::anyhow!("LLM reader not configured"))?;
    let writer = state.llm.writer().await
        .ok_or_else(|| anyhow::anyhow!("LLM writer not configured"))?;
    
    // Load sample
    let sample = state.storage.get(&incident.sample_location.as_ref().unwrap()).await?;
    let sample: serde_json::Value = serde_json::from_slice(&sample)?;
    
    // Triage
    let triage = pipeline::triage::run(&reader, &sample, incident.id, project.id).await?;
    
    if !triage.fixable {
        info!(incident_id = %incident.id, reason = %triage.reason, "Not fixable");
        set_state(state, incident.id, "unfixable").await?;
        return Ok(());
    }
    
    // Context gathering
    set_state(state, incident.id, "fixing").await?;
    let context = pipeline::context::gather(&reader, &state.github, &project, &triage, incident.id).await?;
    
    // Generate fix
    let fix = pipeline::fix::generate(&writer, &context, incident.id, project.id).await?;
    
    // Open PR
    set_state(state, incident.id, "pr_opening").await?;
    let pr = pipeline::pr::create(
        &state.github,
        &writer,
        &project,
        &incident,
        &fix,
        &state.config.safety.pr_branch_prefix,
    ).await?;
    
    // Update incident with PR info
    sqlx::query!(
        r#"
        UPDATE incidents 
        SET state = 'pr_open', pr_url = $1, pr_number = $2, updated_at = NOW()
        WHERE id = $3
        "#,
        pr.url,
        pr.number,
        incident.id
    )
    .execute(&state.db)
    .await?;
    
    // Notify Slack
    if state.config.slack.enabled {
        if let Err(e) = state.slack.notify_pr_opened(&incident, &pr).await {
            warn!(error = %e, "Failed to send Slack notification");
        }
    }
    
    info!(incident_id = %incident.id, pr_url = %pr.url, "PR opened");
    
    Ok(())
}

async fn set_state(state: &AppState, incident_id: Uuid, new_state: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE incidents SET state = $1, updated_at = NOW() WHERE id = $2",
        new_state,
        incident_id
    )
    .execute(&state.db)
    .await?;
    
    sqlx::query!(
        r#"
        INSERT INTO activity_log (id, incident_id, action, details)
        VALUES ($1, $2, $3, '{}'::jsonb)
        "#,
        Uuid::new_v4(),
        incident_id,
        format!("state:{}", new_state),
    )
    .execute(&state.db)
    .await?;
    
    Ok(())
}

async fn set_cooldown(state: &AppState, incident: &Incident) -> anyhow::Result<()> {
    let cooldown_until = chrono::Utc::now() 
        + chrono::Duration::hours(state.config.pipeline.cooldown_hours as i64);
    
    sqlx::query!(
        r#"
        UPDATE incidents 
        SET state = 'cooldown', cooldown_until = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        cooldown_until,
        incident.id
    )
    .execute(&state.db)
    .await?;
    
    Ok(())
}
```

## Docker

```dockerfile
# Dockerfile

# Build UI
FROM node:20-slim AS ui-builder
WORKDIR /app/ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build

# Build Rust
FROM rust:1.75-slim AS rust-builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev protobuf-compiler && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY migrations/ migrations/
COPY --from=ui-builder /app/ui/build ui/build/
RUN cargo build --release

# Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /app/target/release/crispr /usr/local/bin/
COPY config/ /etc/crispr/
ENV CRISPR_CONFIG=/etc/crispr/config.yaml
EXPOSE 8080 4317
CMD ["crispr"]
```

```yaml
# docker-compose.yaml (production-like)

services:
  crispr:
    build: .
    ports:
      - "8080:8080"
      - "4317:4317"
    environment:
      - DATABASE_URL=postgres://crispr:crispr@postgres:5432/crispr
      - GITHUB_CLIENT_ID=${GITHUB_CLIENT_ID}
      - GITHUB_CLIENT_SECRET=${GITHUB_CLIENT_SECRET}
      - SLACK_CLIENT_ID=${SLACK_CLIENT_ID}
      - SLACK_CLIENT_SECRET=${SLACK_CLIENT_SECRET}
    volumes:
      - ./config/config.prod.yaml:/etc/crispr/config.yaml
    depends_on:
      - postgres
  
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: crispr
      POSTGRES_PASSWORD: crispr
      POSTGRES_DB: crispr
    volumes:
      - pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"

volumes:
  pgdata:
```

```yaml
# docker-compose.dev.yaml (local development)

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: crispr
      POSTGRES_PASSWORD: crispr
      POSTGRES_DB: crispr_dev
    ports:
      - "5432:5432"
    volumes:
      - pgdata_dev:/var/lib/postgresql/data

volumes:
  pgdata_dev:
```

## Getting Started

### Local Development

```bash
# Start Postgres
docker compose -f docker-compose.dev.yaml up -d

# Create GitHub PAT with 'repo' scope
# https://github.com/settings/tokens/new

# Set environment
export GITHUB_PAT=ghp_xxxxx
export ANTHROPIC_API_KEY=sk-ant-xxxxx  # or configure via UI

# Run
CRISPR_CONFIG=config/config.dev.yaml cargo run

# Open http://localhost:8080
```

### Production Deployment

```bash
# Create GitHub OAuth App
# https://github.com/settings/applications/new
# Callback: https://crispr.yourcompany.com/api/v1/auth/github/callback

# Create Slack App (optional)
# https://api.slack.com/apps
# Callback: https://crispr.yourcompany.com/api/v1/auth/slack/callback

# Set secrets
export GITHUB_CLIENT_ID=xxx
export GITHUB_CLIENT_SECRET=xxx
export SLACK_CLIENT_ID=xxx
export SLACK_CLIENT_SECRET=xxx
export DATABASE_URL=postgres://...

# Deploy
docker compose up -d
```

### Configure Log Collector

#### Fluent Bit

```ini
[OUTPUT]
    Name  http
    Match *
    Host  crispr.internal
    Port  8080
    URI   /ingest
    Format json

[FILTER]
    Name    grep
    Match   *
    Regex   level (error|fatal|ERROR|FATAL)
```

#### Vector

```toml
[sinks.crispr]
type = "http"
inputs = ["your_source"]
uri = "http://crispr.internal:8080/ingest"
encoding.codec = "json"

[transforms.errors_only]
type = "filter"
inputs = ["your_source"]
condition = '.level == "error" || .level == "ERROR"'
```

#### OpenTelemetry Collector

```yaml
exporters:
  otlphttp:
    endpoint: http://crispr.internal:4318

service:
  pipelines:
    logs:
      exporters: [otlphttp, your_other_exporter]
```

## API Reference

See `/api/v1/` endpoints in `src/api/router.rs` for full API documentation.

Key endpoints:
- `POST /ingest` - Accept log events
- `GET /api/v1/projects` - List watched projects
- `GET /api/v1/incidents` - List incidents
- `GET /api/v1/usage/summary` - LLM cost dashboard
- `PUT /api/v1/settings/llm` - Configure LLM providers

## License

MIT

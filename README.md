# CRISPR

**Continuous Repair and Incident Self-Patching Runtime**

A self-hosted production error monitoring system that automatically generates fixes and opens pull requests using LLMs.

```
Error Detected → LLM Triage → Context Gathering → Fix Generation → PR Created
```

---

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
  - [Prerequisites](#prerequisites)
  - [Development Setup](#development-setup)
  - [Production Deployment](#production-deployment)
- [Configuration](#configuration)
  - [Environment Variables](#environment-variables)
  - [Configuration File](#configuration-file)
- [Connectors](#connectors)
  - [GitHub Integration](#github-integration)
  - [Slack Integration](#slack-integration)
  - [LLM Providers](#llm-providers)
- [API Reference](#api-reference)
  - [Ingest Endpoints](#ingest-endpoints)
  - [Projects API](#projects-api)
  - [Incidents API](#incidents-api)
  - [Settings API](#settings-api)
  - [Auth API](#auth-api)
- [UI Guide](#ui-guide)
- [Demo: dumpster-fire](#demo-dumpster-fire)
- [CRISPR.md Context File](#crisprmd-context-file)
- [Distributed Workers](#distributed-workers)
- [Development](#development)
- [License](#license)

---

## Overview

CRISPR monitors your production logs for errors, automatically triages them using LLMs, and generates code fixes that are submitted as pull requests. It's designed to handle the repetitive bug-fixing work that consumes engineering time.

### How It Works

1. **Ingest**: Errors are received via HTTP POST or OTLP/gRPC
2. **Fingerprint**: Errors are deduplicated using SHA256 + regex normalization
3. **Triage**: A "reader" LLM (cheap, fast) determines if the error is fixable
4. **Context**: Relevant code files are fetched from GitHub
5. **Fix**: A "writer" LLM (capable, expensive) generates a code fix
6. **PR**: A pull request is created with the fix and explanation

---

## Features

- **Multi-LLM Support**: Anthropic Claude, OpenAI GPT, and Ollama (local models)
- **Reader/Writer Split**: Use cheap models for triage, expensive models for fixes
- **Usage Tracking**: Track token usage and costs per provider and purpose
- **GitHub Integration**: PAT or OAuth authentication, automatic PR creation
- **Slack Notifications**: Get notified when PRs are opened or merged
- **Web Dashboard**: Monitor incidents, configure settings, view costs
- **OTLP Support**: Receive logs via OpenTelemetry gRPC protocol
- **S3 Storage**: Scale sample storage with S3, Azure Blob, GCS, or MinIO
- **CRISPR.md Context**: Maintains a context file in each repo with fix history and service info
- **Local Repo Clones**: Keeps repos cloned locally for faster access and offline context
- **Distributed Workers**: Multiple workers can run in parallel with claim-based coordination

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CRISPR Server                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐      │
│  │  Ingest  │──▶│ Pipeline │──▶│  GitHub  │──▶│  Slack   │      │
│  │ HTTP/gRPC│   │ Triage   │   │ PR Create│   │ Notify   │      │
│  └──────────┘   │ Fix Gen  │   └──────────┘   └──────────┘      │
│                 └──────────┘                                    │
│                      │                                          │
│                      ▼                                          │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐                     │
│  │ Postgres │◀──│   LLM    │──▶│  Object  │                     │
│  │ Database │   │ Manager  │   │  Store   │                     │
│  └──────────┘   └──────────┘   └──────────┘                     │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                    Embedded Web UI (Svelte)                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Prerequisites

- **Rust** 1.75+ (for building)
- **Node.js** 18+ (for UI development)
- **Docker** (for PostgreSQL)
- **GitHub Account** (PAT or OAuth app)
- **LLM API Key** (Anthropic, OpenAI, or local Ollama)

### Development Setup

1. **Clone and enter the directory**
   ```bash
   cd crispr
   ```

2. **Start PostgreSQL**
   ```bash
   docker compose -f docker-compose.dev.yaml up -d
   ```

3. **Configure environment**
   ```bash
   cp .env.example .env
   # Edit .env with your credentials
   ```

4. **Run the server**
   ```bash
   cargo run
   ```

5. **Access the UI**
   - Open http://localhost:8081
   - Navigate to **Settings** to configure GitHub, Slack, and LLM providers

### Production Deployment

1. **Build the Docker image**
   ```bash
   docker build -t crispr:latest .
   ```

2. **Configure environment**
   ```bash
   cp .env.example .env
   # Set production values (see Configuration section)
   ```

3. **Start the stack**
   ```bash
   docker compose up -d
   ```

4. **Access the UI**
   - Open http://localhost:8080

---

## Configuration

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | PostgreSQL connection string |
| `GITHUB_PAT` | Yes* | GitHub Personal Access Token (if using PAT mode) |
| `GITHUB_CLIENT_ID` | Yes* | GitHub OAuth App Client ID (if using OAuth mode) |
| `GITHUB_CLIENT_SECRET` | Yes* | GitHub OAuth App Client Secret (if using OAuth mode) |
| `ANTHROPIC_API_KEY` | No | Anthropic API key for Claude models |
| `OPENAI_API_KEY` | No | OpenAI API key for GPT models |
| `SLACK_CLIENT_ID` | No | Slack App Client ID |
| `SLACK_CLIENT_SECRET` | No | Slack App Client Secret |
| `CRISPR_ENCRYPTION_KEY` | Prod | Base64-encoded 32-byte key for encrypting secrets |
| `AWS_ACCESS_KEY_ID` | No | AWS credentials for S3 storage |
| `AWS_SECRET_ACCESS_KEY` | No | AWS credentials for S3 storage |

*Either PAT or OAuth credentials required for GitHub

### Configuration File

Configuration is loaded from `config/config.dev.yaml` (development) or `config/config.prod.yaml` (production).

```yaml
environment: development
display_name: "CRISPR (local dev)"

server:
  host: "127.0.0.1"
  http_port: 8081
  grpc_port: 4318

database:
  url: "postgres://crispr:crispr@localhost:5433/crispr_dev"
  max_connections: 5

storage:
  mode: postgres  # or "object_store" for S3/Azure/GCS
  # object_store:
  #   provider: s3  # s3, minio, r2, azure, gcp
  #   bucket: crispr-samples
  #   region: us-east-1

github:
  mode: pat  # or "oauth"
  personal_access_token: ${GITHUB_PAT}
  # client_id: ${GITHUB_CLIENT_ID}
  # client_secret: ${GITHUB_CLIENT_SECRET}

slack:
  enabled: false
  # client_id: ${SLACK_CLIENT_ID}
  # client_secret: ${SLACK_CLIENT_SECRET}

safety:
  allowed_repos:
    - "yourorg/*"
  dry_run: true
  pr_branch_prefix: "crispr/"

pipeline:
  cooldown_hours: 24
  max_fix_attempts: 3
  auto_approve: false

logging:
  level: debug
  format: pretty  # or "json" for production
```

---

## Connectors

### GitHub Integration

CRISPR supports two GitHub authentication modes:

#### PAT Mode (Recommended for Development)

1. Create a [Personal Access Token](https://github.com/settings/tokens) with `repo` scope
2. Set `GITHUB_PAT` environment variable
3. Set `github.mode: pat` in config

#### OAuth Mode (Recommended for Production)

1. Create a [GitHub OAuth App](https://github.com/settings/developers)
   - **Authorization callback URL**: `http://localhost:8081/api/v1/auth/github/callback`
2. Set environment variables:
   ```bash
   GITHUB_CLIENT_ID=your-client-id
   GITHUB_CLIENT_SECRET=your-client-secret
   ```
3. Set `github.mode: oauth` in config
4. Navigate to **Connectors** in the UI and click "Connect with GitHub"

### Slack Integration

1. Create a [Slack App](https://api.slack.com/apps)
2. Add OAuth scopes: `chat:write`, `channels:read`
3. Set **Redirect URL**: `http://localhost:8081/api/v1/auth/slack/callback`
4. Set environment variables:
   ```bash
   SLACK_CLIENT_ID=your-client-id
   SLACK_CLIENT_SECRET=your-client-secret
   ```
5. Enable in config: `slack.enabled: true`
6. Navigate to **Connectors** in the UI and click "Connect with Slack"
7. Select a notification channel

### LLM Providers

CRISPR uses a **reader/writer split** to optimize costs:

- **Reader** (triage): Cheap, fast model for determining if errors are fixable
- **Writer** (fix generation): Capable model for generating code fixes

#### Anthropic (Recommended)

```bash
ANTHROPIC_API_KEY=sk-ant-your-key
```

| Role | Recommended Model |
|------|-------------------|
| Reader | `claude-3-haiku-20240307` |
| Writer | `claude-sonnet-4-20250514` |

#### OpenAI

```bash
OPENAI_API_KEY=sk-your-key
```

| Role | Recommended Model |
|------|-------------------|
| Reader | `gpt-4o-mini` |
| Writer | `gpt-4o` |

#### Ollama (Local)

1. Install [Ollama](https://ollama.ai)
2. Pull models: `ollama pull llama3.2`
3. Configure base URL in Settings UI

---

## API Reference

### Ingest Endpoints

#### POST /ingest

Receive error logs via HTTP.

```bash
curl -X POST http://localhost:8081/ingest \
  -H "Content-Type: application/json" \
  -d '{
    "project_id": "my-project",
    "level": "error",
    "message": "TypeError: Cannot read property 'foo' of undefined",
    "stack_trace": "at handler (/app/src/api.ts:42:15)..."
  }'
```

#### gRPC :4318

Receive logs via OpenTelemetry OTLP protocol.

### Projects API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/projects` | List all projects |
| POST | `/api/v1/projects` | Create a project |
| GET | `/api/v1/projects/:id` | Get project details |
| PATCH | `/api/v1/projects/:id` | Update project settings |
| DELETE | `/api/v1/projects/:id` | Delete a project |
| GET | `/api/v1/projects/:id/stats` | Get project statistics |

### Incidents API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/incidents` | List incidents (with filters) |
| GET | `/api/v1/incidents/stats` | Get incident statistics |
| GET | `/api/v1/incidents/:id` | Get incident details |
| POST | `/api/v1/incidents/:id/retry` | Retry fix generation |
| POST | `/api/v1/incidents/:id/ignore` | Mark incident as ignored |

### Settings API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/settings/llm` | Get LLM configuration |
| PUT | `/api/v1/settings/llm` | Update LLM configuration |
| POST | `/api/v1/settings/llm/verify` | Verify API key works |
| POST | `/api/v1/settings/llm/models` | List available models |
| GET | `/api/v1/usage/summary` | Get usage summary |
| GET | `/api/v1/usage/details` | Get detailed usage |

### Auth API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/auth/status` | Get connection status |
| GET | `/api/v1/auth/github` | Start GitHub OAuth flow |
| GET | `/api/v1/auth/github/callback` | GitHub OAuth callback |
| DELETE | `/api/v1/auth/github` | Disconnect GitHub |
| GET | `/api/v1/auth/slack` | Start Slack OAuth flow |
| GET | `/api/v1/auth/slack/callback` | Slack OAuth callback |
| DELETE | `/api/v1/auth/slack` | Disconnect Slack |
| GET | `/api/v1/auth/slack/channels` | List Slack channels |
| POST | `/api/v1/auth/slack/channel` | Set notification channel |
| POST | `/api/v1/auth/slack/test` | Send test message |

---

## UI Guide

The CRISPR UI uses a "Surgery Center" metaphor with a calm, clinical aesthetic inspired by an operating room.

### Demo Mode

Toggle between **Demo** and **Live** modes using the switch in the header. Demo mode shows sample data without requiring backend connectivity — useful for exploring the UI.

### Surgery Board (Dashboard)

The main dashboard has two tabs:

**Patients Tab** — Kanban board showing incidents flowing through stages:

| Column | Description |
|--------|-------------|
| 🔬 **Triage** | Newly detected errors awaiting analysis |
| 🏥 **In Surgery** | Incidents being analyzed or fixed by the LLM |
| 🩹 **Recovery** | PRs created, awaiting merge approval |
| ⚠️ **Needs Attention** | Unfixable errors or max attempts reached |

Each "patient card" shows the error message, confidence level, repository, and time since detection. Click a card to view details.

**Services Tab** — Grid view of all monitored repositories (supports 30-40+ services) with controls to:
- Toggle log watching on/off
- Toggle auto-fix generation on/off
- View incident count and last activity

### Recovery Ward

Detailed view of PRs awaiting approval. Each incident shows:

| Section | Description |
|---------|-------------|
| 🔴 **What Went Wrong** | Root cause analysis and error description |
| 🔧 **What We Fixed** | Explanation of the code changes made |
| ✅ **Expected Behavior** | How the service should behave after the fix |

Includes confidence percentage, link to GitHub PR, and quick approve/reject actions.

### Other Pages

| Page | Description |
|------|-------------|
| **Administration** | Cost breakdown by provider and purpose, usage charts over time |
| **Settings** | LLM provider configuration (Anthropic/OpenAI/Ollama), GitHub and Slack connection status |

---

## Demo: dumpster-fire

A companion repository with 10 intentional bugs for testing CRISPR end-to-end.

Located at: `../dumpster-fire/`

### Quick Demo

1. **Start CRISPR** (see Quick Start above)

2. **Add dumpster-fire as a project**
   ```bash
   curl -X POST http://localhost:8081/api/v1/projects \
     -H "Content-Type: application/json" \
     -d '{"repo": "youruser/dumpster-fire"}'
   ```

3. **Start dumpster-fire**
   ```bash
   cd ../dumpster-fire
   npm install
   npm run dev
   ```

4. **Trigger all bugs**
   ```bash
   npm run trigger-errors
   ```

5. **Watch CRISPR work**
   - Open http://localhost:8081
   - Watch incidents appear and fixes being generated

6. **Verify fixes after merging PRs**
   ```bash
   git pull
   npm run verify
   ```

7. **Reset for next demo**
   ```bash
   npm run reset
   ```

### The 10 Bugs

| # | Category | Description |
|---|----------|-------------|
| 1 | Null reference | Accessing property on undefined |
| 2 | Type mismatch | String passed where number expected |
| 3 | Off-by-one | Wrong pagination offset |
| 4 | Missing try/catch | Unhandled async error |
| 5 | String operation | indexOf vs includes |
| 6 | Array bounds | Accessing [0] on empty array |
| 7 | Missing await | Returning Promise instead of value |
| 8 | Wrong method name | Calling non-existent function |
| 9 | Logic error | Wrong comparison operator |
| 10 | API contract | Missing response wrapper |

---

## CRISPR.md Context File

CRISPR maintains a `CRISPR.md` file in each repository it monitors. This file provides persistent context that improves fix quality over time.

### Template Structure

```markdown
# CRISPR Context

## 1. Special Notes
<!-- Developer-provided context about the repo:
     - Unusual build processes
     - Coding conventions
     - Areas requiring human review -->

## 2. Recent Fixes
<!-- Automatically maintained by CRISPR -->
- [timestamp] Error summary
  - Fix: what was changed
  - PR: link (merged/open)

## 3. Service Description
<!-- What this service does conceptually -->
```

### How It's Used

1. **During Triage**: CRISPR reads the context to understand:
   - Any special considerations from developers
   - Patterns from previous fixes
   - The service's purpose

2. **After Fix Generation**: CRISPR updates section 2 with:
   - The error that was fixed
   - What the fix did
   - Link to the PR

3. **For Developers**: You can edit sections 1 and 3 to give CRISPR better context about your codebase.

---

## Distributed Workers

CRISPR supports running multiple worker instances for horizontal scaling.

### How It Works

1. **Worker Registration**: Each worker registers with a unique ID (hostname + PID)
2. **Incident Claiming**: Before processing, a worker claims the incident using a database lock
3. **Heartbeats**: Workers send periodic heartbeats to indicate they're alive
4. **Stale Claim Cleanup**: If a worker dies, its claims are released after the timeout

### Configuration

```yaml
worker:
  # Auto-generated if not set: hostname-pid
  id: "worker-1"
  # How long before a claim is considered stale
  claim_timeout_minutes: 15
  # How often to send heartbeats
  heartbeat_interval_seconds: 30
```

### Local Repository Storage

Workers keep cloned repositories in a local directory for faster access:

```yaml
pipeline:
  repos_dir: "./repos"  # Where to store cloned repos
```

This enables:
- Faster file access (no API calls)
- Offline context from CRISPR.md
- Git operations for committing fixes

---

## Development

### Project Structure

```
crispr/
├── src/
│   ├── main.rs           # Entry point
│   ├── config.rs         # Configuration loading
│   ├── error.rs          # Error types
│   ├── crypto.rs         # Encryption utilities
│   ├── worker.rs         # Background job processor
│   ├── api/              # HTTP handlers
│   │   ├── router.rs     # Route definitions
│   │   ├── auth.rs       # OAuth flows
│   │   ├── ingest.rs     # Error ingestion
│   │   ├── projects.rs   # Project CRUD
│   │   ├── incidents.rs  # Incident management
│   │   └── settings.rs   # LLM & usage
│   ├── integrations/     # External services
│   │   ├── github.rs     # GitHub API client
│   │   ├── slack.rs      # Slack API client
│   │   └── llm/          # LLM providers
│   ├── pipeline/         # Fix generation
│   │   ├── fingerprint.rs
│   │   ├── triage.rs
│   │   ├── context.rs
│   │   ├── fix.rs
│   │   ├── pr.rs
│   │   ├── repo.rs       # Local repo management
│   │   └── crispr_md.rs  # CRISPR.md context file
│   └── store/            # Data storage
│       ├── postgres.rs
│       ├── object.rs
│       └── models.rs
├── ui/                   # Svelte frontend
├── migrations/           # SQL migrations
├── config/               # YAML configs
├── proto/                # OTLP protobuf definitions
├── Dockerfile
├── docker-compose.yaml
└── docker-compose.dev.yaml
```

### Running Tests

```bash
cargo test
```

### Building for Release

```bash
# Build UI first
cd ui && npm run build && cd ..

# Build Rust binary
cargo build --release
```

### Database Migrations

Migrations run automatically on startup. To create a new migration:

```bash
# Create migration file
touch migrations/004_my_feature.sql
```

---

## License

MIT

---

<p align="center">
  <strong>CRISPR</strong> — Let the robots fix the bugs while you build features.
</p>

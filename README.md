# CRISPR

**Continuous Repair and Incident Self-Patching Runtime**

A self-hosted production error monitoring system that automatically generates fixes and opens pull requests using LLMs.

---

## Incident Lifecycle

Every error flows through a defined lifecycle. Understanding these stages helps you know what CRISPR is doing and when human intervention is needed.

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ DETECTED │───▶│ TRIAGING │───▶│  FIXING  │───▶│ PR_OPEN  │───▶│ VERIFYING│───▶│ VERIFIED │
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
                     │               │               │               │
                     ▼               ▼               ▼               ▼
               ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
               │UNFIXABLE │    │FIX_FAILED│    │PR_CLOSED │    │ RECURRED │
               └──────────┘    └──────────┘    └──────────┘    └──────────┘
```

### Stage Descriptions

| Stage | What's Happening | Moves Forward When | Falls Back When |
|-------|------------------|-------------------|-----------------|
| **detected** | Error received and deduplicated | Worker picks it up | — |
| **triaging** | LLM analyzing if error is fixable | LLM says fixable with confidence | LLM says unfixable → `unfixable` |
| **pending_fix** | Waiting in queue for surgical lock | Repo lock acquired | — |
| **fixing** | LLM generating code fix + tests | Fix generated successfully | LLM fails or times out → `fix_failed` |
| **pr_opening** | Creating branch and pull request | PR created on GitHub | GitHub API fails → `fix_failed` |
| **pr_open** | PR awaiting human review | PR merged on GitHub | PR closed without merge → `pr_closed` |
| **pr_merged** | PR merged, preparing verification | Verification monitoring starts | — |
| **verifying** | Monitoring for error recurrence | Monitoring period completes with no recurrence | Error reoccurs → `detected` (retry) |
| **verified** | Fix confirmed working ✓ | — (terminal state) | — |

### Terminal States (Require Human Action)

| State | Meaning | What To Do |
|-------|---------|------------|
| **unfixable** | LLM determined this can't be auto-fixed | Review manually, click "Retry" if you disagree |
| **fix_failed** | Fix generation failed (LLM error, timeout) | Click "Retry" to try again |
| **pr_closed** | PR was closed without merging | Review why, click "Retry" for new attempt |
| **max_attempts** | Hit retry limit (default: 3) | Needs manual fix |
| **cooldown** | Too many recent attempts, waiting | Will auto-retry after cooldown period |
| **ignored** | Manually marked as ignored | Click "Retry" to re-enable |

### Verification: How Fixes Are Confirmed

After a PR is merged, CRISPR doesn't immediately mark the fix as complete. Instead, it enters a **verification period** to ensure the fix actually works in production.

**How it works:**

1. **Duration is per-incident**: The LLM recommends a monitoring period (24 hours to 30 days) based on:
   - Error pattern (transient vs persistent issues)
   - Whether it involves time-based behavior (billing cycles, cron jobs, monthly reports)
   - Impact severity

2. **Monitoring**: CRISPR watches for new occurrences of the same error fingerprint

3. **Outcomes**:
   - **No recurrence** → `verified` (fix confirmed working)
   - **Error reoccurs** → `detected` (back to the start for another fix attempt)

**Example durations:**
- Missing import error: 24 hours (deterministic, either works or doesn't)
- Session handling bug: 48 hours (needs time for sessions to cycle)
- Rate limiting issue: 7 days (needs traffic patterns to exercise the code)
- Billing calculation bug: 30 days (needs to cover a full billing cycle)

### Surgical Queue: One Fix Per Repo At A Time

To prevent merge conflicts and ensure clean commits, CRISPR uses a **surgical lock** system:

- Only one incident per repository can be actively fixed at a time
- Other incidents for the same repo wait in a queue (`pending_fix` state)
- Before starting a fix, CRISPR does a fresh `git pull` to get the latest code
- After the fix is complete (PR opened), the lock is released for the next incident

This ensures each fix is based on the current state of the codebase and avoids conflicting changes.

---

## Table of Contents

- [Incident Lifecycle](#incident-lifecycle)
  - [Stage Descriptions](#stage-descriptions)
  - [Terminal States](#terminal-states-require-human-action)
  - [Verification](#verification-how-fixes-are-confirmed)
  - [Surgical Queue](#surgical-queue-one-fix-per-repo-at-a-time)
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
  - [Webhook Ingestion](#webhook-ingestion)
- [Generated Tests](#generated-tests)
- [Error Pattern Matching](#error-pattern-matching)
- [Internal Medicine](#internal-medicine)
- [API Reference](#api-reference)
  - [Ingest Endpoints](#ingest-endpoints)
  - [Projects API](#projects-api)
  - [Incidents API](#incidents-api)
  - [Generated Tests API](#generated-tests-api)
  - [Patterns API](#patterns-api)
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

1. **Ingest**: Errors are received via HTTP POST, OTLP/gRPC, or webhooks (Sentry/Datadog)
2. **Fingerprint**: Errors are deduplicated using SHA256 + regex normalization
3. **Pattern Match**: Known error patterns are matched for auto-triage (skips LLM if matched)
4. **Triage**: A "reader" LLM (cheap, fast) determines if the error is fixable
5. **Context**: Relevant code files are fetched from GitHub
6. **Fix**: A "writer" LLM (capable, expensive) generates a code fix + unit tests
7. **PR**: A pull request is created with the fix, tests, and explanation

---

## Features

- **Multi-LLM Support**: Anthropic Claude, OpenAI GPT, and Ollama (local models)
- **Reader/Writer/Internist Split**: Use cheap models for triage, expensive models for fixes, high-reasoning models for pattern analysis
- **Generated Tests**: Automatically generates unit tests alongside fixes to verify correctness
- **Error Pattern Matching**: 23 built-in patterns for auto-triage without LLM calls (saves costs)
- **Usage Tracking**: Track token usage and costs per provider and purpose
- **GitHub Integration**: PAT or OAuth authentication, automatic PR creation
- **Slack Notifications**: Get notified when PRs are opened or merged
- **Web Dashboard**: Monitor incidents, configure settings, view costs
- **OTLP Support**: Receive logs via OpenTelemetry gRPC protocol
- **Webhook Ingestion**: Sentry and Datadog webhooks with signature verification
- **S3 Storage**: Scale sample storage with S3, Azure Blob, GCS, or MinIO
- **CRISPR.md Context**: Maintains a context file in each repo with fix history and service info
- **Local Repo Clones**: Keeps repos cloned locally for faster access and offline context
- **Distributed Workers**: Multiple workers can run in parallel with claim-based coordination
- **Internal Medicine**: Pattern analysis across incidents to identify architectural issues and propose systemic fixes

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

CRISPR uses a **reader/writer/internist split** to optimize costs and capabilities:

- **Reader** (triage): Cheap, fast model for determining if errors are fixable
- **Writer** (fix generation): Capable model for generating code fixes
- **Internist** (pattern analysis): High-reasoning model for cross-incident pattern analysis and architectural recommendations

#### Anthropic (Recommended)

```bash
ANTHROPIC_API_KEY=sk-ant-your-key
```

| Role | Recommended Model |
|------|-------------------|
| Reader | `claude-3-haiku-20240307` |
| Writer | `claude-sonnet-4-20250514` |
| Internist | `claude-sonnet-4-20250514` |

#### OpenAI

```bash
OPENAI_API_KEY=sk-your-key
```

| Role | Recommended Model |
|------|-------------------|
| Reader | `gpt-4o-mini` |
| Writer | `gpt-4o` |
| Internist | `gpt-4o` |

#### Ollama (Local)

1. Install [Ollama](https://ollama.ai)
2. Pull models: `ollama pull llama3.2`
3. Configure base URL in Settings UI

### Webhook Ingestion

CRISPR can receive errors directly from monitoring platforms via webhooks.

#### Sentry

1. In Sentry, go to **Settings → Integrations → Webhooks**
2. Add webhook URL: `http://your-crispr-server:8081/webhooks/sentry`
3. (Optional) Set a webhook secret for signature verification
4. Configure the secret in CRISPR:
   ```yaml
   webhooks:
     sentry_secret: "your-webhook-secret"
   ```

CRISPR verifies Sentry webhook signatures using HMAC-SHA256 when a secret is configured.

#### Datadog

1. In Datadog, go to **Integrations → Webhooks**
2. Create a new webhook with URL: `http://your-crispr-server:8081/webhooks/datadog`
3. Add a `project:owner/repo` tag to identify the project
4. Configure monitors to send to this webhook on error/alert

#### Grafana

1. In Grafana, go to **Alerting → Contact Points**
2. Create a new contact point with type **Webhook**
3. Set URL: `http://your-crispr-server:8081/webhooks/grafana`
4. (Optional) Add authentication header: `Authorization: Bearer your-token`
5. Configure in CRISPR:
   ```yaml
   webhooks:
     grafana_token: "your-token"
   ```

Add labels to your alert rules for project identification:
- `project: owner/repo` - Explicit project mapping
- `namespace` + `service` - Combined as namespace/service
- `service: owner-repo` - Hyphen converted to slash

CRISPR processes alerts with severity `error`, `critical`, `warning`, or `high`.

---

## Generated Tests

CRISPR automatically generates unit tests alongside code fixes to help verify correctness and prevent regressions.

### How It Works

1. When a fix is generated, CRISPR also generates test code that:
   - Verifies the fix works correctly
   - Would have caught the original bug
   - Uses the appropriate test framework for the language

2. Tests are stored in the database and can be:
   - Viewed in the UI on the incident detail page
   - Included or excluded from the PR via toggle
   - Committed alongside the fix

### Supported Frameworks

| Language | Test Framework |
|----------|----------------|
| TypeScript/JavaScript | Jest, Vitest, Mocha |
| Python | pytest, unittest |
| Go | go test |
| Rust | #[test] |
| Java | JUnit |

### UI Integration

On the incident detail page, expand the **Generated Tests** section to:
- View the test code
- Toggle "Include in PR" for each test
- See which tests have been committed

---

## Error Pattern Matching

CRISPR includes 23 built-in error patterns that enable auto-triage without LLM calls, reducing costs and latency.

### How It Works

1. When an error is received, it's matched against known patterns using regex
2. If a pattern with `auto_triage = true` matches:
   - The LLM triage step is skipped
   - The pattern's `auto_fixable` setting determines if fix generation proceeds
3. Pattern matches are tracked for analytics

### Built-in Patterns

| Category | Examples |
|----------|----------|
| Null Reference | Java NPE, Go nil pointer, JS TypeError, Python NoneType |
| Connection | Timeouts, socket errors, pool exhaustion |
| Authentication | HTTP 401/403, invalid token |
| Rate Limiting | HTTP 429, quota exceeded |
| Database | Connection failed, query timeout |
| Memory | OOM, stack overflow |
| Bounds | Array index, slice bounds |
| Type Errors | Type mismatch, cast errors |
| Import | Module not found |
| Parsing | JSON syntax errors |
| File System | File not found |
| Network | HTTP 500/502/503/504, DNS resolution |

### Custom Patterns

Create custom patterns via the API:

```bash
curl -X POST http://localhost:8081/api/v1/patterns \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Redis Connection Error",
    "regex_pattern": "Redis.*ECONNREFUSED|Cannot connect to Redis",
    "category": "connection",
    "auto_triage": true,
    "auto_fixable": false,
    "suggested_action": "Check Redis server status"
  }'
```

---

## Internal Medicine

The Internal Medicine feature provides high-level pattern analysis across incidents, identifying architectural issues and proposing systemic fixes rather than ad-hoc patches.

### How It Works

1. **Automatic Triggering**: After each incident is verified (fix confirmed working), the Internist LLM analyzes recent incidents for patterns
2. **Cross-Service Analysis**: The Internist can see incidents across all projects, detecting when multiple services fail together due to shared infrastructure
3. **Recommendations**: Patterns are turned into actionable recommendations with root cause analysis and proposed changes
4. **Interactive Refinement**: Chat with the Internist to refine recommendations before approval
5. **Conversion to Tickets**: Approved recommendations become detailed triage tickets for the Writer LLM to fix

### Recommendation Categories

| Category | Description | Auto-Fixable |
|----------|-------------|--------------|
| `api_contract` | Breaking changes to APIs, inconsistent interfaces | Yes |
| `architecture` | Structural problems, tight coupling, missing abstractions | Yes |
| `error_pattern` | Recurring error types that need systematic handling | Yes |
| `performance` | Systemic performance issues | Yes |
| `security` | Security patterns or vulnerabilities | Yes |
| `infrastructure` | Database sizing, resource limits, scaling issues | **No** (human required) |

### Human-Only Recommendations

Some issues cannot be fixed by code changes alone. When the Internist identifies infrastructure problems (e.g., "increase database connection pool size"), it creates a recommendation marked as **requires human intervention**. These appear with a special indicator in the UI and create tickets for humans to address.

### UI Integration

The **Internal Medicine** section appears on the Surgery Board dashboard, showing:
- Active recommendations with status (proposed, approved, implementing)
- Conflict warnings for long-lived proposals
- Pattern confidence scores
- Related incident counts

Click a recommendation to:
- View full analysis and proposed changes
- Chat with the Internist to refine the recommendation
- Approve and convert to a fix ticket
- Reject with a reason

---

## API Reference

> 📚 **Full API documentation**: See [docs/API.md](docs/API.md) for comprehensive endpoint documentation with request/response schemas and examples.

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

### Generated Tests API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/incidents/:id/tests` | List generated tests for incident |
| GET | `/api/v1/incidents/:id/tests/:test_id` | Get a specific test |
| PATCH | `/api/v1/incidents/:id/tests/:test_id` | Update test (e.g., include_in_pr) |

### Patterns API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/patterns` | List all patterns |
| POST | `/api/v1/patterns` | Create a custom pattern |
| GET | `/api/v1/patterns/:id` | Get pattern details |
| PATCH | `/api/v1/patterns/:id` | Update a pattern |
| DELETE | `/api/v1/patterns/:id` | Delete a custom pattern |
| GET | `/api/v1/patterns/:id/stats` | Get pattern match statistics |
| GET | `/api/v1/patterns/categories` | List pattern categories |
| POST | `/api/v1/patterns/test` | Test a regex against sample text |
| GET | `/api/v1/incidents/:id/pattern-matches` | Get pattern matches for incident |

### Recommendations API (Internal Medicine)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/recommendations` | List all recommendations |
| GET | `/api/v1/recommendations/:id` | Get recommendation details |
| POST | `/api/v1/recommendations/:id/approve` | Approve recommendation |
| POST | `/api/v1/recommendations/:id/reject` | Reject with reason |
| POST | `/api/v1/recommendations/:id/chat` | Send message to Internist |
| GET | `/api/v1/recommendations/:id/chat` | Get chat history |
| POST | `/api/v1/recommendations/:id/apply-suggestion` | Apply Internist's suggested changes |
| POST | `/api/v1/recommendations/:id/convert-to-incident` | Convert to triage ticket |
| POST | `/api/v1/internist/analyze` | Trigger manual analysis |

### Settings API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/settings/llm` | Get LLM configuration |
| PUT | `/api/v1/settings/llm` | Update LLM configuration |
| POST | `/api/v1/settings/llm/verify` | Verify API key works |
| POST | `/api/v1/settings/llm/models` | List available models |
| GET | `/api/v1/usage/summary` | Get usage summary (includes Internist costs) |
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

### Internal Medicine Section

Below the main dashboard, the **Internal Medicine** section shows architectural recommendations:

| Element | Description |
|---------|-------------|
| **Recommendation Cards** | Each card shows title, category, priority, and related incident count |
| **Status Badges** | Proposed, Approved, Implementing, Completed, Rejected |
| **Conflict Warning** | Orange indicator when a long-lived proposal has merge conflicts |
| **Human Required** | Special badge for infrastructure changes that can't be auto-fixed |

Click a recommendation to open the detail page with:
- Full root cause analysis
- Proposed code changes
- Interactive chat with the Internist LLM
- Approve/Reject actions

### Other Pages

| Page | Description |
|------|-------------|
| **Administration** | Cost breakdown by provider and purpose (Reader, Writer, Internist), usage charts over time |
| **Settings** | LLM provider configuration (Anthropic/OpenAI/Ollama) for all three tiers, GitHub and Slack connection status |

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
│   │   ├── grpc.rs       # OTLP gRPC server
│   │   ├── webhooks.rs   # Sentry/Datadog webhooks
│   │   ├── patterns.rs   # Error pattern API
│   │   ├── projects.rs   # Project CRUD
│   │   ├── incidents.rs  # Incident management
│   │   ├── recommendations.rs  # Internal Medicine API
│   │   └── settings.rs   # LLM & usage
│   ├── integrations/     # External services
│   │   ├── github.rs     # GitHub API client
│   │   ├── slack.rs      # Slack API client
│   │   └── llm/          # LLM providers
│   ├── patterns/         # Error pattern matching
│   │   └── mod.rs        # Pattern matcher with auto-triage
│   ├── pipeline/         # Fix generation
│   │   ├── fingerprint.rs
│   │   ├── triage.rs
│   │   ├── context.rs
│   │   ├── fix.rs        # Fix + test generation
│   │   ├── pr.rs
│   │   ├── repo.rs       # Local repo management
│   │   ├── crispr_md.rs  # CRISPR.md context file
│   │   ├── internist.rs  # Pattern analysis (Internal Medicine)
│   │   ├── api_contracts.rs  # API breaking change detection
│   │   └── merge.rs      # Intelligent merge for long-lived proposals
│   └── store/            # Data storage
│       ├── postgres.rs
│       ├── object.rs
│       └── models.rs
├── ui/                   # Svelte frontend
│   └── src/lib/components/  # Reusable UI components
├── tests/                # Integration tests
│   ├── api_test.rs       # API endpoint tests
│   ├── pipeline_test.rs  # Pipeline flow tests
│   └── common/           # Test utilities and mocks
├── docs/                 # Documentation
│   └── API.md            # Full API reference
├── migrations/           # SQL migrations
├── config/               # YAML configs
├── Dockerfile
├── docker-compose.yaml
└── docker-compose.dev.yaml
```

### Running Tests

```bash
# Run all tests (unit + integration)
cargo test

# Run only integration tests
cargo test --test api_test --test pipeline_test

# Run with output
cargo test -- --nocapture
```

The test suite includes:
- **62 integration tests** covering API endpoints and pipeline flows
- **Unit tests** inline in source modules
- **Mock implementations** for LLM, GitHub, and Slack

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

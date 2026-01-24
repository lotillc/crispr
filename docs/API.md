# CRISPR API Documentation

This document provides comprehensive documentation for the CRISPR REST API.

**Base URL**: `http://localhost:8081` (development) or your production URL

**API Version**: v1

---

## Table of Contents

- [Authentication](#authentication)
- [Rate Limiting](#rate-limiting)
- [Error Responses](#error-responses)
- [Endpoints](#endpoints)
  - [Health & Status](#health--status)
  - [Incidents](#incidents)
  - [Projects (Services)](#projects-services)
  - [Settings](#settings)
  - [Usage](#usage)
  - [Auth](#auth)
  - [Webhooks](#webhooks)
  - [Ingest](#ingest)
  - [Real-time Events (SSE)](#real-time-events-sse)
- [Webhook Integration Guides](#webhook-integration-guides)
  - [Sentry Integration](#sentry-integration)
  - [Datadog Integration](#datadog-integration)
  - [Grafana Integration](#grafana-integration)

---

## Authentication

CRISPR currently operates as a self-hosted internal service and does not require API key authentication for most endpoints. The API is designed to run within your private network or behind a reverse proxy that handles authentication.

### Security Recommendations

1. **Network Security**: Deploy CRISPR behind a firewall or VPN
2. **Reverse Proxy**: Use nginx/traefik with authentication for external access
3. **Webhook Secrets**: Configure webhook secrets in your config for signature verification

### Webhook Authentication

Webhooks support optional signature/token verification:

| Provider | Header | Verification Method |
|----------|--------|---------------------|
| **Sentry** | `Sentry-Hook-Signature` | HMAC-SHA256 signature |
| **Datadog** | `X-Datadog-Token` | Shared token |
| **Grafana** | `Authorization: Bearer <token>` or `X-Grafana-Token` | Bearer token |

Configure in `config.yaml`:

```yaml
webhooks:
  sentry_secret: "your-sentry-webhook-secret"
  datadog_token: "your-datadog-webhook-token"
  grafana_token: "your-grafana-webhook-token"
```

---

## Rate Limiting

The ingest and webhook endpoints are rate-limited to prevent abuse.

| Endpoint | Default Limit |
|----------|---------------|
| `POST /ingest` | Configured via `ingest.rate_limit_per_second` |
| `POST /webhooks/*` | Same as ingest |

When rate limited, the API returns:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 1
```

---

## Error Responses

All API errors follow a consistent JSON format:

```json
{
  "error": "Error type",
  "message": "Detailed error description"
}
```

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 201 | Created |
| 400 | Bad Request - Invalid input |
| 401 | Unauthorized - Invalid credentials |
| 404 | Not Found - Resource doesn't exist |
| 409 | Conflict - Resource already exists |
| 429 | Too Many Requests - Rate limited |
| 500 | Internal Server Error |

---

## Endpoints

### Health & Status

#### GET /health

Health check endpoint. Returns OK if the server is running.

**Response**
```
OK
```

**curl Example**
```bash
curl http://localhost:8081/health
```

---

#### GET /ready

Readiness check. Returns OK if the database connection is healthy.

**Response (Success)**
```
OK
```

**Response (Failure)**
```http
HTTP/1.1 503 Service Unavailable
```

**curl Example**
```bash
curl http://localhost:8081/ready
```

---

#### GET /api/v1/status

Get detailed API status including configuration state.

**Response**
```json
{
  "status": "ok",
  "version": "0.1.0",
  "environment": "development",
  "display_name": "CRISPR (local dev)",
  "llm_configured": true,
  "github_connected": true
}
```

**curl Example**
```bash
curl http://localhost:8081/api/v1/status
```

---

### Incidents

#### GET /api/v1/incidents

List incidents with optional filtering.

**Query Parameters**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `project_id` | UUID | No | Filter by project |
| `state` | string | No | Filter by state: `detected`, `triaging`, `fixing`, `pr_open`, `pr_merged`, `pr_closed`, `unfixable`, `cooldown` |
| `limit` | integer | No | Max results (default: 50, max: 100) |
| `offset` | integer | No | Pagination offset (default: 0) |

**Response**
```json
{
  "incidents": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "project_id": "550e8400-e29b-41d4-a716-446655440001",
      "fingerprint": "a1b2c3d4e5f6...",
      "title": "TypeError: Cannot read property 'foo' of undefined",
      "state": "detected",
      "severity": "error",
      "occurrence_count": 5,
      "first_seen": "2024-01-15T10:30:00Z",
      "last_seen": "2024-01-15T14:22:00Z",
      "cooldown_until": null,
      "fix_attempts": 0,
      "pr_url": null,
      "pr_number": null,
      "pr_state": null,
      "tests_generated": false,
      "test_count": 0,
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T14:22:00Z"
    }
  ],
  "total": 42
}
```

**curl Example**
```bash
# List all incidents
curl "http://localhost:8081/api/v1/incidents"

# Filter by state
curl "http://localhost:8081/api/v1/incidents?state=detected&limit=10"

# Filter by project
curl "http://localhost:8081/api/v1/incidents?project_id=550e8400-e29b-41d4-a716-446655440001"
```

---

#### GET /api/v1/incidents/:id

Get detailed information about a specific incident.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Incident ID |

**Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "project_id": "550e8400-e29b-41d4-a716-446655440001",
  "fingerprint": "a1b2c3d4e5f6...",
  "title": "TypeError: Cannot read property 'foo' of undefined",
  "state": "pr_open",
  "severity": "error",
  "occurrence_count": 5,
  "first_seen": "2024-01-15T10:30:00Z",
  "last_seen": "2024-01-15T14:22:00Z",
  "cooldown_until": null,
  "fix_attempts": 1,
  "pr_url": "https://github.com/acme/webapp/pull/123",
  "pr_number": 123,
  "pr_state": "open",
  "tests_generated": true,
  "test_count": 2,
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T14:22:00Z",
  "project": {
    "github_owner": "acme",
    "github_repo": "webapp"
  },
  "sample": {
    "message": "TypeError: Cannot read property 'foo' of undefined",
    "stack_trace": "at handler (/app/src/api.ts:42:15)...",
    "level": "ERROR"
  },
  "activity": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440000",
      "action": "ingested",
      "details": { "occurrence": 1, "level": "ERROR", "is_new": true },
      "created_at": "2024-01-15T10:30:00Z"
    },
    {
      "id": "660e8400-e29b-41d4-a716-446655440001",
      "action": "triage_complete",
      "details": { "fixable": true, "confidence": 0.85 },
      "created_at": "2024-01-15T10:31:00Z"
    }
  ]
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/incidents/550e8400-e29b-41d4-a716-446655440000"
```

---

#### POST /api/v1/incidents/:id/retry

Retry processing a failed or ignored incident. Resets the incident state to `detected`.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Incident ID |

**Valid States for Retry**: `fix_failed`, `unfixable`, `cooldown`, `pr_closed`

**Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "project_id": "550e8400-e29b-41d4-a716-446655440001",
  "fingerprint": "a1b2c3d4e5f6...",
  "title": "TypeError: Cannot read property 'foo' of undefined",
  "state": "detected",
  "severity": "error",
  "occurrence_count": 5,
  "first_seen": "2024-01-15T10:30:00Z",
  "last_seen": "2024-01-15T14:22:00Z",
  "cooldown_until": null,
  "fix_attempts": 1,
  "pr_url": null,
  "pr_number": null,
  "pr_state": null,
  "tests_generated": false,
  "test_count": 0,
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T15:00:00Z"
}
```

**Error Response (Invalid State)**
```json
{
  "error": "BadRequest",
  "message": "Cannot retry incident in state 'triaging'"
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/incidents/550e8400-e29b-41d4-a716-446655440000/retry"
```

---

#### POST /api/v1/incidents/:id/ignore

Mark an incident as ignored/unfixable. This prevents automatic fix attempts.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Incident ID |

**Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "project_id": "550e8400-e29b-41d4-a716-446655440001",
  "fingerprint": "a1b2c3d4e5f6...",
  "title": "TypeError: Cannot read property 'foo' of undefined",
  "state": "unfixable",
  "severity": "error",
  "occurrence_count": 5,
  "first_seen": "2024-01-15T10:30:00Z",
  "last_seen": "2024-01-15T14:22:00Z",
  "cooldown_until": null,
  "fix_attempts": 0,
  "pr_url": null,
  "pr_number": null,
  "pr_state": null,
  "tests_generated": false,
  "test_count": 0,
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T15:00:00Z"
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/incidents/550e8400-e29b-41d4-a716-446655440000/ignore"
```

---

#### GET /api/v1/incidents/stats

Get incident statistics.

**Response**
```json
{
  "total": 156,
  "by_state": [
    { "state": "pr_merged", "count": 89 },
    { "state": "detected", "count": 32 },
    { "state": "pr_open", "count": 15 },
    { "state": "unfixable", "count": 12 },
    { "state": "cooldown", "count": 8 }
  ],
  "recent_activity": 23
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/incidents/stats"
```

---

### Projects (Services)

Projects represent GitHub repositories that CRISPR monitors.

#### GET /api/v1/projects

List all projects.

**Query Parameters**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `enabled_only` | boolean | No | Only return enabled projects |
| `limit` | integer | No | Max results (default: 50, max: 100) |
| `offset` | integer | No | Pagination offset (default: 0) |

**Response**
```json
{
  "projects": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "github_owner": "acme",
      "github_repo": "webapp",
      "default_branch": "main",
      "language": "TypeScript",
      "enabled": true,
      "auto_registered": false,
      "settings": {},
      "created_at": "2024-01-10T09:00:00Z",
      "updated_at": "2024-01-15T14:22:00Z"
    }
  ],
  "total": 5
}
```

**curl Example**
```bash
# List all projects
curl "http://localhost:8081/api/v1/projects"

# List only enabled projects
curl "http://localhost:8081/api/v1/projects?enabled_only=true"
```

---

#### POST /api/v1/projects

Create a new project (register a repository for monitoring).

**Request Body**
```json
{
  "github_owner": "acme",
  "github_repo": "webapp",
  "default_branch": "main",
  "language": "TypeScript"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `github_owner` | string | Yes | GitHub organization or user |
| `github_repo` | string | Yes | Repository name |
| `default_branch` | string | No | Default branch (auto-detected if GitHub connected) |
| `language` | string | No | Primary language (auto-detected if GitHub connected) |

**Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "github_owner": "acme",
  "github_repo": "webapp",
  "default_branch": "main",
  "language": "TypeScript",
  "enabled": true,
  "auto_registered": false,
  "settings": {},
  "created_at": "2024-01-15T15:00:00Z",
  "updated_at": "2024-01-15T15:00:00Z"
}
```

**Error Response (Conflict)**
```json
{
  "error": "Conflict",
  "message": "Project acme/webapp already exists"
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/projects" \
  -H "Content-Type: application/json" \
  -d '{
    "github_owner": "acme",
    "github_repo": "webapp"
  }'
```

---

#### GET /api/v1/projects/:id

Get a specific project.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Project ID |

**Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "github_owner": "acme",
  "github_repo": "webapp",
  "default_branch": "main",
  "language": "TypeScript",
  "enabled": true,
  "auto_registered": false,
  "settings": {},
  "created_at": "2024-01-10T09:00:00Z",
  "updated_at": "2024-01-15T14:22:00Z"
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/projects/550e8400-e29b-41d4-a716-446655440001"
```

---

#### PATCH /api/v1/projects/:id

Update a project's settings.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Project ID |

**Request Body**
```json
{
  "default_branch": "develop",
  "language": "JavaScript",
  "enabled": false,
  "settings": {
    "auto_fix": false,
    "notification_channel": "#dev-alerts"
  }
}
```

All fields are optional. Only provided fields will be updated.

**Response**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "github_owner": "acme",
  "github_repo": "webapp",
  "default_branch": "develop",
  "language": "JavaScript",
  "enabled": false,
  "auto_registered": false,
  "settings": {
    "auto_fix": false,
    "notification_channel": "#dev-alerts"
  },
  "created_at": "2024-01-10T09:00:00Z",
  "updated_at": "2024-01-15T15:30:00Z"
}
```

**curl Example**
```bash
curl -X PATCH "http://localhost:8081/api/v1/projects/550e8400-e29b-41d4-a716-446655440001" \
  -H "Content-Type: application/json" \
  -d '{
    "enabled": false
  }'
```

---

#### DELETE /api/v1/projects/:id

Delete a project.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Project ID |

**Response**
```json
{
  "deleted": true,
  "id": "550e8400-e29b-41d4-a716-446655440001"
}
```

**curl Example**
```bash
curl -X DELETE "http://localhost:8081/api/v1/projects/550e8400-e29b-41d4-a716-446655440001"
```

---

#### GET /api/v1/projects/:id/stats

Get statistics for a specific project.

**Path Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Project ID |

**Response**
```json
{
  "total_incidents": 45,
  "open_incidents": 8,
  "fixed_incidents": 32,
  "prs_opened": 35,
  "prs_merged": 32
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/projects/550e8400-e29b-41d4-a716-446655440001/stats"
```

---

### Settings

#### GET /api/v1/settings/llm

Get current LLM configuration.

**Response**
```json
{
  "configured": true,
  "reader": {
    "provider": "anthropic",
    "model": "claude-3-haiku-20240307",
    "has_api_key": true,
    "base_url": null
  },
  "writer": {
    "provider": "anthropic",
    "model": "claude-sonnet-4-20250514",
    "has_api_key": true,
    "base_url": null
  }
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/settings/llm"
```

---

#### PUT /api/v1/settings/llm

Update LLM configuration.

**Request Body**
```json
{
  "reader_provider": "anthropic",
  "reader_model": "claude-3-haiku-20240307",
  "reader_api_key": "sk-ant-...",
  "reader_base_url": null,
  "writer_provider": "anthropic",
  "writer_model": "claude-sonnet-4-20250514",
  "writer_api_key": "sk-ant-...",
  "writer_base_url": null
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `reader_provider` | string | Yes | Provider: `anthropic`, `openai`, `ollama` |
| `reader_model` | string | Yes | Model ID for triage |
| `reader_api_key` | string | No | API key (omit to keep existing) |
| `reader_base_url` | string | No | Custom base URL (for Ollama) |
| `writer_provider` | string | Yes | Provider: `anthropic`, `openai`, `ollama` |
| `writer_model` | string | Yes | Model ID for fix generation |
| `writer_api_key` | string | No | API key (omit to keep existing) |
| `writer_base_url` | string | No | Custom base URL (for Ollama) |

**Response**

Same as GET /api/v1/settings/llm

**curl Example**
```bash
curl -X PUT "http://localhost:8081/api/v1/settings/llm" \
  -H "Content-Type: application/json" \
  -d '{
    "reader_provider": "anthropic",
    "reader_model": "claude-3-haiku-20240307",
    "reader_api_key": "sk-ant-your-key",
    "writer_provider": "anthropic",
    "writer_model": "claude-sonnet-4-20250514",
    "writer_api_key": "sk-ant-your-key"
  }'
```

---

#### POST /api/v1/settings/llm/verify

Verify LLM credentials and list available models.

**Request Body**
```json
{
  "provider": "anthropic",
  "api_key": "sk-ant-...",
  "base_url": null
}
```

**Response (Success)**
```json
{
  "valid": true,
  "error": null,
  "models": [
    { "id": "claude-3-haiku-20240307", "name": "Claude 3 Haiku" },
    { "id": "claude-sonnet-4-20250514", "name": "Claude Sonnet 4" },
    { "id": "claude-3-opus-20240229", "name": "Claude 3 Opus" }
  ]
}
```

**Response (Invalid)**
```json
{
  "valid": false,
  "error": "Invalid API key",
  "models": []
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/settings/llm/verify" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "anthropic",
    "api_key": "sk-ant-your-key"
  }'
```

---

#### POST /api/v1/settings/llm/models

List available models for a provider.

**Request Body**
```json
{
  "provider": "openai",
  "api_key": "sk-...",
  "base_url": null
}
```

**Response**
```json
[
  { "id": "gpt-4o", "name": "GPT-4o" },
  { "id": "gpt-4o-mini", "name": "GPT-4o Mini" },
  { "id": "gpt-4-turbo", "name": "GPT-4 Turbo" }
]
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/settings/llm/models" \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "ollama",
    "base_url": "http://localhost:11434"
  }'
```

---

### Usage

#### GET /api/v1/usage/summary

Get usage summary for the last 7 days.

**Response**
```json
{
  "period": "last_7_days",
  "reader": {
    "requests": 245,
    "input_tokens": 125000,
    "output_tokens": 15000,
    "cost_usd": "0.35"
  },
  "writer": {
    "requests": 45,
    "input_tokens": 450000,
    "output_tokens": 85000,
    "cost_usd": "8.50"
  },
  "total": {
    "requests": 290,
    "input_tokens": 575000,
    "output_tokens": 100000,
    "cost_usd": "8.85"
  },
  "by_project": [
    {
      "project_id": "550e8400-e29b-41d4-a716-446655440001",
      "owner": "acme",
      "repo": "webapp",
      "cost_usd": "4.25"
    },
    {
      "project_id": "550e8400-e29b-41d4-a716-446655440002",
      "owner": "acme",
      "repo": "api",
      "cost_usd": "3.10"
    }
  ],
  "by_day": [
    { "date": "2024-01-09", "reader_cost": "0.05", "writer_cost": "1.20" },
    { "date": "2024-01-10", "reader_cost": "0.08", "writer_cost": "1.85" },
    { "date": "2024-01-11", "reader_cost": "0.04", "writer_cost": "0.95" }
  ]
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/usage/summary"
```

---

#### GET /api/v1/usage/details

Get detailed usage records.

**Query Parameters**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `days` | integer | No | Number of days to look back (default: 7) |
| `project_id` | UUID | No | Filter by project |
| `limit` | integer | No | Max results (default: 100, max: 1000) |

**Response**
```json
[
  {
    "id": "770e8400-e29b-41d4-a716-446655440000",
    "incident_id": "550e8400-e29b-41d4-a716-446655440000",
    "project_id": "550e8400-e29b-41d4-a716-446655440001",
    "role": "writer",
    "provider": "anthropic",
    "model": "claude-sonnet-4-20250514",
    "operation": "fix_generation",
    "input_tokens": 8500,
    "output_tokens": 1200,
    "cost_usd": "0.25",
    "created_at": "2024-01-15T14:30:00Z"
  }
]
```

**curl Example**
```bash
# Get all usage for last 7 days
curl "http://localhost:8081/api/v1/usage/details"

# Get usage for specific project
curl "http://localhost:8081/api/v1/usage/details?project_id=550e8400-e29b-41d4-a716-446655440001&days=30"
```

---

### Auth

#### GET /api/v1/auth/status

Get connection status for all integrations.

**Response**
```json
{
  "github": {
    "connected": true,
    "mode": "oauth",
    "user": {
      "login": "octocat",
      "name": "The Octocat",
      "avatar_url": "https://avatars.githubusercontent.com/u/583231"
    },
    "oauth_url": null
  },
  "slack": {
    "enabled": true,
    "connected": true,
    "team_name": "Acme Inc",
    "channel_name": "crispr-alerts"
  },
  "llm": {
    "configured": true,
    "reader_model": "claude-3-haiku-20240307",
    "writer_model": "claude-sonnet-4-20250514"
  }
}
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/auth/status"
```

---

#### GET /api/v1/auth/github

Initiate GitHub OAuth flow. Redirects to GitHub for authorization.

**Response**

HTTP 307 redirect to GitHub OAuth authorization page.

**Usage**

Navigate to this URL in a browser to start the OAuth flow.

---

#### GET /api/v1/auth/github/callback

GitHub OAuth callback endpoint. Handles the authorization code exchange.

**Query Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `code` | string | Authorization code from GitHub |
| `state` | string | CSRF protection state |
| `error` | string | Error code (if authorization failed) |

**Response**

HTTP 307 redirect to `/settings` with success or error query parameter.

---

#### DELETE /api/v1/auth/github

Disconnect GitHub integration.

**Response**
```json
{
  "success": true,
  "message": "GitHub disconnected"
}
```

**curl Example**
```bash
curl -X DELETE "http://localhost:8081/api/v1/auth/github"
```

---

#### GET /api/v1/auth/slack

Initiate Slack OAuth flow. Redirects to Slack for authorization.

**Response**

HTTP 307 redirect to Slack OAuth authorization page.

---

#### GET /api/v1/auth/slack/callback

Slack OAuth callback endpoint.

---

#### DELETE /api/v1/auth/slack

Disconnect Slack integration.

**Response**
```json
{
  "success": true,
  "message": "Slack disconnected"
}
```

**curl Example**
```bash
curl -X DELETE "http://localhost:8081/api/v1/auth/slack"
```

---

#### GET /api/v1/auth/slack/channels

List available Slack channels for notifications.

**Response**
```json
[
  { "id": "C01234567", "name": "general" },
  { "id": "C01234568", "name": "crispr-alerts" },
  { "id": "C01234569", "name": "dev-team" }
]
```

**curl Example**
```bash
curl "http://localhost:8081/api/v1/auth/slack/channels"
```

---

#### POST /api/v1/auth/slack/channel

Set the notification channel for Slack alerts.

**Request Body**
```json
{
  "channel_id": "C01234568",
  "channel_name": "crispr-alerts"
}
```

**Response**
```json
{
  "success": true,
  "message": "Channel set to #crispr-alerts"
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/auth/slack/channel" \
  -H "Content-Type: application/json" \
  -d '{
    "channel_id": "C01234568",
    "channel_name": "crispr-alerts"
  }'
```

---

#### POST /api/v1/auth/slack/test

Send a test message to the configured Slack channel.

**Response**
```json
{
  "success": true,
  "message": "Test message sent successfully"
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/api/v1/auth/slack/test"
```

---

### Webhooks

#### POST /webhooks/sentry

Receive error events from Sentry.

**Headers**

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `Sentry-Hook-Signature` | No | HMAC-SHA256 signature (if secret configured) |

**Request Body**

Sentry webhook payload. See [Sentry Integration](#sentry-integration) for details.

**Response**
```json
{
  "accepted": true,
  "message": "Processed Sentry webhook: 1 created, 0 updated",
  "incidents_created": 1,
  "incidents_updated": 0
}
```

---

#### POST /webhooks/datadog

Receive error events from Datadog.

**Headers**

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `X-Datadog-Token` | No | Webhook token (if configured) |

**Request Body**

Datadog webhook payload. See [Datadog Integration](#datadog-integration) for details.

**Response**
```json
{
  "accepted": true,
  "message": "Processed Datadog webhook: 1 created, 0 updated",
  "incidents_created": 1,
  "incidents_updated": 0
}
```

---

#### POST /webhooks/grafana

Receive alert notifications from Grafana Alerting (Unified Alerting).

**Headers**

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | Yes | `application/json` |
| `Authorization` | No | `Bearer <token>` (if configured) |
| `X-Grafana-Token` | No | Alternative token header |

**Request Body**

Grafana unified alerting webhook payload. See [Grafana Integration](#grafana-integration) for details.

**Example Payload**
```json
{
  "receiver": "crispr",
  "status": "firing",
  "alerts": [
    {
      "status": "firing",
      "labels": {
        "alertname": "HighErrorRate",
        "severity": "critical",
        "service": "api-gateway",
        "namespace": "production"
      },
      "annotations": {
        "summary": "Error rate exceeded 5% threshold",
        "description": "The API gateway is experiencing elevated error rates"
      },
      "startsAt": "2024-01-15T10:00:00Z",
      "generatorURL": "https://grafana.example.com/alerting/1",
      "fingerprint": "abc123",
      "values": {"error_rate": 0.08},
      "valueString": "error_rate = 8%"
    }
  ],
  "commonLabels": {
    "team": "platform"
  }
}
```

**Response**
```json
{
  "accepted": true,
  "message": "Processed Grafana webhook: 1 created, 0 updated",
  "incidents_created": 1,
  "incidents_updated": 0
}
```

---

### Ingest

#### POST /ingest

Ingest error logs directly via HTTP.

**Request Body**
```json
{
  "project": "acme/webapp",
  "level": "ERROR",
  "message": "TypeError: Cannot read property 'foo' of undefined",
  "stack_trace": "TypeError: Cannot read property 'foo' of undefined\n    at handler (/app/src/api.ts:42:15)\n    at processTicksAndRejections (internal/process/task_queues.js:95:5)",
  "timestamp": "2024-01-15T14:30:00Z",
  "attributes": {
    "request_id": "abc-123",
    "user_id": "user-456",
    "url": "/api/users/123"
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `project` | string | No* | Project in `owner/repo` format |
| `service_name` | string | No* | Alternative: service name (can be mapped to project) |
| `kubernetes_namespace` | string | No | Kubernetes namespace |
| `kubernetes_pod` | string | No | Kubernetes pod name |
| `level` | string | Yes | Log level: `ERROR`, `FATAL`, `CRITICAL`, `SEVERE` |
| `message` | string | Yes | Error message |
| `stack_trace` | string | No | Stack trace (recommended) |
| `timestamp` | string | No | ISO 8601 timestamp |
| `attributes` | object | No | Additional metadata |

*Either `project` or `service_name` must be provided to identify the repository.

**Response (Success)**
```json
{
  "accepted": true,
  "incident_id": "550e8400-e29b-41d4-a716-446655440000",
  "project_id": "550e8400-e29b-41d4-a716-446655440001",
  "message": "New incident created",
  "is_new": true,
  "occurrence_count": 1
}
```

**Response (Existing Incident)**
```json
{
  "accepted": true,
  "incident_id": "550e8400-e29b-41d4-a716-446655440000",
  "project_id": "550e8400-e29b-41d4-a716-446655440001",
  "message": "Occurrence #5 recorded",
  "is_new": false,
  "occurrence_count": 5
}
```

**Response (Rejected - Non-Error Level)**
```json
{
  "accepted": false,
  "incident_id": null,
  "project_id": null,
  "message": "Only ERROR/FATAL/CRITICAL/SEVERE logs are processed",
  "is_new": null,
  "occurrence_count": null
}
```

**curl Example**
```bash
curl -X POST "http://localhost:8081/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "project": "acme/webapp",
    "level": "ERROR",
    "message": "TypeError: Cannot read property '\''foo'\'' of undefined",
    "stack_trace": "at handler (/app/src/api.ts:42:15)\nat processTicksAndRejections (internal/process/task_queues.js:95:5)"
  }'
```

---

### Real-time Events (SSE)

#### GET /api/v1/events

Server-Sent Events endpoint for real-time updates.

**Response**

Stream of SSE events with keep-alive heartbeats every 30 seconds.

**Event Types**

| Event | Description |
|-------|-------------|
| `incident:created` | New incident detected |
| `incident:updated` | Incident status changed |
| `stats:updated` | Dashboard statistics updated |
| `heartbeat` | Keep-alive ping |

**Event Format**

```
event: incident:created
data: {"type":"incident:created","data":{"id":"550e8400-e29b-41d4-a716-446655440000","project_id":"550e8400-e29b-41d4-a716-446655440001","message":"TypeError: Cannot read property 'foo' of undefined","status":"detected"}}

event: incident:updated
data: {"type":"incident:updated","data":{"id":"550e8400-e29b-41d4-a716-446655440000","status":"pr_open","pr_url":"https://github.com/acme/webapp/pull/123","pr_number":123}}

event: stats:updated
data: {"type":"stats:updated","data":{"total":156,"by_state":[{"state":"pr_merged","count":89},{"state":"detected","count":32}]}}
```

**JavaScript Example**
```javascript
const eventSource = new EventSource('http://localhost:8081/api/v1/events');

eventSource.addEventListener('incident:created', (e) => {
  const data = JSON.parse(e.data);
  console.log('New incident:', data.data.message);
});

eventSource.addEventListener('incident:updated', (e) => {
  const data = JSON.parse(e.data);
  console.log('Incident updated:', data.data.id, data.data.status);
});

eventSource.addEventListener('stats:updated', (e) => {
  const data = JSON.parse(e.data);
  console.log('Stats updated:', data.data.total, 'total incidents');
});
```

**curl Example**
```bash
curl -N "http://localhost:8081/api/v1/events"
```

---

## Webhook Integration Guides

### Sentry Integration

CRISPR can receive error events directly from Sentry via webhooks. This allows you to use your existing Sentry installation while having CRISPR automatically generate fixes.

#### Setup Steps

1. **Navigate to Sentry Webhooks**
   - Go to your Sentry project settings
   - Navigate to **Settings > Integrations > Webhooks**

2. **Configure Webhook URL**
   - Add webhook URL: `https://your-crispr-host/webhooks/sentry`
   - Enable the following actions:
     - `issue.created` - New issues
     - `event.alert` - Error events

3. **Configure Webhook Secret (Optional but Recommended)**

   In Sentry:
   - Generate a webhook secret in Sentry's webhook settings

   In CRISPR config:
   ```yaml
   webhooks:
     sentry_secret: "your-sentry-webhook-secret"
   ```

4. **Map Sentry Projects to GitHub Repos**

   CRISPR uses the Sentry project slug to identify the GitHub repository. Projects are identified as `sentry/<project-slug>`.

   You can either:
   - Create a project in CRISPR with matching owner/repo, or
   - Let CRISPR auto-register projects (if `safety.allowed_repos` permits)

#### Sentry Webhook Payload Example

```json
{
  "action": "created",
  "actor": {
    "id": "123",
    "name": "sentry",
    "type": "application"
  },
  "data": {
    "issue": {
      "id": "12345",
      "title": "TypeError: Cannot read property 'foo' of undefined",
      "culprit": "app/api/users.js in getUser",
      "level": "error",
      "status": "unresolved",
      "platform": "javascript",
      "project": {
        "id": "1",
        "name": "My Project",
        "slug": "my-project"
      },
      "metadata": {
        "type": "TypeError",
        "value": "Cannot read property 'foo' of undefined",
        "filename": "app/api/users.js",
        "function": "getUser"
      }
    }
  }
}
```

#### Supported Sentry Actions

| Action | Description |
|--------|-------------|
| `created` | New issue created |
| `triggered` | Issue triggered |
| `resolved` | Issue resolved (ignored by CRISPR) |
| `assigned` | Issue assigned (ignored by CRISPR) |
| `ignored` | Issue ignored (ignored by CRISPR) |

Only `error`, `fatal`, and `critical` level issues are processed.

---

### Datadog Integration

CRISPR can receive error events from Datadog monitors and log-based alerts.

#### Setup Steps

1. **Create a Datadog Webhook**
   - Go to **Integrations > Webhooks** in Datadog
   - Click **New Webhook**
   - Configure:
     - **Name**: CRISPR
     - **URL**: `https://your-crispr-host/webhooks/datadog`
     - **Custom Headers**: `X-Datadog-Token: your-secret-token`

2. **Configure CRISPR**
   ```yaml
   webhooks:
     datadog_token: "your-secret-token"
   ```

3. **Add Project Tags to Your Monitors**

   CRISPR uses Datadog tags to identify the GitHub repository. Add one of these tags to your monitors:
   - `project:owner/repo` - Explicit project mapping
   - `repo:owner/repo` - Alternative format
   - `service:owner-repo` - Service name (hyphen converted to slash)

4. **Configure Monitor to Use Webhook**
   - Edit your Datadog monitor
   - In the notification section, add: `@webhook-CRISPR`

#### Datadog Webhook Payload Example

```json
{
  "id": "event-123",
  "title": "High error rate detected",
  "text": "Error rate exceeded 5% on /api/users endpoint\n\nStack trace:\n  at getUser (app/api/users.js:42)",
  "alert_type": "error",
  "date": 1705329600,
  "priority": "normal",
  "host": "api-server-1",
  "source_type_name": "Monitor Alert",
  "tags": [
    "env:production",
    "project:acme/webapp",
    "service:api"
  ],
  "monitor": {
    "id": 12345,
    "name": "API Error Rate Monitor",
    "type": "metric alert"
  },
  "link": "https://app.datadoghq.com/monitors/12345"
}
```

#### Datadog Alert Types Processed

| Alert Type | Processed |
|------------|-----------|
| `error` | Yes |
| `warning` | Only if priority is `critical` |
| `info` | No |
| `success` | No |

---

### Grafana Integration

CRISPR can receive alert notifications from Grafana Alerting (Unified Alerting).

#### Setup Steps

1. **Create a Contact Point in Grafana**
   - Go to **Alerting > Contact Points**
   - Click **Add contact point**
   - Configure:
     - **Name**: CRISPR
     - **Integration**: Webhook
     - **URL**: `https://your-crispr-host/webhooks/grafana`
     - **HTTP Method**: POST
     - (Optional) **Authorization Header**: `Bearer your-secret-token`

2. **Configure CRISPR**
   ```yaml
   webhooks:
     grafana_token: "your-secret-token"
   ```

3. **Add Labels to Your Alert Rules**

   CRISPR uses Grafana labels to identify the GitHub repository. Add one of these labels to your alert rules:
   - `project: owner/repo` - Explicit project mapping
   - `repo: owner/repo` - Alternative format
   - `repository: owner/repo` - Another alternative
   - `namespace` + `service` - Combined as `namespace/service`
   - `service: owner-repo` - Hyphen converted to slash

4. **Set Severity Labels**

   CRISPR processes alerts based on severity. Add a `severity` label:
   - `critical` or `high` → Processed as CRITICAL
   - `error` → Processed as ERROR
   - `warning` or `warn` → Processed as WARNING

5. **Create a Notification Policy**
   - Go to **Alerting > Notification policies**
   - Add a policy that routes alerts to the CRISPR contact point
   - You can use label matchers to only send certain alerts

#### Grafana Webhook Payload Example

```json
{
  "receiver": "crispr",
  "status": "firing",
  "alerts": [
    {
      "status": "firing",
      "labels": {
        "alertname": "HighErrorRate",
        "severity": "critical",
        "service": "api-gateway",
        "namespace": "production",
        "team": "platform"
      },
      "annotations": {
        "summary": "Error rate exceeded 5% threshold",
        "description": "The API gateway error rate is above 5% for the last 5 minutes",
        "runbook_url": "https://wiki.example.com/runbooks/high-error-rate"
      },
      "startsAt": "2024-01-15T10:00:00Z",
      "endsAt": "0001-01-01T00:00:00Z",
      "generatorURL": "https://grafana.example.com/alerting/grafana/abc123/view",
      "fingerprint": "abc123def456",
      "silenceURL": "https://grafana.example.com/alerting/silence/new",
      "dashboardURL": "https://grafana.example.com/d/abc123/api-gateway",
      "panelURL": "https://grafana.example.com/d/abc123/api-gateway?viewPanel=1",
      "values": {
        "A": 0.08
      },
      "valueString": "[ var='A' labels={} value=0.08 ]"
    }
  ],
  "groupLabels": {
    "alertname": "HighErrorRate"
  },
  "commonLabels": {
    "team": "platform",
    "env": "production"
  },
  "commonAnnotations": {
    "summary": "Error rate exceeded 5% threshold"
  },
  "externalURL": "https://grafana.example.com/",
  "version": "1",
  "groupKey": "{}:{alertname=\"HighErrorRate\"}",
  "truncatedAlerts": 0,
  "orgId": 1
}
```

#### Grafana Alert Statuses

| Status | Processed |
|--------|-----------|
| `firing` | Yes |
| `resolved` | No (used for recovery notifications) |

#### Grafana Severity Levels

| Severity Label | CRISPR Level |
|----------------|--------------|
| `critical`, `high` | CRITICAL |
| `error` | ERROR |
| `warning`, `warn` | WARNING |
| Other | ERROR (default) |

---

## Generated Tests

When CRISPR generates a fix, it also generates accompanying unit tests. These tests are:

1. Stored in the database linked to the incident
2. Included in the pull request (if configured)
3. Viewable via the incident detail endpoint

### Generated Test Schema

```json
{
  "id": "880e8400-e29b-41d4-a716-446655440000",
  "incident_id": "550e8400-e29b-41d4-a716-446655440000",
  "file_path": "src/api/__tests__/users.test.ts",
  "content": "import { getUser } from '../users';\n\ndescribe('getUser', () => {\n  it('should handle undefined user', () => {\n    expect(() => getUser(undefined)).not.toThrow();\n  });\n});",
  "description": "Test to verify getUser handles undefined input",
  "framework": "jest",
  "included_in_pr": true,
  "commit_sha": "abc123def456",
  "created_at": "2024-01-15T15:00:00Z"
}
```

Generated tests are visible in:
- The incident detail response (`tests_generated` and `test_count` fields)
- The generated PR body (under "Generated Tests" section)

---

## Prometheus Metrics

CRISPR exposes Prometheus metrics at `GET /metrics`.

Available metrics:
- `crispr_ingest_total` - Total ingested error logs
- `crispr_incidents_total` - Total incidents by state
- `crispr_llm_requests_total` - LLM API requests by provider/operation
- `crispr_llm_tokens_total` - LLM tokens used by provider/type

**curl Example**
```bash
curl "http://localhost:8081/metrics"
```

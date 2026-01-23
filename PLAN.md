# CRISPR Implementation Plan

## Overview
Build a full MVP of CRISPR - a self-hosted production error monitoring system that automatically generates fixes and opens PRs using LLMs.

**Target:** End-to-end flow from error ingestion → triage → fix generation → PR creation
**LLM Providers:** Anthropic, OpenAI, and Ollama (all three)
**Starting Point:** Greenfield - only SCAFFOLD.md exists

---

## Phase 1: Project Foundation & Database
**Goal:** Set up Rust project, database schema, and verify local development environment.

### Tasks
1. Initialize Rust project with Cargo.toml (all dependencies from scaffold)
2. Create project directory structure matching scaffold
3. Set up Docker Compose for local Postgres
4. Create SQL migrations (001_initial.sql, 002_llm_usage.sql)
5. Implement `src/config.rs` - configuration loading and validation
6. Implement `src/error.rs` - error types
7. Implement `src/store/postgres.rs` - database connection pool
8. Implement `src/store/models.rs` - database model structs
9. Create basic `src/main.rs` that loads config and connects to DB

### Verification
- `cargo build` succeeds
- `docker compose -f docker-compose.dev.yaml up -d` starts Postgres
- App connects to DB and runs migrations
- Config loads from `config/config.dev.yaml`

### Files to Create
```
crispr/
├── Cargo.toml
├── config/
│   ├── config.dev.yaml
│   └── pricing.toml
├── migrations/
│   ├── 001_initial.sql
│   └── 002_llm_usage.sql
├── docker-compose.dev.yaml
└── src/
    ├── main.rs
    ├── config.rs
    ├── error.rs
    └── store/
        ├── mod.rs
        ├── postgres.rs
        └── models.rs
```

---

## Phase 2: HTTP API & Ingest Endpoints (HTTP + OTLP/gRPC)
**Goal:** Build the Axum web server with REST API structure and both HTTP and OTLP ingest.

### Tasks
1. Implement `src/api/mod.rs` - AppState struct
2. Implement `src/api/router.rs` - route setup with embedded UI fallback
3. Implement `src/api/ingest.rs` - HTTP POST /ingest endpoint
4. Add OTLP gRPC server on port 4317 (Tonic)
5. Add OTLP proto definitions and build script
6. Implement `src/pipeline/fingerprint.rs` - error fingerprinting
7. Implement `src/store/object.rs` - sample storage (Postgres mode first)
8. Add CORS and tracing middleware
9. Create test script to POST sample errors

### Verification
- HTTP server starts on port 8080
- gRPC server starts on port 4317
- `POST /ingest` accepts error payloads
- OTLP logs are received via gRPC
- Incidents are created in database
- Samples are stored and compressed
- Fingerprinting deduplicates identical errors

### Files to Create/Modify
```
crispr/
├── build.rs              # tonic-build for OTLP protos
├── proto/
│   └── opentelemetry/    # OTLP proto definitions
└── src/
    ├── api/
    │   ├── mod.rs
    │   ├── router.rs
    │   └── ingest.rs     # HTTP + gRPC handlers
    ├── pipeline/
    │   ├── mod.rs
    │   └── fingerprint.rs
    └── store/
        └── object.rs
```

---

## Phase 3: LLM Integration Layer
**Goal:** Implement all three LLM providers with usage tracking and cost calculation.

### Tasks
1. Implement `src/integrations/llm/mod.rs` - LlmManager, TrackedLlm, traits
2. Implement `src/integrations/llm/anthropic.rs` - Claude API
3. Implement `src/integrations/llm/openai.rs` - GPT API
4. Implement `src/integrations/llm/ollama.rs` - Local models
5. Implement `src/integrations/llm/pricing.rs` - cost calculation from pricing.toml
6. Implement `src/api/settings.rs` - LLM config API endpoints
7. Add verify/list models endpoints

### Verification
- Configure LLM via API (`PUT /api/v1/settings/llm`)
- Verify API key works (`POST /api/v1/settings/llm/verify`)
- List available models (`POST /api/v1/settings/llm/models`)
- Usage is tracked in `llm_usage` table with costs

### Files to Create
```
src/
├── integrations/
│   ├── mod.rs
│   └── llm/
│       ├── mod.rs
│       ├── provider.rs
│       ├── anthropic.rs
│       ├── openai.rs
│       ├── ollama.rs
│       └── pricing.rs
└── api/
    └── settings.rs
```

---

## Phase 4: GitHub Integration
**Goal:** Connect to GitHub for repo access and PR creation.

### Tasks
1. Implement `src/integrations/github.rs` - GitHubClient
2. Add PAT mode support (simpler, for dev)
3. Add OAuth mode support (for prod)
4. Implement repo content fetching (read files, list directory)
5. Implement branch creation
6. Implement file commit
7. Implement PR creation
8. Add auth callback routes to router

### Verification
- Connect GitHub via PAT
- Fetch file contents from a test repo
- Create a branch and commit a file
- Open a PR programmatically

### Files to Create/Modify
```
src/
├── integrations/
│   └── github.rs
└── api/
    └── auth.rs
config/
    └── config.dev.yaml (add GitHub PAT)
```

---

## Phase 5: Core Pipeline (Triage → Context → Fix)
**Goal:** Implement the LLM-powered pipeline that analyzes errors and generates fixes.

### Tasks
1. Implement `src/pipeline/triage.rs` - LLM determines if error is fixable
2. Implement `src/pipeline/context.rs` - gather relevant code context from repo
3. Implement `src/pipeline/fix.rs` - LLM generates code fix
4. Implement `src/pipeline/pr.rs` - create PR with fix and description
5. Write effective prompts for each pipeline stage

### Verification
- Triage correctly identifies fixable vs unfixable errors
- Context gathering fetches relevant files
- Fix generation produces valid code patches
- PR creation includes good title/description

### Files to Create
```
src/
└── pipeline/
    ├── triage.rs
    ├── context.rs
    ├── fix.rs
    └── pr.rs
```

---

## Phase 6: Background Worker
**Goal:** Process incidents asynchronously in the background.

### Tasks
1. Implement `src/worker.rs` - background polling loop
2. Add state machine transitions (detected → triaging → fixing → pr_open)
3. Implement cooldown and retry logic
4. Add max attempts handling
5. Handle failures gracefully with state updates

### Verification
- Worker picks up new incidents automatically
- State transitions are logged in activity_log
- Failed fixes enter cooldown
- Max attempts respected

### Files to Create
```
src/
└── worker.rs
```

---

## Phase 7: REST API Completion
**Goal:** Complete all API endpoints for projects, incidents, and usage.

### Tasks
1. Implement `src/api/projects.rs` - CRUD for watched repos
2. Implement `src/api/incidents.rs` - list, get, retry, ignore
3. Complete usage endpoints in settings.rs
4. Add pagination and filtering

### Verification
- All API endpoints return correct data
- Can list/manage projects via API
- Can view incident details and trigger retry
- Usage stats are accurate

### Files to Create/Modify
```
src/
└── api/
    ├── projects.rs
    └── incidents.rs
```

---

## Phase 8: Svelte UI
**Goal:** Build the web dashboard for configuration and monitoring.

### Tasks
1. Initialize Svelte project in `ui/`
2. Implement `ui/src/lib/api.ts` - API client
3. Implement `ui/src/lib/stores.ts` - Svelte stores
4. Build layout with EnvBanner
5. Build Settings page - LLM configuration
6. Build Connect page - GitHub/Slack setup
7. Build Projects page - repo management
8. Build Incidents page - error list and details
9. Build Usage page - cost dashboard with charts
10. Configure Vite build and embed in Rust binary

### Verification
- UI builds and is served from Rust binary
- Can configure LLM providers through UI
- Can view and manage incidents
- Usage charts display correctly

### Files to Create
```
ui/
├── package.json
├── svelte.config.js
├── vite.config.js
├── src/
│   ├── app.html
│   ├── app.css
│   ├── lib/
│   │   ├── api.ts
│   │   ├── stores.ts
│   │   └── components/
│   │       ├── EnvBanner.svelte
│   │       ├── LlmConfig.svelte
│   │       ├── UsageChart.svelte
│   │       └── IncidentList.svelte
│   └── routes/
│       ├── +layout.svelte
│       ├── +page.svelte
│       ├── connect/
│       ├── projects/
│       ├── incidents/
│       ├── settings/
│       └── usage/
└── static/
```

---

## Phase 9: Slack Integration (Optional)
**Goal:** Add Slack notifications for PR events.

### Tasks
1. Implement `src/integrations/slack.rs` - SlackClient
2. Add OAuth flow for Slack
3. Implement notification methods (PR opened, merged, etc.)
4. Add Slack config to UI

### Verification
- Can connect Slack workspace
- Notifications sent when PRs are opened
- Channel selection works

---

## Phase 10: Production Hardening & dumpster-fire Demo
**Goal:** Prepare for production deployment and create the end-to-end test suite.

### Tasks
1. Implement S3/object storage mode for samples
2. Add proper encryption for stored secrets (AES-GCM)
3. Create production Dockerfile (multi-stage build)
4. Create docker-compose.yaml for production
5. Add health check endpoints (`/health`, `/ready`)
6. Write production config template
7. **Create `dumpster-fire` repo:**
   - TypeScript Express app with 10 intentional bugs
   - Error triggering endpoints that POST to CRISPR
   - `reset.sh` script to git restore all bugs
   - `verify.sh` script to run tests confirming fixes
   - README with demo instructions

### Verification
- Docker build succeeds
- Full docker-compose stack runs
- `dumpster-fire` triggers 10 errors to CRISPR
- CRISPR creates 10 PRs (or fewer if some are unfixable)
- Running `verify.sh` after merging PRs shows all tests pass
- `reset.sh` restores bugs for next demo run

---

---

## Architecture Notes: Minimizing LLM Usage

### Philosophy
Do as much as possible with algorithms and heuristics before involving LLMs. Many great log monitoring tools work without any LLM access.

### Current Approach
1. **Fingerprinting (no LLM)**: SHA256 + regex normalization to deduplicate errors
2. **Occurrence tracking (no LLM)**: Only process first occurrence, track counts
3. **Reader/Writer split**: Cheap model (Haiku/$0.25/MTok) for triage, expensive model (Sonnet/$3/MTok) only for fixes

### Future Enhancements
- [ ] Build local pattern index in Postgres (common error signatures)
- [ ] Regex-based error classification before any LLM involvement
- [ ] Auto-categorize known error types (NPE, OOBE, connection errors, etc.)
- [ ] Create GitHub issues as intermediate step (human review before PR)
- [ ] Only escalate truly novel errors to LLM triage
- [ ] Consider batching similar errors for single LLM call

### Tiered Processing Pipeline
```
Error Ingested
    │
    ├─► Fingerprint (local, no LLM)
    │       │
    │       ├─► Known pattern? → Log + track, no LLM
    │       │
    │       └─► Novel error? → Continue
    │
    ├─► Pattern matching (local regex rules)
    │       │
    │       ├─► Matches rule? → Auto-categorize, maybe auto-fix
    │       │
    │       └─► No match? → Continue
    │
    ├─► Cheap LLM (reader): Triage + describe problem
    │       │
    │       ├─► Not fixable? → Create issue, stop
    │       │
    │       └─► Fixable? → Continue
    │
    └─► Expensive LLM (writer): Generate fix → PR
```

---

## Execution Notes

### For Rust Newcomers
- Each phase builds on the previous
- Run `cargo clippy` frequently for idiomatic suggestions
- Use `cargo watch -x run` for hot reloading during development
- SQLx requires `DATABASE_URL` env var for compile-time checking

### Critical Dependencies
- Phases 1-2 must be sequential (foundation)
- Phase 3 (LLM) and Phase 4 (GitHub) can run in parallel
- Phase 5 requires both Phase 3 and 4
- Phase 8 (UI) can start after Phase 3

### Test Sandbox Repo: `dumpster-fire`
A TypeScript repo pre-packaged with 10 distinct bugs for end-to-end testing. Design:
- **Resettable:** Git reset script to restore all bugs for repeated demos
- **10 Bug Categories:**
  1. Null/undefined reference errors
  2. Type mismatches
  3. Off-by-one errors
  4. Missing error handling (try/catch)
  5. Incorrect string operations
  6. Array index out of bounds
  7. Async/await mistakes (missing await)
  8. Import/export errors
  9. Logic errors (wrong operator)
  10. API contract violations
- **Error emission:** Built-in error triggering that POSTs to CRISPR
- **Verification script:** Runs all fixed code to confirm bugs are resolved

### UI Development
- Run Svelte dev server separately (`npm run dev` in `ui/`)
- Configure Vite proxy to forward `/api/*` and `/ingest` to Rust backend
- Build and embed only for production/Docker builds

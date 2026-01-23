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

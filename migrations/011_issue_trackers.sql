-- Issue Tracker Integration
-- Supports GitHub Issues, Jira, and generic webhooks for external issue tracking

-- Issue tracker configuration per project
CREATE TABLE IF NOT EXISTS project_issue_trackers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,

    -- Provider: 'github', 'jira', 'webhook', 'linear'
    provider TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,

    -- GitHub Issues config
    github_labels TEXT[], -- Labels to apply (e.g., ['bug', 'crispr'])
    github_assignees TEXT[], -- GitHub usernames to assign

    -- Jira config
    jira_site_url TEXT, -- e.g., https://company.atlassian.net
    jira_project_key TEXT, -- e.g., 'BACKEND'
    jira_issue_type TEXT DEFAULT 'Bug', -- Bug, Task, Story, etc.
    jira_priority TEXT, -- Optional priority mapping

    -- Status mapping (JSONB for flexibility)
    -- Maps CRISPR states to external states/transitions
    -- e.g., {"detected": "Open", "fixing": "In Progress", "verified": "Done"}
    status_mapping JSONB DEFAULT '{}',

    -- Webhook config (for generic integrations)
    webhook_url TEXT,
    webhook_headers JSONB DEFAULT '{}', -- {"Authorization": "Bearer xxx"}
    webhook_events TEXT[] DEFAULT ARRAY['all'], -- 'all' or specific: ['detected', 'verified']

    -- Linear config
    linear_team_id TEXT,
    linear_project_id TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- One config per provider per project
    UNIQUE(project_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_project_issue_trackers_project ON project_issue_trackers(project_id);
CREATE INDEX IF NOT EXISTS idx_project_issue_trackers_provider ON project_issue_trackers(provider) WHERE enabled = true;

-- Track external issues linked to incidents
CREATE TABLE IF NOT EXISTS incident_external_issues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,

    -- Which tracker created this
    provider TEXT NOT NULL, -- 'github', 'jira', 'linear', 'webhook'

    -- External identifiers
    external_id TEXT, -- Issue number/key (e.g., '123' for GitHub, 'PROJ-456' for Jira)
    external_url TEXT, -- Full URL to the issue
    external_status TEXT, -- Current status in external system

    -- Sync tracking
    last_synced_at TIMESTAMPTZ,
    sync_error TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- One external issue per provider per incident
    UNIQUE(incident_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_incident_external_issues_incident ON incident_external_issues(incident_id);
CREATE INDEX IF NOT EXISTS idx_incident_external_issues_external ON incident_external_issues(provider, external_id);

-- Jira OAuth credentials (stored separately for security)
CREATE TABLE IF NOT EXISTS jira_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Site info
    site_url TEXT NOT NULL, -- https://company.atlassian.net
    cloud_id TEXT, -- Atlassian cloud ID

    -- OAuth tokens (encrypted in app layer)
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at TIMESTAMPTZ,

    -- API token auth (alternative to OAuth)
    api_token TEXT,
    user_email TEXT, -- Required with API token

    -- Connection info
    connected_by TEXT, -- Username who connected
    scopes TEXT[], -- Granted scopes

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(site_url)
);

-- Linear OAuth credentials
CREATE TABLE IF NOT EXISTS linear_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- OAuth tokens
    access_token TEXT,
    token_expires_at TIMESTAMPTZ,

    -- Organization info
    organization_id TEXT,
    organization_name TEXT,

    -- Connection info
    connected_by TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Webhook delivery log (for debugging)
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    incident_id UUID REFERENCES incidents(id) ON DELETE SET NULL,

    -- Delivery details
    webhook_url TEXT NOT NULL,
    event_type TEXT NOT NULL, -- 'incident.detected', 'incident.verified', etc.
    payload JSONB NOT NULL,

    -- Response
    response_status INT,
    response_body TEXT,

    -- Retry tracking
    attempt_number INT DEFAULT 1,
    next_retry_at TIMESTAMPTZ,

    -- Status
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'success', 'failed', 'retrying'
    error_message TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    delivered_at TIMESTAMPTZ
);

CREATE INDEX idx_webhook_deliveries_project ON webhook_deliveries(project_id);
CREATE INDEX idx_webhook_deliveries_status ON webhook_deliveries(status) WHERE status IN ('pending', 'retrying');
CREATE INDEX idx_webhook_deliveries_retry ON webhook_deliveries(next_retry_at) WHERE status = 'retrying';

-- Comments
COMMENT ON TABLE project_issue_trackers IS 'Issue tracker configuration per project (GitHub, Jira, webhook, Linear)';
COMMENT ON COLUMN project_issue_trackers.status_mapping IS 'Maps CRISPR incident states to external issue states/transitions';
COMMENT ON TABLE incident_external_issues IS 'Links incidents to their external issue tracker issues';
COMMENT ON TABLE jira_connections IS 'Jira OAuth/API credentials (tokens encrypted at app layer)';
COMMENT ON TABLE webhook_deliveries IS 'Log of webhook delivery attempts for debugging';

-- Internal Medicine feature: Internist LLM tier for pattern analysis and architectural recommendations

-- Internist LLM configuration
ALTER TABLE llm_config ADD COLUMN IF NOT EXISTS internist_provider TEXT;
ALTER TABLE llm_config ADD COLUMN IF NOT EXISTS internist_model TEXT;
ALTER TABLE llm_config ADD COLUMN IF NOT EXISTS internist_api_key TEXT;  -- encrypted
ALTER TABLE llm_config ADD COLUMN IF NOT EXISTS internist_base_url TEXT;

-- Architectural recommendations from pattern analysis
CREATE TABLE IF NOT EXISTS recommendations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status TEXT NOT NULL DEFAULT 'analyzing',
    -- analyzing | proposed | discussing | approved | implementing | completed | rejected
    title TEXT NOT NULL,
    summary TEXT,
    analysis JSONB,
    proposed_changes JSONB,
    priority TEXT DEFAULT 'medium',  -- low | medium | high | critical
    category TEXT,  -- api_contract | architecture | error_pattern | performance | security
    confidence REAL,
    source_incident_ids UUID[],
    -- PR tracking (when implementing)
    pr_url TEXT,
    pr_number INT,
    pr_state TEXT,
    pr_branch TEXT,
    -- Merge conflict detection
    base_commit_sha TEXT,
    has_conflicts BOOLEAN DEFAULT false,
    conflict_details JSONB,
    -- Timestamps
    analyzed_at TIMESTAMPTZ,
    proposed_at TIMESTAMPTZ,
    approved_at TIMESTAMPTZ,
    approved_by TEXT,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recommendations_status ON recommendations(status);
CREATE INDEX IF NOT EXISTS idx_recommendations_category ON recommendations(category);
CREATE INDEX IF NOT EXISTS idx_recommendations_priority ON recommendations(priority);
CREATE INDEX IF NOT EXISTS idx_recommendations_created ON recommendations(created_at DESC);

-- API contract files for breaking change detection
CREATE TABLE IF NOT EXISTS api_contract_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    file_type TEXT,  -- openapi | protobuf | graphql | routes | schema
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(project_id, file_path)
);

CREATE INDEX IF NOT EXISTS idx_api_contract_files_project ON api_contract_files(project_id);

-- Add API contract change flag to incidents
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS has_api_contract_changes BOOLEAN DEFAULT false;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS api_contract_change_details JSONB;

-- Recommendation discussion comments
CREATE TABLE IF NOT EXISTS recommendation_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recommendation_id UUID NOT NULL REFERENCES recommendations(id) ON DELETE CASCADE,
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recommendation_comments_rec ON recommendation_comments(recommendation_id);

-- Link recommendations to incidents
CREATE TABLE IF NOT EXISTS recommendation_incidents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recommendation_id UUID NOT NULL REFERENCES recommendations(id) ON DELETE CASCADE,
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    relevance_score REAL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(recommendation_id, incident_id)
);

CREATE INDEX IF NOT EXISTS idx_recommendation_incidents_rec ON recommendation_incidents(recommendation_id);
CREATE INDEX IF NOT EXISTS idx_recommendation_incidents_incident ON recommendation_incidents(incident_id);

-- Analysis run tracking
CREATE TABLE IF NOT EXISTS internist_analysis_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status TEXT NOT NULL DEFAULT 'running',  -- running | completed | failed
    incident_count INT,
    recommendations_created INT DEFAULT 0,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    error TEXT,
    input_tokens INT,
    output_tokens INT,
    cost_usd DECIMAL(10, 6)
);

CREATE INDEX IF NOT EXISTS idx_internist_analysis_runs_status ON internist_analysis_runs(status);
CREATE INDEX IF NOT EXISTS idx_internist_analysis_runs_started ON internist_analysis_runs(started_at DESC);

-- Comments
COMMENT ON TABLE recommendations IS 'Architectural recommendations from internist pattern analysis';
COMMENT ON COLUMN recommendations.status IS 'analyzing = in progress, proposed = ready for review, discussing = under discussion, approved = ready to implement, implementing = PR in progress, completed = merged, rejected = declined';
COMMENT ON COLUMN recommendations.category IS 'api_contract = breaking changes, architecture = structural issues, error_pattern = recurring bugs, performance = speed/resource issues, security = vulnerabilities';
COMMENT ON COLUMN recommendations.source_incident_ids IS 'Array of incident IDs that contributed to this recommendation';
COMMENT ON COLUMN recommendations.base_commit_sha IS 'Commit SHA the proposed changes were based on, for conflict detection';
COMMENT ON TABLE api_contract_files IS 'Files that define API contracts for breaking change detection';
COMMENT ON COLUMN api_contract_files.file_type IS 'openapi = OpenAPI/Swagger, protobuf = Protocol Buffers, graphql = GraphQL schema, routes = routing files, schema = database/JSON schema';
COMMENT ON COLUMN incidents.has_api_contract_changes IS 'True if this incidents fix modifies API contract files';
COMMENT ON TABLE recommendation_comments IS 'Discussion comments on recommendations';
COMMENT ON TABLE recommendation_incidents IS 'Links recommendations to related incidents';
COMMENT ON TABLE internist_analysis_runs IS 'Tracks each internist analysis execution';

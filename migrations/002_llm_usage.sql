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

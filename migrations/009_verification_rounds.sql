-- Verification tracking for post-deployment validation
-- Monitors merged fixes to ensure they don't regress

-- Add verification columns to incidents
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS verification_round INT DEFAULT 0;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS verification_status TEXT; -- 'verifying' | 'verified' | 'recurred'
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS verified_at TIMESTAMPTZ;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS pr_merged_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_incidents_verification_status ON incidents(verification_status) WHERE verification_status IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_incidents_pr_merged ON incidents(pr_merged_at) WHERE pr_merged_at IS NOT NULL;

-- Verification rounds log - tracks each monitoring period
CREATE TABLE IF NOT EXISTS verification_rounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    round_number INT NOT NULL,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'monitoring', -- 'monitoring' | 'passed' | 'failed'
    occurrence_count_start INT NOT NULL,
    occurrence_count_end INT,
    duration_hours INT NOT NULL,
    UNIQUE(incident_id, round_number)
);

CREATE INDEX IF NOT EXISTS idx_verification_rounds_incident ON verification_rounds(incident_id);
CREATE INDEX IF NOT EXISTS idx_verification_rounds_status ON verification_rounds(status) WHERE status = 'monitoring';

-- Incident relations for tracking overlapping fixes and dependencies
CREATE TABLE IF NOT EXISTS incident_relations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    related_incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL, -- 'overlapping_fix' | 'blocked_by' | 'superseded_by'
    confidence REAL,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(incident_id, related_incident_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_incident_relations_incident ON incident_relations(incident_id);
CREATE INDEX IF NOT EXISTS idx_incident_relations_related ON incident_relations(related_incident_id);
CREATE INDEX IF NOT EXISTS idx_incident_relations_type ON incident_relations(relation_type);

COMMENT ON TABLE verification_rounds IS 'Tracks verification monitoring periods after PR merge';
COMMENT ON COLUMN verification_rounds.round_number IS 'Round 1 = 1hr, Round 2 = 6hr, Round 3 = 24hr';
COMMENT ON COLUMN verification_rounds.occurrence_count_start IS 'Occurrence count at round start';
COMMENT ON COLUMN verification_rounds.occurrence_count_end IS 'Occurrence count at round end (null if still monitoring)';
COMMENT ON TABLE incident_relations IS 'Tracks relationships between incidents (overlapping fixes, blocking)';
COMMENT ON COLUMN incident_relations.relation_type IS 'overlapping_fix = same files modified, blocked_by = must wait, superseded_by = newer fix exists';
COMMENT ON COLUMN incident_relations.confidence IS 'Confidence score for LLM-detected relationships (0.0-1.0)';
COMMENT ON COLUMN incidents.verification_status IS 'verifying = in monitoring rounds, verified = passed all rounds, recurred = error reappeared';

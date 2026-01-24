-- Per-incident verification duration
-- Allows LLM to recommend appropriate monitoring periods (24h to 30 days)
-- based on error patterns and severity

-- Add per-incident verification timing columns
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS verification_duration_hours INT;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS verification_started_at TIMESTAMPTZ;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS verification_ends_at TIMESTAMPTZ;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS recurrence_count INT DEFAULT 0;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS recurrence_detected_at TIMESTAMPTZ;

-- Index for finding incidents with verification ending soon
CREATE INDEX IF NOT EXISTS idx_incidents_verification_ends
    ON incidents(verification_ends_at)
    WHERE verification_status = 'verifying';

COMMENT ON COLUMN incidents.verification_duration_hours IS 'LLM-recommended monitoring duration (24h to 720h/30 days)';
COMMENT ON COLUMN incidents.verification_started_at IS 'When verification monitoring began';
COMMENT ON COLUMN incidents.verification_ends_at IS 'When verification monitoring is expected to complete';
COMMENT ON COLUMN incidents.recurrence_count IS 'Number of new occurrences detected during verification';
COMMENT ON COLUMN incidents.recurrence_detected_at IS 'When recurrence was first detected';

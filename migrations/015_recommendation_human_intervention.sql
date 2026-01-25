-- Add requires_human_intervention flag to recommendations
-- For infrastructure changes that cannot be automated (DB scaling, server resources, etc.)

ALTER TABLE recommendations ADD COLUMN IF NOT EXISTS requires_human_intervention BOOLEAN DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_recommendations_human ON recommendations(requires_human_intervention)
    WHERE requires_human_intervention = true;

COMMENT ON COLUMN recommendations.requires_human_intervention IS 'True if this recommendation requires human action (infrastructure changes, manual configs)';

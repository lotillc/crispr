-- Worker claiming for distributed environments
-- Each worker claims an incident before processing to prevent duplicate work

ALTER TABLE incidents ADD COLUMN IF NOT EXISTS claimed_by TEXT;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_incidents_claimed ON incidents(claimed_by, claimed_at);

-- Add a workers table to track active workers
CREATE TABLE IF NOT EXISTS workers (
    id TEXT PRIMARY KEY,
    hostname TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    active_incident_id UUID REFERENCES incidents(id)
);

CREATE INDEX IF NOT EXISTS idx_workers_heartbeat ON workers(last_heartbeat);

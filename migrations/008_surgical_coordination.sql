-- Per-repo surgical locks for serializing fix operations
-- Prevents conflicts when multiple workers try to fix different errors in the same repo

CREATE TABLE IF NOT EXISTS repo_surgical_locks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    worker_id TEXT NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    UNIQUE(project_id)
);

CREATE INDEX IF NOT EXISTS idx_repo_surgical_locks_project ON repo_surgical_locks(project_id);
CREATE INDEX IF NOT EXISTS idx_repo_surgical_locks_expires ON repo_surgical_locks(expires_at);
CREATE INDEX IF NOT EXISTS idx_repo_surgical_locks_incident ON repo_surgical_locks(incident_id);

-- Queue tracking for incidents waiting on surgical locks
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS queued_at TIMESTAMPTZ;
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS queue_position INT;

CREATE INDEX IF NOT EXISTS idx_incidents_queued ON incidents(queued_at) WHERE queued_at IS NOT NULL;

COMMENT ON TABLE repo_surgical_locks IS 'Per-repo locks to serialize fix operations and prevent conflicts';
COMMENT ON COLUMN repo_surgical_locks.worker_id IS 'ID of the worker holding the lock';
COMMENT ON COLUMN repo_surgical_locks.expires_at IS 'Lock expiration time - allows cleanup of abandoned locks';
COMMENT ON COLUMN incidents.queued_at IS 'Timestamp when incident was queued waiting for repo lock';
COMMENT ON COLUMN incidents.queue_position IS 'Position in queue for repo lock (1 = next)';

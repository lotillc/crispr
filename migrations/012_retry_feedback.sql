-- Retry feedback stored in activity_log details
-- This migration is a no-op since feedback is stored in the activity_log table's
-- details JSONB column with action='retry_requested'
--
-- Example activity_log entry:
-- {
--   "action": "retry_requested",
--   "details": {
--     "feedback": "The fix doesn't handle null values",
--     "include_pr_comments": true,
--     "previous_state": "pr_open",
--     "has_existing_pr": true
--   }
-- }

-- No schema changes needed
SELECT 1;

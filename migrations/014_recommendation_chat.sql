-- Recommendation Chat Messages
-- Store chat history with Internist LLM for each recommendation

CREATE TABLE IF NOT EXISTS recommendation_chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recommendation_id UUID NOT NULL REFERENCES recommendations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,  -- 'user' or 'assistant'
    content TEXT NOT NULL,
    proposed_update JSONB,  -- Proposed changes from assistant messages
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recommendation_chat_messages_rec ON recommendation_chat_messages(recommendation_id);
CREATE INDEX IF NOT EXISTS idx_recommendation_chat_messages_created ON recommendation_chat_messages(recommendation_id, created_at);

COMMENT ON TABLE recommendation_chat_messages IS 'Chat history with Internist LLM for refining recommendations';
COMMENT ON COLUMN recommendation_chat_messages.proposed_update IS 'JSON containing proposed changes (title, summary, analysis, etc.) from assistant';

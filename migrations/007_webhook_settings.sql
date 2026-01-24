-- Webhook integration settings
-- Stores configuration for Sentry, Datadog, and Grafana webhook integrations

CREATE TABLE webhook_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Sentry configuration
    sentry_enabled BOOLEAN DEFAULT false,
    sentry_secret TEXT,  -- HMAC-SHA256 secret for signature verification

    -- Datadog configuration
    datadog_enabled BOOLEAN DEFAULT false,
    datadog_token TEXT,  -- Token for webhook verification

    -- Grafana configuration
    grafana_enabled BOOLEAN DEFAULT false,
    grafana_token TEXT,  -- Bearer token for authentication

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Only one row should exist
CREATE UNIQUE INDEX idx_webhook_settings_singleton ON webhook_settings ((true));

COMMENT ON TABLE webhook_settings IS 'Configuration for external webhook integrations';
COMMENT ON COLUMN webhook_settings.sentry_secret IS 'HMAC-SHA256 secret for Sentry webhook signature verification';
COMMENT ON COLUMN webhook_settings.datadog_token IS 'Token sent in X-Datadog-Token header for verification';
COMMENT ON COLUMN webhook_settings.grafana_token IS 'Bearer token for Grafana webhook authentication';

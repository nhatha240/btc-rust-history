-- ============================================================================
-- PostgreSQL migration 007 — Risk management (Limits & Events)
-- ============================================================================

-- ---------------------------------------------------------------------------
-- risk_limit_profiles (reusable sets of limits)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS risk_limit_profiles (
    profile_id            UUID            PRIMARY KEY DEFAULT uuidv7(),
    name                  TEXT            NOT NULL,
    description           TEXT,
    created_at            TIMESTAMPTZ     NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- risk_limits (concrete limit values)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS risk_limits (
    risk_limit_id         UUID            PRIMARY KEY DEFAULT uuidv7(),
    profile_id            UUID            REFERENCES risk_limit_profiles(profile_id),
    scope_type            TEXT            NOT NULL, -- 'ACCOUNT', 'STRATEGY', 'SYMBOL', 'VENUE'
    scope_ref             TEXT            NOT NULL, -- e.g. account_id or symbol
    limit_name            TEXT            NOT NULL, -- e.g. 'MAX_POSITION_NOTIONAL'
    limit_value           NUMERIC(28, 10) NOT NULL,
    hard_or_soft          TEXT            NOT NULL DEFAULT 'HARD',
    enabled               BOOLEAN         NOT NULL DEFAULT TRUE,
    effective_from        TIMESTAMPTZ     NOT NULL DEFAULT now(),
    effective_to          TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS ix_risk_limits_scope ON risk_limits (scope_type, scope_ref, enabled);

-- ---------------------------------------------------------------------------
-- risk_events (audit trail for EVERY risk check - replaces risk_rejections)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS risk_events (
    id               BIGSERIAL,
    event_time       TIMESTAMPTZ      NOT NULL DEFAULT now(),
    check_type       TEXT             NOT NULL, -- e.g. 'MARGIN', 'NOTIONAL', 'LEVERAGE'
    scope_type       TEXT             NOT NULL,
    scope_ref        TEXT             NOT NULL,
    severity         TEXT             NOT NULL DEFAULT 'INFO', -- 'INFO', 'WARNING', 'CRITICAL'
    pass_flag        BOOLEAN          NOT NULL,
    current_value    NUMERIC(28, 10),
    limit_value      NUMERIC(28, 10),
    action_taken     TEXT,            -- 'APPROVED', 'REJECTED', 'WARNED'
    related_order_id TEXT,
    related_signal_id UUID,
    trace_id         TEXT
);

-- TimescaleDB requires partition column in PK
ALTER TABLE risk_events DROP CONSTRAINT IF EXISTS risk_events_pkey;
ALTER TABLE risk_events ADD CONSTRAINT risk_events_pkey PRIMARY KEY (id, event_time);

-- Convert to hypertable
SELECT create_hypertable(
    'risk_events', 'event_time',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

-- ---------------------------------------------------------------------------
-- Migration: Copy from risk_rejections if it exists
-- ---------------------------------------------------------------------------
DO $$ 
BEGIN
    IF EXISTS (SELECT FROM pg_tables WHERE schemaname = 'public' AND tablename = 'risk_rejections') THEN
        INSERT INTO risk_events (
            event_time, 
            check_type, 
            scope_type, 
            scope_ref, 
            severity, 
            pass_flag, 
            current_value, 
            limit_value, 
            action_taken, 
            related_order_id, 
            trace_id
        )
        SELECT 
            rejected_at, 
            reject_reason, 
            'ACCOUNT', 
            account_id, 
            'CRITICAL', 
            FALSE, 
            notional, 
            0, -- limit_value not stored in old table
            'REJECTED', 
            client_order_id, 
            trace_id
        FROM risk_rejections;
    END IF;
END $$;

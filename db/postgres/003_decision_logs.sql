-- ============================================================================
-- PostgreSQL migration 003 — Decision logs
-- Append-only audit trail of every strategy decision (enter/exit/hold/blocked)
-- ============================================================================

DO $$ BEGIN
    CREATE TYPE signal_direction AS ENUM ('LONG', 'SHORT', 'HOLD');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE decision_action AS ENUM ('ENTER', 'EXIT', 'HOLD', 'BLOCKED');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

CREATE TABLE IF NOT EXISTS decision_logs (
    id               BIGSERIAL,
    trace_id         UUID,
    account_id       TEXT             NOT NULL,
    symbol           TEXT             NOT NULL,
    direction        signal_direction NOT NULL,
    action           decision_action  NOT NULL,
    block_reason     TEXT,                           -- populated when action = BLOCKED or HOLD
    confidence       FLOAT,
    model_version    TEXT,
    feature_version  TEXT,
    strategy_version TEXT,
    entry_price      NUMERIC(20, 8),
    qty              NUMERIC(20, 8),
    tp_price         NUMERIC(20, 8),
    sl_price         NUMERIC(20, 8),
    features         JSONB,                          -- indicator snapshot at decision time
    decided_at       TIMESTAMPTZ      NOT NULL DEFAULT now()
);

-- Timescale requires PRIMARY/UNIQUE keys to include partition column (decided_at).
ALTER TABLE decision_logs DROP CONSTRAINT IF EXISTS decision_logs_pkey;
ALTER TABLE decision_logs ADD CONSTRAINT decision_logs_pkey PRIMARY KEY (id, decided_at);

CREATE INDEX IF NOT EXISTS idx_decision_logs_symbol
    ON decision_logs (symbol);
CREATE INDEX IF NOT EXISTS idx_decision_logs_decided_at
    ON decision_logs (decided_at DESC);
CREATE INDEX IF NOT EXISTS idx_decision_logs_trace_id
    ON decision_logs (trace_id)
    WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_decision_logs_action
    ON decision_logs (action, decided_at DESC);

-- Convert to TimescaleDB hypertable for efficient time-range queries
SELECT create_hypertable(
    'decision_logs', 'decided_at',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

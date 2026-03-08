-- ============================================================================
-- PostgreSQL migration 003 — Strategy Decision Logs
-- ============================================================================

DO $$ BEGIN
    CREATE TYPE signal_direction AS ENUM ('LONG', 'SHORT', 'NEUTRAL');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE decision_action AS ENUM ('ENTER', 'EXIT', 'REJECTED', 'NONE');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

CREATE TABLE IF NOT EXISTS decision_logs (
    id                BIGSERIAL,
    trace_id          UUID            DEFAULT uuid_generate_v4(),
    account_id        TEXT            NOT NULL,
    symbol            TEXT            NOT NULL,
    direction         signal_direction NOT NULL,
    action            decision_action NOT NULL,
    block_reason      TEXT,
    confidence        NUMERIC(20, 8),
    model_version     TEXT,
    feature_version   TEXT,
    strategy_version  TEXT,
    entry_price       NUMERIC(20, 8),
    qty               NUMERIC(20, 8),
    tp_price          NUMERIC(20, 8),
    sl_price          NUMERIC(20, 8),
    features          JSONB,
    decided_at        TIMESTAMPTZ    NOT NULL DEFAULT now()
);

-- Timescale require partition column in PK
ALTER TABLE decision_logs DROP CONSTRAINT IF EXISTS decision_logs_pkey;
ALTER TABLE decision_logs ADD CONSTRAINT decision_logs_pkey PRIMARY KEY (id, decided_at);

CREATE INDEX IF NOT EXISTS idx_dec_logs_account_symbol
    ON decision_logs (account_id, symbol, decided_at DESC);

CREATE INDEX IF NOT EXISTS idx_dec_logs_trace_id
    ON decision_logs (trace_id);

-- Convert to TimescaleDB hypertable for efficient analysis
SELECT create_hypertable(
    'decision_logs', 'decided_at',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

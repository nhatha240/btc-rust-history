-- ============================================================================
-- PostgreSQL migration 004 — Risk rejections audit log
-- Append-only record of every order rejected by risk_guard.
-- Queried by api_gateway → /api/risk/rejections  (web dashboard).
-- ============================================================================

CREATE TABLE IF NOT EXISTS risk_rejections (
    id               BIGSERIAL,
    client_order_id  TEXT            NOT NULL,
    account_id       TEXT            NOT NULL,
    symbol           TEXT            NOT NULL,
    qty              NUMERIC(20, 8)  NOT NULL DEFAULT 0,
    price            NUMERIC(20, 8)  NOT NULL DEFAULT 0,
    notional         NUMERIC(20, 8)  NOT NULL DEFAULT 0,
    -- Normalised SCREAMING_SNAKE_CASE code from hft_risk::RejectReason
    reject_reason    TEXT            NOT NULL,
    -- Human-readable detail string built by the checker
    reject_detail    TEXT            NOT NULL DEFAULT '',
    trace_id         TEXT,
    rejected_at      TIMESTAMPTZ     NOT NULL DEFAULT now()
);

-- TimescaleDB requires partition column in PK
ALTER TABLE risk_rejections DROP CONSTRAINT IF EXISTS risk_rejections_pkey;
ALTER TABLE risk_rejections ADD CONSTRAINT risk_rejections_pkey
    PRIMARY KEY (id, rejected_at);

CREATE INDEX IF NOT EXISTS idx_risk_rej_account
    ON risk_rejections (account_id, rejected_at DESC);

CREATE INDEX IF NOT EXISTS idx_risk_rej_symbol
    ON risk_rejections (symbol, rejected_at DESC);

CREATE INDEX IF NOT EXISTS idx_risk_rej_reason
    ON risk_rejections (reject_reason, rejected_at DESC);

CREATE INDEX IF NOT EXISTS idx_risk_rej_order
    ON risk_rejections (client_order_id);

-- Convert to hypertable for efficient time-range dashboard queries
SELECT create_hypertable(
    'risk_rejections', 'rejected_at',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

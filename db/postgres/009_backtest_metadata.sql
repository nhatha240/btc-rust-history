-- ============================================================================
-- PostgreSQL migration 009 — Backtest Meta
-- ============================================================================

-- ---------------------------------------------------------------------------
-- bt_runs (Historical runs)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS bt_runs (
    run_id                UUID            PRIMARY KEY DEFAULT uuidv7(),
    strategy_name         TEXT            NOT NULL,
    data_slice            TEXT            NOT NULL, -- e.g. '2023-01-01 to 2023-12-31'
    config_hash           TEXT,
    parameter_json        JSONB,
    metrics_json          JSONB,
    started_at            TIMESTAMPTZ     NOT NULL DEFAULT now(),
    finished_at           TIMESTAMPTZ
);

-- ---------------------------------------------------------------------------
-- bt_replay_sessions (Exact session replays)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS bt_replay_sessions (
    replay_id             UUID            PRIMARY KEY DEFAULT uuidv7(),
    venue_code            TEXT            NOT NULL,
    date                  DATE            NOT NULL,
    symbol_universe       TEXT[],
    source_archive        TEXT,
    start_offset          BIGINT,
    end_offset            BIGINT,
    created_at            TIMESTAMPTZ     NOT NULL DEFAULT now()
);

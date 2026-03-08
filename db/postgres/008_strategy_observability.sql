-- ============================================================================
-- PostgreSQL migration 008 — Strategy Observability
-- ============================================================================

-- ---------------------------------------------------------------------------
-- strat_logs (Why they trade or don't trade)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS strat_logs (
    id                BIGSERIAL,
    strategy_version_id TEXT            NOT NULL,
    symbol            TEXT            NOT NULL,
    event_time        TIMESTAMPTZ     NOT NULL DEFAULT now(),
    log_level         TEXT            NOT NULL DEFAULT 'INFO', -- 'DEBUG', 'INFO', 'WARN'
    event_code        TEXT            NOT NULL, -- e.g. 'REGIME_REJECT', 'SPREAD_TOO_WIDE'
    message           TEXT,
    context_json      JSONB
);

-- TimescaleDB requires partition column in PK
ALTER TABLE strat_logs DROP CONSTRAINT IF EXISTS strat_logs_pkey;
ALTER TABLE strat_logs ADD CONSTRAINT strat_logs_pkey PRIMARY KEY (id, event_time);

-- Convert to hypertable
SELECT create_hypertable(
    'strat_logs', 'event_time',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

-- ---------------------------------------------------------------------------
-- strat_health (Service heartbeats)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS strat_health (
    id                BIGSERIAL,
    instance_id       TEXT            NOT NULL, -- unique ID for the process instance
    strategy_name     TEXT            NOT NULL,
    reported_at       TIMESTAMPTZ     NOT NULL DEFAULT now(),
    cpu_pct           NUMERIC(5, 2),
    mem_mb            NUMERIC(10, 2),
    queue_lag_ms      INTEGER,
    last_market_ts    TIMESTAMPTZ,
    last_signal_ts    TIMESTAMPTZ
);

-- TimescaleDB requires partition column in PK
ALTER TABLE strat_health DROP CONSTRAINT IF EXISTS strat_health_pkey;
ALTER TABLE strat_health ADD CONSTRAINT strat_health_pkey PRIMARY KEY (id, reported_at);

-- Convert to hypertable
SELECT create_hypertable(
    'strat_health', 'reported_at',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

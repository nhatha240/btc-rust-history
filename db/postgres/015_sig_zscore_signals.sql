-- ============================================================================
-- PostgreSQL migration 015 — Z-score alpha signal storage
-- ============================================================================

CREATE TABLE IF NOT EXISTS sig_zscore_signals (
    id                  BIGSERIAL,
    signal_id           UUID            NOT NULL DEFAULT uuid_generate_v4(),
    trace_id            UUID            NOT NULL DEFAULT uuid_generate_v4(),
    symbol              TEXT            NOT NULL,
    event_ts            TIMESTAMPTZ     NOT NULL,
    side                signal_direction NOT NULL,
    signal_value        DOUBLE PRECISION NOT NULL,
    predicted_return    DOUBLE PRECISION NOT NULL,
    mean_return         DOUBLE PRECISION NOT NULL,
    std_return          DOUBLE PRECISION NOT NULL,
    confidence          DOUBLE PRECISION NOT NULL,
    suppressed          BOOLEAN         NOT NULL DEFAULT FALSE,
    suppression_reasons TEXT[]          NOT NULL DEFAULT ARRAY[]::TEXT[],
    regime_passed       BOOLEAN         NOT NULL DEFAULT TRUE,
    volatility_passed   BOOLEAN         NOT NULL DEFAULT TRUE,
    factors_used        INTEGER         NOT NULL,
    total_factors       INTEGER         NOT NULL,
    model_version       TEXT            NOT NULL,
    current_regime      TEXT,
    current_volatility  DOUBLE PRECISION,
    metadata            JSONB           NOT NULL DEFAULT '{}'::JSONB,
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT now()
);

ALTER TABLE sig_zscore_signals DROP CONSTRAINT IF EXISTS sig_zscore_signals_pkey;
ALTER TABLE sig_zscore_signals ADD CONSTRAINT sig_zscore_signals_pkey PRIMARY KEY (id, event_ts);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sig_zscore_signals_signal_id
    ON sig_zscore_signals (signal_id);

CREATE INDEX IF NOT EXISTS idx_sig_zscore_signals_symbol_ts
    ON sig_zscore_signals (symbol, event_ts DESC);

CREATE INDEX IF NOT EXISTS idx_sig_zscore_signals_trace_id
    ON sig_zscore_signals (trace_id);

SELECT create_hypertable(
    'sig_zscore_signals', 'event_ts',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);


CREATE TABLE IF NOT EXISTS sig_zscore_factor_contribs (
    id                BIGSERIAL,
    event_ts          TIMESTAMPTZ      NOT NULL,
    signal_id         UUID             NOT NULL,
    symbol            TEXT             NOT NULL,
    factor_name       TEXT             NOT NULL,
    factor_rank       INTEGER,
    correlation       DOUBLE PRECISION,
    sign              SMALLINT,
    zscore            DOUBLE PRECISION,
    raw_weight        DOUBLE PRECISION,
    normalized_weight DOUBLE PRECISION,
    contribution      DOUBLE PRECISION,
    passed            BOOLEAN          NOT NULL DEFAULT FALSE,
    reject_reason     TEXT,
    created_at        TIMESTAMPTZ      NOT NULL DEFAULT now()
);

ALTER TABLE sig_zscore_factor_contribs DROP CONSTRAINT IF EXISTS sig_zscore_factor_contribs_pkey;
ALTER TABLE sig_zscore_factor_contribs ADD CONSTRAINT sig_zscore_factor_contribs_pkey PRIMARY KEY (id, event_ts);

CREATE INDEX IF NOT EXISTS idx_sig_zscore_factor_contribs_signal_id
    ON sig_zscore_factor_contribs (signal_id);

CREATE INDEX IF NOT EXISTS idx_sig_zscore_factor_contribs_symbol_ts
    ON sig_zscore_factor_contribs (symbol, event_ts DESC);

SELECT create_hypertable(
    'sig_zscore_factor_contribs', 'event_ts',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

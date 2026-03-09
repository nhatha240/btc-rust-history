-- ============================================================================
-- PostgreSQL migration 014 — Rich training events for AI labeling
-- ============================================================================

CREATE TABLE IF NOT EXISTS order_training_events (
    id BIGSERIAL PRIMARY KEY,
    client_order_id UUID NOT NULL REFERENCES orders (client_order_id),
    account_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    order_status TEXT NOT NULL,
    execution_mode TEXT NOT NULL,    -- PAPER / REAL
    exchange TEXT NOT NULL,          -- BINANCE / OKX / ...
    strategy_id TEXT,
    signal_id TEXT,
    exit_kind TEXT,                  -- STOP_LOSS / TAKE_PROFIT / NULL
    outcome_label TEXT NOT NULL DEFAULT 'UNKNOWN', -- WIN / LOSS / UNKNOWN
    outcome_reason TEXT,
    coin_tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    decision_meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    filled_qty NUMERIC(20, 8),
    fill_price NUMERIC(20, 8),
    trace_id UUID,
    event_time TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_order_training_events_symbol_time
    ON order_training_events (symbol, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_order_training_events_mode_time
    ON order_training_events (execution_mode, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_order_training_events_outcome
    ON order_training_events (outcome_label, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_order_training_events_strategy
    ON order_training_events (strategy_id, event_time DESC);

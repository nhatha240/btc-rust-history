-- ============================================================================
-- PostgreSQL migration 013 — TP/SL fields and coin-tag snapshot on exit
-- ============================================================================

-- 1) Keep stop_loss and take_profit explicitly in OMS source-of-truth.
ALTER TABLE orders
    ADD COLUMN IF NOT EXISTS take_profit_price NUMERIC(20, 8),
    ADD COLUMN IF NOT EXISTS coin_tags JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_orders_take_profit_price
    ON orders (take_profit_price)
    WHERE take_profit_price IS NOT NULL;

-- 2) Snapshot tags when an order exits by TP/SL.
CREATE TABLE IF NOT EXISTS order_exit_tag_snapshots (
    id BIGSERIAL PRIMARY KEY,
    client_order_id UUID NOT NULL REFERENCES orders (client_order_id),
    account_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    exit_trigger TEXT NOT NULL CHECK (exit_trigger IN ('STOP_LOSS', 'TAKE_PROFIT')),
    exit_price NUMERIC(20, 8) NOT NULL,
    coin_tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    trace_id UUID,
    event_time TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_order_exit_tag_snapshots_order
    ON order_exit_tag_snapshots (client_order_id, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_order_exit_tag_snapshots_symbol_time
    ON order_exit_tag_snapshots (symbol, event_time DESC);

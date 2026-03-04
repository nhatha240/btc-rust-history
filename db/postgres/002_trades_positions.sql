-- ============================================================================
-- PostgreSQL migration 002 — Trades & positions
-- Depends on : 001_orders.sql  (orders table + order_side ENUM)
-- ============================================================================

-- ---------------------------------------------------------------------------
-- trades  (one row per filled leg / partial fill from the exchange)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS trades (
    id               BIGSERIAL      PRIMARY KEY,
    trade_id         BIGINT         NOT NULL,           -- exchange trade id
    order_id         BIGINT         NOT NULL REFERENCES orders (id),
    client_order_id  UUID           NOT NULL,
    account_id       TEXT           NOT NULL,
    symbol           TEXT           NOT NULL,
    side             order_side     NOT NULL,
    qty              NUMERIC(20, 8) NOT NULL,
    price            NUMERIC(20, 8) NOT NULL,
    quote_qty        NUMERIC(20, 8) NOT NULL,           -- qty * price
    commission       NUMERIC(20, 8) NOT NULL DEFAULT 0,
    commission_asset TEXT,
    realized_pnl     NUMERIC(20, 8),
    is_maker         BOOLEAN        NOT NULL DEFAULT FALSE,
    trade_time       TIMESTAMPTZ    NOT NULL,
    recv_time        TIMESTAMPTZ    NOT NULL DEFAULT now(),
    CONSTRAINT uq_trades_id_symbol UNIQUE (trade_id, symbol)
);

CREATE INDEX IF NOT EXISTS idx_trades_order_id
    ON trades (order_id);
CREATE INDEX IF NOT EXISTS idx_trades_account_symbol
    ON trades (account_id, symbol);
CREATE INDEX IF NOT EXISTS idx_trades_trade_time
    ON trades (trade_time DESC);

-- Convert to TimescaleDB hypertable for efficient time-range queries
SELECT create_hypertable(
    'trades', 'trade_time',
    if_not_exists => TRUE,
    migrate_data  => TRUE
);

-- ---------------------------------------------------------------------------
-- positions  (latest snapshot per account/symbol; upserted on each fill)
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE position_side AS ENUM ('LONG', 'SHORT', 'BOTH');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

CREATE TABLE IF NOT EXISTS positions (
    id                BIGSERIAL      PRIMARY KEY,
    account_id        TEXT           NOT NULL,
    symbol            TEXT           NOT NULL,
    side              position_side  NOT NULL DEFAULT 'BOTH',
    qty               NUMERIC(20, 8) NOT NULL DEFAULT 0,
    entry_price       NUMERIC(20, 8),
    unrealized_pnl    NUMERIC(20, 8) NOT NULL DEFAULT 0,
    realized_pnl      NUMERIC(20, 8) NOT NULL DEFAULT 0,
    leverage          INT            NOT NULL DEFAULT 1,
    margin_type       TEXT           NOT NULL DEFAULT 'isolated',
    liquidation_price NUMERIC(20, 8),
    snapshot_time     TIMESTAMPTZ    NOT NULL DEFAULT now(),
    CONSTRAINT uq_positions_account_symbol_side UNIQUE (account_id, symbol, side)
);

CREATE INDEX IF NOT EXISTS idx_positions_account_id
    ON positions (account_id);
CREATE INDEX IF NOT EXISTS idx_positions_symbol
    ON positions (symbol);

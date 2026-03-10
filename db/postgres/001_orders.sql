-- ============================================================================
-- PostgreSQL migration 001 — Orders & order events
-- Database : db_trading  (TimescaleDB)
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ---------------------------------------------------------------------------
-- ENUMs
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE order_side AS ENUM ('BUY', 'SELL');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE order_type AS ENUM (
        'MARKET', 'LIMIT', 'STOP_MARKET', 'STOP_LIMIT',
        'TAKE_PROFIT', 'TAKE_PROFIT_MARKET', 'TRAILING_STOP_MARKET'
    );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE order_status AS ENUM (
        'NEW', 'PARTIALLY_FILLED', 'FILLED',
        'CANCELED', 'REJECTED', 'EXPIRED'
    );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE time_in_force AS ENUM ('GTC', 'IOC', 'FOK', 'GTX');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
    CREATE TYPE order_event_type AS ENUM (
        'SUBMITTED', 'ACKNOWLEDGED', 'PARTIALLY_FILLED', 'FILLED',
        'CANCELED', 'REJECTED', 'EXPIRED', 'REPLACE_REQUESTED'
    );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- ---------------------------------------------------------------------------
-- orders
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS orders (
    id                BIGSERIAL       PRIMARY KEY,
    client_order_id   UUID            NOT NULL UNIQUE DEFAULT uuidv7(),
    exchange_order_id BIGINT,
    account_id        TEXT            NOT NULL,
    symbol            TEXT            NOT NULL,
    side              order_side      NOT NULL,
    type              order_type      NOT NULL,
    tif               time_in_force   NOT NULL DEFAULT 'GTC',
    qty               NUMERIC(20, 8)  NOT NULL,
    price             NUMERIC(20, 8),          -- NULL for MARKET orders
    stop_price        NUMERIC(20, 8),
    status            order_status    NOT NULL DEFAULT 'NEW',
    filled_qty        NUMERIC(20, 8)  NOT NULL DEFAULT 0,
    avg_price         NUMERIC(20, 8),
    reduce_only       BOOLEAN         NOT NULL DEFAULT FALSE,
    trace_id          UUID,
    strategy_version  TEXT,
    created_at        TIMESTAMPTZ     NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ     NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_orders_symbol_status
    ON orders (symbol, status);
CREATE INDEX IF NOT EXISTS idx_orders_account_id
    ON orders (account_id);
CREATE INDEX IF NOT EXISTS idx_orders_created_at
    ON orders (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_exchange_order_id
    ON orders (exchange_order_id)
    WHERE exchange_order_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_orders_trace_id
    ON orders (trace_id)
    WHERE trace_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- order_events  (append-only audit trail of every status transition)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS order_events (
    id               BIGSERIAL        PRIMARY KEY,
    order_id         BIGINT           NOT NULL REFERENCES orders (id),
    client_order_id  UUID             NOT NULL,
    event_type       order_event_type NOT NULL,
    filled_qty       NUMERIC(20, 8),
    price            NUMERIC(20, 8),
    commission       NUMERIC(20, 8),
    commission_asset TEXT,
    event_time       TIMESTAMPTZ      NOT NULL,
    recv_time        TIMESTAMPTZ      NOT NULL DEFAULT now(),
    raw              JSONB
);

CREATE INDEX IF NOT EXISTS idx_order_events_order_id
    ON order_events (order_id);
CREATE INDEX IF NOT EXISTS idx_order_events_event_time
    ON order_events (event_time DESC);

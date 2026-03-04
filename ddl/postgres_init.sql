-- ============================================================================
-- PostgreSQL bootstrap — auto-run by TimescaleDB Docker image on first start
-- Combines migrations: 001_orders → 002_trades_positions → 003_decision_logs
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ── ENUMs ────────────────────────────────────────────────────────────────────
DO $$ BEGIN CREATE TYPE order_side AS ENUM ('BUY','SELL'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE order_type AS ENUM ('MARKET','LIMIT','STOP_MARKET','STOP_LIMIT','TAKE_PROFIT','TAKE_PROFIT_MARKET','TRAILING_STOP_MARKET'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE order_status AS ENUM ('NEW','PARTIALLY_FILLED','FILLED','CANCELED','REJECTED','EXPIRED'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE time_in_force AS ENUM ('GTC','IOC','FOK','GTX'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE order_event_type AS ENUM ('SUBMITTED','ACKNOWLEDGED','PARTIALLY_FILLED','FILLED','CANCELED','REJECTED','EXPIRED','REPLACE_REQUESTED'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE position_side AS ENUM ('LONG','SHORT','BOTH'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE signal_direction AS ENUM ('LONG','SHORT','HOLD'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE TYPE decision_action AS ENUM ('ENTER','EXIT','HOLD','BLOCKED'); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- ── orders ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS orders (
    id                BIGSERIAL       PRIMARY KEY,
    client_order_id   UUID            NOT NULL UNIQUE DEFAULT uuid_generate_v4(),
    exchange_order_id BIGINT,
    account_id        TEXT            NOT NULL,
    symbol            TEXT            NOT NULL,
    side              order_side      NOT NULL,
    type              order_type      NOT NULL,
    tif               time_in_force   NOT NULL DEFAULT 'GTC',
    qty               NUMERIC(20,8)   NOT NULL,
    price             NUMERIC(20,8),
    stop_price        NUMERIC(20,8),
    status            order_status    NOT NULL DEFAULT 'NEW',
    filled_qty        NUMERIC(20,8)   NOT NULL DEFAULT 0,
    avg_price         NUMERIC(20,8),
    reduce_only       BOOLEAN         NOT NULL DEFAULT FALSE,
    trace_id          UUID,
    strategy_version  TEXT,
    created_at        TIMESTAMPTZ     NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ     NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_orders_symbol_status     ON orders (symbol, status);
CREATE INDEX IF NOT EXISTS idx_orders_account_id        ON orders (account_id);
CREATE INDEX IF NOT EXISTS idx_orders_created_at        ON orders (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_exchange_order_id ON orders (exchange_order_id) WHERE exchange_order_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_orders_trace_id          ON orders (trace_id) WHERE trace_id IS NOT NULL;

-- ── order_events ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS order_events (
    id               BIGSERIAL        PRIMARY KEY,
    order_id         BIGINT           NOT NULL REFERENCES orders(id),
    client_order_id  UUID             NOT NULL,
    event_type       order_event_type NOT NULL,
    filled_qty       NUMERIC(20,8),
    price            NUMERIC(20,8),
    commission       NUMERIC(20,8),
    commission_asset TEXT,
    event_time       TIMESTAMPTZ      NOT NULL,
    recv_time        TIMESTAMPTZ      NOT NULL DEFAULT now(),
    raw              JSONB
);
CREATE INDEX IF NOT EXISTS idx_order_events_order_id   ON order_events (order_id);
CREATE INDEX IF NOT EXISTS idx_order_events_event_time ON order_events (event_time DESC);

-- ── trades ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS trades (
    id               BIGSERIAL,
    trade_id         BIGINT         NOT NULL,
    order_id         BIGINT         NOT NULL REFERENCES orders(id),
    client_order_id  UUID           NOT NULL,
    account_id       TEXT           NOT NULL,
    symbol           TEXT           NOT NULL,
    side             order_side     NOT NULL,
    qty              NUMERIC(20,8)  NOT NULL,
    price            NUMERIC(20,8)  NOT NULL,
    quote_qty        NUMERIC(20,8)  NOT NULL,
    commission       NUMERIC(20,8)  NOT NULL DEFAULT 0,
    commission_asset TEXT,
    realized_pnl     NUMERIC(20,8),
    is_maker         BOOLEAN        NOT NULL DEFAULT FALSE,
    trade_time       TIMESTAMPTZ    NOT NULL,
    recv_time        TIMESTAMPTZ    NOT NULL DEFAULT now(),
    CONSTRAINT uq_trades_id_symbol UNIQUE (trade_id, symbol, trade_time)
);
ALTER TABLE trades ADD PRIMARY KEY (id, trade_time);
CREATE INDEX IF NOT EXISTS idx_trades_order_id     ON trades (order_id);
CREATE INDEX IF NOT EXISTS idx_trades_account_symbol ON trades (account_id, symbol);
CREATE INDEX IF NOT EXISTS idx_trades_trade_time   ON trades (trade_time DESC);
SELECT create_hypertable('trades','trade_time', if_not_exists => TRUE, migrate_data => TRUE);

-- ── positions ────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS positions (
    id                BIGSERIAL      PRIMARY KEY,
    account_id        TEXT           NOT NULL,
    symbol            TEXT           NOT NULL,
    side              position_side  NOT NULL DEFAULT 'BOTH',
    qty               NUMERIC(20,8)  NOT NULL DEFAULT 0,
    entry_price       NUMERIC(20,8),
    unrealized_pnl    NUMERIC(20,8)  NOT NULL DEFAULT 0,
    realized_pnl      NUMERIC(20,8)  NOT NULL DEFAULT 0,
    leverage          INT            NOT NULL DEFAULT 1,
    margin_type       TEXT           NOT NULL DEFAULT 'isolated',
    liquidation_price NUMERIC(20,8),
    snapshot_time     TIMESTAMPTZ    NOT NULL DEFAULT now(),
    CONSTRAINT uq_positions_account_symbol_side UNIQUE (account_id, symbol, side)
);
CREATE INDEX IF NOT EXISTS idx_positions_account_id ON positions (account_id);
CREATE INDEX IF NOT EXISTS idx_positions_symbol     ON positions (symbol);

-- ── decision_logs ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS decision_logs (
    id               BIGSERIAL,
    trace_id         UUID,
    account_id       TEXT             NOT NULL,
    symbol           TEXT             NOT NULL,
    direction        signal_direction NOT NULL,
    action           decision_action  NOT NULL,
    block_reason     TEXT,
    confidence       FLOAT,
    model_version    TEXT,
    feature_version  TEXT,
    strategy_version TEXT,
    entry_price      NUMERIC(20,8),
    qty              NUMERIC(20,8),
    tp_price         NUMERIC(20,8),
    sl_price         NUMERIC(20,8),
    features         JSONB,
    decided_at       TIMESTAMPTZ      NOT NULL DEFAULT now()
);
ALTER TABLE decision_logs ADD PRIMARY KEY (id, decided_at);
CREATE INDEX IF NOT EXISTS idx_decision_logs_symbol     ON decision_logs (symbol);
CREATE INDEX IF NOT EXISTS idx_decision_logs_decided_at ON decision_logs (decided_at DESC);
CREATE INDEX IF NOT EXISTS idx_decision_logs_trace_id   ON decision_logs (trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_decision_logs_action     ON decision_logs (action, decided_at DESC);
SELECT create_hypertable('decision_logs','decided_at', if_not_exists => TRUE, migrate_data => TRUE);

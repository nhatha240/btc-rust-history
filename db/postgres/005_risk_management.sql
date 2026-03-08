-- 005_risk_management.sql
-- Risk management tables: account_risk_limits, symbol_risk_limits, circuit_breakers

CREATE SCHEMA IF NOT EXISTS risk;

-- Account risk limits (per account)
CREATE TABLE IF NOT EXISTS risk.account_risk_limits (
    limit_id             SERIAL PRIMARY KEY,
    account_id           VARCHAR(50) NOT NULL,
    max_position_size    NUMERIC(20, 8),           -- Max position size in base asset
    max_notional         NUMERIC(20, 8),           -- Max order notional in quote asset
    max_leverage         NUMERIC(10, 2) DEFAULT 1, -- Max leverage (1 = no leverage)
    max_daily_loss       NUMERIC(20, 8),           -- Max daily loss in quote asset
    max_positions_count  INTEGER,                   -- Max concurrent positions
    max_order_rate       INTEGER,                   -- Max orders per minute
    is_active            BOOLEAN DEFAULT true,
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    updated_at           TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(account_id)
);

CREATE INDEX IF NOT EXISTS idx_risk_account_id ON risk.account_risk_limits(account_id);
CREATE INDEX IF NOT EXISTS idx_risk_is_active ON risk.account_risk_limits(is_active);

-- Symbol-specific risk limits
CREATE TABLE IF NOT EXISTS risk.symbol_risk_limits (
    limit_id             SERIAL PRIMARY KEY,
    symbol               VARCHAR(50) NOT NULL,
    max_position_size    NUMERIC(20, 8),           -- Max position size for this symbol
    max_notional         NUMERIC(20, 8),           -- Max order notional for this symbol
    max_leverage         NUMERIC(10, 2),           -- Max leverage for this symbol
    min_order_qty        NUMERIC(20, 8),           -- Minimum order quantity
    max_order_qty        NUMERIC(20, 8),           -- Maximum order quantity
    is_active            BOOLEAN DEFAULT true,
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    updated_at           TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol)
);

CREATE INDEX IF NOT EXISTS idx_risk_symbol ON risk.symbol_risk_limits(symbol);
CREATE INDEX IF NOT EXISTS idx_risk_symbol_active ON risk.symbol_risk_limits(symbol, is_active);

-- Circuit breakers (emergency halts)
CREATE TABLE IF NOT EXISTS risk.circuit_breakers (
    breaker_id           SERIAL PRIMARY KEY,
    name                 VARCHAR(100) NOT NULL,
    condition_type       VARCHAR(50) NOT NULL,      -- e.g., 'price_spike', 'volume_spike', 'drawdown'
    threshold_value      NUMERIC(20, 8) NOT NULL,   -- Threshold value
    symbol               VARCHAR(50),               -- NULL means all symbols
    action               VARCHAR(50) NOT NULL CHECK (action IN ('halt_trading', 'reduce_position', 'notify')),
    duration_seconds     INTEGER DEFAULT 300,       -- How long breaker stays active
    is_active            BOOLEAN DEFAULT true,
    triggered_at         TIMESTAMPTZ,
    expires_at           TIMESTAMPTZ,
    created_by          VARCHAR(50) DEFAULT 'system',
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    updated_at           TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_circuit_breakers_active ON risk.circuit_breakers(is_active, triggered_at);
CREATE INDEX IF NOT EXISTS idx_circuit_breakers_symbol ON risk.circuit_breakers(symbol);

-- Risk events log (audit trail)
CREATE TABLE IF NOT EXISTS risk.risk_events (
    event_id             BIGSERIAL,
    account_id           VARCHAR(50) NOT NULL,
    symbol               VARCHAR(50),
    event_type           VARCHAR(50) NOT NULL,      -- e.g., 'limit_check', 'circuit_breaker', 'kill_switch'
    decision             VARCHAR(20) NOT NULL CHECK (decision IN ('APPROVED', 'REJECTED', 'MODIFIED')),
    reason               TEXT,
    original_order_json  JSONB,
    modified_order_json  JSONB,
    metadata             JSONB DEFAULT '{}',
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (event_id, created_at)
);

-- Create hypertable for risk_events for better time-series queries
SELECT create_hypertable('risk.risk_events', 'created_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_risk_events_account ON risk.risk_events(account_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_risk_events_symbol ON risk.risk_events(symbol, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_risk_events_type ON risk.risk_events(event_type, created_at DESC);
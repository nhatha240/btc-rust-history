-- 004_reference_data.sql
-- Reference data tables: exchanges, symbols, timeframes

CREATE SCHEMA IF NOT EXISTS reference_data;

-- Exchanges table
CREATE TABLE IF NOT EXISTS reference_data.exchanges (
    exchange_id      VARCHAR(50) PRIMARY KEY,
    name             VARCHAR(100) NOT NULL,
    type             VARCHAR(20) NOT NULL CHECK (type IN ('spot', 'futures', 'margin')),
    status           VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'maintenance')),
    timezone         VARCHAR(50) DEFAULT 'UTC',
    api_url          VARCHAR(255),
    ws_url           VARCHAR(255),
    rate_limit_requests_per_minute INTEGER DEFAULT 1200,
    supported_symbols JSONB DEFAULT '[]',
    metadata         JSONB DEFAULT '{}',
    created_at       TIMESTAMPTZ DEFAULT NOW(),
    updated_at       TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_exchanges_type ON reference_data.exchanges(type);
CREATE INDEX IF NOT EXISTS idx_exchanges_status ON reference_data.exchanges(status);

-- Timeframes table
CREATE TABLE IF NOT EXISTS reference_data.timeframes (
    timeframe_id     VARCHAR(10) PRIMARY KEY,
    interval_seconds INTEGER NOT NULL,
    description      VARCHAR(100),
    is_candle        BOOLEAN DEFAULT true,
    created_at       TIMESTAMPTZ DEFAULT NOW()
);

INSERT INTO reference_data.timeframes (timeframe_id, interval_seconds, description) VALUES
    ('1m', 60, '1 minute'),
    ('3m', 180, '3 minutes'),
    ('5m', 300, '5 minutes'),
    ('15m', 900, '15 minutes'),
    ('30m', 1800, '30 minutes'),
    ('1h', 3600, '1 hour'),
    ('2h', 7200, '2 hours'),
    ('4h', 14400, '4 hours'),
    ('6h', 21600, '6 hours'),
    ('8h', 28800, '8 hours'),
    ('12h', 43200, '12 hours'),
    ('1d', 86400, '1 day'),
    ('3d', 259200, '3 days'),
    ('1w', 604800, '1 week'),
    ('1M', 2592000, '1 month')
ON CONFLICT (timeframe_id) DO NOTHING;

-- Symbols table (trading instruments)
CREATE TABLE IF NOT EXISTS reference_data.symbols (
    symbol_id        VARCHAR(50) PRIMARY KEY,
    base_asset       VARCHAR(20) NOT NULL,
    quote_asset      VARCHAR(20) NOT NULL,
    status           VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'delisted', 'suspended')),
    exchange_id      VARCHAR(50) NOT NULL REFERENCES reference_data.exchanges(exchange_id),
    contract_type    VARCHAR(20) DEFAULT 'spot' CHECK (contract_type IN ('spot', 'futures', 'options')),
    lot_size         NUMERIC(20, 8) NOT NULL,
    tick_size        NUMERIC(20, 8) NOT NULL,
    min_notional     NUMERIC(20, 8),
    max_leverage     INTEGER,
    margin_asset     VARCHAR(20),
    created_at       TIMESTAMPTZ DEFAULT NOW(),
    updated_at       TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_symbols_exchange ON reference_data.symbols(exchange_id);
CREATE INDEX IF NOT EXISTS idx_symbols_base_quote ON reference_data.symbols(base_asset, quote_asset);
CREATE INDEX IF NOT EXISTS idx_symbols_status ON reference_data.symbols(status);

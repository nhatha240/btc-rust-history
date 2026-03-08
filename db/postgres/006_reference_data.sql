-- ============================================================================
-- PostgreSQL migration 006 — Reference data (Venues & Instruments)
-- ============================================================================

-- ---------------------------------------------------------------------------
-- ref_venues
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS ref_venues (
    venue_id              SERIAL          PRIMARY KEY,
    venue_code            TEXT            NOT NULL UNIQUE,
    venue_type            TEXT            NOT NULL, -- 'spot', 'perp', 'futures', 'options', 'broker'
    timezone              TEXT            NOT NULL DEFAULT 'UTC',
    status                TEXT            NOT NULL DEFAULT 'ACTIVE', -- 'ACTIVE', 'INACTIVE', 'MAINTENANCE'
    connection_metadata   JSONB,
    created_at            TIMESTAMPTZ     NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ     NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- ref_instruments
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS ref_instruments (
    instrument_id         SERIAL          PRIMARY KEY,
    venue_id              INTEGER         NOT NULL REFERENCES ref_venues(venue_id),
    symbol_native         TEXT            NOT NULL, -- e.g. BTCUSDT
    symbol_canonical      TEXT            NOT NULL, -- e.g. BTC/USDT
    base_asset            TEXT            NOT NULL,
    quote_asset           TEXT            NOT NULL,
    instrument_type       TEXT            NOT NULL, -- 'SPOT', 'PERP', etc.
    tick_size             NUMERIC(28, 10) NOT NULL,
    lot_size              NUMERIC(28, 10) NOT NULL,
    min_notional          NUMERIC(28, 10),
    price_precision       INTEGER         NOT NULL,
    qty_precision         INTEGER         NOT NULL,
    is_active             BOOLEAN         NOT NULL DEFAULT TRUE,
    created_at            TIMESTAMPTZ     NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ     NOT NULL DEFAULT now(),
    CONSTRAINT uq_venue_symbol_native UNIQUE (venue_id, symbol_native)
);

CREATE INDEX IF NOT EXISTS idx_ref_inst_canonical ON ref_instruments (symbol_canonical, instrument_type);

-- ---------------------------------------------------------------------------
-- Seed initial venue
-- ---------------------------------------------------------------------------
INSERT INTO ref_venues (venue_code, venue_type) 
VALUES ('BINANCE', 'perp')
ON CONFLICT (venue_code) DO NOTHING;

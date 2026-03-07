-- ============================================================================
-- PostgreSQL migration 005 — Coins Management
-- ============================================================================

CREATE TABLE IF NOT EXISTS coins (
    id          BIGSERIAL PRIMARY KEY,
    symbol      TEXT NOT NULL UNIQUE,
    base_asset  TEXT NOT NULL,
    quote_asset TEXT NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Index on symbol for fast lookups
CREATE INDEX IF NOT EXISTS idx_coins_symbol ON coins (symbol);
CREATE INDEX IF NOT EXISTS idx_coins_is_active ON coins (is_active);

-- Trigger to auto-update the updated_at column
CREATE OR REPLACE FUNCTION update_coins_modified_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ language 'plpgsql';

DROP TRIGGER IF EXISTS trg_coins_updated_at ON coins;
CREATE TRIGGER trg_coins_updated_at
BEFORE UPDATE ON coins
FOR EACH ROW
EXECUTE FUNCTION update_coins_modified_column();

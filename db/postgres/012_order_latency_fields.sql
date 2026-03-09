-- Migration to add latency tracking fields to orders table
ALTER TABLE orders ADD COLUMN IF NOT EXISTS ack_at TIMESTAMPTZ;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS done_at TIMESTAMPTZ;

-- Add index for strategy_version if not exists
CREATE INDEX IF NOT EXISTS idx_orders_strategy_version ON orders (strategy_version) WHERE strategy_version IS NOT NULL;

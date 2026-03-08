-- ============================================================================
-- PostgreSQL migration 010 — Fix exchange_order_id type
-- ============================================================================

-- Change exchange_order_id from BIGINT to TEXT to support non-numeric IDs
-- and allow more flexibility across different venues.

ALTER TABLE orders ALTER COLUMN exchange_order_id TYPE TEXT;

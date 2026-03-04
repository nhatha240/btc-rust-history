-- ============================================================================
-- Orders queries  (PostgreSQL / sqlx)
-- ============================================================================

-- name: get_order_by_client_id
-- Returns a single order by its idempotency key.
SELECT *
FROM orders
WHERE client_order_id = $1;

-- name: get_order_by_exchange_id
SELECT *
FROM orders
WHERE exchange_order_id = $1;

-- name: list_open_orders
-- All non-terminal orders for an account, newest first.
SELECT *
FROM orders
WHERE account_id = $1
  AND status IN ('NEW', 'PARTIALLY_FILLED')
ORDER BY created_at DESC;

-- name: list_orders_by_symbol
SELECT *
FROM orders
WHERE account_id = $1
  AND symbol    = $2
ORDER BY created_at DESC
LIMIT $3;

-- name: upsert_order
-- Insert or update an order (idempotent by client_order_id).
INSERT INTO orders (
    client_order_id, exchange_order_id, account_id, symbol,
    side, type, tif, qty, price, stop_price,
    status, filled_qty, avg_price,
    reduce_only, trace_id, strategy_version
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
ON CONFLICT (client_order_id) DO UPDATE
    SET exchange_order_id = EXCLUDED.exchange_order_id,
        status            = EXCLUDED.status,
        filled_qty        = EXCLUDED.filled_qty,
        avg_price         = EXCLUDED.avg_price,
        updated_at        = now()
RETURNING *;

-- name: insert_order_event
INSERT INTO order_events (
    order_id, client_order_id, event_type,
    filled_qty, price, commission, commission_asset,
    event_time, raw
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
RETURNING id;

-- name: list_order_events
SELECT *
FROM order_events
WHERE order_id = $1
ORDER BY event_time ASC;

-- ============================================================================
-- Trades queries  (PostgreSQL / sqlx)
-- ============================================================================

-- name: insert_trade
-- Upsert a trade fill; exchange may send duplicate fill events.
INSERT INTO trades (
    trade_id, order_id, client_order_id, account_id, symbol,
    side, qty, price, quote_qty,
    commission, commission_asset, realized_pnl,
    is_maker, trade_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
ON CONFLICT (trade_id, symbol) DO NOTHING
RETURNING id;

-- name: list_trades_by_symbol
SELECT *
FROM trades
WHERE account_id = $1
  AND symbol     = $2
ORDER BY trade_time DESC
LIMIT $3;

-- name: list_trades_in_range
SELECT *
FROM trades
WHERE account_id = $1
  AND trade_time >= $2
  AND trade_time <  $3
ORDER BY trade_time DESC;

-- name: upsert_position
-- Replace the position snapshot for account/symbol/side.
INSERT INTO positions (
    account_id, symbol, side,
    qty, entry_price,
    unrealized_pnl, realized_pnl,
    leverage, margin_type, liquidation_price,
    snapshot_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (account_id, symbol, side) DO UPDATE
    SET qty               = EXCLUDED.qty,
        entry_price       = EXCLUDED.entry_price,
        unrealized_pnl    = EXCLUDED.unrealized_pnl,
        realized_pnl      = EXCLUDED.realized_pnl,
        leverage          = EXCLUDED.leverage,
        liquidation_price = EXCLUDED.liquidation_price,
        snapshot_time     = EXCLUDED.snapshot_time
RETURNING *;

-- name: list_positions
SELECT *
FROM positions
WHERE account_id = $1
ORDER BY symbol, side;

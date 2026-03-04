-- ============================================================================
-- PnL queries  (PostgreSQL / sqlx)
-- ============================================================================

-- name: realized_pnl_by_symbol
-- Total realized PnL per symbol for an account in a time window.
SELECT
    symbol,
    SUM(realized_pnl)  AS total_pnl,
    SUM(quote_qty)     AS total_volume,
    COUNT(*)           AS trade_count
FROM trades
WHERE account_id = $1
  AND trade_time >= $2
  AND trade_time <  $3
  AND realized_pnl IS NOT NULL
GROUP BY symbol
ORDER BY total_pnl DESC;

-- name: daily_pnl
-- Day-by-day realized PnL for an account.
SELECT
    date_trunc('day', trade_time) AS day,
    SUM(realized_pnl)             AS pnl,
    SUM(quote_qty)                AS volume,
    COUNT(*)                      AS trade_count
FROM trades
WHERE account_id = $1
  AND trade_time >= $2
  AND trade_time <  $3
  AND realized_pnl IS NOT NULL
GROUP BY 1
ORDER BY 1;

-- name: cumulative_pnl
-- Running cumulative PnL ordered by trade time (useful for equity curve).
SELECT
    trade_time,
    symbol,
    realized_pnl,
    SUM(realized_pnl) OVER (
        PARTITION BY account_id
        ORDER BY trade_time
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) AS cumulative_pnl
FROM trades
WHERE account_id = $1
  AND trade_time >= $2
  AND trade_time <  $3
  AND realized_pnl IS NOT NULL
ORDER BY trade_time;

-- name: win_rate
-- Win/loss counts and average PnL per symbol.
SELECT
    symbol,
    COUNT(*) FILTER (WHERE realized_pnl > 0)  AS winning_trades,
    COUNT(*) FILTER (WHERE realized_pnl < 0)  AS losing_trades,
    COUNT(*) FILTER (WHERE realized_pnl = 0)  AS breakeven_trades,
    ROUND(AVG(realized_pnl)::NUMERIC, 8)      AS avg_pnl,
    ROUND(MAX(realized_pnl)::NUMERIC, 8)      AS best_trade,
    ROUND(MIN(realized_pnl)::NUMERIC, 8)      AS worst_trade
FROM trades
WHERE account_id = $1
  AND trade_time >= $2
  AND trade_time <  $3
  AND realized_pnl IS NOT NULL
GROUP BY symbol
ORDER BY symbol;

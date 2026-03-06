#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

DATABASE_URL="${DATABASE_URL:-postgres://trader:traderpw@localhost:5432/db_trading}"

run_sql() {
  if command -v psql >/dev/null 2>&1; then
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 "$@"
    return
  fi

  if command -v docker >/dev/null 2>&1; then
    docker compose -f "${REPO_ROOT}/infra/docker/docker-compose.yml" exec -T postgres \
      psql -U trader -d db_trading -v ON_ERROR_STOP=1 "$@"
    return
  fi

  echo "Error: psql not found and docker compose fallback is unavailable." >&2
  exit 1
}

run_sql <<'SQL'
BEGIN;

INSERT INTO orders (
  client_order_id, exchange_order_id, account_id, symbol, side, type, tif, qty, price,
  status, filled_qty, avg_price, reduce_only, trace_id, strategy_version
) VALUES
  ('11111111-1111-1111-1111-111111111111', 1000001, 'paper-main', 'BTCUSDT', 'BUY', 'LIMIT', 'GTC',
   0.01000000, 62000.00, 'FILLED', 0.01000000, 61990.00, FALSE,
   'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', 'demo-v1'),
  ('22222222-2222-2222-2222-222222222222', 1000002, 'paper-main', 'BTCUSDT', 'SELL', 'LIMIT', 'GTC',
   0.01000000, 62800.00, 'FILLED', 0.01000000, 62790.00, TRUE,
   'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', 'demo-v1')
ON CONFLICT (client_order_id) DO UPDATE
  SET updated_at = now();

INSERT INTO order_events (
  order_id, client_order_id, event_type, filled_qty, price, commission, commission_asset, event_time, raw
)
SELECT
  o.id,
  o.client_order_id,
  'FILLED'::order_event_type,
  o.filled_qty,
  o.avg_price,
  0.20,
  'USDT',
  now() - interval '2 hours',
  '{"source":"seed_demo_data.sh"}'::jsonb
FROM orders o
WHERE o.client_order_id IN (
  '11111111-1111-1111-1111-111111111111',
  '22222222-2222-2222-2222-222222222222'
)
ON CONFLICT DO NOTHING;

INSERT INTO trades (
  trade_id, order_id, client_order_id, account_id, symbol, side, qty, price, quote_qty,
  commission, commission_asset, realized_pnl, is_maker, trade_time, fill_id
)
SELECT
  7000001,
  o.id,
  o.client_order_id,
  'paper-main',
  'BTCUSDT',
  'BUY'::order_side,
  0.01000000,
  61990.00,
  619.90,
  0.20,
  'USDT',
  NULL,
  FALSE,
  now() - interval '2 hours',
  'demo-fill-1'
FROM orders o
WHERE o.client_order_id = '11111111-1111-1111-1111-111111111111'
ON CONFLICT DO NOTHING;

INSERT INTO trades (
  trade_id, order_id, client_order_id, account_id, symbol, side, qty, price, quote_qty,
  commission, commission_asset, realized_pnl, is_maker, trade_time, fill_id
)
SELECT
  7000002,
  o.id,
  o.client_order_id,
  'paper-main',
  'BTCUSDT',
  'SELL'::order_side,
  0.01000000,
  62790.00,
  627.90,
  0.20,
  'USDT',
  8.00,
  FALSE,
  now() - interval '1 hour',
  'demo-fill-2'
FROM orders o
WHERE o.client_order_id = '22222222-2222-2222-2222-222222222222'
ON CONFLICT DO NOTHING;

INSERT INTO positions (
  account_id, symbol, side, qty, entry_price, unrealized_pnl, realized_pnl, leverage, margin_type, snapshot_time
) VALUES
  ('paper-main', 'BTCUSDT', 'BOTH', 0.00000000, NULL, 0.00, 8.00, 5, 'isolated', now())
ON CONFLICT (account_id, symbol, side) DO UPDATE
SET
  qty = EXCLUDED.qty,
  entry_price = EXCLUDED.entry_price,
  unrealized_pnl = EXCLUDED.unrealized_pnl,
  realized_pnl = EXCLUDED.realized_pnl,
  leverage = EXCLUDED.leverage,
  margin_type = EXCLUDED.margin_type,
  snapshot_time = EXCLUDED.snapshot_time;

INSERT INTO decision_logs (
  trace_id, account_id, symbol, direction, action, block_reason,
  confidence, model_version, feature_version, strategy_version,
  entry_price, qty, tp_price, sl_price, features, decided_at
) VALUES
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', 'paper-main', 'BTCUSDT', 'LONG', 'ENTER', NULL,
   0.82, 'model-demo', 'feat-v1', 'demo-v1', 62000.00, 0.01000000, 63000.00, 61500.00,
   '{"rsi":48.2,"ema_fast":61980.4,"ema_slow":61920.1}'::jsonb, now() - interval '2 hours'),
  ('bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', 'paper-main', 'BTCUSDT', 'LONG', 'EXIT', NULL,
   0.76, 'model-demo', 'feat-v1', 'demo-v1', 62800.00, 0.01000000, NULL, NULL,
   '{"reason":"tp_hit"}'::jsonb, now() - interval '1 hour')
ON CONFLICT DO NOTHING;

COMMIT;
SQL

echo "Demo data seeded successfully."

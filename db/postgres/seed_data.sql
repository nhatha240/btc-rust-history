-- Sample data for verification

-- 1. Insert an order
INSERT INTO orders (
    client_order_id, account_id, symbol, side, type, status, qty, price, created_at
) VALUES (
    '550e8400-e29b-41d4-a716-446655440000', 'test_account', 'BTCUSDT', 'BUY', 'LIMIT', 'FILLED', 0.1, 60000.0, now()
) ON CONFLICT (client_order_id) DO NOTHING;

-- 2. Insert corresponding trade
INSERT INTO trades (id, trade_id, order_id, client_order_id, account_id, symbol, side, qty, price, quote_qty, commission, commission_asset, is_maker, trade_time, fill_id)
VALUES (DEFAULT, 1001, (SELECT id FROM orders WHERE client_order_id = '550e8400-e29b-41d4-a716-446655440000'), '550e8400-e29b-41d4-a716-446655440000', 'test_account', 'BTCUSDT', 'BUY', 0.05, 62500.50, 3125.025, 0.3125, 'USDT', false, NOW(), 'seed-fill-001')
ON CONFLICT (trade_id, symbol, trade_time) DO NOTHING;

-- 3. Update position
INSERT INTO positions (
    account_id, symbol, qty, entry_price, snapshot_time
) VALUES (
    'test_account', 'BTCUSDT', 0.1, 60000.0, now()
) ON CONFLICT (account_id, symbol, side) DO UPDATE SET 
    qty = EXCLUDED.qty,
    entry_price = EXCLUDED.entry_price,
    snapshot_time = EXCLUDED.snapshot_time;

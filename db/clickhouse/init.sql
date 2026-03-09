-- ============================================================================
-- ClickHouse bootstrap schema
-- Database : db_trading
-- Run via  : clickhouse-client --multiquery < init.sql
--            OR POST to http://clickhouse:8123/?database=db_trading
-- ============================================================================

CREATE DATABASE IF NOT EXISTS db_trading;

-- ---------------------------------------------------------------------------
-- 1. Raw 1-minute OHLCV candles  (source table for all MV aggregations)
-- ---------------------------------------------------------------------------
-- Bảng nến 1m chuẩn hóa
CREATE TABLE IF NOT EXISTS db_trading.candles_1m_final
(
    symbol                       LowCardinality(String),
    open_time                    DateTime64(3),
    open                         Float64,
    high                         Float64,
    low                          Float64,
    close                        Float64,
    volume                       Float64,
    close_time                   DateTime64(3),
    quote_asset_volume           Float64,
    number_of_trades             UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64,
    ingested_at                  DateTime64(3) DEFAULT now64(3)
)
ENGINE = MergeTree
PARTITION BY (toYYYYMM(open_time), symbol)
ORDER BY (symbol, open_time);

-- ---------------------------------------------------------------------------
-- 1a. Raw Trade Events (tick-level or source-level prints)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS db_trading.md_trades
(
    symbol          LowCardinality(String),
    event_time      DateTime64(3),
    trade_id        UInt64, 
    price           Float64,
    qty             Float64,
    quote_qty       Float64,
    is_buyer_maker  UInt8,
    is_best_match   UInt8,
    ingested_at     DateTime64(3) DEFAULT now64(3)
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(event_time)
ORDER BY (symbol, event_time, trade_id);

-- Bảng dữ liệu phái sinh (Tâm lý & Dòng tiền)
-- Uses ReplacingMergeTree to coalesce multiple events at the same timestamp
-- (open interest, mark price, liquidation) into a single row.
CREATE TABLE IF NOT EXISTS db_trading.futures_context_1m
(
    symbol          LowCardinality(String),
    ts              DateTime64(3),
    open_interest   Float64, -- Dòng tiền đang mở
    funding_rate    Float64, -- Phí duy trì (đo lường độ quá nhiệt)
    liq_buy_vol     Float64, -- Volume thanh khoản lệnh Long
    liq_sell_vol    Float64  -- Volume thanh khoản lệnh Short
)
ENGINE = ReplacingMergeTree(ts)
PARTITION BY (toYYYYMM(ts), symbol)
ORDER BY (symbol, ts);


CREATE TABLE IF NOT EXISTS db_trading.orderbook_l2_updates
(
    venue               LowCardinality(String),
    symbol              LowCardinality(String),

    exchange_ts         DateTime64(6, 'UTC') CODEC(DoubleDelta, LZ4),
    receive_ts          DateTime64(6, 'UTC') CODEC(DoubleDelta, LZ4),

    first_update_id     UInt64,
    final_update_id     UInt64,
    prev_final_update_id UInt64 DEFAULT 0,

    side                Enum8('BID' = 1, 'ASK' = 2),

    price               Decimal64(8),
    quantity            Decimal64(8),

    is_snapshot         UInt8 DEFAULT 0
)
ENGINE = MergeTree
PARTITION BY toYYYYMMDD(exchange_ts)
ORDER BY (venue, symbol, exchange_ts, final_update_id, side, price)
SETTINGS index_granularity = 8192;

-- ---------------------------------------------------------------------------
-- 2. Aggregated candle target tables  (populated by materialized views below)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS db_trading.candles_15m
(
    open_time                    DateTime64(3),
    symbol                       LowCardinality(String),
    open                         Float64,
    high                         Float64,
    low                          Float64,
    close                        Float64,
    volume                       Float64,
    close_time                   DateTime64(3),
    quote_asset_volume           Float64,
    number_of_trades             UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
)
ENGINE = AggregatingMergeTree
PARTITION BY (toYYYYMM(open_time), symbol)
ORDER BY (symbol, open_time);

CREATE TABLE IF NOT EXISTS db_trading.candles_1h
(
    open_time                    DateTime64(3),
    symbol                       LowCardinality(String),
    open                         Float64,
    high                         Float64,
    low                          Float64,
    close                        Float64,
    volume                       Float64,
    close_time                   DateTime64(3),
    quote_asset_volume           Float64,
    number_of_trades             UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
)
ENGINE = AggregatingMergeTree
PARTITION BY (toYYYYMM(open_time), symbol)
ORDER BY (symbol, open_time);

CREATE TABLE IF NOT EXISTS db_trading.candles_4h
(
    open_time                    DateTime64(3),
    symbol                       LowCardinality(String),
    open                         Float64,
    high                         Float64,
    low                          Float64,
    close                        Float64,
    volume                       Float64,
    close_time                   DateTime64(3),
    quote_asset_volume           Float64,
    number_of_trades             UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
)
ENGINE = AggregatingMergeTree
PARTITION BY (toYYYYMM(open_time), symbol)
ORDER BY (symbol, open_time);

CREATE TABLE IF NOT EXISTS db_trading.candles_1d
(
    open_time                    DateTime64(3),
    symbol                       LowCardinality(String),
    open                         Float64,
    high                         Float64,
    low                          Float64,
    close                        Float64,
    volume                       Float64,
    close_time                   DateTime64(3),
    quote_asset_volume           Float64,
    number_of_trades             UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
)
ENGINE = AggregatingMergeTree
PARTITION BY (toYYYYMM(open_time), symbol)
ORDER BY (symbol, open_time);

-- ---------------------------------------------------------------------------
-- 3. Materialized views:  candles_1m_final → aggregated tables
--    FIX: each MV uses the correct interval function.
--    FIX: GROUP BY references the transformed alias (valid in ClickHouse).
-- ---------------------------------------------------------------------------
CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_15m
TO db_trading.candles_15m
AS
SELECT
    toStartOfInterval(open_time, INTERVAL 15 MINUTE) AS open_time,
    symbol,
    argMin(open,  open_time) AS open,
    max(high)                AS high,
    min(low)                 AS low,
    argMax(close, open_time) AS close,
    sum(volume)              AS volume,
    max(close_time)          AS close_time,
    sum(quote_asset_volume)  AS quote_asset_volume,
    sum(number_of_trades)    AS number_of_trades,
    sum(taker_buy_base_asset_volume)  AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_1h
TO db_trading.candles_1h
AS
SELECT
    toStartOfHour(open_time) AS open_time,
    symbol,
    argMin(open,  open_time) AS open,
    max(high)                AS high,
    min(low)                 AS low,
    argMax(close, open_time) AS close,
    sum(volume)              AS volume,
    max(close_time)          AS close_time,
    sum(quote_asset_volume)  AS quote_asset_volume,
    sum(number_of_trades)    AS number_of_trades,
    sum(taker_buy_base_asset_volume)  AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

-- FIX: was toStartOfHour — must be INTERVAL 4 HOUR to produce 4h bars
CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_4h
TO db_trading.candles_4h
AS
SELECT
    toStartOfInterval(open_time, INTERVAL 4 HOUR) AS open_time,
    symbol,
    argMin(open,  open_time) AS open,
    max(high)                AS high,
    min(low)                 AS low,
    argMax(close, open_time) AS close,
    sum(volume)              AS volume,
    max(close_time)          AS close_time,
    sum(quote_asset_volume)  AS quote_asset_volume,
    sum(number_of_trades)    AS number_of_trades,
    sum(taker_buy_base_asset_volume)  AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_1d
TO db_trading.candles_1d
AS
SELECT
    toStartOfDay(open_time) AS open_time,
    symbol,
    argMin(open,  open_time) AS open,
    max(high)                AS high,
    min(low)                 AS low,
    argMax(close, open_time) AS close,
    sum(volume)              AS volume,
    max(close_time)          AS close_time,
    sum(quote_asset_volume)  AS quote_asset_volume,
    sum(number_of_trades)    AS number_of_trades,
    sum(taker_buy_base_asset_volume)  AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

-- ---------------------------------------------------------------------------
-- 4. Feature state  (latest computed indicators per symbol per bar)
--    FIX: upgraded ts to DateTime64(3) for ms precision consistency.
--    FIX: added vwap column (vwap.rs indicator exists in feature_engine).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS db_trading.feature_state
(
    ts          DateTime64(3),
    symbol      LowCardinality(String),
    ema_fast    Float64,
    ema_slow    Float64,
    rsi         Float64,
    macd        Float64,
    macd_signal Float64,
    macd_hist   Float64,
    vwap        Float64,
    adx             Float64, -- Độ mạnh xu hướng
    atr             Float64, -- Biến động (dùng cho Stoploss/Sizing)
    
    -- Phân loại trạng thái thị trường (Market Regime)
    -- 1: TREND_UP, 2: TREND_DOWN, 3: RANGE, 4: VOLATILE_PANIC
    regime          UInt8, 
    
    -- Phân tích dòng tiền
    vol_zscore      Float64, -- Phát hiện Volume Spike
    oi_change_pct   Float64  -- % thay đổi Open Interest
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(ts)
ORDER BY (symbol, ts)
SETTINGS index_granularity = 8192;

-- ---------------------------------------------------------------------------
-- 5. Trading signals
--    FIX: upgraded ts to DateTime64(3).
--    Added confidence + model_version for traceability.
-- ---------------------------------------------------------------------------
-- Lưu tín hiệu từ các Detector (Strategy Output)
CREATE TABLE IF NOT EXISTS db_trading.signals
(
    ts            DateTime64(3),
    symbol        LowCardinality(String),
    side          Enum8('LONG' = 1, 'SHORT' = -1, 'EXIT' = 0),
    strategy_name LowCardinality(String), -- Trend/Reversion/Breakout
    price         Float64,
    stop_loss     Float64,
    take_profit   Float64,
    confidence    Float32, -- Điểm tin cậy (0-1) [cite: 130]
    reason        String   -- Giải thích logic (VD: RSI Divergence + Volume Spike)
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(ts)
ORDER BY (symbol, ts);

-- Lưu vết giao dịch thực tế (Trade Log) để Backtest/Performance Audit
CREATE TABLE IF NOT EXISTS db_trading.trade
(
    trade_id          UUID DEFAULT generateUUIDv4(),
    trace_id          UUID,              -- Link tới decision_logs ở TimescaleDB
    symbol            LowCardinality(String),
    strategy_version  LowCardinality(String),
    
    -- Chi tiết thực thi
    side              Enum8('LONG' = 1, 'SHORT' = -1),
    entry_ts          DateTime64(3),
    exit_ts           DateTime64(3),
    entry_price       Float64,
    exit_price        Float64,
    expected_price    Float64,           -- Giá lúc Signal bắn ra
    
    -- Quản trị rủi ro & Hiệu suất [cite: 311, 375]
    quantity          Float64,
    fee               Float64,
    slippage_bps      Float32,           -- (Actual_Price - Expected_Price) / Expected_Price * 10000
    pnl_net           Float64,           -- Lợi nhuận sau phí
    mae_pct           Float32,           -- Mức sụt giảm sâu nhất trong lệnh
    mfe_pct           Float32,           -- Mức tăng cao nhất trong lệnh
    
    -- Metadata để phân tích Regime [cite: 10, 371]
    regime_at_entry   UInt8,             -- 1: TrendUp, 2: Range, etc.
    entry_reason      LowCardinality(String), -- VD: 'RSI_REVERSION', 'EMA_CROSS'
    exit_reason       Enum8('TP'=1, 'SL'=2, 'SIGNAL_FLIP'=3, 'TIME_EXIT'=4)
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(entry_ts)
ORDER BY (strategy_version, symbol, entry_ts);

-- ---------------------------------------------------------------------------
-- 6. Market-cap snapshots
--    FIX: upgraded ts to DateTime64(3).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS db_trading.mc_snapshot
(
    ts         DateTime64(3),
    symbol     LowCardinality(String),
    marketcap  Float64,
    dominance  Float64
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(ts)
ORDER BY (symbol, ts);

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
CREATE TABLE IF NOT EXISTS db_trading.candles_1m_final
(
    symbol                       LowCardinality(String),
    open_time                    DateTime64(3),          -- exchange open  (ms)
    open                         Float64,
    high                         Float64,
    low                          Float64,
    close                        Float64,
    volume                       Float64,
    close_time                   DateTime64(3),          -- exchange close (ms)
    quote_asset_volume           Float64,
    number_of_trades             UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64,
    ingested_at                  DateTime64(3) DEFAULT now64(3)
)
ENGINE = MergeTree
PARTITION BY (toYYYYMM(open_time), symbol)
ORDER BY (symbol, open_time)
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
    vwap        Float64
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
CREATE TABLE IF NOT EXISTS db_trading.signals
(
    ts            DateTime64(3),
    symbol        LowCardinality(String),
    side          Enum8('LONG' = 1, 'SHORT' = -1),
    reason        String,
    price         Float64,
    confidence    Float32  DEFAULT 0,
    model_version LowCardinality(String) DEFAULT ''
)
ENGINE = ReplacingMergeTree(ts)
PARTITION BY toYYYYMM(ts)
ORDER BY (symbol, ts);

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

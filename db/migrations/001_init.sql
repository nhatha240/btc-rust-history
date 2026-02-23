-- ============================================================================
-- ClickHouse bootstrap schema (NO TTL, FULL HISTORY) - no exchange column
-- ============================================================================

CREATE DATABASE IF NOT EXISTS db_trading;
USE db_trading;

-- 1m raw (epoch-ms)
CREATE TABLE IF NOT EXISTS db_trading.candles_1m_final
(
    symbol                              LowCardinality(String),
    open_time                           DateTime64(3),   -- ms
    open                                Float64,
    high                                Float64,
    low                                 Float64,
    close                               Float64,
    volume                              Float64,
    close_time                          DateTime64(3),   -- ms
    quote_asset_volume                  Float64,
    number_of_trades                    UInt64,
    taker_buy_base_asset_volume         Float64,
    taker_buy_quote_asset_volume        Float64
    )
    ENGINE = MergeTree
    PARTITION BY (toYYYYMM(open_time), symbol)
    ORDER BY (symbol, open_time)
    SETTINGS index_granularity = 8192;

-- Feature state (unchanged)
CREATE TABLE IF NOT EXISTS db_trading.feature_state
(
    ts          DateTime,
    symbol      LowCardinality(String),
    ema_fast    Float64,
    ema_slow    Float64,
    rsi         Float64,
    macd        Float64,
    macd_signal Float64,
    macd_hist   Float64
    )
    ENGINE = MergeTree
    PARTITION BY toYYYYMM(ts)
    ORDER BY (symbol, ts)
    SETTINGS index_granularity = 8192;

-- Signals (unchanged)
CREATE TABLE IF NOT EXISTS db_trading.signals
(
    ts          DateTime,
    symbol      LowCardinality(String),
    side        Enum8('LONG' = 1, 'SHORT' = -1),
    reason      String,
    price       Float64
    )
    ENGINE = ReplacingMergeTree(ts)
    PARTITION BY toYYYYMM(ts)
    ORDER BY (symbol, ts);

-- Market cap snapshot (unchanged)
CREATE TABLE IF NOT EXISTS db_trading.mc_snapshot
(
    ts          DateTime,
    symbol      LowCardinality(String),
    marketcap   Float64,
    dominance   Float64
    )
    ENGINE = MergeTree
    PARTITION BY toYYYYMM(ts)
    ORDER BY (symbol, ts);

CREATE TABLE IF NOT EXISTS db_trading.candles_15m
(
    open_time   DateTime64(3),
    symbol      LowCardinality(String),
    open        Decimal64(8),
    high        Decimal64(8),
    low         Decimal64(8),
    close       Decimal64(8),
    volume                      Float64,
    close_time  DateTime64(3),
    quote_asset_volume          Float64,
    number_of_trades            UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
    )
    ENGINE = MergeTree
    PARTITION BY (toYYYYMM(open_time), symbol)
    ORDER BY (symbol, open_time);

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_15m
TO db_trading.candles_15m
AS
SELECT
    toStartOfInterval(open_time, INTERVAL 15 minute) AS open_time,
    symbol,
    argMin(open, open_time)   AS open,
  max(high)                 AS high,
  min(low)                  AS low,
  argMax(close, open_time)  AS close,
  sum(volume)               AS volume,
  max(close_time)           AS close_time,
  sum(quote_asset_volume)   AS quote_asset_volume,
  sum(number_of_trades)     AS number_of_trades,
  sum(taker_buy_base_asset_volume) AS taker_buy_base_asset_volume,
  sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

CREATE TABLE IF NOT EXISTS db_trading.candles_1h
(
    open_time   DateTime64(3),
    symbol      LowCardinality(String),
    open        Decimal64(8),
    high        Decimal64(8),
    low         Decimal64(8),
    close       Decimal64(8),
    volume                      Float64,
    close_time  DateTime64(3),
    quote_asset_volume          Float64,
    number_of_trades            UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
    )
    ENGINE = MergeTree
    PARTITION BY (toYYYYMM(open_time), symbol)
    ORDER BY (symbol, open_time);

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_1h
TO db_trading.candles_1h
AS
SELECT
    toStartOfHour(open_time) AS open_time,
    symbol,
    argMin(open, open_time)   AS open,
  max(high)                 AS high,
  min(low)                  AS low,
  argMax(close, open_time)  AS close,
  sum(volume)               AS volume,
  max(close_time)           as close_time,
  sum(quote_asset_volume)   AS quote_asset_volume,
  sum(number_of_trades)     AS number_of_trades,
  sum(taker_buy_base_asset_volume) AS taker_buy_base_asset_volume,
  sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

CREATE TABLE IF NOT EXISTS db_trading.candles_4h
(
    open_time   DateTime64(3),
    symbol      LowCardinality(String),
    open        Decimal64(8),
    high        Decimal64(8),
    low         Decimal64(8),
    close       Decimal64(8),
    volume                      Float64,
    close_time  DateTime64(3),
    quote_asset_volume          Float64,
    number_of_trades            UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
    )
    ENGINE = MergeTree
    PARTITION BY (toYYYYMM(open_time), symbol)
    ORDER BY (symbol, open_time);

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_4h
TO db_trading.candles_4h
AS
SELECT
    toStartOfHour(open_time) AS open_time,
    symbol,
    argMin(open, open_time)   AS open,
  max(high)                 AS high,
  min(low)                  AS low,
  argMax(close, open_time)  AS close,
  sum(volume)               AS volume,
  max(close_time)           as close_time,
  sum(quote_asset_volume)   AS quote_asset_volume,
  sum(number_of_trades)     AS number_of_trades,
  sum(taker_buy_base_asset_volume) AS taker_buy_base_asset_volume,
  sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;
CREATE TABLE IF NOT EXISTS db_trading.candles_1d
(
    open_time   DateTime64(3),
    symbol      LowCardinality(String),
    open        Decimal64(8),
    high        Decimal64(8),
    low         Decimal64(8),
    close       Decimal64(8),
    volume                      Float64,
    close_time  DateTime64(3),
    quote_asset_volume          Float64,
    number_of_trades            UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
    )
    ENGINE = MergeTree
    PARTITION BY (toYYYYMM(open_time), symbol)
    ORDER BY (symbol, open_time);

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_1d
TO db_trading.candles_1d
AS
SELECT
    toStartOfDay(open_time) AS open_time,
    symbol,
    argMin(open, open_time)   AS open,
  max(high)                 AS high,
  min(low)                  AS low,
  argMax(close, open_time)  AS close,
  sum(volume)               AS volume,
  max(close_time)           AS close_time,
  sum(quote_asset_volume)   AS quote_asset_volume,
  sum(number_of_trades)     AS number_of_trades,
  sum(taker_buy_base_asset_volume) AS taker_buy_base_asset_volume,
  sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

CREATE TABLE IF NOT EXISTS db_trading.candles_1d
(
    open_time   DateTime64(3),
    symbol      LowCardinality(String),
    open        Decimal64(8),
    high        Decimal64(8),
    low         Decimal64(8),
    close       Decimal64(8),
    volume                      Float64,
    close_time  DateTime64(3),
    quote_asset_volume          Float64,
    number_of_trades            UInt64,
    taker_buy_base_asset_volume  Float64,
    taker_buy_quote_asset_volume Float64
    )
    ENGINE = MergeTree
    PARTITION BY (toYYYYMM(open_time), symbol)
    ORDER BY (symbol, open_time);

CREATE MATERIALIZED VIEW IF NOT EXISTS db_trading.mv_candles_1d
TO db_trading.candles_1d
AS
SELECT
    toStartOfDay(open_time) AS open_time,
    symbol,
    argMin(open, open_time)   AS open,
  max(high)                 AS high,
  min(low)                  AS low,
  argMax(close, open_time)  AS close,
  sum(volume)               AS volume,
  max(close_time)           AS close_time,
  sum(quote_asset_volume)   AS quote_asset_volume,
  sum(number_of_trades)     AS number_of_trades,
  sum(taker_buy_base_asset_volume) AS taker_buy_base_asset_volume,
  sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY symbol, open_time;

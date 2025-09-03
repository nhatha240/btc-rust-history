-- ============================================================================
-- ClickHouse bootstrap schema (NO TTL, FULL HISTORY) - no exchange column
-- ============================================================================

CREATE DATABASE IF NOT EXISTS db_trading;

-- 1m raw (epoch-ms)
CREATE TABLE IF NOT EXISTS db_trading.candles_1m_final
(
    symbol                              LowCardinality(String),
    open_time                           Int64,   -- ms
    open                                Float64,
    high                                Float64,
    low                                 Float64,
    close                               Float64,
    volume                              Float64,
    close_time                          Int64,   -- ms
    quote_asset_volume                  Float64,
    number_of_trades                    UInt64,
    taker_buy_base_asset_volume         Float64,
    taker_buy_quote_asset_volume        Float64
    )
    ENGINE = MergeTree
    PARTITION BY toYYYYMM(toDateTime64(open_time/1000, 3, 'UTC'))
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

-- Aggregated frames schema (DateTime64 buckets, keep interval tag)
DROP TABLE IF EXISTS db_trading.candles_1h;
CREATE TABLE db_trading.candles_1h
(
    symbol                              LowCardinality(String),
    interval                            LowCardinality(String), -- '1h'
    open_time                           DateTime64(3, 'UTC'),
    close_time                          DateTime64(3, 'UTC'),
    open                                Float64,
    high                                Float64,
    low                                 Float64,
    close                               Float64,
    volume                              Float64,
    quote_asset_volume                  Float64,
    number_of_trades                    UInt64,
    taker_buy_base_asset_volume         Float64,
    taker_buy_quote_asset_volume        Float64
)
    ENGINE = ReplacingMergeTree
PARTITION BY toYYYYMM(open_time)
ORDER BY (symbol, open_time)
SETTINGS index_granularity = 8192;

DROP TABLE IF EXISTS db_trading.candles_4h;
CREATE TABLE db_trading.candles_4h AS db_trading.candles_1h;

DROP TABLE IF EXISTS db_trading.candles_1d;
CREATE TABLE db_trading.candles_1d AS db_trading.candles_1h;

DROP TABLE IF EXISTS db_trading.candles_1w;
CREATE TABLE db_trading.candles_1w AS db_trading.candles_1h;

-- ===================== MATERIALIZED VIEWS =====================

DROP VIEW IF EXISTS db_trading.mv_candles_1h;
CREATE MATERIALIZED VIEW db_trading.mv_candles_1h
TO db_trading.candles_1h
AS
WITH
    toDateTime64(open_time/1000,  3, 'UTC') AS open_dt,
    toDateTime64(close_time/1000, 3, 'UTC') AS close_dt
SELECT
    symbol,
    '1h'                                                    AS interval,
    toStartOfInterval(open_dt,  INTERVAL 1 HOUR)            AS open_time,
    max(close_dt)                                           AS close_time,
    argMin(open,  open_dt)                                  AS open,
    max(high)                                               AS high,
    min(low)                                                AS low,
    argMax(close, open_dt)                                  AS close,
    sum(volume)                                             AS volume,
    sum(quote_asset_volume)                                 AS quote_asset_volume,
    sum(number_of_trades)                                   AS number_of_trades,
    sum(taker_buy_base_asset_volume)                        AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume)                       AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY
    symbol,
    toStartOfInterval(open_dt, INTERVAL 1 HOUR);

DROP VIEW IF EXISTS db_trading.mv_candles_4h;
CREATE MATERIALIZED VIEW db_trading.mv_candles_4h
TO db_trading.candles_4h
AS
WITH
    toDateTime64(open_time/1000,  3, 'UTC') AS open_dt,
    toDateTime64(close_time/1000, 3, 'UTC') AS close_dt
SELECT
    symbol,
    '4h'                                                    AS interval,
    toStartOfInterval(open_dt,  INTERVAL 4 HOUR)            AS open_time,
    max(close_dt)                                           AS close_time,
    argMin(open,  open_dt)                                  AS open,
    max(high)                                               AS high,
    min(low)                                                AS low,
    argMax(close, open_dt)                                  AS close,
    sum(volume)                                             AS volume,
    sum(quote_asset_volume)                                 AS quote_asset_volume,
    sum(number_of_trades)                                   AS number_of_trades,
    sum(taker_buy_base_asset_volume)                        AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume)                       AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY
    symbol,
    toStartOfInterval(open_dt, INTERVAL 4 HOUR);

DROP VIEW IF EXISTS db_trading.mv_candles_1d;
CREATE MATERIALIZED VIEW db_trading.mv_candles_1d
TO db_trading.candles_1d
AS
WITH
    toDateTime64(open_time/1000,  3, 'UTC') AS open_dt,
    toDateTime64(close_time/1000, 3, 'UTC') AS close_dt
SELECT
    symbol,
    '1d'                                                    AS interval,
    toStartOfInterval(open_dt,  INTERVAL 1 DAY)             AS open_time,
    max(close_dt)                                           AS close_time,
    argMin(open,  open_dt)                                  AS open,
    max(high)                                               AS high,
    min(low)                                                AS low,
    argMax(close, open_dt)                                  AS close,
    sum(volume)                                             AS volume,
    sum(quote_asset_volume)                                 AS quote_asset_volume,
    sum(number_of_trades)                                   AS number_of_trades,
    sum(taker_buy_base_asset_volume)                        AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume)                       AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY
    symbol,
    toStartOfInterval(open_dt, INTERVAL 1 DAY);

DROP VIEW IF EXISTS db_trading.mv_candles_1w;
CREATE MATERIALIZED VIEW db_trading.mv_candles_1w
TO db_trading.candles_1w
AS
WITH
    toDateTime64(open_time/1000,  3, 'UTC') AS open_dt,
    toDateTime64(close_time/1000, 3, 'UTC') AS close_dt
SELECT
    symbol,
    '1w'                                                    AS interval,
    toStartOfInterval(open_dt,  INTERVAL 1 WEEK)            AS open_time,  -- Monday-based
    max(close_dt)                                           AS close_time,
    argMin(open,  open_dt)                                  AS open,
    max(high)                                               AS high,
    min(low)                                                AS low,
    argMax(close, open_dt)                                  AS close,
    sum(volume)                                             AS volume,
    sum(quote_asset_volume)                                 AS quote_asset_volume,
    sum(number_of_trades)                                   AS number_of_trades,
    sum(taker_buy_base_asset_volume)                        AS taker_buy_base_asset_volume,
    sum(taker_buy_quote_asset_volume)                       AS taker_buy_quote_asset_volume
FROM db_trading.candles_1m_final
GROUP BY
    symbol,
    toStartOfInterval(open_dt, INTERVAL 1 WEEK);


CREATE TABLE IF NOT EXISTS db_trading.instruments_all
(
  `exchange` LowCardinality(String),
  `market` LowCardinality(String),             -- 'spot' | 'um' | 'cm'
  `symbol` String,
  `base_asset` LowCardinality(Nullable(String)),
  `quote_asset` LowCardinality(Nullable(String)),
  `status` LowCardinality(Nullable(String)),
  `contract_type` LowCardinality(Nullable(String)),
  `delivery_date_ms` Nullable(Int64),
  `onboard_date_ms` Nullable(Int64),
  `margin_asset` LowCardinality(Nullable(String)),
  `price_precision` Nullable(UInt32),
  `quantity_precision` Nullable(UInt32),
  `permissions_json` Nullable(String),
  `filters_json` Nullable(String),
  `version` UInt64
)
ENGINE = ReplacingMergeTree(version)
ORDER BY (exchange, market, symbol)
SETTINGS index_granularity = 8192;

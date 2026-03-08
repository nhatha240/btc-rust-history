-- 006_backtest_metadata.sql
-- Backtesting tables: strategies, backtests, backtest_trades, optimization_jobs

CREATE SCHEMA IF NOT EXISTS backtest;

-- Strategies table (metadata about trading strategies)
CREATE TABLE IF NOT EXISTS backtest.strategies (
    strategy_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                 VARCHAR(100) NOT NULL,
    version              VARCHAR(20) NOT NULL,
    description          TEXT,
    author               VARCHAR(100),
    tags                 VARCHAR(200) DEFAULT '',      -- Comma-separated tags
    config_json          JSONB NOT NULL DEFAULT '{}', -- Strategy configuration
    is_active            BOOLEAN DEFAULT true,
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    updated_at           TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(name, version)
);

CREATE INDEX IF NOT EXISTS idx_strategies_active ON backtest.strategies(is_active);
CREATE INDEX IF NOT EXISTS idx_strategies_tags ON backtest.strategies USING GIN(to_tsvector('english', tags));

-- Backtest runs table
CREATE TABLE IF NOT EXISTS backtest.backtests (
    backtest_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id          UUID NOT NULL REFERENCES backtest.strategies(strategy_id),
    name                 VARCHAR(200) NOT NULL,
    description          TEXT,
    status               VARCHAR(50) DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    parameters_json      JSONB NOT NULL DEFAULT '{}',    -- Backtest-specific parameters
    date_range_start     TIMESTAMPTZ NOT NULL,
    date_range_end       TIMESTAMPTZ NOT NULL,
    symbols              VARCHAR(200) NOT NULL,          -- Comma-separated symbols
    timeframes           VARCHAR(100) DEFAULT '1m',     -- Comma-separated timeframes
    initial_capital      NUMERIC(20, 8) DEFAULT 10000,
    currency             VARCHAR(10) DEFAULT 'USDT',
    results_json         JSONB DEFAULT '{}',             -- Summary results: PnL, Sharpe, max_dd, etc.
    metrics_json         JSONB DEFAULT '{}',             -- Detailed metrics
    error_message        TEXT,
    started_at           TIMESTAMPTZ,
    completed_at         TIMESTAMPTZ,
    created_by           VARCHAR(100) DEFAULT 'system',
    created_at           TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_backtests_strategy ON backtest.backtests(strategy_id);
CREATE INDEX IF NOT EXISTS idx_backtests_status ON backtest.backtests(status);
CREATE INDEX IF NOT EXISTS idx_backtests_daterange ON backtest.backtests(date_range_start, date_range_end);
CREATE INDEX IF NOT EXISTS idx_backtests_created_at ON backtest.backtests(created_at DESC);

-- Backtest individual trades (hypertable for time-series analysis)
CREATE TABLE IF NOT EXISTS backtest.backtest_trades (
    trade_id             BIGSERIAL,
    backtest_id          UUID NOT NULL REFERENCES backtest.backtests(backtest_id),
    symbol               VARCHAR(50) NOT NULL,
    side                 VARCHAR(10) NOT NULL CHECK (side IN ('BUY', 'SELL')),
    type                 VARCHAR(20) NOT NULL DEFAULT 'MARKET',
    quantity             NUMERIC(20, 8) NOT NULL,
    price                NUMERIC(20, 8) NOT NULL,
    commission           NUMERIC(20, 8) DEFAULT 0,
    realized_pnl         NUMERIC(20, 8) DEFAULT 0,
    exit_reason          VARCHAR(50),                   -- 'TAKE_PROFIT', 'STOP_LOSS', 'SIGNAL', 'MANUAL'
    entry_time           TIMESTAMPTZ NOT NULL,
    exit_time            TIMESTAMPTZ,
    trade_duration_ms    BIGINT,
    signal_confidence    NUMERIC(5, 4),
    features_json        JSONB DEFAULT '{}',            -- Feature vector at entry
    metadata             JSONB DEFAULT '{}',
    PRIMARY KEY (trade_id, entry_time)
);

-- Create hypertable for backtest_trades
SELECT create_hypertable('backtest.backtest_trades', 'entry_time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_backtest_trades_backtest ON backtest.backtest_trades(backtest_id);
CREATE INDEX IF NOT EXISTS idx_backtest_trades_symbol ON backtest.backtest_trades(symbol, entry_time DESC);
CREATE INDEX IF NOT EXISTS idx_backtest_trades_pnl ON backtest.backtest_trades(backtest_id, realized_pnl DESC);

-- Optimization jobs (parameter optimization runs)
CREATE TABLE IF NOT EXISTS backtest.optimization_jobs (
    job_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id          UUID NOT NULL REFERENCES backtest.strategies(strategy_id),
    name                 VARCHAR(200) NOT NULL,
    status               VARCHAR(50) DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    parameter_space_json JSONB NOT NULL DEFAULT '{}',  -- Parameter search space
    optimization_config_json JSONB DEFAULT '{}',     -- Optimization config (trials, metric, etc.)
    best_parameters_json JSONB DEFAULT '{}',
    best_metric_name     VARCHAR(100),
    best_metric_value    NUMERIC(20, 8),
    results_json         JSONB DEFAULT '{}',
    started_at           TIMESTAMPTZ,
    completed_at         TIMESTAMPTZ,
    error_message        TEXT,
    created_by           VARCHAR(100) DEFAULT 'system',
    created_at           TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_optimization_jobs_strategy ON backtest.optimization_jobs(strategy_id);
CREATE INDEX IF NOT EXISTS idx_optimization_jobs_status ON backtest.optimization_jobs(status);

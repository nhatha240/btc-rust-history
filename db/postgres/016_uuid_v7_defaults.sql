-- ============================================================================
-- PostgreSQL migration 016 — Switch UUID defaults to UUIDv7
-- ============================================================================

-- Orders / OMS
ALTER TABLE IF EXISTS orders
    ALTER COLUMN client_order_id SET DEFAULT uuidv7();

-- Strategy decision logs
ALTER TABLE IF EXISTS decision_logs
    ALTER COLUMN trace_id SET DEFAULT uuidv7();

-- Risk management
ALTER TABLE IF EXISTS risk_limit_profiles
    ALTER COLUMN profile_id SET DEFAULT uuidv7();

ALTER TABLE IF EXISTS risk_limits
    ALTER COLUMN risk_limit_id SET DEFAULT uuidv7();

-- Backtest metadata
ALTER TABLE IF EXISTS bt_runs
    ALTER COLUMN run_id SET DEFAULT uuidv7();

ALTER TABLE IF EXISTS bt_replay_sessions
    ALTER COLUMN replay_id SET DEFAULT uuidv7();

-- Z-score signal store
ALTER TABLE IF EXISTS sig_zscore_signals
    ALTER COLUMN signal_id SET DEFAULT uuidv7();

ALTER TABLE IF EXISTS sig_zscore_signals
    ALTER COLUMN trace_id SET DEFAULT uuidv7();


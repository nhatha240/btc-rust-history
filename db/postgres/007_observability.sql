-- 007_observability.sql
-- Observability and operational metadata tables: execution_snapshots, error_logs, performance_metrics

CREATE SCHEMA IF NOT EXISTS observability;

-- Execution snapshots (periodic position/balance snapshot)
CREATE TABLE IF NOT EXISTS observability.execution_snapshots (
    snapshot_id          BIGSERIAL,
    account_id           VARCHAR(50) NOT NULL,
    timestamp            TIMESTAMPTZ NOT NULL,
    total_balance        NUMERIC(20, 8) DEFAULT 0,
    available_balance    NUMERIC(20, 8) DEFAULT 0,
    positions_json       JSONB DEFAULT '[]',            -- Array of {symbol, side, qty, entry_price, unrealized_pnl}
    orders_json          JSONB DEFAULT '[]',            -- Array of open orders
    metrics_json         JSONB DEFAULT '{}',            -- System metrics (CPU, memory, latency)
    metadata             JSONB DEFAULT '{}',
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (snapshot_id, timestamp)
);

-- Create hypertable for execution_snapshots for time-series analysis
SELECT create_hypertable('observability.execution_snapshots', 'timestamp', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_exec_snapshots_account ON observability.execution_snapshots(account_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_exec_snapshots_timestamp ON observability.execution_snapshots(timestamp DESC);

-- Error logs (structured error logging from services)
CREATE TABLE IF NOT EXISTS observability.error_logs (
    error_id             BIGSERIAL,
    service_name         VARCHAR(100) NOT NULL,
    severity             VARCHAR(20) NOT NULL CHECK (severity IN ('DEBUG', 'INFO', 'WARNING', 'ERROR', 'CRITICAL')),
    error_type           VARCHAR(100),
    message              TEXT NOT NULL,
    stack_trace          TEXT,
    context_json         JSONB DEFAULT '{}',
    trace_id             VARCHAR(50),                   -- Distributed tracing correlation
    span_id              VARCHAR(50),
    occurred_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (error_id, occurred_at)
);

-- Create hypertable for error_logs
SELECT create_hypertable('observability.error_logs', 'occurred_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_error_logs_service ON observability.error_logs(service_name, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_error_logs_severity ON observability.error_logs(severity, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_error_logs_trace ON observability.error_logs(trace_id);
CREATE INDEX IF NOT EXISTS idx_error_logs_type ON observability.error_logs(error_type, occurred_at DESC);

-- Performance metrics (service-level metrics: latency, throughput, errors)
CREATE TABLE IF NOT EXISTS observability.performance_metrics (
    metric_id            BIGSERIAL,
    service_name         VARCHAR(100) NOT NULL,
    metric_name          VARCHAR(100) NOT NULL,        -- e.g., 'request_latency_ms', 'orders_per_second'
    metric_value         NUMERIC(20, 8) NOT NULL,
    labels_json          JSONB DEFAULT '{}',            -- Additional labels (endpoint, method, status_code)
    timestamp            TIMESTAMPTZ NOT NULL,
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (metric_id, timestamp)
);

-- Create hypertable for performance_metrics
SELECT create_hypertable('observability.performance_metrics', 'timestamp', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_perf_metrics_service ON observability.performance_metrics(service_name, metric_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_perf_metrics_timestamp ON observability.performance_metrics(timestamp DESC);

-- Alert rules table
CREATE TABLE IF NOT EXISTS observability.alert_rules (
    rule_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                 VARCHAR(200) NOT NULL,
    description          TEXT,
    metric_query         TEXT NOT NULL,                -- SQL query to evaluate the alert
    condition            VARCHAR(20) NOT NULL CHECK (condition IN ('>', '<', '>=', '<=', '=', '!=', 'change')),
    threshold_value      NUMERIC(20, 8) NOT NULL,
    evaluation_interval  INTEGER DEFAULT 60,           -- Seconds between evaluations
    severity             VARCHAR(20) DEFAULT 'warning' CHECK (severity IN ('info', 'warning', 'critical')),
    notification_channels JSONB DEFAULT '[]',           -- List of channels (email, slack, webhook)
    is_active            BOOLEAN DEFAULT true,
    last_evaluated_at    TIMESTAMPTZ,
    last_value           NUMERIC(20, 8),
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    updated_at           TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_alert_rules_active ON observability.alert_rules(is_active);

-- Alert incidents table
CREATE TABLE IF NOT EXISTS observability.alert_incidents (
    incident_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id              UUID REFERENCES observability.alert_rules(rule_id),
    name                 VARCHAR(200) NOT NULL,
    status               VARCHAR(20) DEFAULT 'open' CHECK (status IN ('open', 'acknowledged', 'resolved', 'closed')),
    severity             VARCHAR(20) NOT NULL,
    triggered_value      NUMERIC(20, 8) NOT NULL,
    threshold_value      NUMERIC(20, 8) NOT NULL,
    context_json         JSONB DEFAULT '{}',
    acknowledged_by      VARCHAR(100),
    acknowledged_at      TIMESTAMPTZ,
    resolved_at          TIMESTAMPTZ,
    created_at           TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_alert_incidents_status ON observability.alert_incidents(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_alert_incidents_rule ON observability.alert_incidents(rule_id);

# Dashboards (Web + Ops)

## Web Dashboard (Trading Ops)
Pages and primary queries:

### 1) Orders
- Filters: symbol, status, time range, min confidence, reject-only
- Columns:
    - created_at, symbol, side, qty, limit_price
    - status, model_version, confidence_score
    - trace_id (clickable)
- API:
    - GET /api/orders?...

### 2) Order Detail
- Order summary (client_order_id, exchange_order_id)
- Timeline from order_events
- Fills table from trades
- Decision log:
    - reason string
    - metrics snapshot (confidence, spread_bps, ema spread, rsi, funding)

### 3) Trades
- filter by symbol/time
- aggregate: fee, slippage, avg fill

### 4) Positions
- current positions
- exposure by symbol
- history snapshots (optional)

### 5) PnL
- daily pnl
- win rate, avg R, drawdown
- per-symbol breakdown

### 6) Latency & Health
- consumer lag per group
- end-to-end latency distribution
- ws reconnect count
- reject rate

## Ops Dashboards (Grafana)
Recommended panels:

### Kafka/Redpanda
- Consumer lag by group (feature_engine, ai_predictor, strategy_engine, execution_router)
- Produce/consume throughput
- Error rate (dlq count)

### Services
- CPU/mem per pod
- p99 message processing time per service
- WS reconnect count
- decode failures

### Trading Risk
- order rejects/min
- notional exposure by symbol
- leverage usage
- stop-loss triggers count
- kill-switch status timeline

## Tracing (OpenTelemetry)
Trace spans:
- ingest.receive
- feature.compute
- ai.infer
- strategy.decide
- execution.submit
- execution.report

Ensure trace_id is propagated across MQ messages and persisted in PostgreSQL for cross-linking.

# Runbook (Operations)

## Environments
- local: docker-compose
- staging: minimal cluster
- prod: replicated MQ + DB + monitoring

## Startup Order (Local/Prod)
1) Redpanda/Kafka
2) PostgreSQL + TimescaleDB
3) Redis
4) (Optional) Grafana/Prometheus
5) MarketData_Ingestor
6) Feature_Engine
7) AI_Predictor
8) Strategy_Engine
9) Execution_Router
10) API Gateway
11) Web Dashboard

## Health Checks
Each service should expose:
- /health (liveness)
- /ready (readiness: MQ connected, schema loaded, DB reachable if needed)

Required metrics:
- consumer_lag
- message_decode_failures
- ws_reconnect_count
- order_reject_rate
- end_to_end_latency_ms (ingest -> feature -> prediction -> decision -> ack)

## On-call Triage

### Symptom: No trades happening
Check:
- Kill-switch status (control.kill_switch / API)
- Strategy gates (stale/confidence/spread)
- Consumer lag spikes
- Exchange WS connectivity
- Model inference health

### Symptom: Many rejects / rate limits
Check:
- Execution rate limiter counters
- order size constraints
- exchange maintenance/incidents
  Mitigation:
- reduce order frequency
- increase cooldown
- temporarily disable entries

### Symptom: Strategy double-orders
Check:
- client_order_id uniqueness
- Execution dedup store working
- Kafka at-least-once duplicates
  Mitigation:
- enable idempotent producer
- stronger dedup window

### Symptom: Position mismatch
Check:
- reconcile logs (Execution_Router)
- missing execution_reports
  Mitigation:
- force reconcile via REST
- block new entries until resolved

## Kill Switch
Behavior:
- Strategy: stop emitting new orders, only allow reduce-only exits (optional)
- Execution: reject new OrderCommand (except reduce-only) and cancel open orders (optional)

## Data Retention
TimescaleDB:
- raw_ticks: 7 days retention
- 1m candles: 3 months
- 1h/1d candles: keep indefinitely
  PostgreSQL:
- orders/trades/order_events: keep indefinitely (or archive policy)
  Kafka:
- md.*: 1 hour
- ai.*: 24 hours
- orders.*: 7-14 days
- dlq.*: 7 days

## Incident Logging Requirements
For each order:
- trace_id
- client_order_id
- decision reason + indicator snapshot
  For each reject:
- exchange error code + message
- rate limit counters snapshot

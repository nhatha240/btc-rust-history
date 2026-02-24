# Architecture

## Goals
- Low-latency, deterministic hot-path for signal generation & order execution
- Clear separation of concerns (microservices)
- Schema-first (Protobuf/FlatBuffers), no JSON in hot-path messaging
- Idempotent OMS loop: Strategy ↔ Execution feedback
- Traceable end-to-end (trace_id + timestamps + versioning)
- Operable in production (health, metrics, kill-switch, config updates)

## High-Level Components

### Data Plane (hot-path)
1) MarketData_Ingestor (Rust)
- Connect exchange WS
- Receive raw market events (trades, book)
- Normalize + timestamp + sequence
- Publish to MQ topics
- No indicator calculation

2) Feature_Engine (Rust)
- Consume raw ticks
- Maintain per-symbol in-memory state (incremental indicators + ring buffers)
- Produce live features to MQ
- Emit quality flags (stale/gap/low_liq)

3) AI_Predictor (Python/FastAPI + PyTorch)
- Consume features
- Run model inference (LSTM/XGBoost/etc.)
- Publish predictions with model_version + feature_version
- Provide control endpoints (health/model reload)

4) Strategy_Engine (Rust)
- Consume features + predictions + execution reports
- Hard gates (staleness/confidence/liquidity)
- Rules/regime detection
- Risk management + sizing + TP/SL planning
- Emit OrderCommand to MQ (orders.commands)
- Write decision logs (DB) for audit/debug

5) Execution_Router (Rust)
- Consume OrderCommand
- Submit to exchange REST
- Track order lifecycle via WS user stream
- Reconcile periodically via REST
- Dedup by client_order_id (idempotency)
- Publish execution reports (orders.execution_reports)
- Persist orders/trades/events to PostgreSQL

### Control Plane (cold-path)
- API Gateway (Rust/Axum or Python/FastAPI)
    - Serves web dashboard
    - Reads PostgreSQL (orders/trades/positions/decision logs)
    - Auth + RBAC
    - Exposes kill-switch and config endpoints
- Web Dashboard (Next.js)
    - Orders list/detail timeline
    - Positions + PnL + latency/health
- Observability
    - Prometheus metrics / OpenTelemetry tracing
    - Grafana dashboards
    - Redpanda Console/Kafka UI for topic inspection

## Data Flow

### Market Data → Features → Predictions → Strategy → Execution
- Ingestor publishes:
    - md.raw.trades (AggTrade)
    - md.raw.book (best bid/ask or depth deltas)
- Feature Engine publishes:
    - md.features.live
- AI Predictor publishes:
    - ai.predictions.signals
- Strategy publishes:
    - orders.commands
- Execution publishes:
    - orders.execution_reports
- Execution persists to PostgreSQL:
    - orders, order_events, trades, positions, decision_logs

## Ordering, Partitioning, and Idempotency

### Partitioning
- Market topics:
    - partition key = symbol
- OMS topics:
    - partition key = account_id:symbol (or account_id)

### Idempotency
- OrderCommand contains client_order_id (idempotency key)
- Execution_Router must dedup on client_order_id
- Strategy_Engine must handle at-least-once by:
    - ignoring already-acknowledged orders
    - state updated via execution_reports

### Time & Sequence
Every message carries:
- event_time_ns (exchange time if available)
- recv_time_ns (local receive time)
- seq (per-symbol sequence, if applicable)
- trace_id (correlation across pipeline)
- schema_version + feature_version + model_version

## Failure Modes & Circuit Breakers

### Data issues
- Stale features/predictions -> HOLD
- Out-of-order / gaps detected -> set quality_flags, optionally HOLD
- Spread too wide -> block entries

### Exchange issues
- Reject spikes or rate-limit hits -> cooldown + reduce aggressiveness
- WS disconnect -> stop trading until resync
- Reconcile mismatch -> block new orders until fixed

### Model issues
- Inference errors -> publish neutral prediction (direction=0, confidence=0)
- Version mismatch -> ignore prediction and log incident

## Security
- API keys stored encrypted (PostgreSQL) + loaded into Execution only
- Dashboard API protected with auth (JWT/basic internal)
- Kill-switch requires a privileged role

## Scaling Notes
- Scale by symbol partitions:
    - multiple Feature_Engine instances by partition
    - Strategy_Engine can scale by account/symbol partition
- Execution_Router often kept minimal instances per account to reduce order-state fragmentation

## Non-Goals (initially)
- Full depth orderbook reconstruction for all symbols (heavy)
- Cross-exchange arbitration
- Complex portfolio margining across venues

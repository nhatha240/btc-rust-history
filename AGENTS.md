# AGENTS.md

> **Audience:** AI coding agents only (Claude / Copilot / Gemini / internal agents).
> **Source of truth:** `trading_system_design_handbook.md` — this file summarizes and constrains behavior for this repo.
> **Non-negotiable:** Follow service boundaries, hot-path constraints, and safety rules. If unsure, choose the safer change.

---

## 0) Operating Mode (AI Agent Contract)

When modifying code, the agent MUST:
- Preserve correctness and trading safety over performance gains.
- Keep changes minimal, localized, and reversible.
- Prefer additive changes (schemas/protos) and backward compatibility.
- Emit structured logs for any decision-making or gating.
- Never introduce secret material into code or logs.
- Always reference `trading_system_design_handbook.md` for architectural decisions.

If requirements are ambiguous, make the safest assumption and document it in code comments and/or docs.

---

## 1) Core Objective

Build and maintain a **low-latency auto-trading platform** with:
- **Rust + Python** microservices
- **Redpanda/Kafka** messaging (Protobuf on hot path)
- **Postgres/Timescale** as OMS source of truth
- **Redis** for ephemeral/hot state
- **ClickHouse** for time-series analytics (non-blocking for P0 flow)
- **OpenTelemetry** for distributed tracing

### System Responsibilities (Handbook §1)
1. Ingest market data correctly
2. Compute features and context consistently
3. Generate trade decisions
4. Enforce risk and operational controls
5. Send, track, and reconcile orders
6. Persist everything needed for replay, audit, and iteration

---

## 2) Mandatory File Placement

Hard constraints (do not violate):

### Database
- All DB schema/migrations/query SQL MUST live under `db/`.
  - Postgres migrations: `db/postgres/*.sql` (numbered: `001_`, `002_`, ...)
  - ClickHouse DDL: `db/clickhouse/*.sql`
  - Queries: `db/queries/*.sql`

### Infrastructure
- All infra YAML and infra-related files MUST live under `infra/`.
  - Docker: `infra/docker/`
  - Scripts: `infra/scripts/`
  - K8s: `infra/k8s/`

### Protobuf
- All cross-service schemas in `proto/` (subdirs: `ai/`, `md/`, `oms/`, `control/`, `common/`)
- Regenerate Rust/Python bindings after proto changes.

---

## 3) Service Boundaries (Do Not Violate)

Each service has a single responsibility. **See handbook §2.2 and §8.2.**

### Rust Services (single Cargo workspace)

| Binary              | Location                              | Role                                              |
|---------------------|---------------------------------------|----------------------------------------------------|
| `marketdata_ingestor` | `services/marketdata_ingestor`      | Ingest raw data + candles → Redpanda + ClickHouse  |
| `feature_state`     | `services/feature_engine/feature_state` | Compute indicators, publish feature vectors      |
| `signal_engine`     | `services/strategy_engine/signal_engine` | Consume features, apply rules, emit signals     |
| `risk_guard`        | `services/execution_router/risk_guard`  | Validate orders against limits, approve/reject   |
| `order_executor`    | `services/execution_router/order_executor` | Submit approved orders to exchange             |
| `paper_trader`      | `services/execution_router/paper_trader`  | Simulate fills without real orders              |
| `execution_router`  | `services/execution_router`           | Route orders to executor or paper trader          |
| `mc_snapshot`       | `services/strategy_engine/mc_snapshot`  | Market-condition snapshot writer                 |
| `api_gateway`       | `services/api_gateway`                | REST API + control plane                           |
| `web`               | `apps/web_dashboard`                  | Dashboard backend (Axum)                           |
| `agg_15m`           | `services/agg_15m`                    | 15-minute candle aggregation                       |

### Python Service

| Service        | Location                | Role                                      |
|----------------|-------------------------|--------------------------------------------|
| `ai_predictor` | `services/ai_predictor` | ML inference; consume features, emit signals |

### Forbidden Crossovers
- No strategy logic inside execution services
- No direct exchange REST from `strategy_engine`
- No model inference outside `ai_predictor`
- No UI direct access to storage/messaging
- No mutable state inside signal computation — strategy code must be pure (handbook §1.2 principle 6)

If a change pushes logic across boundaries, STOP and refactor into the correct service.

---

## 4) Data Plane Rules (Hot Path)

### Serialization (Handbook §2.3)
- Use **Protobuf** for hot-path topics; avoid JSON in the hot path.
- Standardize event contracts: `TradeEvent`, `BookDeltaEvent`, `BarClosedEvent`, `FeatureVectorEvent`, `SignalDecisionEvent`, `RiskDecisionEvent`, `OrderIntentEvent`, `OrderEvent`, `FillEvent`, `PositionSnapshotEvent`, `RiskBreachEvent`, `StrategyLogEvent`

### Idempotency & Dedup
- Preserve idempotency using `trace_id` and `client_order_id`.
- At-least-once consumption MUST NOT create duplicate DB rows.
- Dedup MUST be enforced at the consumer + storage boundary.

### Partitioning
- Market topics: partition by `symbol`
- OMS topics: partition by `account_id:symbol`

### Kafka Topics

| Topic                        | Retention | Producer             | Consumers                       |
|------------------------------|-----------|----------------------|----------------------------------|
| `TOPIC_CANDLES_1M`           | 1h        | `marketdata_ingestor` | `feature_state`                 |
| `TOPIC_FEATURE_STATE`        | compacted | `feature_state`      | `signal_engine`, `ai_predictor` |
| `TOPIC_SIGNALS`              | 24h       | `signal_engine`, `ai_predictor` | `order_executor`    |
| `TOPIC_SIGNAL_STATE`         | compacted | `signal_engine`      | Internal state rebuild           |
| `TOPIC_MC_SNAPSHOT`          | 24h       | `mc_snapshot`        | `signal_engine`                  |
| `TOPIC_ORDERS`               | 7d        | `order_executor`     | `risk_guard`                     |
| `TOPIC_ORDERS_APPROVED`      | 7d        | `risk_guard`         | `order_executor`, `paper_trader` |
| `TOPIC_FILLS`                | 7d        | `order_executor`, `paper_trader` | CH sink, PG writer |
| `md.raw.trades`              | 1h        | `marketdata_ingestor` | analytics                       |
| `md.raw.book`                | 1h        | `marketdata_ingestor` | analytics                       |
| `md.raw.orderbook`           | 1h        | `marketdata_ingestor` | analytics                       |
| `md.raw.open_interest`       | 1h        | `marketdata_ingestor` | analytics                       |
| `md.raw.mark_price`          | 1h        | `marketdata_ingestor` | analytics                       |
| `md.raw.liquidation`         | 1h        | `marketdata_ingestor` | analytics                       |
| `control.config_updates`     | 30d       | `api_gateway`        | all services                     |
| `control.kill_switch`        | 30d       | `api_gateway`        | `risk_guard`, `execution_router` |
| `system.heartbeats`          | 24h       | all services         | monitoring                       |
| `dlq.main`                   | 7d        | all services         | ops                              |

---

## 5) Storage Rules (Handbook §3)

### Polyglot Persistence Policy

| Data Type                    | Store          | Why                              |
|-----------------------------|----------------|-----------------------------------|
| Instruments, venues, configs | PostgreSQL    | Strong constraints, FK support    |
| Orders, fills, positions     | PostgreSQL    | ACID, reliable state transitions  |
| Bars, trades, ticks, features| ClickHouse    | Efficient analytical queries      |
| Order book deltas/snapshots  | ClickHouse    | High ingest, cheap cold storage   |
| Latest market state          | Redis         | Low latency key-based access      |
| Event journal                | Kafka/Redpanda| Durable event distribution        |

### Database Domain Prefixes (Handbook §3.3)
- `ref_*` — reference metadata (venues, instruments)
- `md_*` — market data (trades, bars, books, funding, OI)
- `feat_*` — features and feature store
- `sig_*` — signals
- `ord_*` — orders and execution
- `pos_*` — positions and PnL
- `risk_*` — risk limits and breaches
- `strat_*` — strategies and logs
- `bt_*` — backtest metadata

### Postgres Rules
- **Source of truth** for OMS: `orders`, `order_events`, `trades`, `positions`, `decision_logs`
- Migrations MUST live in `db/postgres/*.sql`
- Use TimescaleDB hypertables for high-volume event tables
- Partition by time first, add second dimension only if partition too large

### ClickHouse Rules
- Append-only; never update hot market data row-by-row
- `PARTITION BY toYYYYMM(event_time)` or `toDate()` depending on volume
- `ORDER BY` for most common filter pattern
- Use materialized views for aggregation (candle rollups, etc.)
- TTL for auto-expiry of old data
- P0 flow MUST NOT depend on ClickHouse readiness

### Redis Rules
- Keys SHOULD use TTL unless explicitly permanent
- Never store secrets in Redis
- Kill switch: `risk:kill` — halts live order submission when set to "1"

---

## 6) Architecture Patterns (Handbook §8)

### Domain Objects to Formalize
- `MarketContext`, `FeatureSnapshot`, `SignalDecision`, `RiskDecision`
- `OrderIntent`, `OrderState`, `Fill`, `PositionState`
- `StrategyLog`, `ReplayContext`

### Strategy Interface Design (Handbook §8.4)
- Input: **immutable context**
- Output: **structured decision** (side, score, confidence, reason_codes)
- Strategy MUST NOT send orders directly
- Same signal/risk code for backtest, paper, and live modes

### Market Data Pipeline (Handbook §8.5)
```text
raw exchange payload
  → schema validate
  → map symbols and venue fields
  → assign event_time and ingest_time
  → update latest in-memory state
  → emit canonical event
  → persist raw and canonical
  → trigger incremental feature updates
```

### Feature Engine Rules (Handbook §8.6)
- Maintain rolling windows in memory
- Update only features touched by new event
- Avoid full-dataframe recomputation in live path
- Precompute higher-timeframe bars incrementally
- Snapshot live feature state periodically for recovery
- **Point-in-time correctness is mandatory** — never use future data

### Risk Engine Responsibilities (Handbook §8.8)
- Pre-trade: `checkSignalRisk`, `checkOrderRisk`
- Post-trade: `checkFillRisk`, `checkPositionRisk`
- Portfolio-level: `checkPortfolioRisk`
- Kill switch: fail-closed behavior
- Risk is a **first-class engine**, not a post-processing step

### Execution Engine Responsibilities (Handbook §8.7)
- Translate `OrderIntent` → venue-specific order
- Generate deterministic `client_order_id`
- Handle retries carefully
- Track acks/rejects (event-sourced order state machine)
- Reconcile fills and fees
- Support kill switch behavior

---

## 7) Strategy Families (Handbook §4)

The system should support multiple strategy families with regime-aware routing:

| Family                 | Best Regime                     | Implementation Priority |
|------------------------|---------------------------------|--------------------------|
| Trend-following        | Directional, expanding         | P0 — first strategy      |
| Mean reversion         | Range-bound                     | P0 — second strategy     |
| Breakout               | Compression → expansion         | P1                       |
| Momentum               | Trending with leadership       | P1                       |
| Volatility expansion   | Transition zones               | P1 (also as filter)      |
| Volume profile / VWAP  | Intraday auction               | P2                       |
| Funding/basis/OI       | Perp/futures dislocations      | P2                       |
| Multi-timeframe        | All                             | P1 (confirmation layer)  |
| Regime detection       | All                             | P0 — strategy router     |

### Regime Labels
```text
TREND_UP, TREND_DOWN, RANGE, VOL_COMPRESSION, VOL_EXPANSION, PANIC, ILLIQUID, HIGH_SPREAD_NO_TRADE
```

---

## 8) Signal Engine Anti-Spam Config (Env-Driven)

```env
SIGNAL_EDGE_MODE=BAR_CLOSE
SIGNAL_STABLE_MS=2000
SIGNAL_HYSTERESIS_TYPE=ATR
SIGNAL_HYSTERESIS_MULT_ENTER=0.5
SIGNAL_HYSTERESIS_MULT_EXIT=0.2
SIGNAL_DEBOUNCE_MS=1000
SIGNAL_COOLDOWN_MS=15000
SIGNAL_MAX_PER_MIN=5
SIGNAL_THROTTLE_MODE=DROP
```

---

## 9) Risk Guard Limits (Env-Driven)

```env
LIMIT_NOTIONAL_PER_SYMBOL=5000
LIMIT_LEVERAGE=5
KILL_SWITCH_KEY=risk:kill
```

Reference risk policy (handbook §13.4):
```yaml
risk:
  max_risk_per_trade_bps: 50
  max_daily_loss_r: 3.0
  max_weekly_loss_r: 8.0
  max_open_positions: 5
  max_gross_exposure_usd: 250000
  stale_market_data_ms: 1500
  stale_feature_data_ms: 2000
  cooldown_after_three_losses_min: 30
```

---

## 10) Coding Rules

### Rust
- Small focused functions; config-driven behavior
- Structured logs (`tracing`) with decision metrics
- Explicit error handling; **never swallow errors**
- Async via `tokio`
- Every service exposes `GET /health` and `GET /ready`
- `rdkafka` for Kafka, `axum` for HTTP, `sqlx` for Postgres, `clickhouse` for CH
- Config from env via `config.rs` per service

### Python (ai_predictor)
- Dependencies in `pyproject.toml`
- Config in `app/config.py`, logging in `app/logging.py`, metrics in `app/metrics.py`
- Separate consumer loop and HTTP server
- Never do inference outside `ai_predictor`

### Web
- UI reads ONLY via `api_gateway`
- Lists MUST support pagination + filtering
- No direct access to storage/messaging from UI

---

## 11) Observability Stack

| Tool                      | Port           | Purpose        |
|---------------------------|----------------|----------------|
| OpenTelemetry Collector   | 4317/4318      | Traces         |
| Prometheus                | 9090           | Metrics        |
| Grafana                   | 3000           | Dashboards     |
| Redpanda Console          | 8085           | Topic browser  |
| Redpanda Connect          | 8083           | Connectors     |

All Rust services export OTLP traces. Configure `OTEL_EXPORTER_OTLP_ENDPOINT` and `OTEL_RESOURCE_ATTRIBUTES`.

Every service MUST expose `/health` and `/ready`. Readiness MUST verify dependencies.

---

## 12) Testing + Validation

After non-trivial changes, the agent MUST ensure:

1. `docker compose up` is stable
2. Topics/configs exist and are correct
3. Run `tests/verify_oms_loop.py`
4. Verify web order list + detail
5. Verify kill-switch behavior
6. Verify end-to-end traceability via `trace_id`

If any step is not possible, explicitly state which checks were not executed and why.

---

## 13) Change Policy

- Topic rename MUST update all relevant docs/config/scripts/tests.
- Schema/proto changes MUST regenerate Rust/Python proto libs.
- Prefer additive schema changes; **avoid destructive changes** (no drops).
- Keep backward compatibility during rollout.
- Never mix ClickHouse DDL and Postgres migrations.
- ClickHouse cannot roll back reliably; test DDL on local containers first.

---

## 14) Build Roadmap (Handbook §Final)

Recommended build order from zero:
1. Canonical schemas and data contracts
2. Order/fill/position/risk transactional backbone
3. Bars + feature pipeline + strategy logs
4. One trend strategy + one mean-reversion strategy + one breakout filter
5. Replay engine and execution quality analytics
6. Regime selector and capital allocation layer
7. Derivatives context features (funding/OI/basis)
8. Microstructure modules only when justified by data and latency budget

### Minimum Viable Serious System
A "serious" system is defined by:
- Clear schemas
- Deterministic decisions
- Point-in-time features
- Risk controls
- Replayability
- Explainable logs
- Execution analytics

---

## 15) Repository Layout

```text
btc-rust-history/
├── apps/
│   └── web_dashboard/
├── services/
│   ├── marketdata_ingestor/        # Raw data + candle ingestion
│   ├── feature_engine/             # Indicator computation
│   │   └── feature_state/
│   ├── strategy_engine/            # Signal generation
│   │   ├── signal_engine/
│   │   └── mc_snapshot/
│   ├── execution_router/           # Order execution pipeline
│   │   ├── order_executor/
│   │   ├── risk_guard/
│   │   └── paper_trader/
│   ├── api_gateway/
│   └── ai_predictor/               # Python ML inference
├── proto/                          # Cross-service Protobuf schemas
│   ├── ai/ md/ oms/ control/ common/
├── libs/rust/                      # Shared Rust libraries
│   ├── hft_proto/ hft_common/ hft_store/ hft_mq/ hft_redis/ hft_exchange/ hft_risk/
├── crates/common/                  # Shared crate
├── db/
│   ├── clickhouse/                 # ClickHouse DDL
│   ├── postgres/                   # Postgres migrations
│   └── queries/                    # Shared SQL queries
├── infra/docker/                   # Docker compose + infra
├── observability/                  # OTEL, Prometheus configs
├── docs/
├── tests/
├── tools/
├── secrets/                        # gitignored
├── Cargo.toml                      # Workspace root
├── Dockerfile.rust-workspace
└── trading_system_design_handbook.md
```

---

## ⚠️ Safety Rules for AI Agents

1. **Never hardcode API keys or secrets** — read via env vars / Docker secrets.
2. **Never disable risk_guard** without explicit user instruction.
3. **Never execute real orders directly** — always go signal → risk → executor.
4. **Kill switch:** Redis key `risk:kill` halts live order submission when set to "1".
5. **Paper trader first** for new strategies; do not default to live execution.
6. **ClickHouse is append-only**; mutable state belongs in compacted topics.
7. **Never drop columns**; prefer additive changes with defaults.
8. **Always consult `trading_system_design_handbook.md`** for design decisions.
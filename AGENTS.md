# AGENTS.md

> **Audience:** AI coding agents only (Claude / Copilot / Gemini / internal agents).  
> **Source of truth:** `.ai/rules.yaml` (this file summarizes and constrains behavior for this repo).  
> **Non-negotiable:** Follow service boundaries, hot-path constraints, and safety rules. If unsure, choose the safer change.

---

## 0) Operating Mode (AI Agent Contract)

When modifying code, the agent MUST:
- Preserve correctness and trading safety over performance gains.
- Keep changes minimal, localized, and reversible.
- Prefer additive changes (schemas/protos) and backward compatibility.
- Emit structured logs for any decision-making or gating.
- Never introduce secret material into code or logs.

If requirements are ambiguous, make the safest assumption and document it in code comments and/or docs.

---

## 1) Core Objective

Build and maintain a **low-latency auto-trading platform** with:
- **Rust + Python** microservices
- **Redpanda/Kafka** messaging (Protobuf on hot path)
- **Postgres/Timescale** as OMS source of truth
- **Redis** for ephemeral/hot state
- Optional **ClickHouse** for analytics (non-blocking for P0 flow)

---

## 2) Mandatory File Placement

Hard constraints (do not violate):

### Database
- All DB schema/migrations/query SQL MUST live under `db/`.
  - Examples: `db/postgres/`, `db/clickhouse/`, `db/queries/`

### Infrastructure
- All infra YAML and infra-related files MUST live under `infra/`.
  - Examples: `infra/docker/`, `infra/scripts/`, `infra/k8s/`

---

## 3) Service Boundaries (Do Not Violate)

Each service has a single responsibility:

- `marketdata_ingestor`: ingest + normalize + publish market events only
- `feature_engine`: compute indicators + publish features only
- `ai_predictor`: inference + publish predictions only
- `strategy_engine`: decision logic + order command planning only
- `execution_router`: execute/dedup/persist OMS only
- `api_gateway`: API + control plane only

### Forbidden crossovers
- No strategy logic inside execution services
- No direct exchange REST from `strategy_engine`
- No model inference outside `ai_predictor`

If a change pushes logic across boundaries, STOP and refactor into the correct service instead.

---

## 4) Data Plane Rules (Hot Path)

### Serialization
- Use **Protobuf** for hot-path topics; avoid JSON in the hot path.

### Idempotency & Dedup
- Preserve idempotency using `trace_id` and `client_order_id`.
- At-least-once consumption MUST NOT create duplicate DB rows.
- Dedup MUST be enforced at the consumer + storage boundary.

### Partitioning
- Market topics partition by `symbol`
- OMS topics partition by `account_id:symbol`

---

## 5) Storage Rules

### Source of truth
- **Postgres/Timescale** is OMS source of truth.

### Required OMS entities
- `orders`, `order_events`, `fills`, `positions`, `decision_logs`

### Migrations
- Postgres migrations MUST live in `db/postgres/*.sql`

### ClickHouse
- Optional analytics only.
- P0 flow MUST NOT depend on ClickHouse readiness.

### Redis
- Redis keys SHOULD use TTL unless explicitly permanent.
- Never store secrets in Redis.

---

## 6) Coding Rules (Language + Service Practices)

### Rust
- Small focused functions; config-driven behavior
- Structured logs (`tracing`) with decision metrics
- Explicit error handling; never swallow errors
- Async via `tokio`
- Every service exposes `GET /health` and `GET /ready`

### Python
- Typed config, clear module boundaries
- Separate consumer loop and HTTP server (avoid entanglement)
- Explicit failure handling + structured logs
- Never do inference outside `ai_predictor`

### Web
- UI reads ONLY via `api_gateway`
- Lists MUST support pagination + filtering
- No direct access to storage/messaging from UI

---

## 7) Testing + Validation (Minimum for Non-trivial Changes)

After non-trivial changes, the agent MUST ensure:

1. `docker compose up` is stable
2. Topics/configs exist and are correct
3. Run `tests/verify_oms_loop.py`
4. Verify web order list + detail
5. Verify kill-switch behavior
6. Verify end-to-end traceability via `trace_id`

If any step is not possible in the current context, explicitly state which checks were not executed and why.

---

## 8) Change Policy

- Topic rename MUST update all relevant docs/config/scripts/tests.
- Schema/proto changes MUST regenerate Rust/Python proto libs where applicable.
- Prefer additive schema changes; avoid destructive changes (no drops).
- Keep backward compatibility during rollout whenever possible.

---

## 9) Workflow Order (When Bringing Up / Modifying the Stack)

1. Align compose + topic config
2. Run DB migrations
3. Start OMS services
4. Run verification tests
5. Verify dashboard/API behavior

---

## 10) Operational Baseline

- Every service MUST expose `/health` and `/ready`.
- Readiness MUST verify dependencies used by that service (MQ/DB/Redis).
- Preserve distributed trace propagation (`trace_id`) end-to-end.

---

# CLAUDE.md (AI Skill File) — `btc-rust-backend`

> **Audience:** AI coding agents only.  
> **Authority:** This file is the canonical repository reference for agents.  
> **Safety first:** Never bypass risk or execute real orders outside the approved pipeline.

---

## 🎯 System Goal

An **automated cryptocurrency trading system** that:
1. Ingests real-time market data (candles) from Binance
2. Computes technical indicators and features
3. Runs signal generation logic (rule-based + AI)
4. Routes orders through a risk gate
5. Executes live orders on Binance **or** simulates via a paper trader
6. Stores events for analytics and a web dashboard

---

## 🏗️ Architecture Overview

```text
Binance WS
    │
    ▼
[ingestion]  ──TOPIC_CANDLES_1M──▶  [feature_state]  ──TOPIC_FEATURE_STATE──▶  [signal_engine]
                                                                                       │
                                                                              TOPIC_SIGNALS
                                                                                       │
                                                              ┌────────────────────────┤
                                                              ▼                        ▼
                                                      [order_executor]         [paper_trader]
                                                              │
                                                          orders.v1
                                                              │
                                                              ▼
                                                        [risk_guard]
                                                              │
                                                    orders.approved.v1
                                                              │
                                                              ▼
                                                    [order_executor → Binance]

All inter-service communication goes through Redpanda (Kafka-compatible).

📦 Services
Rust Binaries (single Cargo workspace)
| Binary           | Location                       | Role                                                             |
| ---------------- | ------------------------------ | ---------------------------------------------------------------- |
| `ingestion`      | `services/marketdata_ingestor` | Streams Binance candles → Redpanda + ClickHouse                  |
| `feature_state`  | `services/feature_engine`      | Consumes candles, computes indicators, publishes feature vectors |
| `signal_engine`  | `services/strategy_engine`     | Consumes features, applies rules, emits trading signals          |
| `risk_guard`     | `services/execution_router`    | Validates orders against limits, approves/rejects                |
| `order_executor` | `services/execution_router`    | Submits approved orders to Binance exchange                      |
| `paper_trader`   | `services/execution_router`    | Simulates fills without real orders                              |
| `web`            | `apps/web_dashboard`           | REST API + dashboard backend (Axum)                              |
| `mc_snapshot`    | `services/strategy_engine`     | Market-condition snapshot writer                                 |

##Python Service

| Service        | Location                | Role                                                  |
| -------------- | ----------------------- | ----------------------------------------------------- |
| `ai_predictor` | `services/ai_predictor` | ML inference; consumes features, publishes AI signals |

##Data Stores

| Store                      | Port                           | Usage                                                  |
| -------------------------- | ------------------------------ | ------------------------------------------------------ |
| **Redpanda** (Kafka)       | `9092`                         | Event streaming between services                       |
| **ClickHouse**             | `8123` (HTTP), `9000` (native) | Time-series analytics (candles/features/signals/fills) |
| **PostgreSQL/TimescaleDB** | `5432`                         | OMS + relational truth (orders/risk/positions)         |
| **Redis**                  | `6379`                         | Hot state (signal state, kill-switch, position cache)  |

##Kafka Topics

| Topic                                          | Retention | Producer                         | Consumers                               |
| ---------------------------------------------- | --------- | -------------------------------- | --------------------------------------- |
| `TOPIC_CANDLES_1M`                             | 1 h       | `ingestion`                      | `feature_state`                         |
| `TOPIC_FEATURE_STATE`                          | compacted | `feature_state`                  | `signal_engine`, `ai_predictor`         |
| `TOPIC_SIGNALS`                                | 24 h      | `signal_engine`, `ai_predictor`  | `order_executor`                        |
| `TOPIC_SIGNAL_STATE`                           | compacted | `signal_engine`                  | Internal state rebuild                  |
| `TOPIC_MC_SNAPSHOT`                            | 24 h      | `mc_snapshot`                    | `signal_engine` (market regime context) |
| `TOPIC_ORDERS` (`orders.v1`)                   | 7 d       | `order_executor`                 | `risk_guard`                            |
| `TOPIC_ORDERS_APPROVED` (`orders.approved.v1`) | 7 d       | `risk_guard`                     | `order_executor`, `paper_trader`        |
| `TOPIC_FILLS` (`fills.v1`)                     | 7 d       | `order_executor`, `paper_trader` | ClickHouse sink, PostgreSQL writer      |

Partition key:

Market topics: symbol

OMS topics: account_id (or account_id:symbol where used)

📐 Signal Engine — Anti-Spam Config (Env-Driven)
SIGNAL_EDGE_MODE              BAR_CLOSE
SIGNAL_STABLE_MS              2000
SIGNAL_HYSTERESIS_TYPE        ATR
SIGNAL_HYSTERESIS_MULT_ENTER  0.5
SIGNAL_HYSTERESIS_MULT_EXIT   0.2
SIGNAL_DEBOUNCE_MS            1000
SIGNAL_COOLDOWN_MS            15000
SIGNAL_MAX_PER_MIN            5
SIGNAL_THROTTLE_MODE          DROP
🛡️ Risk Guard — Limits (Env-Driven)
LIMIT_NOTIONAL_PER_SYMBOL   5000
LIMIT_LEVERAGE              5
KILL_SWITCH_KEY             risk:kill
🌐 Observability Stack
Tool	Port	Purpose
OpenTelemetry Collector	4317 (gRPC), 4318 (HTTP)	Traces
Prometheus	9090	Metrics
Grafana	3000	Dashboards
Redpanda Console	8080	Topic browser
Redpanda Connect	8083	Connectors (e.g., sinks to ClickHouse)

All Rust services export OTLP traces. Configure OTEL_EXPORTER_OTLP_ENDPOINT and OTEL_RESOURCE_ATTRIBUTES.

📁 Repository Layout
btc-rust-backend/
├── apps/
│   └── web_dashboard/
├── services/
│   ├── marketdata_ingestor/
│   ├── feature_engine/
│   │   └── src/indicators/
│   ├── strategy_engine/
│   │   └── src/{consumers, planner, risk, rules, cache, decision_log}
│   ├── execution_router/
│   │   └── src/{oms, exchange, store}
│   └── ai_predictor/
│       └── app/{inference, consumers, producers}
├── proto/
│   ├── ai/
│   ├── md/
│   ├── oms/
│   ├── control/
│   └── common/
├── db/
│   ├── clickhouse/
│   ├── postgres/
│   └── queries/
├── docs/
├── infra/
│   └── docker/
├── docker-compose.yml
├── Dockerfile.rust-workspace
└── secrets/   # gitignored
✅ Code Conventions (Non-negotiable)
Rust

tokio async runtime

rdkafka for Kafka

axum for HTTP services

sqlx for Postgres; SQL in db/queries/

Config from env via config.rs per service

/health endpoint in every service

tracing + OTLP instrumentation

Python (ai_predictor)

Dependencies in pyproject.toml

Config in app/config.py

Logging in app/logging.py

Metrics in app/metrics.py

Protobuf

All cross-service schemas in proto/

Regenerate Rust/Python bindings after proto changes

⚠️ Safety Rules for AI Agents

Never hardcode API keys or secrets — read via env vars / Docker secrets.

Never disable risk_guard without explicit user instruction.

Never execute real orders directly — always go signal → risk → executor.

Kill switch: Redis key risk:kill halts live order submission when set to "1".

Paper trader first for new strategies; do not default to live execution.

ClickHouse is append-only; mutable state belongs in compacted topics (e.g., TOPIC_SIGNAL_STATE).

Migrations are separated by DB:

ClickHouse DDL: db/clickhouse/

Postgres migrations: db/postgres/
Never mix. Never drop columns; prefer additive changes with defaults.

ClickHouse cannot roll back reliably; test DDL on local containers first.
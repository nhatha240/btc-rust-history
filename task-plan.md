# Auto-Trading System — Execution Plan (P0 → P1)

> Goal: A production-minded pipeline that reliably produces **order history** visible in a **web dashboard**.
> Scope P0: **OMS loop + DB persistence + Web dashboard** (ClickHouse deferred).
> Scope P1: Full market-data hot path + AI predictor + analytics.

---

## 0) Working Rules

### 0.1 Definition of Done (DoD)
A task is DONE only when:
- Repo changes committed (code/config/migration/docs)
- Verification steps executed and recorded
- Logs/metrics confirm correct behavior
- No crash-loop; compose stays stable

### 0.2 Conventions (P0)
- Topics: keep current `TOPIC_*` naming (fastest path)
- Idempotency keys:
  - `client_order_id` (unique per order)
  - `trace_id` (correlation across services)
- DB: TimescaleDB/Postgres is the **source of truth** for web order history
- ClickHouse: optional (not a dependency for core services in P0)

---

## 1) Milestone P0.0 — Dev Environment Up

### Objective
`docker compose up` runs stable; dependencies healthy.

### Tasks
#### T1 — Compose health baseline
**Input:** `docker-compose.yml`  
**Output:** stack up + healthy  
**Verify:**
- Redpanda ready: `curl -fsS http://localhost:9644/v1/status/ready`
- Postgres ready: `pg_isready -U trader -d db_trading`
- Redis ready: `redis-cli ping`
- Redpanda Console UI: `http://localhost:8080`

**Acceptance:**
- Dependencies healthy
- Core services not crash-looping for 2–5 minutes

#### T2 — Topics init verified
**Verify:**
- Topics exist:
  - `TOPIC_CANDLES_1M`
  - `TOPIC_FEATURE_STATE` (compact)
  - `TOPIC_SIGNALS`
  - `TOPIC_SIGNAL_STATE` (compact)
  - `TOPIC_ORDERS`
  - `TOPIC_ORDERS_APPROVED`
  - `TOPIC_FILLS`
  - `TOPIC_MC_SNAPSHOT`
- Retention:
  - orders/fills: 7 days
  - stream topics: 1h–24h as configured
- Compact:
  - `TOPIC_FEATURE_STATE`, `TOPIC_SIGNAL_STATE`

**Acceptance:**
- `rpk topic describe <topic>` shows expected configs
- No accidental extra topics from auto-create

---

## 2) Milestone P0.1 — DB Schema for Order History (Web-Ready)

### Objective
Postgres has schema to store:
- orders
- timeline events
- fills
- positions (optional but recommended)
- decision logs (optional but high ROI)

### Tasks
#### T3 — Create migrations for OMS tables
**Output:** SQL migrations applied on startup or via `migrate` command  
**Tables (minimum):**
- `orders`
- `order_events`
- `fills`
- `positions` (optional)
- `decision_logs` (optional)

**Constraints:**
- `orders.client_order_id` UNIQUE
- `order_events.order_id` FK
- fills unique key:
  - `exchange_trade_id` OR `(client_order_id, fill_seq)` UNIQUE

**Indexes:**
- orders: `(symbol, created_at desc)`, `(status, created_at desc)`
- events: `(order_id, recv_time asc)`
- fills: `(symbol, trade_time desc)`

**Acceptance:**
- `\dt` shows all tables
- `\d orders` confirms unique constraint and indexes

#### T4 — Seed sanity data (optional)
**Output:** 1 mock order row for quick UI smoke test  
**Acceptance:** API returns at least 1 order on list endpoint

---

## 3) Milestone P0.2 — OMS Loop Runs End-to-End (No Market Data Needed)

### Objective
Generate one order → risk approve/reject → fill → persist DB → show on web.

### P0 OMS Flow (recommended)
`signal_engine` → `TOPIC_ORDERS` → `risk_guard` → `TOPIC_ORDERS_APPROVED` → `paper_trader` → `TOPIC_FILLS` → `order_executor` → Postgres

> If you want live execution later, swap `paper_trader` with exchange execution. P0 uses paper mode.

### Tasks
#### T5 — risk_guard: deterministic decision + reason
**Input:** messages from `TOPIC_ORDERS`  
**Output:** approved/rejected messages to `TOPIC_ORDERS_APPROVED`  
**Rules (minimum):**
- Kill switch enabled => reject reason `KILL_SWITCH`
- Notional > limit => reject reason `LIMIT_NOTIONAL`
- Leverage > limit => reject reason `LIMIT_LEVERAGE`

**Acceptance:**
- Exactly one decision per input message (no duplication)
- Rejects include explicit reason
- Metrics: approvals/rejects counters by reason

#### T6 — paper_trader: simulate ACK/FILL
**Input:** approved orders  
**Output:** fill events to `TOPIC_FILLS`  
**Behavior:**
- Emit a deterministic fill:
  - preserve `client_order_id` and `trace_id`
  - include status `FILLED` (optional intermediate `ACK`)

**Acceptance:**
- Each approved order yields expected fill message(s)
- Restart does not create duplicates (or duplicates dedup downstream)

#### T7 — order_executor: persist OMS into Postgres
**Input:** fills/events  
**Output:** rows in DB (`orders`, `order_events`, `fills`, optional `positions`)  
**Requirements:**
- Upsert order by `client_order_id`
- Append `order_events` timeline in time order
- Insert fills idempotently

**Acceptance (hard):**
- Replaying same message does NOT create duplicate DB rows
- Order status transitions valid

#### T8 — E2E OMS test procedure documented
**Output:** Runbook steps to produce a test order and verify DB results  
**Acceptance:**
- Clear instructions:
  - produce order → observe approval → observe fill → query DB

---

## 4) Milestone P0.3 — Web/API MVP for Order History

### Objective
Web UI can:
- list orders
- show order detail timeline + fills

### Tasks
#### T9 — Web service exposes REST API (read-only first)
**Endpoints:**
- `GET /api/orders?symbol=&status=&from=&to=&limit=&cursor=`
- `GET /api/orders/{id}` => order + events + fills
- `GET /api/health`

**Acceptance:**
- Filters work (symbol/status/time)
- Pagination works
- Response includes `trace_id`, `client_order_id`, `status`

#### T10 — Dashboard UI pages (Next.js recommended)
**Pages:**
- `/orders` list + filter bar
- `/orders/[id]` detail: timeline + fills

**Acceptance:**
- Can navigate list → detail
- Timeline sorted ascending by time
- Fills table shown
- Load < 2s locally

#### T11 — Trace correlation visible in UI
**Acceptance:**
- `trace_id` visible on list and detail
- Copy-to-clipboard present

---

## 5) Milestone P0.4 — Hardening Minimum (Ops-Ready)

### Objective
Stable operation + debug capability + safety controls.

### Tasks
#### T12 — Kill switch end-to-end
**Input:** Redis key (e.g., `risk:kill`)  
**Acceptance:**
- When enabled: new orders rejected quickly with reason `KILL_SWITCH`
- Optional: reduce-only allowed (if implemented)

#### T13 — Health/readiness for core services
**Services:**
- `risk_guard`, `paper_trader`, `order_executor`, `web`  
**Acceptance:**
- readiness fails if MQ/DB/Redis unavailable
- liveness stays true if process alive

#### T14 — Metrics P0
**Minimum metrics:**
- orders processed count
- reject count by reason
- db write latency p95
- consumer throughput

**Acceptance:**
- Grafana/Prometheus shows live updates during E2E test

---

## 6) Verification Checklist (Run After “Done Coding”)

### 6.1 Build & test
- `cargo build --workspace --release`
- `cargo test --workspace`
- (optional) `cargo clippy --workspace -- -D warnings`

### 6.2 Compose smoke test
- `docker compose up -d`
- confirm no crash loops for 2–5 minutes

### 6.3 MQ config validation
- verify compact/retention/partitions
- verify partition keys (symbol vs account_id:symbol)

### 6.4 E2E OMS
- inject one test order
- confirm:
  - approval/reject exactly once
  - fill generated
  - DB rows exist and are not duplicated
  - web shows order + timeline + fills

### 6.5 Replay/idempotency
- restart consumers
- reprocess messages
- confirm no duplicate DB writes

### 6.6 Safety controls
- toggle kill switch
- confirm system blocks new orders quickly

### 6.7 Observability
- trace_id searchable across service logs
- metrics visible in Grafana

---

## 7) P1 Roadmap (After P0 Stable)

### P1.1 Market data WS hot path
- Add MarketData_Ingestor (WS raw trades/book)
- Feature engine incremental indicators
- Publish `md.features.live` (or equivalent)

### P1.2 AI predictor
- Subscribe live features
- Run inference (Torch/ONNX)
- Publish predictions with model_version + feature_version
- Strategy consumes predictions + features

### P1.3 Analytics (ClickHouse)
- Add persister writing candles/features snapshots to ClickHouse
- Add heavy dashboard queries (PnL breakdown, latency histograms)

---

## 8) Task Ownership & Tracking Template

Use this format for each task in PR/issue:

- **Task ID:** T#
- **Owner:** (name)
- **Dependencies:** (T#)
- **Changes:** (files)
- **Verification commands:** (exact commands)
- **Acceptance evidence:** (logs/screenshots/query results)
- **Status:** TODO / IN PROGRESS / DONE

---
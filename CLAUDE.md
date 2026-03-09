# CLAUDE.md — AI Skill File for `btc-rust-backend`

> This file is the authoritative reference for AI assistants (Claude, Gemini, Copilot, etc.) working in this repository.
> Read this before touching any code. It describes the system goal, architecture, conventions, and how to work safely.
>
> **Full design handbook:** `trading_system_design_handbook.md` — read for database design, strategy classification, feature engineering, risk patterns.
> **Agent contract:** `AGENTS.md` — read for service boundaries, data plane rules, storage policies, build roadmap.
> **Claude skill:** `.agents/skills/SKILL.md` — read for service map, build commands, common task guides.

---

## 🎯 Project Goal

An **automated cryptocurrency trading system** that:
1. Ingests real-time market data (candles) from Binance
2. Computes technical indicators and features
3. Runs signal generation logic (rule-based + AI)
4. Routes orders through a risk gate
5. Executes live orders on Binance **or** simulates via a paper trader
6. Stores everything for analytics and a web dashboard

---

## 🏗️ Architecture Overview

```
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
```

All inter-service communication goes through **Redpanda** (Kafka-compatible).

---

## 📦 Services

### Rust Binaries (compiled from a single Cargo workspace)

| Binary | Location | Role |
|---|---|---|
| `ingestion` | `services/marketdata_ingestor` | Streams Binance candles → Redpanda + ClickHouse |
| `feature_state` | `services/feature_engine` | Consumes candles, computes indicators, publishes feature vectors |
| `signal_engine` | `services/strategy_engine` | Consumes features, applies rules, emits trading signals |
| `risk_guard` | `services/execution_router` | Validates orders against limits, approves/rejects |
| `order_executor` | `services/execution_router` | Submits approved orders to Binance exchange |
| `paper_trader` | `services/execution_router` | Simulates fills without real orders |
| `web` | `apps/web_dashboard` | REST API + dashboard backend (Axum) |
| `mc_snapshot` | `services/strategy_engine` | Market-condition snapshot writer |

### Python Service

| Service | Location | Role |
|---|---|---|
| `ai_predictor` | `services/ai_predictor` | ML inference service; consumes features, publishes AI signals |

---

## 🗃️ Data Stores

| Store | Port | Usage |
|---|---|---|
| **Redpanda** (Kafka) | `9092` | All event streaming between services |
| **ClickHouse** | `8123` (HTTP), `9000` (native) | Time-series storage — candles, features, signals, fills |
| **PostgreSQL/TimescaleDB** | `5432` | Relational data — users, bots, orders, risk limits |
| **Redis** | `6379` | Hot state cache — signal state, kill-switch, position cache |

---

## 🔁 Kafka Topics

| Topic | Retention | Producer | Consumers |
|---|---|---|---|
| `TOPIC_CANDLES_1M` | 1 h | `ingestion` | `feature_state` |
| `TOPIC_FEATURE_STATE` | compacted | `feature_state` | `signal_engine`, `ai_predictor` |
| `TOPIC_SIGNALS` | 24 h | `signal_engine`, `ai_predictor` | `order_executor` |
| `TOPIC_SIGNAL_STATE` | compacted | `signal_engine` | Internal state rebuild |
| `TOPIC_MC_SNAPSHOT` | 24 h | `mc_snapshot` | `signal_engine` (market-regime context) |
| `TOPIC_ORDERS` (`orders.v1`) | 7 d | `order_executor` | `risk_guard` |
| `TOPIC_ORDERS_APPROVED` (`orders.approved.v1`) | 7 d | `risk_guard` | `order_executor`, `paper_trader` |
| `TOPIC_FILLS` (`fills.v1`) | 7 d | `order_executor`, `paper_trader` | ClickHouse sink, PostgreSQL writer |

Partition key: `symbol` for market topics; `account_id` for OMS topics.

---

## 📐 Signal Engine — Anti-Spam Config

The `signal_engine` has configurable anti-spam middleware (via env vars):

```
SIGNAL_EDGE_MODE         BAR_CLOSE        # Fire only on bar close
SIGNAL_STABLE_MS         2000             # Signal must be stable for N ms
SIGNAL_HYSTERESIS_TYPE   ATR              # Hysteresis method (ATR / FIXED)
SIGNAL_HYSTERESIS_MULT_ENTER  0.5        # Entry buffer multiplier
SIGNAL_HYSTERESIS_MULT_EXIT   0.2        # Exit buffer multiplier
SIGNAL_DEBOUNCE_MS       1000            # Debounce window
SIGNAL_COOLDOWN_MS       15000           # Cooldown after signal fires
SIGNAL_MAX_PER_MIN       5               # Rate limit
SIGNAL_THROTTLE_MODE     DROP            # DROP or QUEUE
```

---

## 🛡️ Risk Guard — Limits

Controlled via env vars on `risk_guard`:

```
LIMIT_NOTIONAL_PER_SYMBOL   5000     # Max USD notional per symbol
LIMIT_LEVERAGE              5        # Max leverage
KILL_SWITCH_KEY             risk:kill  # Redis key — set to 1 to halt all orders
```

---

## 🌐 Observability Stack

| Tool | Port | Purpose |
|---|---|---|
| OpenTelemetry Collector | `4317` (gRPC), `4318` (HTTP) | Trace collection |
| Prometheus | `9090` | Metrics scraping |
| Grafana | `3000` | Dashboards |
| Redpanda Console | `8080` | Kafka topic browser |
| Redpanda Connect | `8083` | Kafka connectors (sinks to ClickHouse) |

All Rust services export OTLP traces. Set `OTEL_EXPORTER_OTLP_ENDPOINT` + `OTEL_RESOURCE_ATTRIBUTES` per service.

---

## 📁 Repository Layout

```
btc-rust-backend/
├── apps/
│   └── web_dashboard/          # Axum-based REST API + dashboard
├── services/
│   ├── marketdata_ingestor/    # Binance → Kafka ingestion (Rust)
│   ├── feature_engine/         # Technical indicators (Rust)
│   │   └── src/indicators/     # ema.rs, rsi.rs, macd.rs, vwap.rs
│   ├── strategy_engine/        # Signal generation (Rust)
│   │   └── src/{consumers, planner, risk, rules, cache, decision_log}
│   ├── execution_router/       # Risk + order execution (Rust)
│   │   └── src/{oms, exchange, store}
│   └── ai_predictor/           # ML inference (Python/FastAPI)
│       └── app/{inference, consumers, producers}
├── proto/                      # Protobuf definitions
│   ├── ai/                     # AI signal proto
│   ├── md/                     # Market data proto
│   ├── oms/                    # Order/fill proto
│   ├── control/                # Control messages
│   └── common/                 # Shared types
├── db/
│   ├── clickhouse/
│   │   └── init.sql            # Full ClickHouse DDL (run once at bootstrap)
│   ├── postgres/
│   │   ├── 001_orders.sql      # orders + order_events tables
│   │   ├── 002_trades_positions.sql  # trades (hypertable) + positions
│   │   └── 003_decision_logs.sql     # decision_logs (hypertable)
│   └── queries/                # sqlx named query files
│       ├── orders.sql
│       ├── trades.sql
│       └── pnl.sql
├── docs/                       # Architecture docs
├── infra/
│   └── docker/
│       └── docker-compose.yml  # Canonical infra compose file
├── docker-compose.yml          # Full local stack (app + infra)
├── Dockerfile.rust-workspace   # Multi-binary Rust builder
└── secrets/                    # binance_key.txt, binance_secret.txt (gitignored)
```

---

## 🔧 Local Development

### Prerequisites
- Docker + Docker Compose
- Rust toolchain (`rustup`)
- Python 3.11+ (for `ai_predictor`)
- Binance API keys in `secrets/binance_key.txt` and `secrets/binance_secret.txt`

### Start the full stack
```bash
docker compose up -d
```

### Start only infrastructure (no app binaries)
```bash
docker compose up -d redpanda clickhouse postgres redis
```

### Build a specific Rust binary
```bash
cargo build -p ingestion
cargo build -p signal_engine
```

### Run a specific binary locally
```bash
KAFKA_BROKERS=localhost:9092 cargo run -p ingestion
```

### Run AI predictor locally
```bash
cd services/ai_predictor
pip install -e .
python -m app.main
```

---

## ✅ Code Conventions

### Rust
- Use `tokio` for async runtime across all services
- Use `rdkafka` for Kafka consumers/producers
- Use `axum` for HTTP services
- Use `sqlx` for PostgreSQL queries; queries live in `db/queries/`
- Use `clickhouse-rs` or HTTP client for ClickHouse writes
- Config always loaded from env vars via a `config.rs` module per service
- Health check endpoint at `GET /health` in every service
- Instrument with `tracing` crate; export via OTLP

### Python (`ai_predictor`)
- `pyproject.toml` defines dependencies
- Config via `app/config.py`
- Logging via `app/logging.py`
- Metrics via `app/metrics.py`

### Protobuf
- All cross-service message schemas are defined in `proto/`
- Generate with `protoc` or `prost` (Rust)

---

## ⚠️ Safety Rules for AI Assistants

1. **Never hardcode API keys or secrets** — always read from env vars or Docker secrets
2. **Never disable the `risk_guard`** without explicit user instruction
3. **Never execute real orders** directly — always go through signal → risk → executor pipeline
4. **The kill switch** `KILL_SWITCH_KEY=risk:kill` in Redis stops all orders — use it in emergencies
5. **Paper trader first** — when adding new strategies, test with `paper_trader` before `order_executor`
6. **ClickHouse is append-only** — use compacted Kafka topics (`TOPIC_SIGNAL_STATE`) for mutable state
7. **Migrations are separated by database** — ClickHouse DDL lives in `db/clickhouse/`, PostgreSQL migrations in `db/postgres/`. Never mix them. Never drop columns; always add columns with defaults.
8. **ClickHouse cannot roll back** — test DDL against a local container before applying to production

---

## 🗄️ Database Schema Reference

### ClickHouse (`db/clickhouse/init.sql`)

| Table | Engine | Key Columns | Purpose |
|---|---|---|---|
| `candles_1m_final` | MergeTree | `symbol, open_time` | Raw 1m OHLCV from Binance; source for all MVs |
| `candles_15m` | AggregatingMergeTree | `symbol, open_time` | Auto-aggregated by `mv_candles_15m` |
| `candles_1h` | AggregatingMergeTree | `symbol, open_time` | Auto-aggregated by `mv_candles_1h` |
| `candles_4h` | AggregatingMergeTree | `symbol, open_time` | Auto-aggregated by `mv_candles_4h` |
| `candles_1d` | AggregatingMergeTree | `symbol, open_time` | Auto-aggregated by `mv_candles_1d` |
| `feature_state` | MergeTree | `symbol, ts` | Computed indicators per bar: `ema_fast`, `ema_slow`, `rsi`, `macd`, `macd_signal`, `macd_hist`, `vwap` |
| `signals` | ReplacingMergeTree(ts) | `symbol, ts` | Trading signals: `side` (LONG/SHORT), `reason`, `price`, `confidence`, `model_version` |
| `mc_snapshot` | MergeTree | `symbol, ts` | Market cap + dominance snapshots |

> All timestamp columns are `DateTime64(3)` (millisecond precision).
> Partition by `toYYYYMM(open_time)` and `symbol` on candle tables.

**ClickHouse query pattern:**
```sql
-- Always qualify with database name
SELECT symbol, close FROM db_trading.candles_1m_final
WHERE symbol = 'BTCUSDT'
  AND open_time >= now() - INTERVAL 1 HOUR
ORDER BY open_time DESC
LIMIT 60;
```

### PostgreSQL / TimescaleDB (`db/postgres/`)

| Table | Type | Key Columns | Purpose |
|---|---|---|---|
| `orders` | Regular | `client_order_id` (UUID, unique), `exchange_order_id`, `account_id`, `symbol`, `side`, `type`, `status`, `qty`, `price`, `avg_price`, `filled_qty` | Live + historical orders |
| `order_events` | Regular | `order_id → orders(id)`, `event_type`, `event_time` | Append-only audit trail of every state transition |
| `trades` | Hypertable on `trade_time` | `trade_id`, `order_id → orders(id)`, `symbol`, `qty`, `price`, `realized_pnl`, `commission` | Confirmed fills; unique on `(trade_id, symbol)` |
| `positions` | Regular | `UNIQUE(account_id, symbol, side)`, `qty`, `entry_price`, `unrealized_pnl`, `realized_pnl` | Latest position snapshot per account/symbol/side (upserted) |
| `decision_logs` | Hypertable on `decided_at` | `trace_id`, `symbol`, `direction`, `action`, `block_reason`, `confidence`, `features JSONB` | Strategy decision audit log |

**ENUMs defined:**
- `order_side`: `BUY`, `SELL`
- `order_type`: `MARKET`, `LIMIT`, `STOP_MARKET`, `STOP_LIMIT`, `TAKE_PROFIT`, `TAKE_PROFIT_MARKET`, `TRAILING_STOP_MARKET`
- `order_status`: `NEW`, `PARTIALLY_FILLED`, `FILLED`, `CANCELED`, `REJECTED`, `EXPIRED`
- `time_in_force`: `GTC`, `IOC`, `FOK`, `GTX`
- `order_event_type`: `SUBMITTED`, `ACKNOWLEDGED`, `PARTIALLY_FILLED`, `FILLED`, `CANCELED`, `REJECTED`, `EXPIRED`, `REPLACE_REQUESTED`
- `position_side`: `LONG`, `SHORT`, `BOTH`
- `signal_direction`: `LONG`, `SHORT`, `HOLD`
- `decision_action`: `ENTER`, `EXIT`, `HOLD`, `BLOCKED`

---

## 🚀 Running Migrations

### ClickHouse (one-shot DDL)
```bash
# Via HTTP API (as done by clickhouse_init container)
curl -fsS "http://localhost:8123/?database=db_trading" \
     --data-binary @db/clickhouse/init.sql

# Or via clickhouse-client
clickhouse-client --host localhost --multiquery < db/clickhouse/init.sql
```

### PostgreSQL (sequential, order matters)
```bash
# Via psql — run in order
psql postgres://trader:traderpw@localhost:5432/db_trading \
     -f db/postgres/001_orders.sql \
     -f db/postgres/002_trades_positions.sql \
     -f db/postgres/003_decision_logs.sql

# Or via sqlx CLI (if using sqlx migrate)
DATABASE_URL=postgres://trader:traderpw@localhost:5432/db_trading \
    sqlx migrate run --source db/postgres
```

---

## 🧮 Feature Vector Structure

Fields published on `TOPIC_FEATURE_STATE` by `feature_state` service and stored in `db_trading.feature_state`:

| Field | Type | Description |
|---|---|---|
| `symbol` | string | Trading pair e.g. `BTCUSDT` |
| `ts` | timestamp (ms) | Bar close time |
| `ema_fast` | f64 | Fast EMA (short period) |
| `ema_slow` | f64 | Slow EMA (long period) |
| `rsi` | f64 | RSI 0–100 |
| `macd` | f64 | MACD line (ema_fast − ema_slow) |
| `macd_signal` | f64 | Signal line (EMA of MACD) |
| `macd_hist` | f64 | Histogram (macd − macd_signal) |
| `vwap` | f64 | Volume-weighted average price |

> **Indicators implemented:** `ema.rs`, `rsi.rs`, `macd.rs`, `vwap.rs`
> Bollinger Bands are **not** implemented — do not add references to them.

---

## 🔴 Redis Key Conventions

| Key | Type | Set by | Description |
|---|---|---|---|
| `risk:kill` | String (`"1"`) | Operator / API | Global kill-switch — halts all order submission when set |
| `signal:state:{symbol}` | String (JSON/proto) | `signal_engine` | Latest signal per symbol (mirrors `TOPIC_SIGNAL_STATE`) |
| `position:{account_id}:{symbol}` | Hash | `order_executor` | Hot position cache for risk checks |
| `instruments:binance:spot` | String (JSON) | External feed | Spot instrument list |
| `instruments:binance:um` | String (JSON) | External feed | USDT-margined futures instrument list |
| `instruments:binance:cm` | String (JSON) | External feed | Coin-margined futures instrument list |

---

## 🔧 Cargo Workspace

All Rust services live in a single Cargo workspace. Binary names map directly to docker-compose service names:

| Binary (`--bin`) | Cargo package | docker-compose service |
|---|---|---|
| `ingestion` | `marketdata_ingestor` | `ingestion` |
| `feature_state` | `feature_engine` | `feature_state` |
| `signal_engine` | `strategy_engine` | `signal_engine` |
| `mc_snapshot` | `strategy_engine` | `mc_snapshot` |
| `risk_guard` | `execution_router` | `risk_guard` |
| `order_executor` | `execution_router` | `order_executor` |
| `paper_trader` | `execution_router` | `paper_trader` |
| `web` | `web_dashboard` | `web` |

```bash
# Build all
cargo build --workspace

# Build one binary
cargo build --bin signal_engine

# Run with local infra
KAFKA_BROKERS=localhost:9092 \
CLICKHOUSE_HTTP_URL=http://localhost:8123 \
CLICKHOUSE_DB=db_trading \
REDIS_URL=redis://localhost:6379/0 \
  cargo run --bin feature_state
```

---

## ⚙️ ClickHouse Environment Variable Reference

Different services use different variable names for the same ClickHouse endpoint. This is a known inconsistency — follow the pattern already in each service:

| Service | Variable | Example value |
|---|---|---|
| `ingestion` | `CH_URL`, `CH_DB`, `CH_USER`, `CH_PASSWORD` | `http://clickhouse:8123`, `db_trading`, `default`, `""` |
| `feature_state` | `CLICKHOUSE_HTTP_URL`, `CLICKHOUSE_DB` | `http://clickhouse:8123`, `db_trading` |
| `signal_engine` | `CLICKHOUSE_HTTP_URL`, `CLICKHOUSE_DB` | `http://clickhouse:8123`, `db_trading` |
| `mc_snapshot` | `CLICKHOUSE_HTTP_URL`, `CLICKHOUSE_DB` | `http://clickhouse:8123`, `db_trading` |
| `web` | `CLICKHOUSE_DSN` | `http://clickhouse:8123` |

> When adding a new service that needs ClickHouse, use `CLICKHOUSE_HTTP_URL` + `CLICKHOUSE_DB` (the most common pattern).

---

## 🔑 Environment Variables (Critical)

| Variable | Used By | Description |
|---|---|---|
| `KAFKA_BROKERS` | All services | Redpanda broker address |
| `BINANCE_API_KEY_FILE` | ingestion, order_executor | Path to API key secret |
| `BINANCE_API_SECRET_FILE` | ingestion, order_executor | Path to API secret |
| `DATABASE_URL` | web, risk_guard, order_executor | PostgreSQL DSN |
| `CLICKHOUSE_HTTP_URL` | feature_state, signal_engine | ClickHouse HTTP endpoint |
| `REDIS_URL` | feature_state, signal_engine, risk_guard | Redis connection |
| `SYMBOLS` | ingestion | Comma-separated symbols e.g. `BTCUSDT,ETHUSDT` |
| `INTERVAL` | ingestion | Candle interval e.g. `1m` |
| `EXCHANGE` | order_executor | Exchange name (`binance`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | All services | OpenTelemetry collector endpoint |

---

## 📊 Key Domain Concepts

| Term | Meaning |
|---|---|
| **Candle** | OHLCV bar for a symbol at a given interval |
| **Feature vector** | Set of computed indicator values for a candle |
| **Signal** | BUY / SELL / CLOSE directive produced by strategy |
| **Signal state** | Last known signal per symbol (compacted topic) |
| **Risk gate** | Validates order size, notional, leverage limits |
| **Fill** | Confirmed trade execution from exchange |
| **Paper trade** | Simulated fill, no real money |
| **Kill switch** | Redis flag that halts all live order submission |

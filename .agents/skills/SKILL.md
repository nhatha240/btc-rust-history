---
name: btc-rust-trading-system
description: How to work with the btc-rust-history auto-trading platform — architecture, conventions, and handbook reference
---

# BTC Rust Trading System — Claude Skill

## Overview

This is a **production auto-trading platform** for cryptocurrency that ingests real-time market data from Binance, computes technical features, generates trading signals via rule-based and AI models, routes orders through risk checks, and executes on Binance or simulates via paper trader.

**Always read `trading_system_design_handbook.md` at the project root for authoritative design guidance.**

---

## Architecture Quick Reference

```text
Binance WS
    │
    ▼
[marketdata_ingestor]  ──candles + raw──▶  [feature_state]  ──features──▶  [signal_engine]
                                                                                   │
                                                                            TOPIC_SIGNALS
                                                                                   │
                                                             ┌─────────────────────┤
                                                             ▼                     ▼
                                                     [order_executor]      [paper_trader]
                                                             │
                                                         TOPIC_ORDERS
                                                             │
                                                             ▼
                                                       [risk_guard]
                                                             │
                                                    TOPIC_ORDERS_APPROVED
                                                             │
                                                             ▼
                                                   [order_executor → Binance]
```

All inter-service communication via **Redpanda (Kafka-compatible)**.

---

## Key Files to Read First

| File | Why |
|------|-----|
| `trading_system_design_handbook.md` | Full architecture, database design, strategy classification, feature engineering, risk patterns |
| `AGENTS.md` | Service boundaries, data plane rules, coding conventions, safety rules |
| `infra/docker/docker-compose.yml` | Service topology, env vars, ports |
| `Cargo.toml` | Workspace members and shared dependencies |
| `db/clickhouse/init.sql` | All ClickHouse tables and materialized views |
| `db/postgres/*.sql` | Postgres migrations (orders, trades, positions, risk, backtests, observability) |
| `proto/` | Cross-service Protobuf schemas |

---

## Service Map

### Rust Services (single Cargo workspace)

| Binary                | Crate Location                                  | Role                                          |
|----------------------|------------------------------------------------|------------------------------------------------|
| `marketdata_ingestor` | `services/marketdata_ingestor`                 | Raw market data + candle ingestion             |
| `feature_state`       | `services/feature_engine/feature_state`        | Compute indicators, publish feature vectors    |
| `signal_engine`       | `services/strategy_engine/signal_engine`       | Consume features, apply rules, emit signals    |
| `risk_guard`          | `services/execution_router/risk_guard`         | Validate orders against limits                 |
| `order_executor`      | `services/execution_router/order_executor`     | Submit approved orders to Binance              |
| `paper_trader`        | `services/execution_router/paper_trader`       | Simulate fills                                 |
| `execution_router`    | `services/execution_router`                    | Route orders                                   |
| `mc_snapshot`         | `services/strategy_engine/mc_snapshot`         | Market-condition snapshots                     |
| `api_gateway`         | `services/api_gateway`                         | REST API + control plane                       |
| `web`                 | `apps/web_dashboard`                           | Dashboard backend (Axum)                       |

### Python Service

| Service        | Location                | Role                              |
|----------------|-------------------------|-----------------------------------|
| `ai_predictor` | `services/ai_predictor` | ML inference for trading signals  |

---

## Data Stores

| Store                      | Port           | Usage                                    |
|---------------------------|----------------|------------------------------------------|
| **Redpanda** (Kafka)      | 9092, 29092    | Event streaming between services         |
| **ClickHouse**            | 8123, 19000    | Time-series analytics                    |
| **PostgreSQL/TimescaleDB**| 5432           | OMS + relational state                   |
| **Redis**                 | 6379           | Hot state (signals, kill-switch, cache)  |

---

## Shared Libraries

| Crate          | Path                        | Purpose                              |
|----------------|-----------------------------|---------------------------------------|
| `hft_proto`    | `libs/rust/hft_proto`       | Protobuf encode/decode                |
| `hft_common`   | `libs/rust/hft_common`      | Shared types, time utilities          |
| `hft_store`    | `libs/rust/hft_store`       | Database access helpers               |
| `hft_mq`       | `libs/rust/hft_mq`          | Kafka/Redpanda abstractions           |
| `hft_redis`    | `libs/rust/hft_redis`       | Redis access helpers                  |
| `hft_exchange` | `libs/rust/hft_exchange`    | Exchange API abstractions             |
| `hft_risk`     | `libs/rust/hft_risk`        | Risk check library                    |
| `common`       | `crates/common`             | Workspace common utilities            |

---

## Code Conventions

### Rust
- **Async runtime**: `tokio`
- **Kafka**: `rdkafka`
- **HTTP**: `axum`
- **Postgres**: `sqlx` (SQL in `db/queries/`)
- **ClickHouse**: `clickhouse` crate
- **Config**: from env via `config.rs` per service
- **Health**: every service exposes `GET /health` and `GET /ready`
- **Tracing**: `tracing` + OTLP instrumentation
- **Error handling**: explicit, never swallowed

### Building
```bash
# Check single service
cargo check -p marketdata_ingestor

# Build entire workspace
cargo build

# Docker compose
cd infra/docker && docker compose up -d --build
```

---

## Database Design Rules

### ClickHouse (append-only analytics)
- Tables in `db/clickhouse/init.sql`
- Use `MergeTree` family engines
- `PARTITION BY toYYYYMM(event_time)`
- `ORDER BY` for most common filter pattern
- Materialized views for candle aggregation (1m → 15m/1h/4h/1d)
- Never do row-level updates on hot data

### PostgreSQL (transactional state)
- Migrations in `db/postgres/*.sql` (numbered sequentially)
- Use TimescaleDB hypertables for high-volume event tables
- Domain prefix convention: `ref_*`, `ord_*`, `risk_*`, `strat_*`, `bt_*`
- **Source of truth** for orders, fills, positions, risk events

---

## Safety Rules (Non-Negotiable)

1. **Never hardcode API keys or secrets** — use env vars / Docker secrets
2. **Never disable risk_guard** without explicit user instruction
3. **Never execute real orders directly** — signal → risk → executor pipeline
4. **Kill switch**: Redis key `risk:kill` = "1" halts live orders
5. **Paper trader first** for new strategies
6. **Never drop columns** — additive schema changes only
7. **ClickHouse cannot roll back** — test DDL on local containers first
8. **Point-in-time correctness is mandatory** — never use future data in features

---

## Strategy Implementation Guide

### Supported Strategy Families (see handbook §4)
1. **Trend-following** — EMA cross, ADX filters, trailing exits
2. **Mean reversion** — RSI/z-score extremes, VWAP reversion
3. **Breakout** — Range compression → expansion, volume confirmation
4. **Momentum** — Relative strength ranking, cross-sectional
5. **Regime detection** — Route to appropriate strategy family

### Feature Categories (see handbook §5)
- Price action: `log_return`, `candle_body_ratio`, `close_location_value`
- Trend: `ema_dist`, `ema_spread`, `adx`, `hh_hl_score`
- Volatility: `atr`, `atr_pct`, `vol_percentile`, `bb_width`
- Volume: `volume_ratio`, `obv`, `buy_sell_volume_ratio`
- Momentum: `rsi`, `macd_hist`, `roc`
- Derivatives: `funding_rate`, `oi_delta`, `basis_pct`
- Regime: `trend_strength_score`, `vol_state_score`, `liquidity_score`

### Rule Engine Pattern (see handbook §6)
Every signal evaluation answers:
1. Am I **allowed** to trade right now?
2. Is there a **valid setup**?
3. What is the **entry quality**?
4. What is the **appropriate size** and risk?

---

## Common Tasks

### Adding a new service
1. Create directory under `services/`
2. Add to workspace `Cargo.toml` members
3. Create `src/main.rs` with health endpoint
4. Add to `infra/docker/docker-compose.yml`
5. Add Kafka topics if needed

### Adding a new indicator
1. Add computation in `services/feature_engine/`
2. Add column to `db/clickhouse/init.sql` → `feature_state` table
3. Add to Protobuf schema in `proto/`
4. Update feature_state publisher

### Adding a new Postgres migration
1. Create `db/postgres/NNN_description.sql` (next number)
2. Use `IF NOT EXISTS` / `IF EXISTS` for idempotency
3. Never drop columns — add with defaults

### Adding a new ClickHouse table
1. Add to `db/clickhouse/init.sql`
2. Use `CREATE TABLE IF NOT EXISTS db_trading.table_name`
3. Choose appropriate engine (MergeTree, AggregatingMergeTree, etc.)

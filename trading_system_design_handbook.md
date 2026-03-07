
# Production Trading System Design Handbook

**Purpose:** This document is a practical blueprint for designing and implementing a production-ready trading system that can evolve from bar-based systematic trading into event-driven, low-latency, and microstructure-aware execution. It is written for engineering implementation first, not for theory-first discussion.

**Primary outcomes of this handbook:**

- Design a database architecture that supports live trading, backtest, replay, analytics, and model research.
- Select and implement trading algorithms that are realistic to code and operate.
- Build a feature pipeline that supports rule engines, scoring engines, and ML filters.
- Convert professional trading principles into explicit rule logic.
- Provide a modular system design that can become a real trading engine.

---

## Table of Contents

1. [System Objectives and Engineering Principles](#1-system-objectives-and-engineering-principles)
2. [Reference Trading System Architecture](#2-reference-trading-system-architecture)
3. [Database and Storage Design](#3-database-and-storage-design)
4. [Trading Algorithm Research and Classification](#4-trading-algorithm-research-and-classification)
5. [Feature Engineering for Better Signal Quality](#5-feature-engineering-for-better-signal-quality)
6. [Codifying Professional Trading Rules into a Rule Engine](#6-codifying-professional-trading-rules-into-a-rule-engine)
7. [Algorithms You Can Build Yourself](#7-algorithms-you-can-build-yourself)
8. [Implementation Architecture and Code Modules](#8-implementation-architecture-and-code-modules)
9. [Backtest, Replay, Validation, and Monitoring](#9-backtest-replay-validation-and-monitoring)
10. [Recommended Build Roadmap](#10-recommended-build-roadmap)
11. [Common Failure Modes and Anti-Patterns](#11-common-failure-modes-and-anti-patterns)
12. [Appendix A: Example Table Definitions](#12-appendix-a-example-table-definitions)
13. [Appendix B: Example Rule and Strategy Interfaces](#13-appendix-b-example-rule-and-strategy-interfaces)
14. [Appendix C: Reference Materials](#14-appendix-c-reference-materials)

---

## 1. System Objectives and Engineering Principles

A trading system is not just a signal generator. It is an integrated decision-and-execution platform with at least six responsibilities:

1. **Ingest market data correctly**
2. **Compute features and context consistently**
3. **Generate trade decisions**
4. **Enforce risk and operational controls**
5. **Send, track, and reconcile orders**
6. **Persist everything needed for replay, audit, and iteration**

### 1.1 What this document assumes

This handbook assumes the system may need to support:

- Spot and derivative markets
- Single-venue or multi-venue execution
- Bar-based and event-driven strategies
- Research, backtest, paper trading, and live trading
- Feature-based and rule-based signal generation
- Future evolution into microstructure and order-flow strategies

### 1.2 Non-negotiable design principles

#### Principle 1: Separate the hot path from the analytical path
The code that reacts to incoming market data and sends orders should not depend on expensive joins, dashboard queries, or batch analytics.

- **Hot path:** ingest → normalize → compute critical state → generate decision → risk check → send order
- **Cold path:** long-horizon analytics, BI, model training, batch reporting

#### Principle 2: Prefer immutable event logs for critical history
Market data, order events, fills, risk breaches, and signal evaluations should be stored as append-only records whenever possible. Current state can be derived from immutable history.

#### Principle 3: Preserve point-in-time correctness
Every feature, signal, and backtest decision must only use information available up to that timestamp.

This matters for:
- feature store design
- strategy replay
- model training
- post-trade analytics
- debugging false positives

#### Principle 4: Design for replay from day one
If you cannot replay a market session and reconstruct:
- the market state,
- the strategy state,
- the risk state,
- the order/fill state,

then you do not yet have a reliable trading platform.

#### Principle 5: Risk is a first-class engine, not a post-processing step
Risk checks must exist:
- before order submission
- after fill
- at portfolio level
- at session/day/week level
- during degraded venue/network conditions

#### Principle 6: Keep strategy code pure where possible
A strategy should ideally consume a deterministic `MarketContext` and return a deterministic decision object. The smaller the mutable shared state, the easier it is to test and replay.

#### Principle 7: Research-live parity matters more than complexity
A simple system that behaves the same in backtest, replay, paper, and live is more valuable than a sophisticated model that only works in notebooks.

---

## 2. Reference Trading System Architecture

### 2.1 High-level architecture

```text
Exchange Feeds / Broker APIs
        |
        v
+-------------------------+
| Market Data Ingestion   |
| - WebSocket / FIX / UDP |
| - REST snapshots        |
| - Recovery / gaps       |
+-----------+-------------+
            |
            v
+---------------------------+
| Data Normalization Layer  |
| - unified symbols         |
| - event-time stamps       |
| - venue mapping           |
| - trade/book schemas      |
+-----------+---------------+
            |
            v
+---------------------------+        +-------------------------+
| Event Bus / Stream Log    | -----> | Raw Data Storage        |
| Kafka / Redpanda / NATS   |        | ClickHouse / Parquet    |
+-----------+---------------+        +-------------------------+
            |
            v
+---------------------------+
| Online State / Cache      |
| - latest book             |
| - latest bars             |
| - current features        |
| - open orders             |
| - open positions          |
+-----------+---------------+
            |
            v
+---------------------------+
| Feature Engine            |
| - online indicators       |
| - regime state            |
| - microstructure metrics  |
+-----------+---------------+
            |
            v
+---------------------------+
| Signal Engine             |
| - rule engine             |
| - scoring engine          |
| - model inference         |
+-----------+---------------+
            |
            v
+---------------------------+
| Risk Engine               |
| - pre-trade checks        |
| - exposure limits         |
| - drawdown controls       |
| - no-trade filters        |
+-----------+---------------+
            |
            v
+---------------------------+
| OMS / Execution Engine    |
| - order intent            |
| - child order logic       |
| - acks / rejects          |
| - fill reconciliation     |
+-----------+---------------+
            |
            v
+---------------------------+
| Broker / Exchange         |
+---------------------------+

Parallel persistence and analytics:
- PostgreSQL / Timescale for transactional state
- ClickHouse / Parquet for heavy time-series and analytics
- Redis / in-memory cache for current state
- Object storage for cold historical archives
```

### 2.2 Functional components

| Component | Purpose | Latency Sensitivity | Recommended Design |
|---|---|---:|---|
| Market data ingestion | Consume raw exchange feeds, reconnect, gap recovery | Very high | Async, event-driven, isolated process per venue |
| Normalizer | Convert venue-specific payloads into internal schemas | Very high | Deterministic pure transform layer |
| Online cache/state | Keep latest market snapshot and current trading state | Very high | In-memory + Redis fallback if needed |
| Feature engine | Compute incremental features | High | Rolling windows, incremental updates, avoid full recompute |
| Signal engine | Produce BUY/SELL/HOLD decisions or scores | High | Pure functions over current state |
| Risk engine | Hard pre-trade and post-trade controls | Very high | Independent service or library with fail-closed behavior |
| OMS / execution | Submit, amend, cancel, reconcile orders | Very high | Event-sourced order state machine |
| Persistence | Store all events/state for replay/audit | Medium | Separate OLTP and OLAP paths |
| Replay/backtest engine | Rebuild sessions and test strategy logic | Medium | Must reuse same signal and risk code paths |
| Analytics | Performance, slippage, feature analysis | Low | Columnar engine / notebooks / dashboards |

### 2.3 Event contracts you should standardize

At minimum, standardize these internal message types:

- `TradeEvent`
- `BookDeltaEvent`
- `BookSnapshotEvent`
- `BarClosedEvent`
- `FeatureVectorEvent`
- `SignalDecisionEvent`
- `RiskDecisionEvent`
- `OrderIntentEvent`
- `OrderEvent`
- `FillEvent`
- `PositionSnapshotEvent`
- `RiskBreachEvent`
- `StrategyLogEvent`

A trading platform fails when each service invents its own ad hoc payload format.

### 2.4 Suggested execution modes

| Mode | Purpose | Data Source | Order Handling | Should Use Same Signal/Risk Code? |
|---|---|---|---|---|
| Research | Feature exploration, notebooks | Historical | None | Yes |
| Backtest | Strategy validation | Historical replay | Simulated | Yes |
| Replay | Session reconstruction and debugging | Historical exact events | Simulated or audit-only | Yes |
| Paper | Dry-run against live market | Live | Virtual order book / broker sandbox | Yes |
| Live | Production execution | Live | Real orders | Yes |

The closer the code parity across these modes, the lower the model risk.

---

## 3. Database and Storage Design

### 3.1 Core design choice: do not force one database to do everything

A serious trading platform usually needs **polyglot persistence**.

### Recommended storage roles

| Data Type | Write Pattern | Read Pattern | Best-Fit Store | Why |
|---|---|---|---|---|
| Instruments, venues, configs | Low write, relational | Point lookups, joins | PostgreSQL | Strong constraints, FK support |
| Orders, fills, positions, risk state | Transactional | Point reads + audits | PostgreSQL | ACID and reliable state transitions |
| Bars, trades, ticks, features | Append-heavy time-series | Range scans, aggregations | ClickHouse or TimescaleDB | Efficient analytical queries |
| Raw order book deltas/snapshots | Very high append volume | Replay/range scans | ClickHouse + Parquet | High ingest and cheap cold storage |
| Latest market state | Constant update | Key-based read | Redis / in-memory | Low latency access |
| Streaming event journal | Ordered append | Streaming consumers / recovery | Kafka / Redpanda | Durable event distribution |
| Cold archives | Batch append | Offline replay / training | Parquet on S3/MinIO | Cheap and scalable |

### 3.2 Storage tiers

#### Hot tier
Used by live trading path:
- latest book
- latest bars
- current feature snapshot
- open orders
- open positions
- live exposure
- session limits

Typical implementation:
- process memory
- lock-free structures where needed
- Redis only if cross-process/shared access is required

#### Warm tier
Used by operations and nearline analytics:
- orders
- fills
- positions
- risk events
- strategy logs
- latest feature snapshots
- recent market aggregates

Typical implementation:
- PostgreSQL / TimescaleDB
- ClickHouse for nearline analytical range scans

#### Cold tier
Used for:
- historical backtest
- multi-month replay
- model training
- compliance archive

Typical implementation:
- Parquet partitioned by date/venue/instrument
- object storage, optionally with Iceberg/Delta catalog

### 3.3 Database domains

Break the schema into clear domains:

- `ref_*` reference metadata
- `md_*` market data
- `feat_*` features and feature store
- `sig_*` signals
- `ord_*` orders and execution
- `pos_*` positions and PnL
- `risk_*` risk limits and breaches
- `strat_*` strategies and logs
- `bt_*` backtest and replay metadata
- `ml_*` models and predictions

This makes ownership and migration cleaner.

---

### 3.4 Reference tables

#### `ref_venues`
Purpose:
- one row per exchange/broker/venue

Key fields:
- `venue_id` PK
- `venue_code` unique
- `venue_type` (`spot`, `perp`, `futures`, `options`, `broker`)
- `timezone`
- `status`
- connection metadata

Indexes:
- unique on `venue_code`

#### `ref_instruments`
Purpose:
- canonical instrument dictionary

Key fields:
- `instrument_id` PK
- `venue_id` FK
- `symbol_native`
- `symbol_canonical`
- `base_asset`
- `quote_asset`
- `instrument_type`
- `tick_size`
- `lot_size`
- `min_notional`
- `price_precision`
- `qty_precision`
- `contract_multiplier`
- `expiry`
- `is_active`

Indexes:
- unique on `(venue_id, symbol_native)`
- index on `(symbol_canonical, instrument_type)`

#### `ref_trading_sessions`
Purpose:
- session boundaries, special market hours, maintenance windows, settlement windows

Useful for:
- time-of-day features
- no-trade windows
- session-aware strategies

---

### 3.5 Market data tables

Market data storage should use **append-only tables** with event timestamps and ingestion timestamps.

#### `md_trades`
Purpose:
- raw trade prints from venue
- base source for trade flow, delta proxies, volume stats, VWAP

Key fields:
- `venue_id`
- `instrument_id`
- `event_time`
- `ingest_time`
- `trade_id`
- `price`
- `qty`
- `side_aggressor` if provided or inferred
- `is_block_trade` if available
- raw payload reference / checksum

Primary key recommendation:
- OLAP: no true PK needed; sort key matters more
- OLTP mirror (if any): `(venue_id, instrument_id, trade_id)` or dedupe hash

Partitioning:
- by day or month depending volume
- optionally sub-partition by `venue_id` or hash of `instrument_id`

Sort/order key:
- `(venue_id, instrument_id, event_time, trade_id)`

Indexes:
- analytical engine sort key
- skip index/projection on `event_time`
- optional projection on `(instrument_id, event_time)`

#### `md_ticks`
Purpose:
- top-of-book or best bid/ask stream if provided separately
- useful for mark price, spread, microprice, short-horizon execution logic

Key fields:
- `event_time`
- `bid_px`
- `bid_sz`
- `ask_px`
- `ask_sz`
- `mid_px`
- `spread_ticks`

Partitioning and sort:
- same pattern as `md_trades`

#### `md_order_book_l2_snapshots`
Purpose:
- periodic full depth snapshots for replay and state recovery

Key fields:
- `snapshot_id`
- `event_time`
- `venue_id`
- `instrument_id`
- `depth`
- `bids_json` or nested columns
- `asks_json` or nested columns

Design note:
- for analytics and replay at scale, prefer structured columns or flattened nested arrays over opaque JSON if possible.
- store snapshots every N seconds or every M deltas.

#### `md_order_book_l2_deltas`
Purpose:
- incremental book updates between snapshots

Key fields:
- `delta_id`
- `event_time`
- `sequence_no`
- `side`
- `price`
- `size`
- `action` (`upsert`, `delete`)
- `snapshot_anchor_id` if helpful

Design note:
- replay is much easier if snapshots + deltas are both stored
- never depend only on deltas without resync points

#### `md_order_book_l3_events` (optional)
Purpose:
- individual order-level events, queue modeling, true microstructure strategies

Store only if:
- the venue actually provides reliable order IDs or queue semantics
- the strategy needs queue-position logic, hidden liquidity detection, or matching-engine inference

This table is expensive. Do not store L3 by default if your system is bar-based or swing-based.

#### `md_ohlcv_bars`
Purpose:
- normalized bars for research and strategy triggers

Recommended fields:
- `bar_time`
- `timeframe`
- `open`
- `high`
- `low`
- `close`
- `volume`
- `quote_volume`
- `trade_count`
- `buy_volume`
- `sell_volume`
- `vwap`
- `is_final`

Partitioning:
- by month/day on `bar_time`
- sub-partition by `timeframe` if bar cardinality is large

Indexes / sort:
- `(instrument_id, timeframe, bar_time)`

#### `md_funding_rates`
Purpose:
- derivatives funding history

Fields:
- `instrument_id`
- `event_time`
- `funding_rate`
- `predicted_funding_rate`
- `funding_interval_hours`
- `mark_price`
- `index_price`

#### `md_open_interest`
Purpose:
- open interest history for perp/futures interpretation

Fields:
- `instrument_id`
- `event_time`
- `open_interest_contracts`
- `open_interest_notional`
- optional `oi_source`

#### `md_basis_spreads`
Purpose:
- spot-perp and future-spot premium/basis tracking

Fields:
- `spot_instrument_id`
- `derivative_instrument_id`
- `event_time`
- `spot_mid`
- `derivative_mid`
- `basis_abs`
- `basis_pct`
- `basis_annualized`

---

### 3.6 Feature store tables

A feature store for trading should serve both:
1. **online inference/rule evaluation**
2. **offline backtest/model training**

#### Feature store design principle
Store both:
- **wide, point-in-time snapshots** for fast reads
- **feature metadata/registry** for governance and reproducibility

#### `feat_registry`
Purpose:
- definition of every feature used anywhere in the platform

Fields:
- `feature_name` PK
- `version`
- `owner`
- `description`
- `formula`
- `source_tables`
- `timeframe`
- `lookback_window`
- `update_frequency`
- `null_policy`
- `normalization_policy`
- `status`

#### `feat_values_wide`
Purpose:
- one row per `(instrument_id, feature_time, feature_set_version)`
- low-latency retrieval of many features at once

Example columns:
- `ret_1`
- `ret_5`
- `ema_20_dist`
- `rsi_14`
- `atr_pct_14`
- `vol_ratio_20`
- `obi_top5`
- `funding_zscore`
- `oi_delta_1h`
- `regime_id`
- `feature_quality_score`

Key:
- `(instrument_id, timeframe, feature_time, feature_set_version)`

Use for:
- live scoring
- backtest with consistent feature sets

#### `feat_values_long` (optional)
Purpose:
- normalized long-format feature history for ad hoc exploration

Fields:
- `instrument_id`
- `feature_time`
- `timeframe`
- `feature_name`
- `feature_value`
- `feature_version`

Use when:
- feature count changes often
- research team needs exploratory flexibility

Trade-off:
- slower for production reads than a wide table

#### `feat_online_latest`
Purpose:
- latest feature snapshot only
- current-state lookup for live engine

Fields:
- `instrument_id`
- `timeframe`
- `feature_payload`
- `updated_at`

Implementation options:
- Redis hash
- Postgres JSONB table for small scale
- in-memory cache backed by periodic persistence

---

### 3.7 Strategy and signal tables

#### `strat_definitions`
Purpose:
- stable strategy identity and metadata

Fields:
- `strategy_id`
- `strategy_code`
- `owner`
- `description`
- `asset_universe`
- `mode` (`long_only`, `long_short`, `market_making`, etc.)
- `status`

#### `strat_versions`
Purpose:
- versioned strategy configs and parameters

Fields:
- `strategy_version_id`
- `strategy_id`
- `git_commit`
- `config_hash`
- `parameter_json`
- `feature_set_version`
- `risk_profile_id`
- `created_at`

#### `sig_signals`
Purpose:
- immutable record of every signal evaluation result

Fields:
- `signal_id`
- `strategy_version_id`
- `instrument_id`
- `event_time`
- `signal_type` (`BUY`, `SELL`, `HOLD`, `EXIT`, `REDUCE`, `ADD`)
- `side`
- `score`
- `confidence`
- `reason_codes`
- `feature_snapshot_ref`
- `market_context_ref`
- `is_actionable`
- `ttl_ms`

Indexes:
- `(instrument_id, event_time DESC)`
- `(strategy_version_id, event_time DESC)`
- `(is_actionable, event_time DESC)`

Design note:
Store **non-actionable signals too**. They are useful for false-positive analysis.

#### `sig_signal_components`
Purpose:
- decomposed factor-level scores

Fields:
- `signal_id`
- `component_name`
- `component_score`
- `weight`
- `pass_flag`
- `explanation`

Useful for:
- debugging scoring engines
- explainability
- feature importance inspection

---

### 3.8 Order, execution, and fill tables

Use an **event-sourced order model**: the canonical truth is the sequence of order events, not a mutable order row alone.

#### `ord_order_intents`
Purpose:
- records internal decision to place or amend an order before sending to venue

Fields:
- `order_intent_id`
- `signal_id`
- `strategy_version_id`
- `instrument_id`
- `intent_time`
- `side`
- `order_type`
- `time_in_force`
- `limit_price`
- `stop_price`
- `target_qty`
- `reduce_only`
- `post_only`
- `urgency`
- `expected_slippage_bps`
- `risk_decision_id`

Usefulness:
- decouples decision generation from transport/exchange adapter behavior

#### `ord_orders`
Purpose:
- current materialized state of each order

Fields:
- `order_id` PK
- `venue_id`
- `instrument_id`
- `client_order_id`
- `venue_order_id`
- `parent_order_id`
- `order_intent_id`
- `status`
- `side`
- `order_type`
- `time_in_force`
- `limit_price`
- `orig_qty`
- `leaves_qty`
- `cum_fill_qty`
- `avg_fill_price`
- `created_at`
- `updated_at`

Indexes:
- unique `(venue_id, client_order_id)`
- unique `(venue_id, venue_order_id)` when present
- `(instrument_id, status, created_at DESC)`

#### `ord_order_events`
Purpose:
- immutable order lifecycle events

Event types:
- `CREATED`
- `SENT`
- `ACKED`
- `PARTIALLY_FILLED`
- `FILLED`
- `CANCELED`
- `REJECTED`
- `EXPIRED`
- `REPLACED`
- `AMENDED`
- `CANCEL_REJECTED`

Fields:
- `order_event_id`
- `order_id`
- `event_time`
- `event_type`
- `status_before`
- `status_after`
- `payload_json`
- `venue_message_id`

Indexes:
- `(order_id, event_time)`
- `(event_time DESC)`

#### `ord_fills`
Purpose:
- fill-level execution truth

Fields:
- `fill_id`
- `order_id`
- `venue_trade_id`
- `instrument_id`
- `event_time`
- `fill_qty`
- `fill_price`
- `fee_amount`
- `fee_asset`
- `liquidity_flag` (`maker`, `taker`, `unknown`)
- `is_self_trade`
- `commission_bps`

Indexes:
- `(order_id, event_time)`
- `(instrument_id, event_time DESC)`

#### `ord_execution_metrics`
Purpose:
- per-order slippage and execution quality statistics

Fields:
- `order_id`
- `arrival_mid`
- `decision_mid`
- `fill_vwap`
- `implementation_shortfall_bps`
- `markout_1s_bps`
- `markout_10s_bps`
- `markout_60s_bps`
- `book_imbalance_at_send`
- `spread_ticks_at_send`

This table is critical if you care about execution quality and not just signal quality.

---

### 3.9 Position and portfolio tables

#### `pos_positions`
Purpose:
- current materialized position by strategy, instrument, and optionally account

Granularity options:
- account + strategy + instrument
- account + portfolio + instrument
- parent and child strategy views

Fields:
- `position_id`
- `account_id`
- `strategy_version_id`
- `instrument_id`
- `side`
- `net_qty`
- `avg_entry_price`
- `mark_price`
- `unrealized_pnl`
- `realized_pnl`
- `cost_basis`
- `last_updated_at`

Indexes:
- unique `(account_id, strategy_version_id, instrument_id)`

#### `pos_position_lots` (recommended)
Purpose:
- optional lot-level accounting for exact PnL decomposition

Fields:
- `lot_id`
- `position_id`
- `open_fill_id`
- `open_time`
- `remaining_qty`
- `entry_price`

Useful for:
- FIFO/LIFO tax logic
- partial close attribution
- advanced execution analysis

#### `pos_pnl_snapshots`
Purpose:
- periodic account/strategy/instrument equity history

Fields:
- `snapshot_time`
- `account_id`
- `strategy_version_id`
- `instrument_id` nullable for aggregated views
- `gross_exposure`
- `net_exposure`
- `realized_pnl`
- `unrealized_pnl`
- `equity`
- `drawdown_pct`

Use for:
- intraday controls
- day-end reports
- drawdown dashboards
- strategy health

#### `pos_performance_daily`
Purpose:
- daily rollup of performance

Fields:
- `trade_date`
- `account_id`
- `strategy_version_id`
- `gross_pnl`
- `net_pnl`
- `fees`
- `funding_cost`
- `turnover`
- `win_rate`
- `profit_factor`
- `max_intraday_drawdown`

---

### 3.10 Risk tables

#### `risk_limit_profiles`
Purpose:
- reusable risk configurations

Examples:
- max position notional
- max leverage
- max daily loss
- max weekly loss
- max order rate
- max correlated exposure
- max funding exposure
- allowed sessions
- no-trade windows
- spread threshold
- volatility threshold

#### `risk_limits`
Purpose:
- concrete limit rows at account, strategy, symbol, or portfolio scope

Fields:
- `risk_limit_id`
- `profile_id`
- `scope_type` (`account`, `strategy`, `instrument`, `portfolio`, `venue`)
- `scope_ref`
- `limit_name`
- `limit_value`
- `hard_or_soft`
- `enabled`
- `effective_from`
- `effective_to`

#### `risk_events`
Purpose:
- every risk check result and breach

Fields:
- `risk_event_id`
- `event_time`
- `check_type`
- `scope_type`
- `scope_ref`
- `severity`
- `pass_flag`
- `current_value`
- `limit_value`
- `action_taken`
- `related_order_id`
- `related_signal_id`

Do not only store failures. Store passes too when possible for auditability.

---

### 3.11 Strategy logs and observability tables

#### `strat_logs`
Purpose:
- explain why the engine did or did not trade

Fields:
- `log_id`
- `strategy_version_id`
- `instrument_id`
- `event_time`
- `log_level`
- `event_code`
- `message`
- `context_json`

Examples:
- `REGIME_REJECT`
- `RR_TOO_LOW`
- `SPREAD_TOO_WIDE`
- `BREAKOUT_QUALITY_FAIL`
- `TREND_FILTER_PASS`

#### `strat_health`
Purpose:
- heartbeat and strategy process health

Fields:
- `instance_id`
- `strategy_version_id`
- `reported_at`
- `cpu_pct`
- `mem_mb`
- `queue_lag_ms`
- `last_market_event_time`
- `last_signal_time`

---

### 3.12 Backtest and replay metadata tables

#### `bt_runs`
Purpose:
- one record per backtest run

Fields:
- `run_id`
- `strategy_version_id`
- `data_slice`
- `execution_model_version`
- `feature_set_version`
- `cost_model_version`
- `parameter_hash`
- `started_at`
- `finished_at`
- `metrics_json`

#### `bt_run_artifacts`
Purpose:
- links to generated reports, plots, trades, confusion matrices, feature importance, notebooks

#### `bt_replay_sessions`
Purpose:
- exact market session replay metadata

Fields:
- `replay_id`
- `venue_id`
- `date`
- `instrument_universe`
- `source_archive`
- `start_event_offset`
- `end_event_offset`

---

### 3.13 Partitioning and indexing strategy

### For PostgreSQL / Timescale transactional and time-series tables
Use:
- **range partitioning by date** for high-volume event tables
- **hash/list partitioning by venue or instrument family** only if range partitions become too large
- local indexes per partition
- retention and archiving policies

Best candidates for partitioning:
- `ord_order_events`
- `ord_fills`
- `pos_pnl_snapshots`
- `risk_events`
- `strat_logs`
- `md_ohlcv_bars` if kept in Timescale/Postgres

General rules:
- partition by **time first**
- only add second dimension if a single partition is still too large
- avoid too many tiny partitions

### For ClickHouse
Use:
- `PARTITION BY toYYYYMM(event_time)` or `toDate(event_time)` depending write volume and retention
- `ORDER BY (venue_id, instrument_id, event_time, sequence_no)` or similar
- projections/materialized views for alternative read patterns
- TTL to move older data to cheaper storage or delete after archive

General rules:
- choose `ORDER BY` for the most common filter pattern
- keep append-only
- pre-aggregate bars/materialized views if needed
- do not update hot market data row-by-row unless you explicitly choose an update-friendly engine

### 3.14 Real-time vs historical storage

#### Real-time storage goals
- current state lookup
- low-latency writes
- deterministic updates
- resilience across restarts

#### Historical storage goals
- cheap retention
- long range scans
- compression
- backtest/replay throughput
- auditability

### Recommended pattern
- live events go to stream log
- hot path updates in-memory state
- same events are persisted to OLTP/OLAP sinks
- end of day or rolling windows are archived to Parquet

### 3.15 Backtest, replay, analytics, and live trading optimization

| Use Case | What matters most | Storage Optimization |
|---|---|---|
| Live trading | latest state, low latency, deterministic access | in-memory cache + compact OLTP state |
| Backtest | sequential time-range reads, feature parity | Parquet/ClickHouse sorted by instrument/time |
| Replay | exact event ordering and resync ability | snapshots + deltas + stream offsets |
| Analytics | large aggregations, joins, rollups | ClickHouse columnar tables and materialized views |
| Post-trade execution analysis | fill/event joins, markout windows | order/fill tables + market data keyed by time |

### 3.16 Recommended database stack by maturity stage

#### Stage 1: Practical single-node system
- PostgreSQL
- TimescaleDB extension or plain Postgres for bars/features
- Redis
- Parquet files for archives

#### Stage 2: Research + live scaling
- PostgreSQL for orders/fills/positions/risk
- ClickHouse for trades, books, features, analytics
- Kafka/Redpanda for event distribution
- Redis for latest state
- MinIO/S3 for Parquet archives

#### Stage 3: Multi-venue, heavy analytics, microstructure
- PostgreSQL clustered/HA for transactional state
- ClickHouse cluster for raw market data and analytics
- Kafka/Redpanda for event sourcing
- object storage + Iceberg/Parquet lake
- dedicated replay service reading archived event streams

---

## 4. Trading Algorithm Research and Classification

The right framing is not “which strategy wins most often?” The right framing is:

> Which strategies have positive expectancy after costs under specific market regimes and can be implemented with data I actually have?

A robust platform should support multiple strategy families and a regime-aware selection layer.

### 4.1 Summary classification table

| Strategy Family | Core Idea | Required Data | Best Regime | Implementation Difficulty | Latency Needs |
|---|---|---|---|---|---|
| Trend-following | Ride persistent directional moves | OHLCV, trend filters | directional, expanding markets | Low to Medium | Low |
| Mean reversion | Fade temporary dislocations | OHLCV, VWAP, volatility | range-bound, mean-reverting | Low to Medium | Low |
| Breakout | Trade acceptance beyond established range | OHLCV, volume, vol compression | transition from compression to expansion | Medium | Low to Medium |
| Momentum | Continue recent strength/weakness | OHLCV, returns, relative strength | trending and rotation markets | Medium | Low |
| Market microstructure | Exploit book/trade short-horizon edge | ticks, L2/L3, execution feedback | liquid venues, short horizons | High | High |
| Volatility expansion/contraction | Trade transitions in volatility state | OHLCV, ATR, squeeze metrics | pre/post compression states | Medium | Low |
| Volume profile / VWAP | Trade around value, acceptance, auction logic | intraday volume, price distribution | intraday mean reversion or trend days | Medium | Low to Medium |
| Funding/basis/OI/order-flow | Use derivatives positioning context | funding, OI, basis, order flow | perp/futures dislocations | Medium to High | Low to Medium |
| Multi-timeframe confirmation | Align higher timeframe context with lower trigger | multi-timeframe bars/features | broad applicability | Medium | Low |
| Regime detection | Switch or disable strategies by state | features across trend/vol/liquidity | all markets | Medium | Low |

---

### 4.2 Trend-following

#### Core logic
Trend-following assumes price persistence. The system enters in the direction of an identified trend and exits when that trend weakens or invalidates.

#### Typical inputs
- OHLCV
- EMA/SMA slopes and spreads
- ADX / trend strength
- Donchian channel breakout levels
- session VWAP
- optional OI/funding confirmation for derivatives

#### Typical entry conditions
Examples:
- `EMA20 > EMA50 > EMA200`
- `close > EMA20`
- `ADX > 20`
- pullback into EMA20 or session VWAP followed by rejection candle
- breakout above N-bar high with acceptable spread and volume confirmation

#### Typical exit conditions
- close below trailing EMA
- break of prior swing low/high
- opposite signal
- time stop if no follow-through
- volatility spike against position

#### Suitable SL/TP patterns
- initial stop based on ATR or swing low/high
- partial scale-out at 1.5R or 2R
- trailing stop using ATR or moving average
- no fixed TP if strategy wants to capture long tails

#### Suitable regime
- directional, expansionary markets
- assets with persistent trend behavior
- sectors under strong narrative or macro flow

#### Advantages
- simple to code
- robust conceptually
- can catch outsized moves
- works on multiple timeframes

#### Weaknesses
- chopped up in range markets
- sensitive to delayed entries
- can give back open profits without strong exit logic

#### Easy-to-make mistakes
- trading trend signals during low-ADX chop
- entering too extended from moving average
- no market regime filter
- ignoring funding extremes in perp markets

#### Code implementation suitability
Very high. This should be one of the first strategies you implement.

#### Practical module split
- trend filter
- pullback detector
- breakout detector
- volatility guard
- trailing exit engine

---

### 4.3 Mean reversion

#### Core logic
Mean reversion assumes short-term dislocations revert toward fair value or equilibrium.

#### Typical fair-value anchors
- rolling mean
- EMA
- VWAP
- anchored VWAP
- value area high/low
- fair microprice for short horizons

#### Required inputs
- OHLCV
- volatility bands
- RSI / z-score
- VWAP distance
- regime filter
- optional order-flow slowdown or exhaustion clues

#### Typical entry conditions
Examples:
- z-score below `-2`
- RSI below 25 or 30
- close outside Bollinger lower band
- price far below intraday VWAP but no structural breakdown
- overshoot into support with absorption

#### Typical exit conditions
- mean reversion back to VWAP / EMA / value area
- signal normalization
- adverse continuation beyond stop
- time stop if bounce fails quickly

#### Suitable SL/TP patterns
- tight stop relative to expected snapback
- TP at mean or partial TP at half reversion + trail
- lower tolerance for delay than trend strategies

#### Suitable regime
- range, balanced auction, low directional persistence
- liquid assets with repeated overshoots

#### Advantages
- often better entry prices
- smaller stops possible
- high trade frequency possible

#### Weaknesses
- catastrophic if fading true trend continuation
- many “cheap gets cheaper” traps
- regime dependence is severe

#### Easy-to-make mistakes
- fading a breakout that is actually valid
- using mean-reversion logic during volatility expansion
- no event/news filter
- no trend strength rejection logic

#### Code implementation suitability
High. Good second baseline strategy after trend-following.

#### Practical module split
- equilibrium anchor module
- distance/z-score module
- oversold/overbought detector
- trend veto filter
- quick-exit module

---

### 4.4 Breakout

#### Core logic
Breakout strategies enter when price exits a well-defined range and shows evidence of acceptance beyond it.

#### Required inputs
- OHLCV
- local highs/lows or Donchian channels
- volatility compression metrics
- volume expansion
- spread and slippage conditions
- optional book pressure confirmation

#### Typical entry conditions
Examples:
- 20-bar range break
- Bollinger band squeeze or low ATR percentile before break
- close near candle high after break
- relative volume > threshold
- breakout bar not too extended relative to ATR

#### Typical exit conditions
- failed acceptance back inside range
- close back below breakout level
- opposite impulse
- trailing stop after range expansion

#### Suitable SL/TP patterns
- stop just inside broken range or ATR-based
- first target at range height projection
- trail once expansion confirms

#### Suitable regime
- compression to expansion transitions
- post-news directional continuation
- market open/session hand-off periods

#### Advantages
- can capture explosive moves
- clean rule definitions
- scalable from bar-based to microstructure-confirmed logic

#### Weaknesses
- fake breakouts frequent
- slippage can erase edge
- overtrading around obvious levels is common

#### Easy-to-make mistakes
- entering late after breakout candle exhaustion
- treating every level touch as breakout
- no retest/acceptance logic
- no breakout quality filter

#### Code implementation suitability
High, especially when paired with a quality filter and fake-break detector.

#### Practical module split
- level detector
- compression detector
- breakout validator
- quality score
- failure exit

---

### 4.5 Momentum

Momentum can mean:
1. **time-series momentum**: an asset continues moving in its recent direction
2. **cross-sectional momentum**: relatively stronger assets outperform weaker ones

#### Required inputs
- rolling returns
- relative strength rankings
- trend persistence metrics
- sector or correlated asset context
- volume and volatility filters

#### Typical entry conditions
- top-N relative strength universe selection
- positive return over medium lookback and positive short-term continuation
- price above moving average cluster
- momentum plus trend filter alignment

#### Typical exit conditions
- rank deterioration
- momentum roll-over
- volatility shock
- trailing stop or time-based rebalance

#### Best regime
- rotational markets
- trending markets with persistent leadership

#### Advantages
- can be turned into systematic ranking engine
- scalable across many symbols
- works well with portfolio construction

#### Weaknesses
- crashes during sudden reversals
- requires good universe filtering
- ranking noise if data quality is poor

#### Code implementation suitability
High for bar-based systems, medium for cross-sectional portfolio engines.

---

### 4.6 Market microstructure strategies

#### Core logic
Exploit short-horizon predictive signals in order flow, spread dynamics, queue imbalance, trade aggressor flow, and book resilience.

#### Required inputs
- tick data
- best bid/ask stream
- L2 or L3 order book
- aggressor trades
- cancellations, replenishment, spread
- latency measurements and fill model

#### Example signals
- top-of-book imbalance
- microprice deviation from mid
- aggressive buy burst with book thinning on ask
- repeated absorption at a level
- spread collapse with directional trade imbalance
- queue depletion leading short-term move

#### Typical entry conditions
- imbalance threshold crossed
- trade flow confirms
- spread acceptable
- queue risk acceptable
- no venue degradation

#### Typical exit conditions
- edge decay within seconds
- book flips
- spread widens
- fill risk exceeds expected edge
- inventory/risk cap

#### SL/TP patterns
- usually small and very tight
- often inventory/edge/time based rather than chart based

#### Best regime
- highly liquid instruments
- stable low-latency feed/execution
- venues where queue priority matters

#### Advantages
- potentially strong short-horizon alpha
- independent from medium-term chart patterns
- excellent for execution-aware systems

#### Weaknesses
- data and infrastructure intensive
- edge decays quickly
- backtest realism is hard
- queue modeling is non-trivial

#### Easy-to-make mistakes
- using candle data to simulate microstructure alpha
- ignoring latency and matching-engine behavior
- no partial fill model
- no realistic fees/slippage

#### Code implementation suitability
Medium to High difficulty. Do not start here unless you already have tick/L2/L3 infrastructure.

---

### 4.7 Volatility expansion / contraction

#### Core logic
Volatility is cyclical. Compression often precedes expansion; expansion often mean-reverts or transitions into trend.

#### Required inputs
- ATR and ATR percentile
- realized volatility
- Bollinger bandwidth
- true range expansion
- volume context
- session timing

#### Example entry styles
- enter on breakout after low-volatility squeeze
- fade late expansion after exhaustion if trend quality is weak
- switch stop width and sizing based on volatility state

#### Best regime
- transition zones
- session opens
- pre/post catalyst periods

#### Advantages
- useful both as a standalone strategy and as a strategy filter
- improves stop placement and position sizing

#### Weaknesses
- volatility increase does not guarantee direction
- needs directional confirmation to avoid noise

#### Code implementation suitability
Very high. Even if not standalone, use it as a filter.

---

### 4.8 Volume profile / VWAP-based logic

#### Core logic
Auction-market style interpretation:
- price around fair value rotates
- price away from fair value either rejects and reverts, or accepts and trends

#### Required inputs
- intraday trades/volume
- session VWAP
- anchored VWAP
- volume-at-price histogram / profile
- value area high/low, HVN/LVN
- session context

#### Typical entry conditions
- VWAP reclaim in trend direction
- rejection from value area boundary
- LVN breakout into price discovery
- pullback to anchored VWAP in directional session

#### Exits
- target opposing value area edge
- target VWAP reversion
- trail during acceptance outside value

#### Best regime
- intraday auction markets
- instruments with reliable volume distributions

#### Advantages
- strong market-structure intuition
- great combination with breakout and mean reversion
- more context-rich than plain RSI signals

#### Weaknesses
- requires intraday trade volume quality
- session boundaries and anchor choice matter
- easier to misuse on illiquid markets

#### Code implementation suitability
Medium. Straightforward once trade aggregation pipeline exists.

---

### 4.9 Funding / basis / open-interest / order-flow driven strategies

#### Core logic
In derivatives markets, positioning and carry matter. Price alone is not enough.

#### Required inputs
- funding rate
- predicted funding rate
- open interest
- long-short ratio if available
- spot/perp basis
- liquidation flow if available
- trade/order flow

#### Example patterns
- price up + OI up + moderate funding = continuation-friendly
- price up + OI up + funding extremely positive = squeeze/exhaustion risk
- price down + OI down = long liquidation unwinding
- perp premium dislocation reverting toward spot
- basis mean reversion or cash-and-carry style logic

#### Exits
- normalization of basis/funding
- OI regime change
- adverse price move beyond invalidation
- funding window risk passed

#### Best regime
- active perp/futures markets
- high retail leverage environments
- event-driven liquidations

#### Advantages
- adds non-price context
- useful as filter for chart-based strategies
- helps avoid crowded continuation entries

#### Weaknesses
- data availability varies by venue
- cross-venue alignment can be messy
- retail long-short ratio can be noisy

#### Code implementation suitability
Medium to High depending on data collection maturity.

---

### 4.10 Multi-timeframe confirmation

#### Core logic
Use higher timeframe for context, lower timeframe for trigger.

A common pattern:
- 4H trend says “only long”
- 15m says “pullback complete”
- 1m says “enter on reclaim / order flow confirm”

#### Required inputs
- multi-timeframe OHLCV/features
- consistent timestamp alignment
- strict point-in-time feature joins

#### Advantages
- reduces false signals
- prevents trading lower timeframe noise against higher timeframe trend

#### Weaknesses
- can delay entry
- feature alignment bugs are common
- too many filters can kill trade frequency

#### Code implementation suitability
High.

---

### 4.11 Regime detection

#### Core logic
Do not ask one strategy to solve every market condition. First classify the market state, then route to:
- trend strategy
- mean reversion strategy
- breakout strategy
- no-trade state

#### Example regime labels
- `TREND_UP`
- `TREND_DOWN`
- `RANGE`
- `VOL_COMPRESSION`
- `VOL_EXPANSION`
- `PANIC`
- `ILLIQUID`
- `HIGH_SPREAD_NO_TRADE`

#### Inputs
- trend strength
- volatility percentile
- spread/liquidity
- volume participation
- macro session/event context
- optional cross-asset leadership

#### Implementation styles
- threshold state machine
- hidden Markov model
- classifier
- scoring system

#### Why this matters
A strategy framework with regime detection is usually more robust than a single monolithic strategy with many exceptions.

---

## 5. Feature Engineering for Better Signal Quality

The purpose of features is not to create as many numbers as possible. The purpose is to represent market state in a way that improves decision quality, ranking quality, or risk filtering.

### 5.1 Feature engineering principles

1. **Every feature must have a trading meaning**
2. **Every feature must be point-in-time correct**
3. **Every feature must declare its lookback and update cadence**
4. **Prefer incremental computation over full-window recomputation in live engines**
5. **Use the same formula online and offline**
6. **Monitor missingness, staleness, and drift**
7. **Features should help either direction, timing, quality, or risk**

---

### 5.2 Price action features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `log_return_n` | `ln(close_t / close_t-n)` | directional impulse | all | momentum/trend ranking | vol filter | noisy on tiny windows |
| `range_n` | `high_n - low_n` or rolling range | expansion/compression | all | breakout setup | volume ratio | insensitive to direction |
| `candle_body_ratio` | `abs(close-open)/(high-low+eps)` | conviction inside candle | 1m to 4H | breakout/trend continuation | volume spike | single candles can mislead |
| `upper_wick_ratio` | `(high-max(open,close))/(range+eps)` | rejection from highs | all | trend exhaustion / fake breakout | RSI, OBI | depends on data resolution |
| `lower_wick_ratio` | `(min(open,close)-low)/(range+eps)` | rejection from lows | all | sweep/reversal detection | delta proxy | same caveat |
| `close_location_value` | `(close-low)/(high-low+eps)` | where close sits in range | intraday+ | follow-through quality | breakout logic | zero-range candles |
| `distance_from_high_n` | `(rolling_high_n-close)/close` | extension below resistance | 5m+ | breakout readiness | compression features | regime-dependent |
| `distance_from_low_n` | `(close-rolling_low_n)/close` | extension above support | 5m+ | pullback quality | trend filter | regime-dependent |

**Implementation note:** Price action features are best when paired with regime and volatility context. A strong bullish candle in a compression regime means something different than the same candle after a 4-sigma move.

---

### 5.3 Trend features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `ema_dist_k` | `(close-EMA_k)/EMA_k` | extension relative to trend anchor | all | pullback/trend-following | ATR%, RSI | can be too lagging alone |
| `ema_spread_fast_slow` | `(EMA_fast-EMA_slow)/EMA_slow` | trend structure strength | 5m+ | trend filter | ADX, volume | delayed on reversals |
| `ema_slope_k` | `EMA_k - EMA_k_prev` | directional slope | all | regime classification | spread/liquidity | slope unit needs normalization |
| `adx_n` | standard ADX | directional persistence strength | 15m+ | trend vs range filter | moving averages | late in some markets |
| `regression_slope_n` | OLS slope over lookback | trend line strength | 15m+ | momentum scoring | R² | sensitive to outliers |
| `hh_hl_score` | structural higher-high/higher-low logic | market structure confirmation | 5m+ | trend and pullback continuation | swing points | requires clean pivot logic |

**Use case:** Trend features are essential not only for trend strategies, but also for vetoing mean-reversion trades.

---

### 5.4 Volatility features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `atr_n` | Average True Range | stop width, noise floor | all | stop sizing | position sizing | price-scale dependent |
| `atr_pct_n` | `ATR_n / close` | normalized volatility | all | cross-asset comparison | leverage rules | spikes around gaps |
| `realized_vol_n` | std of log returns | actual recent volatility | 1m+ | regime state | breakout filters | scaling choices matter |
| `vol_percentile_n` | percentile of vol in rolling history | whether vol is extreme | 5m+ | compression/expansion classifier | session features | unstable under regime shift |
| `bb_width_n` | `(upper-lower)/middle` | squeeze measure | 5m+ | breakout prep | volume ratio | duplicate of vol features if overused |
| `parkinson_vol_n` | range-based estimator | intrabar volatility estimate | 5m+ | richer vol estimate | ATR | less useful with noisy highs/lows |

**Trading meaning:** Volatility features determine whether you should even trade, what stop width to use, and how much size is justified.

---

### 5.5 Volume features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `volume_ratio_n` | `volume / SMA(volume,n)` | participation surge | all | breakout confirmation | candle body ratio | distorted by session seasonality |
| `relative_volume_tod` | volume vs same time-of-day baseline | unusual activity for that clock time | intraday | session-aware trading | time-of-day features | requires seasonal baseline |
| `obv_n` | On-Balance Volume | cumulative pressure proxy | 15m+ | trend confirmation | price slope | noisy intraday |
| `buy_sell_volume_ratio` | `buy_volume / sell_volume` | directional pressure | 1m+ | intraday continuation | OBI | depends on aggressor classification quality |
| `volume_at_price_density` | volume histogram concentration | value acceptance | intraday | profile logic | VWAP/profile | requires trade-level aggregation |

---

### 5.6 Momentum features

| Feature | Formula / Logic | Trading Meaning | Good Timeframes | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `roc_n` | `(close/close_n)-1` | raw momentum | all | ranking, continuation | vol filter | noisy |
| `rsi_n` | standard RSI | exhaustion/impulse gauge | 5m+ | reversion or continuation depending regime | trend filter | regime-sensitive |
| `stoch_rsi` | RSI normalized within range | fast momentum oscillator | 1m+ | short-term timing | trend direction | whipsaws in chop |
| `macd_hist` | MACD histogram | acceleration/deceleration | 15m+ | trend strength and roll-over | EMA structure | lagging in fast markets |
| `momentum_persistence` | fraction of up bars in lookback | directional consistency | all | trend quality | range compression | simplistic alone |

---

### 5.7 Order book imbalance and microstructure features

These features require tick/L2/L3 data and much stricter implementation discipline.

| Feature | Formula / Logic | Trading Meaning | Useful Horizon | Combine With | Caveats |
|---|---|---|---|---|---|
| `obi_top_k` | `(sum(bid_sz_1..k)-sum(ask_sz_1..k))/total` | near-book pressure | milliseconds to seconds | trade aggressor flow | spoofing can distort |
| `microprice` | weighted mid using top bid/ask sizes | short-horizon fair value | sub-second to seconds | spread, trade burst | only useful in liquid books |
| `spread_ticks` | `(ask-bid)/tick_size` | transaction cost and fragility | all short horizons | urgency and execution | hard veto in wide spread |
| `queue_imbalance` | queue size imbalance at best levels | fill probability and direction | sub-second | passive execution logic | venue-specific semantics |
| `cancel_rate` | order cancellations per unit time | unstable liquidity/spoofing clues | seconds | OBI, spread | may be exchange-noise heavy |
| `book_replenishment_ratio` | replenishment after aggressive hits | absorption vs depletion | seconds | sweep detection | requires book event tracking |

**Critical warning:** book imbalance features can look predictive in historical data and fail live due to latency, hidden orders, queue priority, or stale-book effects.

---

### 5.8 Liquidity sweep, absorption, and spoofing-like clue features

These are not perfect truth labels. They are heuristics.

| Feature / Pattern | Logic | Trading Meaning | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|
| `liquidity_sweep_flag` | fast move through local high/low + aggressive volume burst + wick | stops likely triggered | breakout fade or continuation analysis | wick ratio, delta proxy | many false positives |
| `absorption_score` | repeated aggressive prints into a level with little price progress | passive side absorbing flow | reversal or breakout failure analysis | OBI, replenishment | requires trade+book alignment |
| `spoofing_like_score` | large visible size appears then disappears before trade | possibly deceptive liquidity | execution/risk filter | cancel rate, spread | do not over-interpret as intent |
| `iceberg_hint_score` | repeated fills at same level with resilient displayed size | hidden liquidity suspicion | breakout filter | replenishment ratio | inference only, not ground truth |

**Implementation note:** treat these as soft features, never as hard proof of manipulation.

---

### 5.9 Funding, open interest, long-short ratio, and basis features

| Feature | Formula / Logic | Trading Meaning | Timeframe | Useful When | Combine With | Caveats |
|---|---|---|---|---|---|---|
| `funding_rate_raw` | current/next funding rate | crowd carry pressure | 1h to 8h | perp context | trend, OI delta | venue-specific regimes |
| `funding_zscore` | standardized funding over rolling lookback | extreme positioning | 4h+ | squeeze risk filter | basis, price trend | non-stationary |
| `oi_delta_n` | change in open interest | build-up or flush of positioning | 5m+ | continuation vs liquidation interpretation | price return | venue aggregation issues |
| `price_oi_divergence` | compare price move and OI change | separates short covering from fresh positioning | 5m+ | derivatives context | funding | interpretation still heuristic |
| `long_short_ratio` | retail or account positioning ratio | crowding indicator | 1h+ | contrarian filters | funding, basis | often noisy |
| `basis_pct` | `(deriv_mid-spot_mid)/spot_mid` | premium/discount | minutes to daily | arbitrage/dislocation context | funding, OI | requires synced spot/deriv data |
| `basis_annualized` | normalized basis over time to expiry or funding interval | carry attractiveness | hourly+ | basis strategies | term structure | formula differs by product |

---

### 5.10 Time-of-day and session features

| Feature | Logic | Why It Matters | Use Cases | Caveats |
|---|---|---|---|---|
| `hour_sin`, `hour_cos` | cyclical encoding of time | avoids discontinuity at hour boundaries | models/classifiers | timezone correctness |
| `session_tag` | Asia / Europe / US / overlap | market behavior changes by session | strategy gating | crypto is 24/7 but still session-structured |
| `minutes_since_session_open` | clock distance from open | opening volatility effects | breakout/vol filters | define session consistently |
| `funding_window_proximity` | minutes to next funding | order flow distortion around funding | no-trade windows / filters | venue-specific funding times |
| `day_of_week` | categorical or cyclical | recurring seasonality | filters and analytics | weak standalone signal |

---

### 5.11 Cross-asset and market-context features

| Feature | Logic | Trading Meaning | Use Cases | Caveats |
|---|---|---|---|---|
| `btc_return_lead_n` | BTC recent return | altcoins often follow BTC | alt filters | may lag or decouple |
| `rolling_beta_to_btc` | beta estimate vs BTC | position sizing and hedging | portfolio risk | unstable in crises |
| `sector_strength_score` | average return of asset cluster | theme rotation | ranking | universe definition matters |
| `dominance_change` | BTC dominance / major index move | capital rotation context | alt strategies | external data needed |
| `correlation_regime` | rolling correlation matrix state | diversification and contagion risk | portfolio construction | fragile with short samples |

---

### 5.12 Regime classification features

| Feature | Logic | Purpose | Best Use |
|---|---|---|---|
| `trend_strength_score` | blend of EMA spread, slope, ADX | identify trend state | strategy selection |
| `vol_state_score` | ATR percentile + realized vol percentile | identify compression/expansion/panic | risk sizing and strategy routing |
| `liquidity_score` | spread, depth, trade rate | identify tradability | no-trade filters |
| `participation_score` | relative volume, trade count | detect active participation | breakout validation |
| `stability_score` | gap frequency, feed health, stale-book checks | operational state | execution safeguard |

---

### 5.13 Feature combinations that are especially useful

#### Combination A: Trend continuation quality
- `ema_spread_fast_slow`
- `ema_slope`
- `pullback_distance_to_ema`
- `volume_ratio`
- `atr_pct`
- `funding_zscore` as crowding filter

#### Combination B: Breakout quality
- `range_compression_score`
- `breakout_close_location`
- `volume_ratio`
- `relative_spread`
- `obi_top_k`
- `retest_acceptance_flag`

#### Combination C: Mean reversion quality
- `zscore_from_vwap`
- `rsi`
- `trend_strength_score` as veto
- `absorption_score`
- `vol_state_score`

#### Combination D: Liquidation / squeeze context
- `price_return`
- `oi_delta`
- `funding_zscore`
- `basis_pct`
- `trade_flow_imbalance`

---

### 5.14 Feature store design rules for real systems

- version every feature definition
- persist feature-generation metadata
- include `feature_time` and `available_time` if asynchronous sources exist
- use a shared feature library between research and live
- validate null rate and stale rate
- never compute training features using future-corrected or backfilled values if live would not have them
- store online feature snapshots used to make each live decision

---

## 6. Codifying Professional Trading Rules into a Rule Engine

This section translates common professional trading principles into explicit, machine-implementable rules. The idea is not to imitate specific personalities; the idea is to codify recurring rules that consistently appear in professional discretionary and systematic practice.

### 6.1 Rule engine design goals

A rule engine should answer four questions:

1. **Am I allowed to trade right now?**
2. **Is this setup valid?**
3. **How large can I trade?**
4. **When must I reduce, exit, or disable trading?**

### 6.2 Rule categories

- market context rules
- signal confirmation rules
- no-trade rules
- position sizing rules
- order placement rules
- add/reduce rules
- stop and take-profit rules
- drawdown and session loss rules
- portfolio correlation rules
- operational safety rules

---

### 6.3 Rule table: professional principle → code-ready rule

| Principle | Rule Logic | Example Condition | Action |
|---|---|---|---|
| Trade with context, not isolated candles | only trade if setup aligns with regime | `regime in {TREND_UP, VOL_EXPANSION_UP}` for longs | reject otherwise |
| Preserve capital first | no new risk after drawdown threshold | `daily_loss_r <= -3` | disable new entries for day |
| Do not trade noise | reject when spread/vol/liquidity poor | `spread_ticks > max_spread_ticks` or `liquidity_score < min` | reject |
| Require confirmation | entry only if trigger + context + risk pass | `setup.pass && confirm.pass && risk.pass` | allow |
| Never widen stop | stop may tighten, not loosen | `new_stop <= old_stop` for longs only if tighter | reject wider stop |
| Add only to winners | pyramiding only after favorable move and improved stop | `unrealized_r >= 1 && regime_supportive` | allow add |
| Cut losers quickly | immediate exit when invalidation occurs | `mark_price <= stop_price` or `breakout_failed` | exit |
| Take asymmetric trades | require minimum reward/risk or edge | `expected_rr >= min_rr` | reject low-R trades |
| Avoid revenge trading | cap trade count after losses | `consecutive_losses >= max` | cool-down |
| Protect weekly capital | aggregate weekly risk shutdown | `weekly_loss_r <= -8` | disable entries |
| Avoid correlated overexposure | sum risk across cluster | `cluster_gross_exposure > limit` | reject new correlated trade |
| Do not trade stale data | market feed freshness check | `now - last_market_event_ms > threshold` | reject/cancel |
| No trading during degraded exchange state | venue health required | `venue_health != OK` | reject |
| Limit churn | minimum spacing between entries | `now - last_entry_time < cooldown_ms` | reject |

---

### 6.4 Concrete code-oriented rule logic

#### Context rule
```text
IF regime not in allowed_regimes(strategy, side)
THEN reject signal
```

#### Signal confirmation rule
```text
IF setup_score < min_setup_score
OR confirmation_score < min_confirmation_score
THEN reject signal
```

#### Risk-reward rule
```text
expected_reward = target_price - entry_price
risk = entry_price - stop_price
IF expected_reward / risk < min_rr
THEN reject signal
```

#### Daily loss shutdown
```text
IF realized_pnl_today + unrealized_pnl_open_positions <= -daily_loss_limit
THEN disable new entries until next session
```

#### Max exposure rule
```text
IF current_gross_exposure + proposed_exposure > max_gross_exposure
THEN reject signal
```

#### Correlation cluster rule
```text
IF proposed_trade.cluster == existing_position.cluster
AND cluster_risk + proposed_risk > cluster_risk_limit
THEN reject signal
```

#### Spread sanity rule
```text
IF spread_ticks > max_spread_ticks_for_strategy
THEN reject signal
```

#### Volatility sanity rule
```text
IF atr_pct > max_atr_pct
OR vol_state == PANIC
THEN reject signal
```

#### Trade frequency rule
```text
IF trades_today(strategy, instrument) >= max_trades_per_day
THEN reject signal
```

#### No-stale-data rule
```text
IF current_time - last_feature_update_time > feature_stale_threshold_ms
THEN reject signal
```

---

### 6.5 Rules for adding, reducing, and exiting positions

#### Add-to-winner rule
Allow adds only if all are true:
- current position is profitable
- regime still valid
- stop can be tightened or held without increasing total portfolio risk beyond limit
- new add is not chasing parabolic extension
- cumulative risk remains within allowed cap

#### Reduce-position rule
Reduce when:
- target 1 hit
- volatility spikes against position
- order flow reverses
- basis/funding context deteriorates
- higher timeframe invalidation appears
- portfolio needs de-risking

#### Stop-loss rule
A stop should be derived from one of:
- structure invalidation
- ATR multiple
- volatility-adjusted value area failure
- microstructure edge decay
- time-based invalidation

Bad stop logic:
- arbitrary wide stop to “avoid being wicked”
- stop widened after entry without new thesis

#### Take-profit rule
Common valid implementations:
- fixed R multiple
- opposing structure
- reversion to fair value
- scale-out plus trailing remainder
- dynamic exit when score decays below threshold

---

### 6.6 Conditions where entry must be forbidden

A production engine should explicitly define `NO_TRADE` conditions. Examples:

- spread too wide
- book too thin
- stale data
- venue reconnecting / sequence gap unresolved
- within maintenance or funding window
- daily or weekly risk shutdown
- strategy health degraded
- too many open positions
- low-quality breakout score
- mean-reversion attempt against strong trend regime
- event/news blackout if strategy is not event-driven
- feature missingness exceeds threshold

---

### 6.7 Example rule-engine evaluation order

```text
1. Operational health checks
2. Market data freshness checks
3. Venue tradability checks
4. Regime checks
5. Strategy-specific setup checks
6. Signal confirmation checks
7. Risk-reward checks
8. Position sizing checks
9. Portfolio / correlation checks
10. Final order intent creation
```

This order reduces wasted computation and keeps the reasoning trace clean.

---

## 7. Algorithms You Can Build Yourself

This section focuses on modular trading algorithms that are worth building from scratch because they create reusable infrastructure and do not lock you into one brittle strategy.

### 7.1 Signal scoring engine

#### Core idea
Instead of binary indicator logic, compute a weighted score representing trade quality.

#### Inputs
- trend features
- volatility features
- volume features
- regime label
- execution cost estimates

#### Output
- `score`
- `confidence`
- `side`
- `reason_codes`

#### General logic
```text
score = w1*trend + w2*momentum + w3*breakout_quality
      + w4*volume_confirmation + w5*regime_alignment
      - w6*execution_cost_penalty - w7*crowding_penalty
```

#### Module split
- feature normalization
- factor scoring
- weighting
- confidence calibration
- thresholding
- explanation generator

#### Difficulty
Low to Medium

#### Mandatory data
OHLCV minimum, better with funding/OI and spread metrics

#### Best simple first version
Start with 5-8 hand-engineered components and fixed weights. Do not start with ML weighting.

---

### 7.2 Weighted multi-factor model

#### Core idea
Combine multiple orthogonal factors:
- trend
- mean reversion distance
- volatility state
- volume participation
- market context

#### Inputs
- feature vector at decision time
- per-factor weights
- optional side-specific rules

#### Output
- long score
- short score
- hold score

#### General logic
- compute each factor on normalized scale, e.g. `[-1, 1]` or `[0, 1]`
- aggregate by weighted sum
- pass through gating rules
- map to action and size bucket

#### Module split
- factor calculators
- factor registry
- score composer
- side resolver
- threshold config

#### Difficulty
Medium

#### Mandatory data
OHLCV plus context features

#### Best simple first version
Use 4 factors and static weights, with regime gating.

---

### 7.3 Regime-aware strategy selector

#### Core idea
First classify market regime, then route traffic to the strategy best suited for that regime.

#### Inputs
- trend strength
- volatility state
- spread/liquidity
- volume participation
- optional derivatives context

#### Output
- `selected_strategy_family`
- `allowed_sides`
- `risk_mode`

#### General logic
```text
if illiquid or stale -> NO_TRADE
elif trend_strength high and vol stable -> TREND
elif low trend and balanced auction -> MEAN_REVERSION
elif compression then breakout conditions -> BREAKOUT
elif panic -> REDUCED_RISK or NO_TRADE
```

#### Module split
- regime feature builder
- regime classifier
- strategy router
- fallback/no-trade policy

#### Difficulty
Medium

#### Mandatory data
OHLCV + spread/liquidity + vol state

#### Best simple first version
Use a hand-tuned state machine before ML classification.

---

### 7.4 Order flow confirmation engine

#### Core idea
Use short-horizon trade/book behavior to confirm or reject a higher-level setup.

#### Inputs
- aggressor trade flow
- book imbalance
- spread
- replenishment / depletion
- microprice trend

#### Output
- `pass: boolean`
- `score`
- `reason`

#### Example uses
- only take breakout if order flow confirms
- only fade sweep if absorption is detected
- veto mean reversion if aggressive continuation persists

#### Module split
- order flow window aggregator
- imbalance calculator
- sweep/absorption detector
- execution cost estimator

#### Difficulty
High

#### Mandatory data
Ticks + L2 at minimum

#### Best simple first version
Use only:
- top-5 imbalance
- aggressor flow delta
- spread filter

---

### 7.5 Breakout quality filter

#### Core idea
Not all breakouts are equal. Score breakout quality before allowing entry.

#### Inputs
- range compression score
- breakout candle structure
- close location
- relative volume
- spread
- distance from level
- retest acceptance

#### Output
- breakout quality score
- allow/reject

#### Logic
High-quality breakout characteristics:
- break after compression
- strong close near extreme
- above-normal participation
- acceptable spread/slippage
- not already too extended from stop anchor

#### Module split
- level detector
- compression scorer
- participation scorer
- structural validator
- late-entry penalty

#### Difficulty
Medium

#### Mandatory data
OHLCV minimum, preferably volume and tick spread

#### Best simple first version
Score 5 components equally and require a threshold.

---

### 7.6 Fake breakout detector

#### Core idea
Detect breakouts likely to fail quickly.

#### Inputs
- breakout failure back into range
- long upper/lower wick
- weak close
- low relative volume
- adverse order flow after break
- mean reversion regime

#### Output
- `fake_breakout_probability` or rule-based flag

#### Module split
- failed acceptance detector
- wick/rejection detector
- volume insufficiency detector
- reversal-flow detector

#### Difficulty
Medium to High

#### Mandatory data
OHLCV and, for better quality, order flow

#### Best simple first version
Use:
- close back inside range within N bars
- low volume
- wick ratio
- ADX low or range regime

---

### 7.7 Trend exhaustion detector

#### Core idea
Identify when a trend is still up/down in price but losing quality.

#### Inputs
- momentum deceleration
- volatility spike
- distance from EMA/VWAP
- divergence (price vs RSI/MACD/OI)
- rejection candles
- funding extremes

#### Output
- `exhaustion_score`
- reduce/exit/veto-new-entry recommendation

#### Module split
- extension calculator
- momentum-decay calculator
- divergence detector
- crowding detector
- exit advisor

#### Difficulty
Medium

#### Mandatory data
OHLCV; funding/OI improve quality

#### Best simple first version
Use:
- EMA distance z-score
- RSI divergence or MACD roll-over
- wick rejection
- funding extreme filter

---

### 7.8 Volatility state classifier

#### Core idea
Classify market into states such as:
- compressed
- normal
- expanding
- panic

#### Inputs
- ATR percentile
- realized vol percentile
- range expansion
- spread regime
- gap / jump frequency

#### Output
- `vol_state`
- `recommended_size_multiplier`
- `strategy_permissions`

#### Difficulty
Low to Medium

#### Mandatory data
OHLCV and spread if intraday

#### Best simple first version
Threshold classifier with 4 states.

---

### 7.9 Entry confidence ranking

#### Core idea
If many signals fire at once, rank them and only take the best.

#### Inputs
- signal score
- expected RR
- spread/slippage penalty
- regime alignment
- cross-asset crowding / correlation

#### Output
- ranked candidate list
- allocate top K only

#### Why useful
This improves capital efficiency and reduces low-quality marginal trades.

#### Difficulty
Low

#### Mandatory data
Whatever feeds the signal engine plus cost estimates

#### Best simple first version
Weighted rank of `signal_score`, `RR`, and `cost_penalty`.

---

### 7.10 Adaptive TP/SL engine

#### Core idea
Target and stop distances should respond to regime and signal quality, not be fixed constants.

#### Inputs
- ATR%
- regime
- structure levels
- liquidity/spread
- confidence score

#### Output
- stop price
- target ladder
- trailing rules
- time stop

#### Example logic
- trend regime: wider stop, looser trail, larger target
- mean reversion: tighter stop, target at equilibrium
- panic regime: smaller size, wider stop only if expected edge justifies

#### Module split
- structure stop engine
- ATR stop engine
- target engine
- trailing engine
- time decay engine

#### Difficulty
Medium

#### Mandatory data
OHLCV at minimum

#### Best simple first version
Choose between:
- ATR stop
- structure stop
- VWAP target
based on regime label.

---

### 7.11 Position sizing engine by volatility and confidence

#### Core idea
Size should depend on both risk per trade and expected quality.

#### Inputs
- account equity
- max risk budget
- stop distance
- instrument volatility
- signal confidence
- correlation exposure
- leverage constraints

#### Output
- target size
- leverage
- allowed notional
- reject/approve flag

#### Example logic
```text
base_risk = equity * risk_fraction
raw_size = base_risk / stop_distance
vol_adjusted_size = raw_size * vol_multiplier
confidence_adjusted_size = vol_adjusted_size * confidence_multiplier
final_size = min(vol_adjusted_size, portfolio_limits, venue_limits)
```

#### Difficulty
Low to Medium

#### Mandatory data
equity, stop distance, volatility, venue rules

#### Best simple first version
Volatility-normalized fixed-fractional sizing.

---

### 7.12 Strategy ensemble

#### Core idea
Run several strategy families and combine them under allocation and conflict rules.

#### Inputs
- scores from multiple strategies
- regime label
- portfolio exposure
- capital constraints

#### Output
- final side
- final size
- selected strategies or weighted blend

#### Example policies
- regime selects one primary strategy family
- ensemble takes consensus of strategies
- portfolio layer allocates weights by recent stability
- conflicting signals cancel or reduce

#### Difficulty
Medium to High

#### Mandatory data
same as all child strategies

#### Best simple first version
Two-strategy ensemble:
- trend-following
- mean reversion
with regime routing and common risk engine.

---

## 8. Implementation Architecture and Code Modules

### 8.1 Language split recommendation

Choose language by latency sensitivity and system role.

| Layer | Good Language Choices | Notes |
|---|---|---|
| Research, feature prototyping, model training | Python | Fastest iteration, rich numeric stack |
| Control plane, APIs, dashboards, orchestration | Node.js / TypeScript, Go | Strong productivity and service tooling |
| Hot path execution, book processing, microstructure | Rust / C++ / Java | Better for tight latency budgets |
| SQL/OLAP transforms | SQL + Python | Use db-native materialization where possible |

**Practical recommendation:**  
If you are not yet doing true sub-millisecond microstructure trading, a Python research stack plus TypeScript/Node.js service layer is perfectly workable.  
If you move into L2/L3 short-horizon execution, move the hot path into Rust/C++/Java and keep Node.js as control plane.

### 8.2 Recommended service boundaries

```text
apps/
  market-ingest/
  market-normalizer/
  feature-engine/
  signal-engine/
  risk-engine/
  execution-engine/
  reconciliation-engine/
  replay-engine/
  analytics-api/
  config-service/

packages/
  domain-models/
  db-access/
  indicators/
  feature-library/
  strategy-sdk/
  risk-sdk/
  execution-sdk/
  event-contracts/
  common-utils/
```

### 8.3 Domain objects you should formalize

At minimum:

- `MarketContext`
- `FeatureSnapshot`
- `SignalDecision`
- `RiskDecision`
- `OrderIntent`
- `OrderState`
- `Fill`
- `PositionState`
- `StrategyLog`
- `ReplayContext`

### 8.4 Design your strategy interface for testability

Recommended idea:
- input is immutable context
- output is structured decision
- strategy should not send orders directly

#### Example decision contract
```ts
export interface DetectorResult {
  pass: boolean;
  reason: string;
  note?: string;
  score?: number;
}

export interface SignalDecision {
  strategyId: string;
  strategyVersion: string;
  instrumentId: string;
  eventTime: number;
  side: 'LONG' | 'SHORT' | 'HOLD' | 'EXIT' | 'REDUCE';
  score: number;
  confidence: number;
  actionable: boolean;
  reasonCodes: string[];
  stopPrice?: number;
  targetPrice?: number;
  metadata?: Record<string, unknown>;
}
```

### 8.5 Suggested market data processing pipeline

```text
raw exchange payload
  -> schema validate
  -> map symbols and venue fields
  -> assign event_time and ingest_time
  -> update latest in-memory state
  -> emit canonical event
  -> persist raw and canonical versions if needed
  -> trigger incremental feature updates
```

### 8.6 Incremental feature engine rules

To keep live latency under control:

- maintain rolling windows in memory
- update only features touched by new event
- avoid full-dataframe recomputation in live path
- precompute higher-timeframe bars incrementally
- snapshot live feature state periodically for recovery

### 8.7 Execution engine responsibilities

The execution engine is not “just place order”.

It must:
- translate `OrderIntent` into venue-specific order request
- generate deterministic `client_order_id`
- handle retries carefully
- track acknowledgements and rejects
- manage amend/cancel state machine
- reconcile fills and fees
- emit order events
- support kill switch behavior

### 8.8 Risk engine responsibilities

The risk engine should expose functions like:
- `checkSignalRisk`
- `checkOrderRisk`
- `checkPortfolioRisk`
- `checkOperationalRisk`
- `computePositionSize`
- `shouldDisableTrading`

It should be able to answer:
- whether a trade is allowed
- how large it may be
- whether current open positions must be reduced

### 8.9 Recommended processing sequence for a live decision

```text
1. Receive market event
2. Update market state
3. Update relevant features
4. Build MarketContext
5. Run strategy evaluation
6. Run risk evaluation
7. Build OrderIntent if approved
8. Send order via execution engine
9. Persist signal/risk/order events
10. Await order events/fills and update state
```

---

## 9. Backtest, Replay, Validation, and Monitoring

### 9.1 Backtest realism checklist

A backtest is not credible unless it models:

- exchange fees
- maker/taker differences
- slippage
- spread
- latency or delayed fill assumptions
- partial fills if needed
- funding rates for perps
- contract size/tick/lot precision
- rejected orders and no-fill scenarios where relevant

### 9.2 Replay engine requirements

Replay should be able to:
- consume the exact event order
- rebuild order book from snapshot + deltas
- reproduce feature calculations
- reproduce strategy decisions
- reproduce risk checks
- compare simulated and recorded live actions

### 9.3 Validation metrics beyond win rate

Track at least:

- expectancy
- average win / average loss
- payoff ratio
- profit factor
- Sharpe / Sortino if appropriate
- max drawdown
- time under water
- turnover
- fees and funding drag
- slippage bps
- fill ratio
- markout after fills
- performance by regime
- performance by session
- performance by feature bucket
- false breakout rate
- stop-out rate by setup type

### 9.4 Monitoring for live systems

Monitor:
- feed lag
- sequence gaps
- stale features
- strategy heartbeat
- order ack latency
- cancel latency
- reject rate
- fill rate
- PnL drift
- position reconciliation differences
- risk breach count
- database sink lag
- event-bus consumer lag

### 9.5 Research-live parity tests

Before shipping a strategy:
- run historical backtest
- run exact session replay
- run paper trading
- compare feature values at sampled timestamps
- compare signal outputs on same events
- compare simulated vs live execution assumptions

---

## 10. Recommended Build Roadmap

### Phase 0: Contracts and schemas
Build first:
- canonical instrument model
- market event schemas
- order and fill event schemas
- strategy and risk interfaces

### Phase 1: Minimum viable trading core
Implement:
- `ref_*` tables
- `md_ohlcv_bars`
- `ord_orders`, `ord_order_events`, `ord_fills`
- `pos_positions`, `pos_pnl_snapshots`
- `risk_limits`, `risk_events`
- one signal table and one strategy log table

Strategy scope:
- 1 trend-following strategy
- 1 mean-reversion strategy
- volatility state filter
- position sizing by ATR

### Phase 2: Replay and analytics foundation
Add:
- raw trades
- replay engine
- feature registry
- feature snapshot storage
- daily performance reporting
- execution quality metrics

### Phase 3: Multi-strategy and regime routing
Add:
- breakout quality filter
- fake breakout detector
- regime selector
- entry ranking
- portfolio/correlation limits

### Phase 4: Derivatives context
Add:
- funding rates
- open interest
- basis
- long-short ratio if reliable
- liquidation heuristics

### Phase 5: Microstructure and low-latency upgrade
Add only if justified:
- ticks
- L2 snapshots/deltas
- order-flow confirmation
- book imbalance features
- short-horizon execution alpha
- hot path in Rust/C++/Java if needed

### Phase 6: ML / meta-labeling
After rule-based system is stable:
- label outcomes
- use model as filter or calibrator
- probability-based sizing
- confidence calibration

---

## 11. Common Failure Modes and Anti-Patterns

### 11.1 Database anti-patterns
- storing all data in one giant mutable table
- using JSON for everything
- no partitioning on large time-series
- no immutable order events
- no archive strategy
- no feature versioning

### 11.2 Strategy anti-patterns
- one strategy for all regimes
- too many indicators with overlapping information
- no execution cost penalty
- optimizing only on win rate
- using future-known labels accidentally
- mean reversion without trend veto
- breakout without quality filter

### 11.3 Risk anti-patterns
- no daily loss shutdown
- no portfolio exposure cap
- widening stops after entry
- adding to losers
- ignoring correlated positions
- no stale-data veto
- no venue health kill switch

### 11.4 Engineering anti-patterns
- research code not reusable in production
- signal engine sending orders directly
- mixing analytics queries into hot path
- no deterministic replay
- no structured reason codes
- no audit trail for rejected trades
- no alerting for feed gaps

### 11.5 HFT-specific anti-pattern
Calling a 1-minute OHLCV strategy “HFT” is an architecture mistake.  
If you need HFT-like behavior, you need:
- tick/L2/L3 data
- precise timestamps
- execution/queue model
- realistic latency budget
- specialized hot path implementation

---

## 12. Appendix A: Example Table Definitions

These examples are illustrative, not final DDL for every deployment.

### 12.1 PostgreSQL: `ord_orders`

```sql
CREATE TABLE ord_orders (
    order_id              UUID PRIMARY KEY,
    venue_id              INTEGER NOT NULL REFERENCES ref_venues(venue_id),
    instrument_id         BIGINT NOT NULL REFERENCES ref_instruments(instrument_id),
    client_order_id       TEXT NOT NULL,
    venue_order_id        TEXT,
    parent_order_id       UUID,
    order_intent_id       UUID,
    status                TEXT NOT NULL,
    side                  TEXT NOT NULL,
    order_type            TEXT NOT NULL,
    time_in_force         TEXT,
    limit_price           NUMERIC(28, 10),
    stop_price            NUMERIC(28, 10),
    orig_qty              NUMERIC(28, 10) NOT NULL,
    leaves_qty            NUMERIC(28, 10) NOT NULL,
    cum_fill_qty          NUMERIC(28, 10) NOT NULL DEFAULT 0,
    avg_fill_price        NUMERIC(28, 10),
    reduce_only           BOOLEAN NOT NULL DEFAULT FALSE,
    post_only             BOOLEAN NOT NULL DEFAULT FALSE,
    created_at            TIMESTAMPTZ NOT NULL,
    updated_at            TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX ux_ord_orders_venue_client
    ON ord_orders (venue_id, client_order_id);

CREATE UNIQUE INDEX ux_ord_orders_venue_order
    ON ord_orders (venue_id, venue_order_id)
    WHERE venue_order_id IS NOT NULL;

CREATE INDEX ix_ord_orders_instrument_status_created
    ON ord_orders (instrument_id, status, created_at DESC);
```

### 12.2 PostgreSQL: `ord_order_events`

```sql
CREATE TABLE ord_order_events (
    order_event_id        UUID PRIMARY KEY,
    order_id              UUID NOT NULL REFERENCES ord_orders(order_id),
    event_time            TIMESTAMPTZ NOT NULL,
    event_type            TEXT NOT NULL,
    status_before         TEXT,
    status_after          TEXT,
    payload_json          JSONB,
    venue_message_id      TEXT
);

CREATE INDEX ix_ord_order_events_order_time
    ON ord_order_events (order_id, event_time);

CREATE INDEX ix_ord_order_events_time_desc
    ON ord_order_events (event_time DESC);
```

### 12.3 PostgreSQL: `ord_fills`

```sql
CREATE TABLE ord_fills (
    fill_id               UUID PRIMARY KEY,
    order_id              UUID NOT NULL REFERENCES ord_orders(order_id),
    venue_trade_id        TEXT,
    instrument_id         BIGINT NOT NULL REFERENCES ref_instruments(instrument_id),
    event_time            TIMESTAMPTZ NOT NULL,
    fill_qty              NUMERIC(28, 10) NOT NULL,
    fill_price            NUMERIC(28, 10) NOT NULL,
    fee_amount            NUMERIC(28, 10) DEFAULT 0,
    fee_asset             TEXT,
    liquidity_flag        TEXT,
    commission_bps        NUMERIC(12, 6)
);

CREATE INDEX ix_ord_fills_order_time
    ON ord_fills (order_id, event_time);

CREATE INDEX ix_ord_fills_instrument_time_desc
    ON ord_fills (instrument_id, event_time DESC);
```

### 12.4 PostgreSQL: `risk_limits`

```sql
CREATE TABLE risk_limits (
    risk_limit_id         UUID PRIMARY KEY,
    profile_id            UUID,
    scope_type            TEXT NOT NULL,
    scope_ref             TEXT NOT NULL,
    limit_name            TEXT NOT NULL,
    limit_value           NUMERIC(28, 10) NOT NULL,
    hard_or_soft          TEXT NOT NULL,
    enabled               BOOLEAN NOT NULL DEFAULT TRUE,
    effective_from        TIMESTAMPTZ NOT NULL,
    effective_to          TIMESTAMPTZ
);

CREATE INDEX ix_risk_limits_scope
    ON risk_limits (scope_type, scope_ref, enabled);
```

### 12.5 ClickHouse: `md_trades`

```sql
CREATE TABLE md_trades
(
    venue_id              UInt16,
    instrument_id         UInt64,
    event_time            DateTime64(6, 'UTC'),
    ingest_time           DateTime64(6, 'UTC'),
    trade_id              String,
    price                 Decimal64(10),
    qty                   Decimal64(10),
    side_aggressor        LowCardinality(String),
    is_block_trade        UInt8
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(event_time)
ORDER BY (venue_id, instrument_id, event_time, trade_id);
```

### 12.6 ClickHouse: `md_ohlcv_bars`

```sql
CREATE TABLE md_ohlcv_bars
(
    instrument_id         UInt64,
    timeframe             LowCardinality(String),
    bar_time              DateTime64(3, 'UTC'),
    open                  Float64,
    high                  Float64,
    low                   Float64,
    close                 Float64,
    volume                Float64,
    quote_volume          Float64,
    trade_count           UInt32,
    buy_volume            Float64,
    sell_volume           Float64,
    vwap                  Float64,
    is_final              UInt8
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(bar_time)
ORDER BY (instrument_id, timeframe, bar_time);
```

### 12.7 ClickHouse: `feat_values_wide`

```sql
CREATE TABLE feat_values_wide
(
    instrument_id             UInt64,
    timeframe                 LowCardinality(String),
    feature_time              DateTime64(3, 'UTC'),
    feature_set_version       UInt32,
    ret_1                     Float64,
    ret_5                     Float64,
    ema_20_dist               Float64,
    ema_50_dist               Float64,
    rsi_14                    Float64,
    atr_pct_14                Float64,
    volume_ratio_20           Float64,
    breakout_quality_score    Float64,
    trend_strength_score      Float64,
    vol_state_score           Float64,
    funding_zscore            Float64,
    oi_delta_1h               Float64,
    regime_id                 LowCardinality(String),
    feature_quality_score     Float64
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(feature_time)
ORDER BY (instrument_id, timeframe, feature_time, feature_set_version);
```

### 12.8 Snapshot + delta guidance for order books

Recommended pattern:
- snapshot every 1 to 10 seconds, or every N deltas
- store sequence numbers
- persist checksum if venue offers one
- during replay:
  1. load last snapshot before target time
  2. apply ordered deltas
  3. verify sequence continuity
  4. resync on gap

---

## 13. Appendix B: Example Rule and Strategy Interfaces

### 13.1 TypeScript-style strategy result contract

```ts
export interface DecisionResult {
  pass: boolean;
  reason: string;
  note?: string;
  score?: number;
}

export interface MarketContext {
  instrumentId: string;
  eventTime: number;
  price: {
    bid?: number;
    ask?: number;
    mid?: number;
    last: number;
  };
  bars: Record<string, {
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
  }>;
  features: Record<string, number | string | boolean | null>;
  positions: {
    netQty: number;
    avgEntryPrice?: number;
    unrealizedPnl?: number;
  };
  venueState: {
    isHealthy: boolean;
    isTradable: boolean;
    spreadTicks?: number;
    latencyMs?: number;
  };
}

export interface SignalDecision {
  action: 'BUY' | 'SELL' | 'HOLD' | 'EXIT' | 'REDUCE';
  score: number;
  confidence: number;
  reasonCodes: string[];
  stopPrice?: number;
  targetPrice?: number;
  metadata?: Record<string, unknown>;
}
```

### 13.2 Example detector pattern

```ts
export async function breakoutQualityDetector(
  ctx: MarketContext,
): Promise<DecisionResult> {
  try {
    const score = Number(ctx.features.breakout_quality_score ?? 0);
    const spreadTicks = Number(ctx.venueState.spreadTicks ?? 999);

    if (!ctx.venueState.isHealthy || !ctx.venueState.isTradable) {
      return {
        pass: false,
        reason: 'VENUE_UNHEALTHY',
        note: 'Trading disabled because venue state is degraded.',
      };
    }

    if (spreadTicks > 3) {
      return {
        pass: false,
        reason: 'SPREAD_TOO_WIDE',
        note: `Current spread ticks = ${spreadTicks}.`,
      };
    }

    if (score < 0.72) {
      return {
        pass: false,
        reason: 'BREAKOUT_QUALITY_TOO_LOW',
        note: `Score ${score.toFixed(2)} below threshold.`,
        score,
      };
    }

    return {
      pass: true,
      reason: 'BREAKOUT_QUALITY_PASS',
      note: `Score ${score.toFixed(2)} accepted.`,
      score,
    };
  } catch (error) {
    return {
      pass: false,
      reason: 'BREAKOUT_QUALITY_ERROR',
      note: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}
```

### 13.3 Example signal pipeline orchestration

```ts
export async function evaluateTradeCandidate(
  ctx: MarketContext,
): Promise<SignalDecision> {
  try {
    const regime = String(ctx.features.regime_id ?? 'UNKNOWN');
    const trendScore = Number(ctx.features.trend_strength_score ?? 0);
    const volScore = Number(ctx.features.vol_state_score ?? 0);
    const breakoutScore = Number(ctx.features.breakout_quality_score ?? 0);

    if (!ctx.venueState.isHealthy || !ctx.venueState.isTradable) {
      return {
        action: 'HOLD',
        score: 0,
        confidence: 0,
        reasonCodes: ['VENUE_BLOCK'],
      };
    }

    if (regime === 'PANIC' || volScore > 0.95) {
      return {
        action: 'HOLD',
        score: 0,
        confidence: 0,
        reasonCodes: ['VOL_STATE_BLOCK'],
      };
    }

    const longScore =
      0.35 * trendScore +
      0.35 * breakoutScore +
      0.15 * Number(ctx.features.volume_ratio_20 ?? 0) +
      0.15 * Number(ctx.features.participation_score ?? 0);

    if (regime === 'TREND_UP' && longScore >= 0.75) {
      return {
        action: 'BUY',
        score: longScore,
        confidence: Math.min(1, longScore),
        reasonCodes: [
          'REGIME_TREND_UP',
          'TREND_SCORE_PASS',
          'BREAKOUT_SCORE_PASS',
        ],
        stopPrice: ctx.price.last - Number(ctx.features.atr_stop_distance ?? 0),
      };
    }

    return {
      action: 'HOLD',
      score: longScore,
      confidence: Math.min(1, longScore),
      reasonCodes: ['NO_ACTION_THRESHOLD_NOT_MET'],
    };
  } catch (error) {
    return {
      action: 'HOLD',
      score: 0,
      confidence: 0,
      reasonCodes: [
        error instanceof Error ? `PIPELINE_ERROR_${error.message}` : 'PIPELINE_ERROR',
      ],
    };
  }
}
```

### 13.4 Example YAML-like risk policy

```yaml
risk:
  max_risk_per_trade_bps: 50
  max_daily_loss_r: 3.0
  max_weekly_loss_r: 8.0
  max_open_positions: 5
  max_cluster_risk_r: 2.0
  max_gross_exposure_usd: 250000
  max_spread_ticks:
    breakout: 3
    mean_reversion: 2
    trend_follow: 4
  stale_market_data_ms: 1500
  stale_feature_data_ms: 2000
  cooldown_after_three_losses_min: 30
  min_expected_rr:
    breakout: 1.8
    mean_reversion: 1.5
    trend_follow: 2.0
```

---

## 14. Appendix C: Reference Materials

The following references are useful for the infrastructure patterns discussed in this handbook:

### Core database and streaming references

- PostgreSQL declarative partitioning  
  `https://www.postgresql.org/docs/current/ddl-partitioning.html`

- Timescale hypertables and chunks  
  `https://docs.timescale.com/use-timescale/latest/hypertables/`

- Kafka / Confluent log compaction and event-sourcing patterns  
  `https://docs.confluent.io/kafka/design/log_compaction.html`

- ClickHouse documentation  
  `https://clickhouse.com/docs/en`

- ClickHouse projections and secondary-index style acceleration  
  `https://clickhouse.com/blog/projections-secondary-indices`

### Exchange documentation checklist

When implementing the production system, prefer official venue documentation as the source of truth for:
- symbol metadata
- precision and notional rules
- funding schedules
- order state semantics
- sequence numbers and book checksum logic
- rate limits
- websocket reconnect and snapshot recovery rules
- liquidation / mark price semantics where applicable

### Recommended implementation note

Use official docs to verify:
- field definitions before schema freezes
- websocket ordering guarantees before replay design
- exact fee and funding semantics before backtests
- order lifecycle transitions before building the OMS state machine

---

# Final Engineering Recommendations

If you are building from zero, the most productive and safest build order is:

1. **Canonical schemas and data contracts**
2. **Order/fill/position/risk transactional backbone**
3. **Bars + feature pipeline + strategy logs**
4. **One trend strategy, one mean-reversion strategy, one breakout filter**
5. **Replay engine and execution quality analytics**
6. **Regime selector and capital allocation layer**
7. **Derivatives context features (funding/OI/basis)**
8. **Microstructure modules only when justified by data and latency budget**

### Minimum viable serious system
A “serious” version is not defined by ML or HFT labels. It is defined by:
- clear schemas
- deterministic decisions
- point-in-time features
- risk controls
- replayability
- explainable logs
- execution analytics

### Practical conclusion
The best trading platform is not the one with the most indicators. It is the one that:
- stores the right data with the right shape,
- routes decisions through explicit risk logic,
- can be replayed exactly,
- and can evolve from simple rule-based strategies into more advanced scoring, routing, and execution models without rewriting the whole system.

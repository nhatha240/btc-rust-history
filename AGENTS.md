# AGENTS.md

> For AI coding agents only.  
> Source of truth: `trading_system_design_handbook.md`

## Core Rules
- Prioritize trading safety and correctness over speed.
- Keep changes minimal, local, reversible, and backward-compatible.
- Add structured logs for decisions/gates.
- Never leak secrets in code or logs.
- If unclear, choose the safer design and document it.

## Repo Placement
- Postgres migrations: `db/postgres/`
- ClickHouse DDL: `db/clickhouse/`
- Shared SQL: `db/queries/`
- Infra files: `infra/`
- Protobuf schemas: `proto/`
- Regenerate Rust/Python bindings after proto changes.

## Service Boundaries
- `marketdata_ingestor`: market data ingestion
- `feature_state`: feature computation
- `signal_engine`: signal generation
- `risk_guard`: risk validation
- `order_executor`: real execution
- `paper_trader`: simulated execution
- `execution_router`: execution routing
- `ai_predictor`: ML inference only
- `api_gateway` / `web_dashboard`: API and dashboard access only

## Forbidden
- No strategy logic in execution services
- No exchange calls from strategy layer
- No inference outside `ai_predictor`
- No UI direct access to DB or messaging
- Strategy logic must be pure / immutable

## Data + Storage
- Use Protobuf on hot path, not JSON
- Idempotency via `trace_id` and `client_order_id`
- Market partition key: `symbol`
- OMS partition key: `account_id:symbol`
- PostgreSQL = OMS source of truth
- ClickHouse = append-only analytics
- Redis = hot ephemeral state
- Kafka/Redpanda = event journal
- Kill switch key: `risk:kill`

## Engineering Rules
- Strategy input = immutable context; output = structured decision
- Strategy never sends orders directly
- Same signal/risk logic for backtest, paper, and live
- Feature computation must be incremental and point-in-time correct
- Risk must fail closed
- Execution must use deterministic `client_order_id`

## Runtime Rules
- Rust: small functions, `tracing`, explicit errors, `/health`, `/ready`
- Python: only `ai_predictor`, clean config/logging/metrics split
- Web: read only through `api_gateway`, support pagination/filtering

## Validation
- After non-trivial changes, verify compose, topics/config, OMS loop, web orders, kill switch, and `trace_id` flow.
- If checks were not run, state exactly what was skipped and why.

## Safety
- Never hardcode secrets
- Never bypass `risk_guard`
- Never send real orders directly
- New strategies must go through paper trading first
- Never drop columns; prefer additive schema changes
- Never mix Postgres migrations with ClickHouse DDL
- Always follow `trading_system_design_handbook.md`

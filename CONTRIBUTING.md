# AGENTS.md

This file defines operating rules for AI coding agents in this repository.
Source of truth: `.ai/rules.yaml`.

## 1. Core Objective
Build and maintain a low-latency auto-trading platform with:
- Rust + Python microservices
- Redpanda/Kafka messaging (Protobuf on hot path)
- Postgres/Timescale as OMS source of truth
- Redis for ephemeral state
- Optional ClickHouse analytics

## 2. Mandatory File Placement
- All DB schema/migrations/query SQL must live under `db/`.
  - Examples: `db/postgres`, `db/clickhouse`, `db/queries`
- All infra YAML and infra-related files must live under `infra/`.
  - Examples: `infra/docker`, `infra/scripts`, `infra/k8s`

## 3. Service Boundaries (Do Not Violate)
- `marketdata_ingestor`: ingest + normalize + publish market events only
- `feature_engine`: compute indicators + publish features only
- `ai_predictor`: inference + publish predictions only
- `strategy_engine`: decision logic + order command planning only
- `execution_router`: execute/dedup/persist OMS only
- `api_gateway`: API + control plane only

Forbidden crossovers:
- No strategy logic in execution services
- No direct exchange REST from strategy
- No model inference outside predictor

## 4. Data Plane Rules
- Use Protobuf for hot-path topics; avoid JSON in hot path.
- Preserve idempotency with `trace_id` and `client_order_id`.
- At-least-once consumption must not create duplicate DB rows.
- Partitioning:
  - market topics by `symbol`
  - OMS topics by `account_id:symbol`

## 5. Storage Rules
- Postgres/Timescale is OMS source of truth.
- Required OMS entities: `orders`, `order_events`, `fills`, `positions`, `decision_logs`.
- Postgres migrations must live in `db/postgres/*.sql`.
- ClickHouse is optional analytics; P0 flow must not depend on ClickHouse readiness.
- Redis keys should use TTL unless explicitly permanent.

## 6. Coding Rules
- Rust: small focused functions, config-driven behavior, structured logs, explicit errors.
- Python: typed config, separated consumer loop and HTTP server, clear failure handling.
- Web: UI reads only via `api_gateway`, with pagination/filtering for lists.

## 7. Testing + Validation
Minimum checks after non-trivial changes:
1. `docker compose up` is stable
2. Topics/configs exist and are correct
3. Run `tests/verify_oms_loop.py`
4. Verify web order list + detail
5. Verify kill-switch behavior
6. Verify traceability via `trace_id`

## 8. Change Policy
- Topic rename must update all relevant docs/config/scripts/tests.
- Schema/proto changes must regenerate Rust/Python proto libs where applicable.
- Prefer additive schema changes; avoid destructive changes.

## 9. Workflow Order
1. Align compose + topic config
2. Run DB migrations
3. Start OMS services
4. Run verification tests
5. Verify dashboard/API behavior

## 10. Operational Baseline
- Every service should expose `/health` and `/ready`.
- Readiness should verify dependencies used by that service (MQ/DB/Redis).
- Preserve distributed trace propagation (`trace_id`) end-to-end.

# Project Test Cases

## Scope
This suite validates the trading platform hot path:

- Kafka/Redpanda ingestion and topic flow
- OMS persistence in Postgres (`orders`, `trades`, `positions`, `decision_logs`)
- Risk controls (`risk:kill` kill-switch)
- End-to-end traceability (`trace_id`)
- API gateway read behavior

## Preconditions

1. Start stack:
   ```bash
   docker compose -f infra/docker/docker-compose.yml up -d
   ```
2. Create topics:
   ```bash
   BROKERS=localhost:9092 ./infra/scripts/create_topics.sh
   ```
3. Run DB migrations/init:
   ```bash
   ./infra/scripts/migrate.sh
   ```
4. Ensure Python deps for verification script are installed.

## TC-001: Service Health/Readiness

- Objective: all core services expose healthy runtime.
- Steps:
  1. Call `/health` and `/ready` for running services.
  2. Verify dependency-aware readiness (DB/Redis/Kafka-backed services).
- Expected:
  - `/health` returns success.
  - `/ready` returns success when dependencies are reachable.

## TC-002: OMS Happy Path (E2E)

- Objective: approved order reaches DB and updates position.
- Steps:
  1. Run:
     ```bash
     python3 tests/verify_oms_loop.py
     ```
  2. Observe output for order, trade, position checks.
- Expected:
  - Order exists in `orders`.
  - Fill exists in `trades`.
  - Position is updated in `positions`.

## TC-003: Kill-Switch Blocks New Orders

- Objective: verify risk gate blocks execution when kill-switch is ON.
- Steps:
  1. Set kill-switch:
     ```bash
     redis-cli SET risk:kill 1
     ```
  2. Send a new order (reuse `tests/verify_oms_loop.py` payload pattern).
  3. Check DB and logs.
  4. Reset kill-switch:
     ```bash
     redis-cli SET risk:kill 0
     ```
- Expected:
  - Order is rejected or not forwarded for execution.
  - No live execution/fill is created for blocked order.
  - Rejection reason is observable in logs/audit records.

## TC-004: Idempotency/Dedup (Same `client_order_id`)

- Objective: at-least-once consumption does not create duplicate OMS records.
- Steps:
  1. Produce the same `OrderCommand` twice with identical:
     - `client_order_id`
     - `trace_id`
  2. Query DB counts for this `client_order_id`.
- Expected:
  - Single canonical order row.
  - No duplicate fills/trades caused by re-delivery.
  - Dedup behavior visible in logs.

## TC-005: Traceability (`trace_id`) End-to-End

- Objective: one trace id is preserved across decision, order, execution artifacts.
- Steps:
  1. Send one order with known `trace_id`.
  2. Query DB/logs by `trace_id`.
- Expected:
  - `trace_id` appears consistently in:
    - order command handling logs
    - risk decision logs
    - persisted OMS records

## TC-006: API Order List + Detail

- Objective: dashboard/API reads from source-of-truth correctly.
- Steps:
  1. Request order list endpoint with pagination/filter.
  2. Request order detail endpoint by `client_order_id`.
- Expected:
  - List endpoint supports pagination/filtering and returns expected rows.
  - Detail endpoint matches DB state for status, qty, price, and trace fields.

## TC-007: Negative Validation (Risk Limits)

- Objective: over-limit order is rejected with explicit reason.
- Steps:
  1. Send order exceeding configured notional/leverage limits.
  2. Inspect result in DB/logs.
- Expected:
  - Order is rejected.
  - Rejection reason is explicit and structured.
  - No downstream fill for rejected order.

## Quick Regression Run Order

1. TC-001
2. TC-002
3. TC-003
4. TC-004
5. TC-005
6. TC-006
7. TC-007

## Notes

- Keep test messages partition keys consistent:
  - market topics: `symbol`
  - OMS topics: `account_id` (or `account_id:symbol` where configured)
- Prefer paper-trading path for validation unless explicitly approved for live execution.

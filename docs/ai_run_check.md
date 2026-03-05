# AI Run And Check Guide

This guide is for AI agents (or operators) to run, verify, and debug the local stack quickly.

## 1. Bring Up Core Services

```bash
docker compose up -d redpanda postgres redis clickhouse connect topic_init
docker compose ps
```

Check health:

```bash
docker compose ps redpanda postgres redis clickhouse connect
```

Expected:
- `redpanda` -> `healthy`
- `connect` -> `healthy`
- DB/cache services -> `healthy` or `up`

## 2. Build And Start App Services

Start minimum trading flow services:

```bash
docker compose up -d --build ingestion feature_state signal_engine order_executor risk_guard paper_trader web
docker compose ps
```

## 3. Quick Functional Checks

### 3.1 Topic list
```bash
docker compose exec -T redpanda rpk topic list
```

### 3.2 Service logs (tail)
```bash
docker compose logs --no-color --tail=120 ingestion feature_state signal_engine risk_guard order_executor
```

### 3.3 Health endpoints
```bash
curl -fsS http://localhost:8088/health || true
curl -fsS http://localhost:8083/ready || true
```

### 3.4 OMS DB row counts
```bash
psql "postgres://trader:traderpw@localhost:5432/db_trading" -c \
"select 'orders' as tbl,count(*) from orders
 union all select 'trades',count(*) from trades
 union all select 'positions',count(*) from positions
 union all select 'decision_logs',count(*) from decision_logs;"
```

## 4. Debug Checklist (Common Failures)

## 4.1 `connect-1` keeps restarting
Symptoms:
- Logs show `Input type stdin` and `Pipeline has terminated`.

Checks:
```bash
docker compose logs --no-color --tail=80 connect
docker compose ps connect
```

Fix:
- Ensure compose mounts `./infra/docker/connect/connect.yaml:/connect.yaml:ro`.
- Ensure healthcheck uses `wget` (image does not include `curl`).
- Ensure port mapping is `8083:4195`.

## 4.2 `marketdata_ingestor-1` panic with rustls CryptoProvider
Symptoms:
- Panic message mentions:
  `Could not automatically determine the process-level CryptoProvider`.

Fix in code:
- Install crypto provider at startup before websocket connect:
  `rustls::crypto::ring::default_provider().install_default()`.

Then rebuild:
```bash
docker compose up -d --build ingestion
docker compose logs --no-color --tail=120 ingestion
```

For full panic trace:
```bash
docker compose run --rm -e RUST_BACKTRACE=1 ingestion
```

## 4.3 Kafka `Connection refused` to `redpanda:9092`
Symptoms:
- `Broker transport failure` in service logs.

Checks:
```bash
docker compose ps redpanda
docker compose logs --no-color --tail=120 redpanda
```

Fix:
- Wait until redpanda is `healthy`.
- If redpanda crashes with memory allocation failure, increase compose memory (`--memory 1G` or higher) and recreate:
```bash
docker compose up -d --force-recreate redpanda
```

## 5. Build Speed Notes (Rust in Docker)

If Docker re-compiles many crates every time:
- Do not use `--no-cache`.
- Keep BuildKit enabled.
- Avoid changing `Cargo.lock` unnecessarily.
- Build one target at a time (`docker compose build ingestion`).
- Keep `.dockerignore` strict (exclude `target`, `node_modules`, `.next`, logs, IDE dirs).
- Re-run the same build once to confirm cache hits.

## 6. Minimal Verification Targets

After non-trivial changes, run at least:
1. `docker compose up` stable (no crash loop in critical services).
2. topic presence check.
3. OMS row-count query.
4. service health/ready endpoints.
5. traceability checks via logs (`trace_id` where applicable).

If any check cannot be run, report exactly which check was skipped and why.

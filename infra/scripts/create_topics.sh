#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

BROKERS="${BROKERS:-localhost:29092}"
TOPICS=(
  TOPIC_CANDLES_1M
  TOPIC_FEATURE_STATE
  TOPIC_SIGNALS
  TOPIC_SIGNAL_STATE
  TOPIC_ORDERS
  TOPIC_ORDERS_APPROVED
  TOPIC_FILLS
  TOPIC_MC_SNAPSHOT
  md.raw.trades
  md.raw.book
  control.config_updates
  control.kill_switch
  system.heartbeats
  dlq.main
)

run_rpk() {
  if command -v rpk >/dev/null 2>&1; then
    rpk "$@"
    return
  fi

  if command -v docker >/dev/null 2>&1; then
    docker compose -f "${REPO_ROOT}/infra/docker/docker-compose.yml" exec -T redpanda rpk "$@"
    return
  fi

  echo "Error: rpk not found and docker compose fallback is unavailable." >&2
  exit 1
}

echo "Creating topics on brokers: ${BROKERS}"
run_rpk topic create "${TOPICS[@]}" --brokers "${BROKERS}" || true

echo "Applying retention settings..."
run_rpk topic alter-config TOPIC_CANDLES_1M --set retention.ms=3600000 --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_SIGNALS --set retention.ms=86400000 --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_MC_SNAPSHOT --set retention.ms=86400000 --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_ORDERS --set retention.ms=604800000 --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_ORDERS_APPROVED --set retention.ms=604800000 --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_FILLS --set retention.ms=604800000 --brokers "${BROKERS}" || true
run_rpk topic alter-config md.raw.trades --set retention.ms=3600000 --brokers "${BROKERS}" || true
run_rpk topic alter-config md.raw.book --set retention.ms=3600000 --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_FEATURE_STATE --set cleanup.policy=compact --brokers "${BROKERS}" || true
run_rpk topic alter-config TOPIC_SIGNAL_STATE --set cleanup.policy=compact --brokers "${BROKERS}" || true

# New control & system topics
run_rpk topic alter-config control.config_updates --set retention.ms=2592000000 --brokers "${BROKERS}" || true
run_rpk topic alter-config control.kill_switch --set retention.ms=2592000000 --brokers "${BROKERS}" || true
run_rpk topic alter-config system.heartbeats --set retention.ms=86400000 --brokers "${BROKERS}" || true
run_rpk topic alter-config dlq.main --set retention.ms=604800000 --brokers "${BROKERS}" || true

echo "Done."

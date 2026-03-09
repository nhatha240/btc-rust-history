#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Config (change if needed)
CLICKHOUSE_HOST="${CLICKHOUSE_HOST:-localhost}"
CLICKHOUSE_PORT="${CLICKHOUSE_PORT:-8123}"
CLICKHOUSE_DB="${CLICKHOUSE_DB:-db_trading}"
SQL_FILE="${SQL_FILE:-${REPO_ROOT}/db/clickhouse/init.sql}"

if [ ! -f "$SQL_FILE" ]; then
  echo "SQL file not found: $SQL_FILE"
  exit 1
fi

echo "Initializing ClickHouse database '$CLICKHOUSE_DB' on ${CLICKHOUSE_HOST}:${CLICKHOUSE_PORT}..."
# Use multiquery=1 to allow multiple statements in init.sql
# We don't specify ?database= here because init.sql handles DB creation and uses fully qualified names.
curl -sS "http://${CLICKHOUSE_HOST}:${CLICKHOUSE_PORT}/?multiquery=1" \
  --data-binary @"$SQL_FILE"

echo "ClickHouse initialization complete."
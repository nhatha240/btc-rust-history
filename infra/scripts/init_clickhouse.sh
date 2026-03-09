#!/usr/bin/env bash
set -euo pipefail

# Config (change if needed)
CLICKHOUSE_HOST="${CLICKHOUSE_HOST:-localhost}"
CLICKHOUSE_PORT="${CLICKHOUSE_PORT:-8123}"
CLICKHOUSE_DB="${CLICKHOUSE_DB:-db_trading}"
SQL_FILE="${SQL_FILE:-db/clickhouse/init.sql}"

if [ ! -f "$SQL_FILE" ]; then
  echo "SQL file not found: $SQL_FILE"
  exit 1
fi

echo "Initializing ClickHouse database '$CLICKHOUSE_DB' on ${CLICKHOUSE_HOST}:${CLICKHOUSE_PORT}..."
curl -sS "http://${CLICKHOUSE_HOST}:${CLICKHOUSE_PORT}/?database=${CLICKHOUSE_DB}" \
  --data-binary @"$SQL_FILE"

echo "ClickHouse initialization complete."
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

DATABASE_URL="${DATABASE_URL:-postgres://trader:traderpw@localhost:5432/db_trading}"
MIGRATIONS=( $(ls "${REPO_ROOT}/db/postgres"/[0-9][0-9][0-9]_*.sql | sort) )

run_sql_file() {
  local sql_file="$1"

  if command -v psql >/dev/null 2>&1; then
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -f "${sql_file}"
    return
  fi

  if command -v docker >/dev/null 2>&1; then
    docker compose -f "${REPO_ROOT}/infra/docker/docker-compose.yml" exec -T postgres \
      psql -U trader -d db_trading -v ON_ERROR_STOP=1 -f - <"${sql_file}"
    return
  fi

  echo "Error: psql not found and docker compose fallback is unavailable." >&2
  exit 1
}

for migration in "${MIGRATIONS[@]}"; do
  echo "Applying migration: ${migration}"
  run_sql_file "${migration}"
done

echo "All migrations applied successfully."

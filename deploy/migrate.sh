#!/usr/bin/env bash
# Applies deploy/migrations/*.sql in filename order.
#
# Migrations run at DEPLOY time, never at api startup (see
# deploy/migrations/README.md). Always run this BEFORE installing/restarting
# the api service: the new binary may read new columns unconditionally.
#
# Usage:
#   ./migrate.sh "postgres://user:pass@host:5432/db"
#   DATABASE_URL=postgres://... ./migrate.sh
#
# Without an argument or DATABASE_URL the standard libpq environment
# variables are used (PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE).
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MIGRATIONS_DIR="${DIR}/migrations"

DB_URL="${1:-${DATABASE_URL:-}}"

command -v psql >/dev/null 2>&1 || { echo "[migrate] ERROR: psql not found" >&2; exit 1; }
[[ -d "${MIGRATIONS_DIR}" ]] || { echo "[migrate] ERROR: ${MIGRATIONS_DIR} not found" >&2; exit 1; }

PSQL=(psql --no-psqlrc -v ON_ERROR_STOP=1)
if [[ -n "${DB_URL}" ]]; then
    PSQL+=("${DB_URL}")
else
    echo "[migrate] no URL given, using libpq environment (PG*)"
fi

shopt -s nullglob
files=("${MIGRATIONS_DIR}"/*.sql)
shopt -u nullglob

if (( ${#files[@]} == 0 )); then
    echo "[migrate] no *.sql in ${MIGRATIONS_DIR}, nothing to do"
    exit 0
fi

# The shell sorts glob expansions, so files are applied 0001, 0002, ... in order.
for file in "${files[@]}"; do
    echo "[migrate] applying $(basename "${file}")"
    "${PSQL[@]}" -q -f "${file}"
done

echo "[migrate] done: ${#files[@]} file(s)"

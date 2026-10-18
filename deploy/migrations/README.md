# Schema migrations

**Policy: migrations run at deploy time, never at api startup.**

Reasons:

- a new binary may read new columns unconditionally (e.g. `nodes.pool` in
  `PgNode::all()`), so the schema must be there before it starts;
- startup DDL delays boot and hides failures in log noise instead of failing
  the deploy;
- a deploy must be either fully applied or failed, not half-applied on the
  next restart.

## How to apply

```bash
# with an explicit URL
./deploy/migrate.sh "postgres://user:pass@host:5432/db"

# or via libpq env vars (PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE)
PGPASSWORD=... ./deploy/migrate.sh
```

`deploy/api-deploy.sh` calls `migrate.sh` before installing the new binary when
`DATABASE_URL` is set, otherwise it prints a warning — apply the migrations
manually in that case.

Order is strict: **migrate → start/restart api**.

## Rules for new files

- Name: `NNNN_short_description.sql` (`0001_`, `0002_`, …). Applied in
  filename order, so never rename or reorder existing files.
- Must be **idempotent** (`ADD COLUMN IF NOT EXISTS`, `CREATE ... IF NOT
  EXISTS`, …) — there is no ledger table yet, every file runs on every deploy.
- Keep each file to one logical change; wrap in `BEGIN;`/`COMMIT;` if the
  statements must be atomic.
- `dev/dev.sql` is the full-schema reference for fresh dev databases; a new
  migration must also be reflected there.

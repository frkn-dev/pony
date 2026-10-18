-- nodes.pool / nodes.pool_translation
--
-- Pool (group) name of a node and its per-language translations
-- (JSONB map lang -> name). The api reads both columns unconditionally in
-- PgNode::all(), so they must exist BEFORE a binary that uses them starts.
--
-- Applied at deploy time by deploy/migrate.sh — deliberately NOT at api
-- startup (see deploy/migrations/README.md). Must stay idempotent.

ALTER TABLE nodes ADD COLUMN IF NOT EXISTS pool TEXT;
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS pool_translation JSONB;

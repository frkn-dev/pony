use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

use fcore::Result;

use super::pg::PgClientManager;

/// Marks a connection as a share-issued child: created at mint time for the
/// recipient of a `frkn://conn/<token>` link. NULL means a normal connection.
pub const ISSUED_VIA_SHARE: &str = "share";

/// Marks a connection as the carrier of an env-wide shared subscription
/// (`frkn://sub/shared/<uuid>`): the recipient gets the whole env/proto.
pub const ISSUED_VIA_SHARED_SUB: &str = "shared_sub";

/// Token kinds. `conn` = one connection (app share of a single server);
/// `sub` = a whole env+proto shared as a subscription.
pub const SHARE_KIND_CONN: &str = "conn";
pub const SHARE_KIND_SUB: &str = "sub";

/// One minted share token row. The token is a scoped credential for exactly
/// one child connection (`connection_id`). `node_id` set = the token is
/// pinned to that node (app share of one server); `node_id` null = the token
/// is env-wide and every online node of the child's env serves it.
#[derive(Debug, Clone)]
pub struct ShareTokenRow {
    pub token: String,
    pub subscription_id: uuid::Uuid,
    /// Child connection created at mint time (issued_via = 'share' /
    /// 'shared_sub'). It defines the env and protocol the token serves.
    pub connection_id: uuid::Uuid,
    pub node_id: Option<uuid::Uuid>,
    /// Connection the link was minted from; NULL for shared subscriptions
    /// (they are minted per env+proto, not from one device).
    pub source_connection_id: Option<uuid::Uuid>,
    /// SHARE_KIND_CONN or SHARE_KIND_SUB.
    pub kind: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Persists share tokens. No in-memory sync on purpose: the token lookup
/// happens once per share-token /v1/config fetch (low volume), everything
/// else is owner-side management.
pub struct PgShare {
    pub manager: Arc<Mutex<PgClientManager>>,
}

impl PgShare {
    pub fn new(manager: Arc<Mutex<PgClientManager>>) -> Self {
        Self { manager }
    }

    pub async fn ensure_table(&self) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        client
            .batch_execute(
                r#"
                ALTER TABLE connections
                    ADD COLUMN IF NOT EXISTS issued_via TEXT;

                CREATE TABLE IF NOT EXISTS share_tokens (
                    token TEXT PRIMARY KEY,
                    subscription_id UUID NOT NULL,
                    connection_id UUID NOT NULL,
                    node_id UUID NOT NULL,
                    source_connection_id UUID NOT NULL,
                    label TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                    last_used_at TIMESTAMPTZ,
                    revoked_at TIMESTAMPTZ
                );

                -- Env-wide tokens (node_id IS NULL) predate this column being
                -- nullable in the schema; keep the migration idempotent.
                ALTER TABLE share_tokens ALTER COLUMN node_id DROP NOT NULL;

                -- Shared subscriptions are minted per env+proto, not from a
                -- device, so there is no source connection to point at.
                ALTER TABLE share_tokens ALTER COLUMN source_connection_id DROP NOT NULL;

                ALTER TABLE share_tokens
                    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'conn';

                -- One live shared subscription per (subscription, label):
                -- the label carries "region · protocol".
                CREATE UNIQUE INDEX IF NOT EXISTS share_tokens_active_sub
                    ON share_tokens (subscription_id, label)
                    WHERE revoked_at IS NULL AND kind = 'sub';

                -- Idempotent mint: one active token per (source, node, label).
                CREATE UNIQUE INDEX IF NOT EXISTS share_tokens_active_triple
                    ON share_tokens (source_connection_id, node_id, label)
                    WHERE revoked_at IS NULL;

                -- Same for env-wide tokens: NULLs never collide in a plain
                -- unique index, so they need their own predicate.
                CREATE UNIQUE INDEX IF NOT EXISTS share_tokens_active_pair
                    ON share_tokens (source_connection_id, label)
                    WHERE revoked_at IS NULL AND node_id IS NULL;
                "#,
            )
            .await?;

        Ok(())
    }

    fn map_row(row: tokio_postgres::Row) -> ShareTokenRow {
        ShareTokenRow {
            token: row.get("token"),
            subscription_id: row.get("subscription_id"),
            connection_id: row.get("connection_id"),
            node_id: row.get("node_id"),
            source_connection_id: row.get("source_connection_id"),
            kind: row.get("kind"),
            label: row.get("label"),
            created_at: row.get("created_at"),
            last_used_at: row.get("last_used_at"),
            revoked_at: row.get("revoked_at"),
        }
    }

    /// The live (non-revoked) token for an exact (source, node, label)
    /// triple, if one exists — the idempotency hook of mint. `node_id: None`
    /// matches the env-wide row (`IS NOT DISTINCT FROM` treats NULL = NULL).
    pub async fn find_active(
        &self,
        source_connection_id: &uuid::Uuid,
        node_id: Option<&uuid::Uuid>,
        label: &str,
    ) -> Result<Option<ShareTokenRow>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                r#"
                SELECT token, subscription_id, connection_id, node_id,
                       source_connection_id, label, kind, created_at, last_used_at, revoked_at
                FROM share_tokens
                WHERE source_connection_id = $1
                  AND node_id IS NOT DISTINCT FROM $2
                  AND label = $3
                  AND revoked_at IS NULL
                "#,
                &[source_connection_id, &node_id, &label],
            )
            .await?;

        Ok(row.map(Self::map_row))
    }

    /// The live shared-subscription token for (subscription, label) — the
    /// idempotency hook of `POST /share/sub`.
    pub async fn find_active_sub(
        &self,
        subscription_id: &uuid::Uuid,
        label: &str,
    ) -> Result<Option<ShareTokenRow>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                r#"
                SELECT token, subscription_id, connection_id, node_id,
                       source_connection_id, label, kind, created_at, last_used_at, revoked_at
                FROM share_tokens
                WHERE subscription_id = $1 AND label = $2 AND kind = 'sub'
                  AND revoked_at IS NULL
                "#,
                &[subscription_id, &label],
            )
            .await?;

        Ok(row.map(Self::map_row))
    }

    /// Active (non-revoked) token count of a subscription — the mint limit.
    pub async fn count_active(&self, subscription_id: &uuid::Uuid) -> Result<i64> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_one(
                "SELECT COUNT(*) FROM share_tokens WHERE subscription_id = $1 AND revoked_at IS NULL",
                &[subscription_id],
            )
            .await?;

        Ok(row.get(0))
    }

    /// Inserts a freshly minted token. Returns false when the insert lost a
    /// race: either the active-triple index fired (a concurrent mint won —
    /// caller re-reads with `find_active`) or, astronomically rarely, the
    /// token primary key collided (caller retries with a new token).
    pub async fn insert(&self, row: &ShareTokenRow) -> Result<bool> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let affected = client
            .execute(
                r#"
                INSERT INTO share_tokens
                (token, subscription_id, connection_id, node_id, source_connection_id, label, kind)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT DO NOTHING
                "#,
                &[
                    &row.token,
                    &row.subscription_id,
                    &row.connection_id,
                    &row.node_id,
                    &row.source_connection_id,
                    &row.label,
                    &row.kind,
                ],
            )
            .await?;

        Ok(affected > 0)
    }

    pub async fn get(&self, token: &str) -> Result<Option<ShareTokenRow>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let row = client
            .query_opt(
                r#"
                SELECT token, subscription_id, connection_id, node_id,
                       source_connection_id, label, kind, created_at, last_used_at, revoked_at
                FROM share_tokens
                WHERE token = $1
                "#,
                &[&token],
            )
            .await?;

        Ok(row.map(Self::map_row))
    }

    pub async fn list_active(&self, subscription_id: &uuid::Uuid) -> Result<Vec<ShareTokenRow>> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT token, subscription_id, connection_id, node_id,
                       source_connection_id, label, kind, created_at, last_used_at, revoked_at
                FROM share_tokens
                WHERE subscription_id = $1 AND revoked_at IS NULL
                ORDER BY created_at
                "#,
                &[subscription_id],
            )
            .await?;

        Ok(rows.into_iter().map(Self::map_row).collect())
    }

    /// Marks the token revoked. Returns false when the token was unknown or
    /// already revoked (the client treats both as success).
    pub async fn revoke(&self, token: &str) -> Result<bool> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        let affected = client
            .execute(
                "UPDATE share_tokens SET revoked_at = now() WHERE token = $1 AND revoked_at IS NULL",
                &[&token],
            )
            .await?;

        Ok(affected > 0)
    }

    /// Bookkeeping for /v1/config fetches by token. Best-effort: callers
    /// spawn it and only log failures.
    pub async fn touch_last_used(&self, token: &str) -> Result<()> {
        let mut manager = self.manager.lock().await;
        let client = manager.get_client().await?;

        client
            .execute(
                "UPDATE share_tokens SET last_used_at = now() WHERE token = $1",
                &[&token],
            )
            .await?;

        Ok(())
    }
}

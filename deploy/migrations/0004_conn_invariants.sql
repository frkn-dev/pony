-- Connection invariants.
--
-- 1) Region-share children (`issued_via = 'shared_sub'`) are hidden extras
--    minted for recipients, exactly like the older single-connection shares
--    (`issued_via = 'share'`). `connections_single_proto_idx` excluded only
--    'share', so minting a shared region for any non-WireGuard protocol
--    (hysteria2, vless, …) collided with the owner's own live connection and
--    the mint died with a unique violation — HTTP 500 "Failed to share
--    region" with the raw constraint text in the api log.
--
-- 2) A hysteria2 credential is the connection token, and the auth service
--    keeps a token → connection map. Nothing in the schema stopped two live
--    rows from carrying the same token, which would make one credential valid
--    for two users; make that invariant explicit.
--
-- Both indexes stay partial on `is_deleted = false`, like their neighbours:
-- a deleted row never blocks a re-created connection.
--
-- Idempotent: safe to re-run.

DROP INDEX IF EXISTS connections_single_proto_idx;

CREATE UNIQUE INDEX IF NOT EXISTS connections_single_proto_idx
    ON connections (subscription_id, env, proto)
    WHERE is_deleted = false
      AND proto <> ALL (ARRAY['wireguard'::proto, 'amnezia_wg'::proto, 'amnezia_wg_mobile'::proto])
      AND issued_via IS DISTINCT FROM 'share'
      AND issued_via IS DISTINCT FROM 'shared_sub';

CREATE UNIQUE INDEX IF NOT EXISTS connections_token_uniq
    ON connections (token)
    WHERE is_deleted = false
      AND token IS NOT NULL;

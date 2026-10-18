-- Plan-issued connections are unique per (subscription, env, proto).
--
-- The tariff ensure step (activation/renewal) creates one connection per
-- entitled (env, proto) and marks it issued_via = 'plan'. The existing
-- connections_single_proto_idx allows several connections for the wg family
-- (extra devices); this index keeps the *plan* connection single even there,
-- so a double activation or a concurrent ensure cannot duplicate it.
--
-- NULL (user-created extra device) and 'share' (share child) rows are not
-- covered: users may still add device connections. Idempotent.

CREATE UNIQUE INDEX IF NOT EXISTS connections_plan_single_proto_idx
    ON connections (subscription_id, env, proto)
    WHERE is_deleted = false
      AND issued_via = 'plan';

-- Premium term, separate from the subscription term.
--
-- A premium slot is paid for on its own schedule: it may run out while the
-- subscription is still live, and the other way round. Storing one date for
-- both made "premium expired" indistinguishable from "subscription expired",
-- and the cabinet could only show the subscription's date.
--
-- Backfill: existing premium rows have no agreed term of their own, so they
-- start from the subscription's expiry. From then on the two move
-- independently — renewing the subscription does not extend the slot.

BEGIN;

ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS premium_until TIMESTAMPTZ;

UPDATE subscriptions
   SET premium_until = expires_at
 WHERE premium_until IS NULL
   AND scope_env LIKE 'custompremium-%'
   AND NOT is_deleted;

COMMIT;

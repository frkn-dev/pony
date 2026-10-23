-- Premium slots as first-class inventory.
--
-- Premium used to be a bare `subscriptions.scope_env` string plus
-- `premium_token`: no stock, no expiry, no owner uniqueness. In practice that
-- produced a slot shared by two live subscriptions (`custompremium-prem02-1`)
-- and a lapsed premium that stayed occupied for days. A slot row is the unit of
-- stock; `assigned_sub` is its single owner.
--
-- Deliberate differences from the draft in `env/ai-tasks/premium-slots-tz.md`,
-- each one found while checking the draft against the live schema:
--
--   * four statuses, `reserved` included. The draft kept an offered slot as
--     `free` with `reserved_for` set, so it stayed in the stock count *and* in
--     the "pick a free slot" query — the offer could be handed out twice.
--   * `slot_key` is the primary key and `env` is nullable. Dedicated machines
--     (`prem1…prem5`) have no stable env (it is `custompersonal<uuid>`), so an
--     `env TEXT PRIMARY KEY` could not hold them at all.
--   * `env` is stored in DB form (`custompremium-prem02-4`), the same string as
--     `subscriptions.scope_env`. The prefix-less form would force `'custom' || env`
--     in every join (`Env::Custom` adds `custom` when serializing).
--   * the owner index matches `custompremium-%`, not `custompremium-prem%`:
--     `custompremium-kyrgyzstan` and `custompremium-a65e52c6af0e` are live
--     premium envs too and must obey the same invariant.
--   * premium children are left out (`parent_id IS NULL`): they may inherit the
--     parent env later and are not slots of their own.
--   * `country` is nullable — two legacy envs have no known country, and
--     guessing one in a migration is worse than a NULL.
--
-- Idempotent: safe to re-run. Everything runs in one transaction so a failed
-- precondition leaves no half-applied schema behind (the duplicate-env check
-- below is expected to fail until the stale row is detached).

BEGIN;

ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS premium_until TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS premium_slots (
    slot_key     TEXT PRIMARY KEY,              -- prem02-4 / prem1 / legacy-kyrgyzstan
    env          TEXT UNIQUE,                   -- custompremium-prem02-4; NULL for dedicated
    kind         TEXT NOT NULL DEFAULT 'pool',
    pool         TEXT NOT NULL,                 -- prem02 / prem03 / prem-nl-01 / prem1 / legacy
    slot_no      SMALLINT NOT NULL,
    address      INET,
    domain       TEXT,                          -- H2 SNI; NULL = AWG only
    country      TEXT,
    status       TEXT NOT NULL DEFAULT 'free',
    assigned_sub UUID REFERENCES subscriptions(id),
    assigned_at  TIMESTAMPTZ,
    reserved_for UUID,                          -- FK added after premium_requests exists
    note         TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT premium_slots_kind_check   CHECK (kind IN ('pool','dedicated')),
    CONSTRAINT premium_slots_status_check CHECK (status IN ('free','reserved','assigned','disabled')),
    -- A slot is occupied by exactly one subscription, or by none.
    CONSTRAINT premium_slots_owner_check  CHECK ((status = 'assigned') = (assigned_sub IS NOT NULL)),
    -- A reservation always names the request it is held for.
    CONSTRAINT premium_slots_reserved_check CHECK ((status = 'reserved') = (reserved_for IS NOT NULL)),
    -- Pool slots are addressable by env; dedicated ones may or may not be
    -- (legacy premium envs are dedicated rows that do carry one).
    CONSTRAINT premium_slots_env_check    CHECK (kind <> 'pool' OR env IS NOT NULL),
    CONSTRAINT premium_slots_pool_slot_uniq UNIQUE (pool, slot_no)
);

-- One slot per subscription, and one waiting request per subscription.
CREATE UNIQUE INDEX IF NOT EXISTS premium_slots_owner_uniq
    ON premium_slots (assigned_sub) WHERE assigned_sub IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS premium_slots_one_offer
    ON premium_slots (reserved_for) WHERE reserved_for IS NOT NULL;

-- Stock and the "pick a slot" query both filter on (kind, status, pool).
CREATE INDEX IF NOT EXISTS premium_slots_stock_idx
    ON premium_slots (kind, status, pool, slot_no);

CREATE TABLE IF NOT EXISTS premium_requests (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id  UUID NOT NULL REFERENCES subscriptions(id),
    months           SMALLINT NOT NULL DEFAULT 1,
    contact          TEXT,
    status           TEXT NOT NULL DEFAULT 'waiting',
    priority         SMALLINT NOT NULL DEFAULT 0,   -- higher = earlier; paying clients first
    -- How many offers this request has already missed. A missed offer puts the
    -- request back at the end of its priority group; after `MAX` misses it dies,
    -- so one holiday does not keep a slot frozen forever.
    attempts         SMALLINT NOT NULL DEFAULT 0,
    offered_env      TEXT REFERENCES premium_slots(env),
    offered_at       TIMESTAMPTZ,
    offer_expires_at TIMESTAMPTZ,
    note             TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT premium_requests_status_check CHECK (status IN ('waiting','offered','converted','cancelled','expired')),
    CONSTRAINT premium_requests_months_check CHECK (months BETWEEN 1 AND 120)
);

CREATE UNIQUE INDEX IF NOT EXISTS premium_requests_one_active
    ON premium_requests (subscription_id) WHERE status IN ('waiting','offered');

CREATE INDEX IF NOT EXISTS premium_requests_queue_idx
    ON premium_requests (status, priority DESC, created_at);

ALTER TABLE premium_slots DROP CONSTRAINT IF EXISTS premium_slots_reserved_for_fkey;
ALTER TABLE premium_slots
    ADD CONSTRAINT premium_slots_reserved_for_fkey
    FOREIGN KEY (reserved_for) REFERENCES premium_requests(id);

-- Precondition: the owner invariant cannot be created while two live
-- subscriptions share a premium env. Fail loudly and list every offender —
-- a silent `IF NOT EXISTS` here would leave the invariant unenforced.
DO $$
DECLARE
    offenders TEXT;
BEGIN
    SELECT string_agg(scope_env || ' ×' || n, ', ' ORDER BY scope_env)
      INTO offenders
      FROM (SELECT scope_env, count(*) AS n
              FROM subscriptions
             WHERE scope_env LIKE 'custompremium-%'
               AND NOT is_deleted
               AND parent_id IS NULL
             GROUP BY scope_env
            HAVING count(*) > 1) d;

    IF offenders IS NOT NULL THEN
        RAISE EXCEPTION
            'premium env is shared by several live subscriptions: %. '
            'Detach the stale one (scope_env = NULL, premium_token = NULL) before migrating.',
            offenders;
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS subscriptions_premium_scope_uniq
    ON subscriptions (scope_env)
    WHERE NOT is_deleted
      AND parent_id IS NULL
      AND scope_env LIKE 'custompremium-%';

-- Backfill 1: pool slots from the CSV inventory (`prem02-1…5`, `prem03-1…5`,
-- `prem-nl-01/02`). prem02-5 (.166) exists in DNS but was never provisioned
-- (no awg8/awg9, no fnode unit, no H2 cert), so it enters as `disabled` —
-- otherwise the stock count would advertise a slot that cannot serve traffic.
WITH pool(slot_key, env, pool, slot_no, address, domain, country) AS (
    VALUES
        ('prem02-1', 'custompremium-prem02-1', 'prem02', 1::SMALLINT, '195.133.80.161', 'prem02-1.frkn.org', 'PL'),
        ('prem02-2', 'custompremium-prem02-2', 'prem02', 2::SMALLINT, '195.133.80.162', 'prem02-2.frkn.org', 'PL'),
        ('prem02-3', 'custompremium-prem02-3', 'prem02', 3::SMALLINT, '195.133.80.163', 'prem02-3.frkn.org', 'PL'),
        ('prem02-4', 'custompremium-prem02-4', 'prem02', 4::SMALLINT, '195.133.80.165', 'prem02-4.frkn.org', 'PL'),
        ('prem02-5', 'custompremium-prem02-5', 'prem02', 5::SMALLINT, '195.133.80.166', 'prem02-5.frkn.org', 'PL'),
        ('prem03-1', 'custompremium-prem03-1', 'prem03', 1::SMALLINT, '195.133.80.167', 'prem03-1.frkn.org', 'PL'),
        ('prem03-2', 'custompremium-prem03-2', 'prem03', 2::SMALLINT, '195.133.80.168', 'prem03-2.frkn.org', 'PL'),
        ('prem03-3', 'custompremium-prem03-3', 'prem03', 3::SMALLINT, '195.133.80.169', 'prem03-3.frkn.org', 'PL'),
        ('prem03-4', 'custompremium-prem03-4', 'prem03', 4::SMALLINT, '195.133.80.21',  'prem03-4.frkn.org', 'PL'),
        ('prem03-5', 'custompremium-prem03-5', 'prem03', 5::SMALLINT, '195.133.80.172', 'prem03-5.frkn.org', 'PL'),
        ('prem-nl-01', 'custompremium-prem-nl-01', 'prem-nl-01', 1::SMALLINT, '77.110.103.111', 'prem-nl-01.frkn.org', 'NL'),
        ('prem-nl-02', 'custompremium-prem-nl-02', 'prem-nl-02', 2::SMALLINT, '77.110.102.155', 'prem-nl-02.frkn.org', 'NL')
)
INSERT INTO premium_slots (slot_key, env, kind, pool, slot_no, address, domain, country,
                           status, assigned_sub, assigned_at, note)
SELECT p.slot_key, p.env, 'pool', p.pool, p.slot_no, p.address::INET, p.domain, p.country,
       CASE WHEN s.id IS NOT NULL       THEN 'assigned'
            WHEN p.slot_key = 'prem02-5' THEN 'disabled'
            ELSE 'free' END,
       s.id,
       CASE WHEN s.id IS NOT NULL THEN now() END,
       CASE WHEN p.slot_key = 'prem02-5'
            THEN 'not provisioned: needs awg8/awg9 (51824/8447), fnode unit, H2 cert; .166 A-record already in DNS' END
  FROM pool p
  LEFT JOIN subscriptions s
         ON s.scope_env = p.env
        AND NOT s.is_deleted
        AND s.parent_id IS NULL
ON CONFLICT (slot_key) DO NOTHING;

-- Backfill 2: dedicated machines. Their env is `custompersonal<uuid>`, i.e. it
-- changes per client and is not in the subscriptions table as a premium env, so
-- the owner cannot be derived here — they enter `disabled` (out of stock by
-- kind anyway) and an admin links them once the mapping is confirmed.
INSERT INTO premium_slots (slot_key, env, kind, pool, slot_no, address, domain, country, status, note)
VALUES
    ('prem1', NULL, 'dedicated', 'prem1', 1::SMALLINT, '195.133.80.48',  'prem1.frkn.org', 'PL', 'disabled', 'legacy dedicated, owner not linked (env = custompersonal<uuid>)'),
    ('prem2', NULL, 'dedicated', 'prem2', 1::SMALLINT, '195.133.80.148', 'prem2.frkn.org', 'PL', 'disabled', 'legacy dedicated, owner not linked (env = custompersonal<uuid>)'),
    ('prem3', NULL, 'dedicated', 'prem3', 1::SMALLINT, '195.133.80.22',  'prem3.frkn.org', 'PL', 'disabled', 'legacy dedicated, owner not linked (env = custompersonal<uuid>)'),
    ('prem4', NULL, 'dedicated', 'prem4', 1::SMALLINT, '195.133.80.25',  'prem4.frkn.org', 'PL', 'disabled', 'legacy dedicated, owner not linked (env = custompersonal<uuid>)'),
    ('prem5', NULL, 'dedicated', 'prem5', 1::SMALLINT, '195.133.80.24',  'prem5.frkn.org', 'PL', 'disabled', 'legacy dedicated, owner not linked (env = custompersonal<uuid>)')
ON CONFLICT (slot_key) DO NOTHING;

-- Backfill 3: premium envs that exist in the DB but are not in the CSV
-- inventory (`custompremium-kyrgyzstan`, `custompremium-a65e52c6af0e`). They are
-- registered as dedicated so the admin panel shows them and detaching works;
-- they are outside stock because stock only counts `kind = 'pool'`.
INSERT INTO premium_slots (slot_key, env, kind, pool, slot_no, address, domain, country,
                           status, assigned_sub, assigned_at, note)
SELECT 'legacy-' || substr(s.scope_env, 15),
       s.scope_env, 'dedicated', 'legacy',
       (row_number() OVER (ORDER BY s.scope_env))::SMALLINT,
       NULL, NULL, NULL, 'assigned', s.id, now(),
       'legacy premium env outside the CSV inventory'
  FROM subscriptions s
 WHERE s.scope_env LIKE 'custompremium-%'
   AND NOT s.is_deleted
   AND s.parent_id IS NULL
   AND NOT EXISTS (SELECT 1 FROM premium_slots ps WHERE ps.env = s.scope_env)
ON CONFLICT (slot_key) DO NOTHING;

-- Backfill 3b: premium envs that exist only on live connections. Two of them
-- (`custompremium26a248937b30`, `custompremium79634f2cb404`) were provisioned by
-- hand, so their subscriptions carry no `scope_env` at all and the block above
-- cannot see them. They are real premium inventory — register them as dedicated
-- (owner = the subscription whose connections live there) so the admin panel
-- shows every premium env, not just the ones the API knows about.
INSERT INTO premium_slots (slot_key, env, kind, pool, slot_no, address, domain, country,
                           status, assigned_sub, assigned_at, note)
SELECT 'conn-' || substr(c.env, 15),
       c.env, 'dedicated', 'legacy-conn',
       (row_number() OVER (ORDER BY c.env))::SMALLINT,
       NULL, NULL, NULL, 'assigned',
       (array_agg(c.subscription_id))[1], now(),
       'premium env found on live connections; subscription has no scope_env'
  FROM connections c
 WHERE c.env LIKE 'custompremium%'
   AND NOT c.is_deleted
   AND NOT EXISTS (SELECT 1 FROM premium_slots ps WHERE ps.env = c.env)
 GROUP BY c.env
ON CONFLICT (slot_key) DO NOTHING;

-- Backfill 4: terms. Everything stays NULL (= indefinite) on purpose: 11 of the
-- 14 premium subscriptions have no payment at all and no agreed end date, so
-- filling a date here would schedule a mass revocation. Only the one agreed
-- term is set. terms are extended later via the admin API.
UPDATE subscriptions
   SET premium_until = TIMESTAMPTZ '2026-11-21'
 WHERE id = 'd2cd1be7-030e-463a-9fb4-52bf00f9733e'   -- prem02-4, premium granted 2026-10-22 for 1 month
   AND premium_until IS NULL;

COMMIT;

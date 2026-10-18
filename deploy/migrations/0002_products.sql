-- Tariff products: keys.product / subscriptions.product.
--
-- The product is stored next to the code, never inside the activation code
-- string. The contract lives in code (fcore::Product):
--
--   standard            — the classic set (enabled_conns, wl included)
--   gaming              — the gaming env only
--   bs-prem             — the bs-prem env only
--   standard-gaming     — classic set + gaming
--   standard-bs-prem    — classic set + bs-prem
--   full                — classic set + gaming + bs-prem
--
-- Every existing row becomes `standard`, which is byte-for-byte the access
-- those subscriptions had before products existed. Applied at deploy time
-- (deploy/migrate.sh), never at api startup.
--
-- Idempotent: the column is added without a default first so that both a
-- fresh column and one created by an interrupted run end up with every row
-- filled and the default set afterwards.

ALTER TABLE keys ADD COLUMN IF NOT EXISTS product TEXT;
UPDATE keys SET product = 'standard'
    WHERE product IS NULL OR product IN ('legacy', 'classic', 'dev');
UPDATE keys SET product = 'standard-gaming' WHERE product = 'dev-gaming';
UPDATE keys SET product = 'standard-bs-prem' WHERE product = 'dev-bs-prem';
ALTER TABLE keys ALTER COLUMN product SET DEFAULT 'standard';
ALTER TABLE keys ALTER COLUMN product SET NOT NULL;

ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS product TEXT;
UPDATE subscriptions SET product = 'standard'
    WHERE product IS NULL OR product IN ('legacy', 'classic', 'dev');
UPDATE subscriptions SET product = 'standard-gaming' WHERE product = 'dev-gaming';
UPDATE subscriptions SET product = 'standard-bs-prem' WHERE product = 'dev-bs-prem';
ALTER TABLE subscriptions ALTER COLUMN product SET DEFAULT 'standard';
ALTER TABLE subscriptions ALTER COLUMN product SET NOT NULL;

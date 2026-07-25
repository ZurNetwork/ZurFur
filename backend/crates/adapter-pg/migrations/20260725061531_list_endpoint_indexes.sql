-- Indexes for the ZMVP-157 listing endpoints. Both are read on every signed-in
-- page load, and both filter tables that grow with EVERY user's data rather
-- than the caller's — so an unindexed predicate is a whole-table scan that any
-- authenticated caller can drive repeatedly.

-- `account_members` is keyed `PRIMARY KEY (account_id, user_id)`. The listing
-- constrains only `user_id` — the TRAILING column — which a composite btree
-- cannot range-seek, so `list_for_user` would scan every membership row on the
-- platform. (Postgres 16 here, per docker-compose; B-tree skip scan is 18+.)
-- The index also serves the `users` foreign key, which Postgres does not index
-- on its own.
CREATE INDEX account_members_by_user ON account_members (user_id);

-- `commission.owner_id` had no index at all: `REFERENCES users (id)` creates a
-- constraint, never an index. Partial on the query's own `archived_at IS NULL`
-- predicate, so the index stays proportional to live commissions rather than to
-- all history.
CREATE INDEX commission_by_owner ON commission (owner_id) WHERE archived_at IS NULL;

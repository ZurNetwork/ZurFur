-- Makes the per-kind actor tables (users, accounts) shared-PK PROJECTIONS of
-- the actor super-table: backfills one `actor_identity` row per existing
-- users/accounts row, then asserts the mapping is unambiguous before a later
-- migration adds the composite FK. See NODE.md
-- ("20260718193956_backfill_actor_identity_from_projections.sql") for the
-- full rationale.

-- Guard 1 — a DID shared between a users row and an accounts row would need TWO
-- identity rows (one per kind), but `actor_identity.did` is UNIQUE, so the backfill
-- could create only one, orphaning the other projection from its (id, kind) parent.
-- This is pre-prod with no real data, and there is no safe automatic merge of two
-- actors' identities, so STOP LOUDLY rather than silently drop one.
DO $$
DECLARE
    shared_dids int;
    shared_ids  int;
BEGIN
    SELECT count(*) INTO shared_dids
    FROM users u
    JOIN accounts a ON u.did = a.did;
    IF shared_dids > 0 THEN
        RAISE EXCEPTION
            'ZMVP-123 backfill aborted: % DID(s) are shared between users and accounts. '
            'Each DID must map to exactly one actor_identity row; there is no automatic '
            'merge. Resolve the duplicate identities, then re-run.', shared_dids;
    END IF;

    -- Guard 2 — a users row and an accounts row sharing an internal id would collide
    -- on the identity PK. Astronomically unlikely for independent UUIDv7 keys, but a
    -- clear diagnostic beats a raw duplicate-key error.
    SELECT count(*) INTO shared_ids
    FROM users u
    JOIN accounts a ON u.id = a.id;
    IF shared_ids > 0 THEN
        RAISE EXCEPTION
            'ZMVP-123 backfill aborted: % id(s) are shared between a users and an accounts '
            'row. Each actor needs its own identity PK.', shared_ids;
    END IF;
END $$;

-- Backfill: one identity row per existing user, then per existing account.
INSERT INTO actor_identity (id, kind, did, state, handle, first_seen)
SELECT id, 'user', did, 'active', NULL, created_at
FROM users
ON CONFLICT (did) DO NOTHING;

INSERT INTO actor_identity (id, kind, did, state, handle, first_seen)
SELECT id, 'account', did, 'active', NULL, created_at
FROM accounts
ON CONFLICT (did) DO NOTHING;

-- Coverage assertion (the composite FK's precondition) — every projection row must
-- now have its EXACT (id, kind, did) parent in actor_identity. Catches the residual
-- case the ON CONFLICT skips: a DID already interned under a DIFFERENT id.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM users u
        WHERE NOT EXISTS (
            SELECT 1 FROM actor_identity ai
            WHERE ai.id = u.id AND ai.kind = 'user' AND ai.did = u.did
        )
    ) THEN
        RAISE EXCEPTION
            'ZMVP-123 backfill aborted: a users row has no matching actor_identity parent '
            '(its DID was already interned under a different id). Reconcile before migrating.';
    END IF;

    IF EXISTS (
        SELECT 1 FROM accounts a
        WHERE NOT EXISTS (
            SELECT 1 FROM actor_identity ai
            WHERE ai.id = a.id AND ai.kind = 'account' AND ai.did = a.did
        )
    ) THEN
        RAISE EXCEPTION
            'ZMVP-123 backfill aborted: an accounts row has no matching actor_identity parent '
            '(its DID was already interned under a different id). Reconcile before migrating.';
    END IF;
END $$;

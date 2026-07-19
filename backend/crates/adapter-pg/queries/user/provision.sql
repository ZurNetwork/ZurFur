-- Insert (or resolve) the User projection row keyed by its shared actor_identity id —
-- the id the caller has just interned the visitor's DID under (ZMVP-123). The DID and
-- the one-DID-one-actor race are the intern step's job now; this only lands the
-- projection. Idempotent on the shared PK: a repeat sign-in resolves to the same
-- identity id, whose users row already exists, so the no-op DO UPDATE lets RETURNING
-- hand back the ORIGINAL created_at. `kind` is filled by its constant column DEFAULT
-- ('user') and never named here.
INSERT INTO users (id, created_at)
VALUES ($1, $2)
ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id
RETURNING id, created_at

-- The account handle-change audit log (ZMVP-46, DD "Account Handle Change Flow"
-- 27852802). One row per successful handle change; append-only.
--
-- This single table backs BOTH policy knobs of the change flow, so there is no
-- second bespoke "rate limit" or "quarantine" store to keep in sync:
--   * Rate limit (§3) — a light anti-abuse throttle: count an account's rows within
--     the recent window (account_id, changed_at index) and refuse a further change
--     past the limit.
--   * Quarantine (§4) — a vacated *.zurfur.app handle stays RESERVED to the account
--     that left it, reclaimable, for a window: a handle appears as `old_handle` in a
--     recent row iff its former holder still has a claim on it (old_handle, changed_at
--     index). The availability check at BOTH claim sites (founding + change) treats a
--     handle quarantined to a *different* account as taken. Within the window a given
--     *.zurfur.app old_handle maps to at most one account (no one else could have held
--     it while it was reserved), so "reserved to another" is unambiguous; once the
--     window passes the row simply stops matching and the handle frees.
--
-- BYO (brought-domain) handles are NOT quarantined (the user owns that DNS), so the
-- availability check only consults this table for the Zurfur namespace; a BYO
-- old_handle recorded here is inert.
--
-- id          UUIDv7 minted app-side; opaque internal key (same convention as accounts.id).
-- account_id  The account whose handle changed. ON DELETE CASCADE: a hard-deleted
--             account frees its handle (DD 23003138), so its quarantine/rate-limit
--             history goes with it — never holding a name for an account that is gone.
-- old_handle  The handle vacated by this change (drives the quarantine reservation).
-- new_handle  The handle adopted. Kept for audit/history (name-history proper is the
--             native did:plc log / future ZMVP-64; this is the private record).
-- changed_at  When the change committed (application-supplied, UTC), the window anchor.
CREATE TABLE account_handle_changes (
    id         uuid        PRIMARY KEY,
    account_id uuid        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    old_handle text        NOT NULL,
    new_handle text        NOT NULL,
    changed_at timestamptz NOT NULL
);

-- Rate-limit read: an account's recent changes.
CREATE INDEX account_handle_changes_account_time ON account_handle_changes (account_id, changed_at);

-- Quarantine read: recent vacations of a given handle.
CREATE INDEX account_handle_changes_old_handle_time ON account_handle_changes (old_handle, changed_at);

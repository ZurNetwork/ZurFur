-- The `account_invitations` table backing invite-then-accept (ZMVP-32 — the
-- issuing half; acceptance is ZMVP-20). A row is a pending offer of account
-- membership until it is accepted or revoked. `role` and `state` are the
-- Role::as_str / InvitationState::as_str discriminants; `inviter` is recorded
-- because on acceptance they become the new member's Parent (DESIGN/Roles rule 4a).
--
-- The partial unique index enforces AC5 — at most one *pending* invitation per
-- (account, invited_user) — while leaving accepted/revoked history free to
-- accumulate, so a revoked offer never blocks a fresh invite.

CREATE TABLE account_invitations (
  id            UUID          PRIMARY KEY,
  account_id    UUID        NOT NULL REFERENCES accounts(id),
  invited_user  UUID        NOT NULL REFERENCES users(id),
  role          TEXT        NOT NULL,
  inviter       UUID        NOT NULL REFERENCES users(id),
  state         TEXT        NOT NULL,
  created_at    TIMESTAMPTZ NOT NULL,
  updated_at    TIMESTAMPTZ NOT NULL

);

CREATE UNIQUE INDEX one_pending_invitation_per_account_user 
  ON account_invitations (account_id, invited_user)
  WHERE state = 'pending';

-- Add migration script here

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

-- The `commission_invitation` table backing seat invite-then-accept (ZMVP-78 —
-- the issuing half; acceptance is ZMVP-79). A row is a pending offer of a
-- commission Seat until it is accepted or revoked — the Seat mirror of
-- `account_invitations` (ZMVP-32). `state` is the InvitationState::as_str
-- discriminant (the same pending|accepted|revoked machine, reused wholesale, no
-- expiry); `inviter` is the owner who issued the offer, recorded like the
-- account-invitation's issuer. Singular table name — the house convention since
-- ZMVP-65 (the plural `account_invitations` is the old inconsistency).
--
-- commission_id  The commission the Seat belongs to. ON DELETE CASCADE:
--                a pending invitation is commission-owned bookkeeping, not a
--                Fact (Deletion DD 3014657) — ZMVP-66's "gone entirely" relies
--                on the cascade sweeping it (the direct cascade onto
--                commission(id), the epic's ruling-E35 convention, so hard-delete
--                sweeps invitations even though it predates this table).
-- seat_id        The Seat the User is offered. ON DELETE CASCADE from the
--                `commission_seat` satellite: pruning the seat's subtree (ZMVP-73)
--                sweeps its pending offers with it.
-- invited_user   The User being invited. They fill the Seat only by accepting
--                (ZMVP-79). FK onto users(id): a live invitee, not history.
-- inviter        The owner who issued the offer (the route's authority gate
--                settles owner-only before a row lands). FK onto users(id).
-- state          The InvitationState::as_str discriminant.
-- created_at     When the offer was issued (equals updated_at at issuance).
-- updated_at     When the offer last changed state (e.g. on revoke).
--
-- The partial unique index enforces at most one *pending* invitation per
-- (seat, invited_user) — while leaving accepted/revoked history free to
-- accumulate, so a revoked offer never blocks a fresh invite. Several *different*
-- Users may hold pending invitations to ONE Seat at once (the acceptance race is
-- ZMVP-79's to resolve, not this table's to forbid); only a duplicate pending for
-- the *same* (seat, user) pair is barred.

CREATE TABLE commission_invitation (
    id            uuid        PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    seat_id       uuid        NOT NULL REFERENCES commission_seat (id) ON DELETE CASCADE,
    invited_user  uuid        NOT NULL REFERENCES users (id),
    inviter       uuid        NOT NULL REFERENCES users (id),
    state         text        NOT NULL,
    created_at    timestamptz NOT NULL,
    updated_at    timestamptz NOT NULL
);

CREATE UNIQUE INDEX one_pending_invitation_per_seat_user
    ON commission_invitation (seat_id, invited_user)
    WHERE state = 'pending';

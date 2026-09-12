-- The member's own optional alias for their role on this account (a
-- free-form label such as an Owner aliased "Studio Head"), carried on the
-- membership row itself — never on `Role`. Nullable: unset on the floor.
ALTER TABLE account_members ADD COLUMN alias text;

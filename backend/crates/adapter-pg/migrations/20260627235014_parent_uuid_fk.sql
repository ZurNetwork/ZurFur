-- ZMVP-20: the role-tree parent edge is the inviter's internal UserId — a UUIDv7
-- minted app-side (`users.id`), NOT a DID and not a loose string. Promote
-- `account_members.parent` from `text` to a real `uuid` foreign key now, while the
-- column is still entirely NULL (this ticket is its first writer). This reverses the
-- earlier "store the string form / strong typing deferred" stance, taken when nothing
-- wrote the column; doing it now — before any data exists — is free.
--
-- `parent` stays NULLable on purpose: an Owner has no parent (Roles rule 5). The
-- USING cast is safe precisely because every existing value is NULL.
ALTER TABLE account_members
    ALTER COLUMN parent TYPE uuid USING parent::uuid,
    ADD CONSTRAINT account_members_parent_fkey FOREIGN KEY (parent) REFERENCES users (id);

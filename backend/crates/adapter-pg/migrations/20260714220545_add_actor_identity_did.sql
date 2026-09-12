-- actor_identity: the optional DID column.
-- NULLABLE: actor-ness is anchored on the internal
-- id, not a DID — Characters are actors and carry no DID. UNIQUE binds only
-- DID-bearing rows (Postgres treats NULLs as distinct), giving one-DID-one-actor
-- across every kind — the invariant a partitioned did column could never enforce.
ALTER TABLE actor_identity
    ADD COLUMN did text UNIQUE;

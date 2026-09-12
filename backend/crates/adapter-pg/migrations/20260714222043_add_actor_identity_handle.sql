-- actor_identity: the cached handle column.
-- A refreshable display cache of the actor's atproto handle — never the
-- authoritative claim (that lives with the Account handle machinery) and never
-- validated against Zurfur's claim rules (external handles are foreign). NULL =
-- no handle cached (DID-less actors, or simply not fetched yet). Deliberately
-- NOT unique: a cache may transiently disagree with the network.
ALTER TABLE actor_identity
    ADD COLUMN handle text;

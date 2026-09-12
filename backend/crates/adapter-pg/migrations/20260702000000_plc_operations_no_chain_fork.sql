-- A partial UNIQUE index over (did, prev) makes a non-genesis fork of a
-- did:plc operation chain unrepresentable: two concurrent handle updates
-- racing to append onto the same `prev` can no longer both succeed. See
-- NODE.md ("20260702000000_plc_operations_no_chain_fork.sql") for the full
-- rationale.
CREATE UNIQUE INDEX plc_operations_did_prev_unique
    ON plc_operations (did, prev)
    WHERE prev IS NOT NULL;

-- ZMVP-50 (security-review F1): a minted did:plc is a strictly LINEAR chain of
-- operations — every non-genesis op chains onto exactly one `prev` (the CID of the
-- op before it), and a given op may be chained onto AT MOST ONCE. Without this,
-- two concurrent handle updates both read the same `latest_cid` as their `prev`,
-- build different operations (different `cid`, so the `UNIQUE(cid)` index does not
-- catch them), and both append — FORKING the local chain. `latest_cid`
-- (ORDER BY seq DESC) would then return whichever landed last, and at launch our
-- log would permanently disagree with the canonical directory's real tip (the
-- directory accepts only the first op chaining a given `prev`; the fork is signed
-- by the same operational key, so there is no higher-authority override to resolve
-- it) — wedging every future handle change and tripping the ZMVP-51 monitor.
--
-- A partial UNIQUE index over (did, prev) makes a non-genesis fork UNREPRESENTABLE:
-- the losing concurrent writer's INSERT fails, and `update_handle`'s benign-replay
-- guard (which returns Ok only when the log's tip already IS our op) sees a
-- DIFFERENT tip, propagates the error, and the caller's retry re-reads the new tip
-- and chains onto it — serializing concurrent writers into one linear chain.
--
-- Scoped `WHERE prev IS NOT NULL`: a genesis op has `prev = NULL` and is already
-- one-per-DID by construction (its hash defines the DID), and Postgres treats NULLs
-- as distinct, so genesis rows are correctly exempt.
CREATE UNIQUE INDEX plc_operations_did_prev_unique
    ON plc_operations (did, prev)
    WHERE prev IS NOT NULL;

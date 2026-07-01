-- Append-only log of the PLC operations Zurfur has submitted for each minted
-- account did:plc (ZMVP-34 tombstone; reused by ZMVP-50 alsoKnownAs updates and
-- the ZMVP-51 transparency-log monitor). DD "Account Deletion, Tombstoning &
-- Handle Reuse" (DESIGN/23003138) + DD 26804226 (custody).
--
-- A did:plc is a chain of operations: every non-genesis operation references the
-- CID of the DID's most recent operation as its `prev`. We do not (in v1) fetch
-- that chain back from the canonical directory — submission is a gated no-op until
-- launch — so we keep our own record of what we published, both to chain the next
-- operation and to audit it against plc.directory later.
--
-- seq        Monotonic insertion order; the DID's latest operation is its highest
--            `seq`. Surrogate key so the same `cid` can never wedge an insert.
-- did        The account did:plc this operation belongs to. Deliberately NO foreign
--            key to accounts(did): the genesis op is logged during minting, BEFORE
--            the account row exists (same reason account_keys carries no FK), and a
--            tombstone is logged as the account row is being hard-deleted.
-- cid        The content id (CIDv1 / dag-cbor / sha-256, base32 `b…`) of the signed
--            operation — the value a subsequent operation references as its `prev`.
--            Globally unique by construction (content-addressed).
-- type       The operation `type` discriminant: `plc_operation` (genesis) or
--            `plc_tombstone` (and rotation/update types as they are built).
-- prev       The CID this operation chained onto, or NULL for a genesis operation.
-- operation  The signed operation as submitted, JSON. Kept for audit/replay; never
--            contains private key material (only public keys, handle, and a sig).
-- created_at When the operation was logged. Application-supplied (no DEFAULT now()),
--            matching the codebase convention.
CREATE TABLE plc_operations (
    seq        bigserial   PRIMARY KEY,
    did        text        NOT NULL,
    cid        text        NOT NULL UNIQUE,
    type       text        NOT NULL,
    prev       text,
    operation  jsonb       NOT NULL,
    created_at timestamptz NOT NULL
);

-- The hot path is "the DID's most recent operation" (its `prev` for the next op):
-- filter by did, take the highest seq.
CREATE INDEX plc_operations_did_seq ON plc_operations (did, seq DESC);

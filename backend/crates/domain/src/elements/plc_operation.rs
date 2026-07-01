//! [`PlcOperationRecord`] — one entry in the append-only log of `did:plc`
//! operations Zurfur has submitted for a minted account identity.
//!
//! A `did:plc` is a signed chain of operations (genesis, then rotations, updates,
//! and finally a tombstone). Each non-genesis operation references the CID of the
//! DID's most recent operation as its `prev`, so to build the next operation we must
//! know the last one's CID. In v1 the canonical directory is a gated no-op, so
//! Zurfur keeps its own record of what it published — enough to chain the next
//! operation and to audit against `plc.directory` later (ZMVP-34, DD/23003138;
//! reused by ZMVP-50/51). Persisted through [`crate::ports::PlcOperationLog`].

use crate::elements::did::Did;

/// One submitted `did:plc` operation, as recorded in the operation log.
///
/// Carries just enough to chain the next operation (`cid` becomes that operation's
/// `prev`) and to audit what was published. The signed operation itself is kept as
/// its JSON text ([`operation_json`](PlcOperationRecord::operation_json)) — the exact
/// body submitted (or that would be submitted) to the directory. It contains only
/// public material (rotation/verification `did:key`s, the handle, and a signature),
/// never a private key.
pub struct PlcOperationRecord {
    /// The account `did:plc` this operation belongs to.
    pub did: Did,
    /// The content id of the signed operation — CIDv1, `dag-cbor` codec, `sha-256`
    /// multihash, base32 (`b…`). A subsequent operation references this as its `prev`.
    pub cid: String,
    /// The operation `type` discriminant: `"plc_operation"` (a full operation, e.g.
    /// the genesis) or `"plc_tombstone"`.
    pub op_type: String,
    /// The CID this operation chained onto, or `None` for a genesis operation.
    pub prev: Option<String>,
    /// The signed operation serialized as JSON — exactly the body submitted to the
    /// directory. Never contains private key material.
    pub operation_json: String,
}

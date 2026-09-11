//! [`PlcOperationRecord`] — one entry in the append-only log of `did:plc`
//! operations Zurfur has submitted for a minted account identity.
//!
//! Each non-genesis operation cites the previous operation's CID as its `prev`,
//! so the log is what lets the next operation be chained, and what an audit
//! against `plc.directory` compares. Persisted through
//! [`crate::ports::PlcOperationLog`]. (DD 26804226)

use crate::elements::did::Did;

/// One submitted `did:plc` operation, as recorded in the operation log — enough
/// to chain the next operation and to audit what was published. The stored
/// operation body carries only public material, never a private key.
pub struct PlcOperationRecord {
    /// The account `did:plc` this operation belongs to.
    pub did: Did,
    /// The content id of the signed operation — CIDv1, `dag-cbor`, `sha-256`,
    /// base32. The next operation cites this as its `prev`.
    pub cid: String,
    /// The operation `type` discriminant: `"plc_operation"` or `"plc_tombstone"`.
    pub op_type: String,
    /// The CID this operation chained onto, or `None` for a genesis operation.
    pub prev: Option<String>,
    /// The signed operation serialized as JSON — exactly the body submitted to
    /// the directory. Never contains private key material.
    pub operation_json: String,
}

//! The [`Fact`] contract (ZMVP-67): what it means for a type to be
//! commission-anchored evidence that work happened.
//!
//! A **fact** is anything whose existence would be orphaned by destroying its
//! commission — the Deletion DD (`3014657`) names the canonical trigger list:
//! Products, ratings, EXP, achievements, payment records. A commission bearing
//! any fact can never be hard-deleted, only archived; the delete/archive gates
//! (ZMVP-66/68) answer that question through the
//! [`commission_has_facts`](crate::ports::CommissionWrites::commission_has_facts)
//! port, never ad-hoc checks. No production implementor exists yet — every fact
//! kind is a future ticket — so today the port answers `false` for every
//! commission by construction.

use super::CommissionId;

/// A commission-anchored fact: implementing this trait is what makes a type
/// **fact-bearing** (ZMVP-67 AC1).
///
/// The contract is deliberately minimal (conductor ruling E18): a marker plus
/// the one obligation every fact shares — naming the commission it anchors to.
/// No storage, rendering, or lifecycle machinery lives here; a fact's only
/// domain-wide meaning is "this commission has evidence attached and may not be
/// hard-deleted" (Deletion DD `3014657`).
///
/// **Registry duty — read before implementing.** There is no dynamic registry
/// and no blanket impl: the compile-time trait implementation and the runtime
/// predicate are kept in sync by hand, under test. Every implementor's storage
/// **must join the `commission_has_facts` query in the same change** that
/// introduces it — in the pg adapter that means registering the new table in
/// `COMMISSION_FACT_TABLES` (a schema tripwire test fails until the table is
/// classified, and a compile-time guard then refuses the constant-`false`
/// predicate) and mirroring the check in the mem fake. A `Fact` implementor
/// whose rows the predicate cannot see would let the delete gate destroy the
/// very evidence the trait exists to protect.
pub trait Fact {
    /// The commission this fact anchors to — the row a hard delete would orphan.
    fn anchor(&self) -> CommissionId;
}

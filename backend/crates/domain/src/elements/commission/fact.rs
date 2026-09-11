//! The [`Fact`] contract: what it means for a type to be commission-anchored
//! evidence that work happened. A commission bearing any fact can never be hard
//! deleted, only archived. (DD 3014657)

use super::CommissionId;

/// A commission-anchored fact: implementing this trait is what makes a type
/// fact-bearing. A marker plus the one obligation every fact shares — naming
/// the commission it anchors to.
///
/// **Registry duty — read before implementing.** There is no dynamic registry:
/// every implementor's storage must join the `commission_has_facts` query in
/// the same change (pg: register the table in `COMMISSION_FACT_TABLES`; mirror
/// the check in the mem fake). A `Fact` whose rows the predicate cannot see
/// would let the delete gate destroy the evidence the trait exists to protect.
pub trait Fact {
    /// The commission this fact anchors to — the row a hard delete would orphan.
    fn anchor(&self) -> CommissionId;
}

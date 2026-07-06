//! The Fact contract (ZMVP-67): implementing [`Fact`] is what makes a type
//! fact-bearing, and every fact anchors to exactly one commission. No production
//! implementor exists yet (Products, ratings, EXP, achievements, payments are all
//! future tickets — Deletion DD `3014657`), so a stub implementor exercises the
//! contract's shape here.

use domain::elements::commission::{CommissionId, Fact};

/// A stand-in fact-bearing type: the minimal shape a future fact (a Product, a
/// rating, …) will have — some payload plus the commission it anchors to.
struct StubFact {
    anchored_to: CommissionId,
}

impl Fact for StubFact {
    fn anchor(&self) -> CommissionId {
        self.anchored_to
    }
}

/// AC1: a type becomes fact-bearing by implementing [`Fact`], and the trait's one
/// obligation is naming the commission the fact anchors to.
#[test]
fn a_fact_bearing_type_reports_its_commission_anchor() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let fact = StubFact {
        anchored_to: commission,
    };
    assert_eq!(fact.anchor(), commission);
}

/// The trait stays object-safe: gates and registries may hold facts behind
/// `dyn Fact` without knowing the concrete kind. A compile-time contract —
/// adding a non-dispatchable method to [`Fact`] breaks this test's build.
#[test]
fn fact_is_object_safe() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let fact = StubFact {
        anchored_to: commission,
    };
    let dynamic: &dyn Fact = &fact;
    assert_eq!(dynamic.anchor(), commission);
}

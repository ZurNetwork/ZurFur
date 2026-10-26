use kernel::elements::maturity::Maturity;

use super::CommissionState;

/// A rating a rule passed: the rating held, if any, and the one given. Built only by
/// [`rate_maturity`]; the value the store's rating write takes, and what its entry records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaturitySet {
    from: Option<Maturity>,
    to: Maturity,
}

impl MaturitySet {
    /// The rating held before, or `None` while it was unrated.
    pub fn before(&self) -> Option<Maturity> {
        self.from
    }

    /// The rating given.
    pub fn after(&self) -> Maturity {
        self.to
    }
}

/// Rates `state` `to`, replacing whatever rating it holds; `None` when it already holds `to`,
/// rating and Graphic flag alike. No state refuses it, and nothing clears a rating.
pub fn rate_maturity(state: &CommissionState, to: Maturity) -> Option<MaturitySet> {
    let from = state.maturity();
    (from != Some(to)).then_some(MaturitySet { from, to })
}

#[cfg(test)]
mod tests;

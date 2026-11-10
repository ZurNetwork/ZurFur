/// The role a User holds on one commission, ordered from least authority to most:
/// `Participant < Owner`. A variant added later takes its place in that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, strum::VariantArray)]
pub enum CommissionRole {
    /// Holds a participant row on the commission.
    Participant,
    /// Owns the commission.
    Owner,
}

impl CommissionRole {
    /// The role read from two facts: Owner when the viewer owns the commission, else
    /// Participant when they hold a participant row, else none. Not `From`: it reads
    /// two separate facts, and no role at all is an answer.
    pub fn held(owns: bool, participates: bool) -> Option<Self> {
        if owns {
            return Some(Self::Owner);
        }
        participates.then_some(Self::Participant)
    }
}

#[cfg(test)]
mod tests;

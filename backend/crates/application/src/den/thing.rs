use domain::elements::{account::AccountId, commission::CommissionId};

/// The thing an open node stands for, by the id a typed write call acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenThing {
    /// An Account, by its DID.
    Account(AccountId),
    /// A commission, by its id.
    Commission(CommissionId),
}

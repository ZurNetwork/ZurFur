//! The fixture-account seam ZMVP-105 binds to: everything a test needs to act
//! as a provisioned identity against a throwaway PDS.

/// A fixture account provisioned on a throwaway PDS — the contract downstream
/// adapter tests (ZMVP-105) construct their authenticated atproto client from.
///
/// `#[non_exhaustive]`: the seam may grow fields without breaking consumers;
/// construct it only through [`crate::ThrowawayPds::provision_account`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FixtureAccount {
    /// Base URL of the PDS hosting this account: `http://127.0.0.1:{mapped}`
    /// (host + the container's dynamically mapped port).
    pub endpoint: String,
    /// The account's `did:plc:…`, minted against the harness's stub PLC.
    pub did: String,
    /// The account's handle (a `.test` handle, e.g. `alice.test`).
    pub handle: String,
    /// How to act as this account. Extensible — see [`ActingCredential`].
    pub credential: ActingCredential,
}

/// The credential a test acts with — deliberately an extensible enum, **not**
/// a bare secret string.
///
/// ZMVP-105 still holds an open fork on how the adapter authenticates
/// (Jacquard OAuth vs the PDS's local credentials); this seam must not
/// pre-commit it. `#[non_exhaustive]` forces downstream matches to carry a
/// wildcard arm, so adding an OAuth (or other) variant later is not a
/// breaking change.
///
/// The contained tokens authenticate against a throwaway localhost container
/// that is destroyed on drop — they protect nothing durable.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ActingCredential {
    /// An access/refresh JWT pair from `com.atproto.server.createAccount` —
    /// a live session on the throwaway PDS, usable as a Bearer token.
    PdsSession {
        access_jwt: String,
        refresh_jwt: String,
    },
}

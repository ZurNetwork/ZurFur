/// The longest a whole handle may be, in `char`s.
pub const HANDLE_MAX_LEN: usize = 253;

/// The longest a single handle label (dot-separated segment) may be.
pub const LABEL_MAX_LEN: usize = 63;

/// The Zurfur-issued handle namespace, gated by [`RESERVED_LABELS`].
pub(super) const ZURFUR_NAMESPACE_SUFFIX: &str = ".zurfur.app";

/// Top-level domains the atproto handle spec disallows: reserved or
/// non-resolvable names that must fail resolution. An [`AtHandle`](super::AtHandle)
/// refuses these.
pub(super) const DISALLOWED_TLDS: [&str; 8] = [
    "alt",
    "arpa",
    "example",
    "internal",
    "invalid",
    "local",
    "localhost",
    "onion",
];

/// Top-level domains no real-world handle or identity host uses: the spec's
/// disallowed eight plus `test`. A claimed [`Handle`](super::Handle) refuses
/// these, and so does every identity lookup and fetch.
pub const RESERVED_TLDS: [&str; 9] = with_test(DISALLOWED_TLDS);

/// `disallowed` followed by `test`; derived, so the two lists cannot drift.
const fn with_test(disallowed: [&'static str; 8]) -> [&'static str; 9] {
    let mut reserved = ["test"; 9];
    let mut index = 0;
    while index < disallowed.len() {
        reserved[index] = disallowed[index];
        index += 1;
    }
    reserved
}

/// Labels Zurfur withholds from its own `*.zurfur.app` namespace. Checked
/// against the leftmost label only; a BYO domain is never gated by this set.
pub(super) const RESERVED_LABELS: &[&str] = &[
    // infra / service
    "api",
    "admin",
    "www",
    "app",
    "cdn",
    "assets",
    "static",
    "media",
    "blob",
    "status",
    "health",
    "metrics",
    // auth / identity
    "auth",
    "login",
    "logout",
    "signin",
    "signup",
    "oauth",
    "sso",
    "account",
    "accounts",
    "did",
    "plc",
    // comms / abuse
    "mail",
    "smtp",
    "support",
    "help",
    "contact",
    "abuse",
    "security",
    "postmaster",
    "webmaster",
    "hostmaster",
    "noc",
    // brand / staff
    "zurfur",
    "official",
    "root",
    "system",
    "staff",
    "team",
    "moderator",
    "mod",
    // protocol / well-known
    "well-known",
    "atproto",
    "xrpc",
    "ns",
];

#[cfg(test)]
mod tests;

/// The longest a whole handle may be, in `char`s.
pub const HANDLE_MAX_LEN: usize = 253;

/// The longest a single handle label (dot-separated segment) may be.
pub const LABEL_MAX_LEN: usize = 63;

/// The Zurfur-issued handle namespace, gated by [`RESERVED_LABELS`].
pub(super) const ZURFUR_NAMESPACE_SUFFIX: &str = ".zurfur.app";

/// Top-level domains the atproto handle spec forbids as handles.
pub(super) const RESERVED_TLDS: &[&str] = &[
    "alt",
    "arpa",
    "example",
    "internal",
    "invalid",
    "local",
    "localhost",
    "onion",
    "test",
];

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

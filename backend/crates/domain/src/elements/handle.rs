//! The [`Handle`] — a validated, normalized atproto-style Account handle — and
//! the [`HandleDomain`] it may live under.
//!
//! This module is the one shared validation gate every claim source funnels
//! through: constructing a [`Handle`] enforces normalization, the
//! charset/segment/length rules, an outright reject of any `xn--` punycode
//! label, and the Zurfur reserved-label reject in a single pass. Namespace membership is
//! [`Handle::is_in_namespace`], against a [`HandleDomain`] parsed once at config
//! load, so the claim checks and the resolver cannot disagree.

use std::str::FromStr;

/// The longest a whole handle may be, in `char`s.
pub const HANDLE_MAX_LEN: usize = 253;

/// The longest a single handle label (dot-separated segment) may be.
pub const LABEL_MAX_LEN: usize = 63;

/// The Zurfur-issued handle namespace, gated by [`RESERVED_LABELS`].
const ZURFUR_NAMESPACE_SUFFIX: &str = ".zurfur.app";

/// Top-level domains the atproto handle spec forbids as handles.
const RESERVED_TLDS: &[&str] = &[
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
const RESERVED_LABELS: &[&str] = &[
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

/// A validated, normalized atproto-style Account handle. The stored value is
/// always lowercase, trimmed, and has no trailing dot.
///
/// ```
/// use domain::elements::handle::Handle;
///
/// // Normalized: trimmed, lowercased, trailing dot stripped.
/// let h = "  Alice.Zurfur.APP.  ".parse::<Handle>().unwrap();
/// assert_eq!(h.as_str(), "alice.zurfur.app");
///
/// // A brought (BYO) domain is fine.
/// assert!("alice.example.com".parse::<Handle>().is_ok());
///
/// // Punycode labels are rejected outright.
/// assert!("xn--80ak6aa92e.zurfur.app".parse::<Handle>().is_err());
///
/// // Reserved labels in the *.zurfur.app namespace are rejected...
/// assert!("api.zurfur.app".parse::<Handle>().is_err());
/// // ...but the same word is claimable on a BYO domain.
/// assert!("api.example.com".parse::<Handle>().is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle(String);

/// Why a string was rejected as a [`Handle`] — one variant per failure class,
/// each rendering a human message via [`Display`](std::fmt::Display).
///
/// ```
/// use domain::elements::handle::{Handle, HandleError};
///
/// assert_eq!("".parse::<Handle>(), Err(HandleError::Empty));
/// assert_eq!("alice".parse::<Handle>(), Err(HandleError::TooFewSegments));
/// assert_eq!("foo.local".parse::<Handle>(), Err(HandleError::ReservedTld("local".into())));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleError {
    /// Empty once normalized.
    Empty,
    /// Longer than [`HANDLE_MAX_LEN`] chars overall; carries the length.
    TooLong(usize),
    /// Fewer than two dot-separated segments (e.g. bare `"alice"`).
    TooFewSegments,
    /// A dot-separated segment is empty (e.g. `"alice..app"`).
    EmptySegment,
    /// A segment is longer than [`LABEL_MAX_LEN`] chars; carries the length.
    SegmentTooLong(usize),
    /// A character outside the `[a-z0-9-]` charset; carries the char.
    InvalidChar(char),
    /// A segment starts or ends with a hyphen.
    HyphenEdge,
    /// The rightmost (top-level) segment starts with a digit.
    TldLeadingDigit,
    /// The rightmost segment is a reserved TLD (e.g. `.local`); carries it.
    ReservedTld(String),
    /// Some label begins with `xn--`. Rejected in both namespaces.
    PunycodeLabel,
    /// The leftmost label of a `*.zurfur.app` handle is reserved; carries it.
    ReservedLabel(String),
}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleError::Empty => write!(f, "handle must not be empty"),
            HandleError::TooLong(len) => {
                write!(f, "handle is {len} chars; the max is {HANDLE_MAX_LEN}")
            }
            HandleError::TooFewSegments => write!(
                f,
                "handle must have at least two segments (e.g. `name.zurfur.app`)"
            ),
            HandleError::EmptySegment => write!(f, "handle has an empty segment"),
            HandleError::SegmentTooLong(len) => {
                write!(
                    f,
                    "handle segment is {len} chars; the max is {LABEL_MAX_LEN}"
                )
            }
            HandleError::InvalidChar(c) => write!(
                f,
                "handle contains an invalid character {c:?}; only a-z, 0-9, and '-' are allowed"
            ),
            HandleError::HyphenEdge => {
                write!(f, "a handle segment must not start or end with a hyphen")
            }
            HandleError::TldLeadingDigit => {
                write!(
                    f,
                    "the rightmost handle segment must not start with a digit"
                )
            }
            HandleError::ReservedTld(tld) => {
                write!(
                    f,
                    "`.{tld}` is a reserved top-level domain and cannot be a handle"
                )
            }
            HandleError::PunycodeLabel => {
                write!(f, "punycode (`xn--`) handle labels are not allowed")
            }
            HandleError::ReservedLabel(label) => write!(
                f,
                "`{label}` is a reserved label in the .zurfur.app namespace"
            ),
        }
    }
}

impl std::error::Error for HandleError {}

impl FromStr for Handle {
    type Err = HandleError;

    /// Validate and wrap a handle, enforcing every rule in one pass: normalize
    /// (trim, lowercase, strip one trailing dot), then check length, segment
    /// count and shape, charset, the `xn--` reject, the TLD rules, and — in the
    /// Zurfur namespace only, apex included — the reserved leftmost label.
    ///
    /// ```
    /// use domain::elements::handle::{Handle, HandleError};
    ///
    /// assert_eq!("alice.zurfur.app".parse::<Handle>().unwrap().as_str(), "alice.zurfur.app");
    /// assert_eq!("XN--abc.com".parse::<Handle>(), Err(HandleError::PunycodeLabel)); // case-insensitive
    /// assert_eq!("admin.zurfur.app".parse::<Handle>(), Err(HandleError::ReservedLabel("admin".into())));
    /// // The bare platform apex is reserved too — no one claims the Zurfur root handle.
    /// assert_eq!("zurfur.app".parse::<Handle>(), Err(HandleError::ReservedLabel("zurfur".into())));
    /// ```
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        // 1. NORMALIZE: trim, lowercase, strip a single trailing dot (FQDN root).
        let lowered = raw.trim().to_lowercase();
        let normalized = lowered.strip_suffix('.').unwrap_or(&lowered).to_owned();

        // 2. Overall length.
        if normalized.is_empty() {
            return Err(HandleError::Empty);
        }
        let len = normalized.chars().count();
        if len > HANDLE_MAX_LEN {
            return Err(HandleError::TooLong(len));
        }

        // 3. Segments: at least two, none empty.
        let labels: Vec<&str> = normalized.split('.').collect();
        if labels.len() < 2 {
            return Err(HandleError::TooFewSegments);
        }
        if labels.iter().any(|label| label.is_empty()) {
            return Err(HandleError::EmptySegment);
        }

        // 4. Per-label charset / length / hyphen-edge (every label, before any
        //    punycode rejection — so a malformed label reports its real fault).
        for label in &labels {
            let label_len = label.chars().count();
            if label_len > LABEL_MAX_LEN {
                return Err(HandleError::SegmentTooLong(label_len));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(HandleError::HyphenEdge);
            }
            if let Some(bad) = label
                .chars()
                .find(|&c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            {
                return Err(HandleError::InvalidChar(bad));
            }
        }

        // 5. Punycode reject (ZMVP-48): any label beginning with `xn--`. The form
        //    is already lowercased, so a plain prefix check is case-insensitive.
        if labels.iter().any(|label| label.starts_with("xn--")) {
            return Err(HandleError::PunycodeLabel);
        }

        // 6. The rightmost (top-level) segment must not start with a digit.
        let tld = *labels.last().expect("at least two labels checked above");
        if tld.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Err(HandleError::TldLeadingDigit);
        }

        // 7. Reserved TLDs.
        if RESERVED_TLDS.contains(&tld) {
            return Err(HandleError::ReservedTld(tld.to_owned()));
        }

        // 8. Reserved labels — the Zurfur namespace only (ZMVP-45), leftmost label.
        // The bare platform apex `zurfur.app` has no label in front of it, so it
        // never matches the leading-dot suffix on its own — but its own leftmost
        // label is "zurfur", which is already in RESERVED_LABELS. Folding the apex
        // into the same namespace test (the suffix minus its leading dot) routes it
        // through the one gate instead of adding a parallel special case.
        let zurfur_apex = &ZURFUR_NAMESPACE_SUFFIX[1..];
        if normalized == zurfur_apex || normalized.ends_with(ZURFUR_NAMESPACE_SUFFIX) {
            let leftmost = labels[0];
            if RESERVED_LABELS.contains(&leftmost) {
                return Err(HandleError::ReservedLabel(leftmost.to_owned()));
            }
        }

        Ok(Self(normalized))
    }
}

impl Handle {
    /// The normalized handle string (lowercase, trimmed, no trailing dot).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this handle is a strict subdomain of `domain`'s namespace — the
    /// bare apex is not, and neither is a look-alike without a label boundary.
    /// The one namespace test the whole system shares.
    ///
    /// ```
    /// use domain::elements::handle::{Handle, HandleDomain};
    ///
    /// let zurfur: HandleDomain = "zurfur.app".parse().unwrap();
    /// assert!("alice.zurfur.app".parse::<Handle>().unwrap().is_in_namespace(&zurfur));
    /// assert!(!"alice.example.com".parse::<Handle>().unwrap().is_in_namespace(&zurfur));
    /// // A look-alike that only *ends* with the domain's text is not a member:
    /// // membership needs a real label boundary.
    /// assert!(!"notzurfur.app".parse::<Handle>().unwrap().is_in_namespace(&zurfur));
    /// ```
    pub fn is_in_namespace(&self, domain: &HandleDomain) -> bool {
        self.0
            .strip_suffix(domain.as_str())
            .is_some_and(|prefix| prefix.ends_with('.'))
    }
}

/// The DNS namespace Zurfur issues Account handles under, as deployment
/// configures it — normalized (trimmed, lowercased, outer dots stripped) and
/// parsed once at config load so no call site re-normalizes it. Only an empty
/// result is rejected; stricter DNS-label validation is an open follow-up.
///
/// ```
/// use domain::elements::handle::HandleDomain;
///
/// let domain: HandleDomain = "  Zurfur.App.  ".parse().unwrap();
/// assert_eq!(domain.as_str(), "zurfur.app");
/// assert_eq!(domain.to_string(), "zurfur.app");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleDomain(String);

/// Why a string was rejected as a [`HandleDomain`].
///
/// ```
/// use domain::elements::handle::{HandleDomain, HandleDomainError};
///
/// assert_eq!("  . ".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleDomainError {
    /// Empty once normalized.
    Empty,
}

impl std::fmt::Display for HandleDomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleDomainError::Empty => write!(f, "the handle domain must not be empty"),
        }
    }
}

impl std::error::Error for HandleDomainError {}

impl FromStr for HandleDomain {
    type Err = HandleDomainError;

    /// Normalize and wrap a configured handle namespace: trim, lowercase, and
    /// strip leading and trailing dots. Only an empty result is rejected — an
    /// empty namespace would make every handle look like a member. Stricter
    /// DNS-label validation is an open follow-up.
    ///
    /// ```
    /// use domain::elements::handle::{HandleDomain, HandleDomainError};
    ///
    /// assert_eq!(".zurfur.app.".parse::<HandleDomain>().unwrap().as_str(), "zurfur.app");
    /// assert_eq!("".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
    /// ```
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let normalized = raw.trim().trim_matches('.').to_lowercase();
        if normalized.is_empty() {
            return Err(HandleDomainError::Empty);
        }
        Ok(Self(normalized))
    }
}

impl HandleDomain {
    /// The normalized namespace string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for HandleDomain {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for HandleDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for Handle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests;

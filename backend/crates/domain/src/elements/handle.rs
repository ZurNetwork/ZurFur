//! The [`Handle`] — a validated, normalized atproto-style Account handle — and
//! the [`HandleDomain`] it may live under. (DD 24870914)
//!
//! This module is the one shared validation gate every claim source funnels
//! through: constructing a [`Handle`] enforces normalization, the
//! charset/segment/length rules, the `xn--` punycode reject (DD 26050561), and
//! the Zurfur reserved-label reject in a single pass. Namespace membership is
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
/// // Punycode labels are rejected outright (ZMVP-48).
/// assert!("xn--80ak6aa92e.zurfur.app".parse::<Handle>().is_err());
///
/// // Reserved labels in the *.zurfur.app namespace are rejected (ZMVP-45)...
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
    /// Some label begins with `xn--`. Rejected in both namespaces. (DD 26050561)
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
mod tests {
    use super::*;

    // ---- Normalization -----------------------------------------------------

    #[test]
    fn lowercases_the_handle() {
        assert_eq!(
            "Alice.Zurfur.APP".parse::<Handle>().unwrap().as_str(),
            "alice.zurfur.app"
        );
    }

    #[test]
    fn strips_a_single_trailing_dot() {
        assert_eq!(
            "alice.zurfur.app.".parse::<Handle>().unwrap().as_str(),
            "alice.zurfur.app"
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            "  alice.example.com  ".parse::<Handle>().unwrap().as_str(),
            "alice.example.com"
        );
    }

    // ---- Charset / segment / length ---------------------------------------

    #[test]
    fn rejects_a_single_segment() {
        assert_eq!("alice".parse::<Handle>(), Err(HandleError::TooFewSegments));
    }

    #[test]
    fn rejects_an_empty_input() {
        assert_eq!("   ".parse::<Handle>(), Err(HandleError::Empty));
        assert_eq!(".".parse::<Handle>(), Err(HandleError::Empty));
    }

    #[test]
    fn rejects_an_empty_segment() {
        assert_eq!(
            "alice..app".parse::<Handle>(),
            Err(HandleError::EmptySegment)
        );
    }

    #[test]
    fn rejects_a_segment_over_63_chars() {
        let long_label = "a".repeat(64);
        assert_eq!(
            format!("{long_label}.app").parse::<Handle>(),
            Err(HandleError::SegmentTooLong(64))
        );
    }

    #[test]
    fn rejects_a_handle_over_253_chars() {
        // Build a >253-char handle out of legal 63-char labels.
        let label = "a".repeat(63);
        let raw = format!("{label}.{label}.{label}.{label}.com"); // 4*63 + 3 + 4 = 259
        let len = raw.chars().count();
        assert_eq!(raw.parse::<Handle>(), Err(HandleError::TooLong(len)));
    }

    #[test]
    fn rejects_a_leading_or_trailing_hyphen() {
        assert_eq!("-alice.app".parse::<Handle>(), Err(HandleError::HyphenEdge));
        assert_eq!("alice-.app".parse::<Handle>(), Err(HandleError::HyphenEdge));
    }

    #[test]
    fn rejects_out_of_charset_bytes() {
        assert_eq!(
            "ali_ce.app".parse::<Handle>(),
            Err(HandleError::InvalidChar('_'))
        );
        assert_eq!(
            "ali ce.app".parse::<Handle>(),
            Err(HandleError::InvalidChar(' '))
        );
        assert_eq!(
            "café.app".parse::<Handle>(),
            Err(HandleError::InvalidChar('é'))
        );
    }

    #[test]
    fn rejects_a_digit_leading_tld() {
        assert_eq!(
            "alice.123".parse::<Handle>(),
            Err(HandleError::TldLeadingDigit)
        );
    }

    // ---- Reserved TLDs -----------------------------------------------------

    #[test]
    fn rejects_reserved_tlds() {
        assert_eq!(
            "foo.local".parse::<Handle>(),
            Err(HandleError::ReservedTld("local".into()))
        );
        assert_eq!(
            "foo.test".parse::<Handle>(),
            Err(HandleError::ReservedTld("test".into()))
        );
        assert_eq!(
            "foo.onion".parse::<Handle>(),
            Err(HandleError::ReservedTld("onion".into()))
        );
    }

    // ---- Punycode (ZMVP-48) -----------------------------------------------

    #[test]
    fn rejects_punycode_zurfur_label() {
        assert_eq!(
            "xn--80ak6aa92e.zurfur.app".parse::<Handle>(),
            Err(HandleError::PunycodeLabel)
        );
    }

    #[test]
    fn rejects_punycode_byo_domain() {
        assert_eq!(
            "xn--e1awd7f.com".parse::<Handle>(),
            Err(HandleError::PunycodeLabel)
        );
    }

    #[test]
    fn rejects_punycode_anywhere_and_mixed_case() {
        // Not just the leftmost label.
        assert_eq!(
            "good.xn--abc.com".parse::<Handle>(),
            Err(HandleError::PunycodeLabel)
        );
        // Mixed-case `XN--` is normalized then caught.
        assert_eq!(
            "XN--abc.com".parse::<Handle>(),
            Err(HandleError::PunycodeLabel)
        );
    }

    // ---- Reserved labels (ZMVP-45) ----------------------------------------

    #[test]
    fn rejects_reserved_labels_in_zurfur_namespace() {
        for label in ["api", "admin", "www"] {
            assert_eq!(
                format!("{label}.zurfur.app").parse::<Handle>(),
                Err(HandleError::ReservedLabel(label.into())),
                "{label}.zurfur.app should be reserved"
            );
        }
    }

    // The platform root handle is reserved too — the leading-dot suffix check
    // alone would let the bare apex slip past.
    #[test]
    fn rejects_the_bare_platform_apex() {
        assert_eq!(
            "zurfur.app".parse::<Handle>(),
            Err(HandleError::ReservedLabel("zurfur".into()))
        );
    }

    #[test]
    fn accepts_a_normal_zurfur_subdomain() {
        assert_eq!(
            "alice.zurfur.app".parse::<Handle>().unwrap().as_str(),
            "alice.zurfur.app"
        );
    }

    #[test]
    fn accepts_reserved_word_on_byo_domain() {
        // The reserved set guards only the *.zurfur.app namespace.
        assert_eq!(
            "api.example.com".parse::<Handle>().unwrap().as_str(),
            "api.example.com"
        );
    }

    // ---- Happy path --------------------------------------------------------

    #[test]
    fn accepts_well_formed_handles() {
        assert_eq!(
            "alice.zurfur.app".parse::<Handle>().unwrap().as_str(),
            "alice.zurfur.app"
        );
        assert_eq!(
            "alice.example.com".parse::<Handle>().unwrap().as_str(),
            "alice.example.com"
        );
    }

    // ---- Error quality -----------------------------------------------------

    #[test]
    fn every_error_variant_renders_a_message() {
        let variants = [
            HandleError::Empty,
            HandleError::TooLong(300),
            HandleError::TooFewSegments,
            HandleError::EmptySegment,
            HandleError::SegmentTooLong(64),
            HandleError::InvalidChar('_'),
            HandleError::HyphenEdge,
            HandleError::TldLeadingDigit,
            HandleError::ReservedTld("local".into()),
            HandleError::PunycodeLabel,
            HandleError::ReservedLabel("api".into()),
        ];
        for v in variants {
            assert!(!v.to_string().is_empty(), "{v:?} rendered an empty message");
        }
    }

    // ---- The configured namespace (HandleDomain) -------------------------

    fn domain(raw: &str) -> HandleDomain {
        raw.parse().expect("a valid handle domain")
    }

    #[test]
    fn domain_normalizer_strips_case_whitespace_and_dots() {
        assert_eq!(domain(" Zurfur.App. ").as_str(), "zurfur.app");
        assert_eq!(domain(".zurfur.app").as_str(), "zurfur.app");
        assert_eq!(domain("zurfur.app").as_str(), "zurfur.app");
    }

    #[test]
    fn domain_rejects_an_empty_value() {
        // An empty namespace would make every handle look like a member.
        assert_eq!("".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
        assert_eq!("   ".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
        assert_eq!("...".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
    }

    #[test]
    fn domain_error_renders_a_message() {
        assert!(!HandleDomainError::Empty.to_string().is_empty());
    }

    #[test]
    fn namespace_membership_is_a_strict_subdomain() {
        let alice = "alice.zurfur.app"
            .parse::<Handle>()
            .expect("a valid handle");
        assert!(alice.is_in_namespace(&domain("zurfur.app")));
        // The same namespace however deployment spelled it.
        assert!(alice.is_in_namespace(&domain("Zurfur.App.")));
        assert!(alice.is_in_namespace(&domain(".zurfur.app")));
        // A brought (BYO) domain is not a member.
        let byo = "alice.example.com"
            .parse::<Handle>()
            .expect("a valid handle");
        assert!(!byo.is_in_namespace(&domain("zurfur.app")));
    }

    #[test]
    fn namespace_membership_refuses_the_apex_and_lookalikes() {
        let zurfur = domain("zurfur.app");
        // The apex is not a member of its own namespace. `zurfur.app` is not a
        // constructible Handle, so another domain stands in for the shape.
        let apex_domain = domain("example.com");
        let apex = "example.com".parse::<Handle>().expect("a valid handle");
        assert!(!apex.is_in_namespace(&apex_domain));
        // A look-alike that ends with the domain's *text* but not on a label
        // boundary is refused — the leading dot is the boundary.
        for look_alike in ["notzurfur.app", "evil-zurfur.app", "xzurfur.app"] {
            let handle = look_alike.parse::<Handle>().expect("a valid handle");
            assert!(
                !handle.is_in_namespace(&zurfur),
                "{look_alike} must not be in the zurfur.app namespace"
            );
        }
        // A handle that merely *contains* the domain mid-string is refused too.
        let embedded = "zurfur.app.evil.com"
            .parse::<Handle>()
            .expect("a valid handle");
        assert!(!embedded.is_in_namespace(&zurfur));
    }
}

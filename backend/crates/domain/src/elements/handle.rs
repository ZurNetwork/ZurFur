//! The [`Handle`] — a validated, normalized atproto-style Account handle.
//!
//! An Account's handle is the public, human-typeable name it is reached by. Per
//! the DD *The Account Handle* (DESIGN/24870914 §6) it is user-chosen at
//! `POST /accounts` and is one of two things: a Zurfur-issued `<label>.zurfur.app`
//! subdomain, or a brought (BYO) domain the user already controls. Both verify
//! through the same atproto handle mechanism; the difference is only who controls
//! the DNS.
//!
//! This module is the **one shared validation gate** every future claim source
//! (onboarding ZMVP-30, issuance/resolution ZMVP-44) funnels through. Constructing
//! a [`Handle`] enforces — in a single pass — atproto normalization, the charset /
//! segment / length rules, the `xn--` punycode reject (ZMVP-48,
//! DD/26050561), and the Zurfur reserved-label reject (ZMVP-45). One gate, not
//! many: every consumer inherits the whole rule set by building a `Handle`.
//!
//! It mirrors the [`crate::elements::account::AccountName`] idiom exactly — a
//! `String` newtype with a validating `try_new`, an `as_str()`, a typed error
//! enum, and `///` doctests. It is a plain struct + free function: no trait,
//! because nothing consumes one polymorphically.

/// The longest a whole handle may be, in `char`s (atproto handle spec; DD §6).
pub const HANDLE_MAX_LEN: usize = 253;

/// The longest a single handle label (dot-separated segment) may be (DD §6).
pub const LABEL_MAX_LEN: usize = 63;

/// The Zurfur-issued handle namespace. A handle ending in `.zurfur.app` is gated
/// by [`RESERVED_LABELS`] on its leftmost label (ZMVP-45).
const ZURFUR_NAMESPACE_SUFFIX: &str = ".zurfur.app";

/// Top-level domains the atproto handle spec forbids as handles (DD §6).
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

/// Labels Zurfur withholds from its own `*.zurfur.app` namespace (ZMVP-45).
///
/// Checked against the **leftmost** label of a `*.zurfur.app` handle only — a BYO
/// domain such as `api.example.com` is the owner's to claim, so it is **not**
/// gated by this set (DD/24870914; Engineer-approved starter set, 2026-06-30).
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

/// A validated, normalized atproto-style Account handle (DD/24870914 §6).
///
/// Validate on the way in, then expose the normalized form via [`Handle::as_str`].
/// The stored value is always lowercase, trimmed, and has no trailing dot.
///
/// ```
/// use domain::elements::handle::Handle;
///
/// // Normalized: trimmed, lowercased, trailing dot stripped.
/// let h = Handle::try_new("  Alice.Zurfur.APP.  ").unwrap();
/// assert_eq!(h.as_str(), "alice.zurfur.app");
///
/// // A brought (BYO) domain is fine.
/// assert!(Handle::try_new("alice.example.com").is_ok());
///
/// // Punycode labels are rejected outright (ZMVP-48).
/// assert!(Handle::try_new("xn--80ak6aa92e.zurfur.app").is_err());
///
/// // Reserved labels in the *.zurfur.app namespace are rejected (ZMVP-45)...
/// assert!(Handle::try_new("api.zurfur.app").is_err());
/// // ...but the same word is claimable on a BYO domain.
/// assert!(Handle::try_new("api.example.com").is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle(String);

/// Why a string was rejected as a [`Handle`]. One variant per failure class,
/// mirroring [`crate::elements::account::AccountNameError`].
///
/// Each variant renders a clear human message via [`Display`](std::fmt::Display).
///
/// ```
/// use domain::elements::handle::{Handle, HandleError};
///
/// assert_eq!(Handle::try_new(""), Err(HandleError::Empty));
/// assert_eq!(Handle::try_new("alice"), Err(HandleError::TooFewSegments));
/// assert_eq!(Handle::try_new("foo.local"), Err(HandleError::ReservedTld("local".into())));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleError {
    /// Empty once normalized. Example: `""` or `"   "` or `"."`.
    Empty,
    /// Longer than [`HANDLE_MAX_LEN`] chars overall. Carries the offending length.
    TooLong(usize),
    /// Fewer than two dot-separated segments (e.g. bare `"alice"`).
    TooFewSegments,
    /// A dot-separated segment is empty (e.g. `"alice..app"`).
    EmptySegment,
    /// A segment is longer than [`LABEL_MAX_LEN`] chars. Carries the offending length.
    SegmentTooLong(usize),
    /// A character outside the `[a-z0-9-]` charset. Carries the offending char.
    InvalidChar(char),
    /// A segment starts or ends with a hyphen.
    HyphenEdge,
    /// The rightmost (top-level) segment starts with a digit.
    TldLeadingDigit,
    /// The rightmost segment is a reserved TLD (e.g. `.local`, `.test`). Carries it.
    ReservedTld(String),
    /// Some label begins with `xn--` (punycode). Rejected in both namespaces
    /// (ZMVP-48, DD/26050561) to kill the homoglyph-IDN impersonation vector.
    PunycodeLabel,
    /// The leftmost label of a `*.zurfur.app` handle is reserved (ZMVP-45).
    /// Carries the offending label.
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

impl Handle {
    /// Validate and wrap a handle, enforcing every rule in one pass.
    ///
    /// The input is first **normalized** (trim surrounding whitespace, lowercase,
    /// strip a single trailing dot), then checked, in order:
    /// 1. non-empty and `≤` [`HANDLE_MAX_LEN`] chars;
    /// 2. at least two dot-separated segments, none empty;
    /// 3. each segment `≤` [`LABEL_MAX_LEN`] chars, no leading/trailing hyphen,
    ///    every char in `[a-z0-9-]`;
    /// 4. no label begins with `xn--` (punycode reject, ZMVP-48 — uniform across
    ///    both namespaces);
    /// 5. the rightmost segment does not start with a digit;
    /// 6. the rightmost segment is not a [reserved TLD](RESERVED_TLDS);
    /// 7. for a `*.zurfur.app` handle, the leftmost label is not
    ///    [reserved](RESERVED_LABELS) (ZMVP-45) — this gate applies to the Zurfur
    ///    namespace only, never to a BYO domain.
    ///
    /// ```
    /// use domain::elements::handle::{Handle, HandleError};
    ///
    /// assert_eq!(Handle::try_new("alice.zurfur.app").unwrap().as_str(), "alice.zurfur.app");
    /// assert_eq!(Handle::try_new("XN--abc.com"), Err(HandleError::PunycodeLabel)); // case-insensitive
    /// assert_eq!(Handle::try_new("admin.zurfur.app"), Err(HandleError::ReservedLabel("admin".into())));
    /// ```
    pub fn try_new(raw: impl Into<String>) -> Result<Self, HandleError> {
        // 1. NORMALIZE: trim, lowercase, strip a single trailing dot (FQDN root).
        let lowered = raw.into().trim().to_lowercase();
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
        if normalized.ends_with(ZURFUR_NAMESPACE_SUFFIX) {
            let leftmost = labels[0];
            if RESERVED_LABELS.contains(&leftmost) {
                return Err(HandleError::ReservedLabel(leftmost.to_owned()));
            }
        }

        Ok(Self(normalized))
    }

    /// The normalized handle string (lowercase, trimmed, no trailing dot).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Normalization -----------------------------------------------------

    #[test]
    fn lowercases_the_handle() {
        assert_eq!(
            Handle::try_new("Alice.Zurfur.APP").unwrap().as_str(),
            "alice.zurfur.app"
        );
    }

    #[test]
    fn strips_a_single_trailing_dot() {
        assert_eq!(
            Handle::try_new("alice.zurfur.app.").unwrap().as_str(),
            "alice.zurfur.app"
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            Handle::try_new("  alice.example.com  ").unwrap().as_str(),
            "alice.example.com"
        );
    }

    // ---- Charset / segment / length ---------------------------------------

    #[test]
    fn rejects_a_single_segment() {
        assert_eq!(Handle::try_new("alice"), Err(HandleError::TooFewSegments));
    }

    #[test]
    fn rejects_an_empty_input() {
        assert_eq!(Handle::try_new("   "), Err(HandleError::Empty));
        assert_eq!(Handle::try_new("."), Err(HandleError::Empty));
    }

    #[test]
    fn rejects_an_empty_segment() {
        assert_eq!(
            Handle::try_new("alice..app"),
            Err(HandleError::EmptySegment)
        );
    }

    #[test]
    fn rejects_a_segment_over_63_chars() {
        let long_label = "a".repeat(64);
        assert_eq!(
            Handle::try_new(format!("{long_label}.app")),
            Err(HandleError::SegmentTooLong(64))
        );
    }

    #[test]
    fn rejects_a_handle_over_253_chars() {
        // Build a >253-char handle out of legal 63-char labels.
        let label = "a".repeat(63);
        let raw = format!("{label}.{label}.{label}.{label}.com"); // 4*63 + 3 + 4 = 259
        let len = raw.chars().count();
        assert_eq!(Handle::try_new(raw), Err(HandleError::TooLong(len)));
    }

    #[test]
    fn rejects_a_leading_or_trailing_hyphen() {
        assert_eq!(Handle::try_new("-alice.app"), Err(HandleError::HyphenEdge));
        assert_eq!(Handle::try_new("alice-.app"), Err(HandleError::HyphenEdge));
    }

    #[test]
    fn rejects_out_of_charset_bytes() {
        assert_eq!(
            Handle::try_new("ali_ce.app"),
            Err(HandleError::InvalidChar('_'))
        );
        assert_eq!(
            Handle::try_new("ali ce.app"),
            Err(HandleError::InvalidChar(' '))
        );
        assert_eq!(
            Handle::try_new("café.app"),
            Err(HandleError::InvalidChar('é'))
        );
    }

    #[test]
    fn rejects_a_digit_leading_tld() {
        assert_eq!(
            Handle::try_new("alice.123"),
            Err(HandleError::TldLeadingDigit)
        );
    }

    // ---- Reserved TLDs -----------------------------------------------------

    #[test]
    fn rejects_reserved_tlds() {
        assert_eq!(
            Handle::try_new("foo.local"),
            Err(HandleError::ReservedTld("local".into()))
        );
        assert_eq!(
            Handle::try_new("foo.test"),
            Err(HandleError::ReservedTld("test".into()))
        );
        assert_eq!(
            Handle::try_new("foo.onion"),
            Err(HandleError::ReservedTld("onion".into()))
        );
    }

    // ---- Punycode (ZMVP-48) -----------------------------------------------

    #[test]
    fn rejects_punycode_zurfur_label() {
        assert_eq!(
            Handle::try_new("xn--80ak6aa92e.zurfur.app"),
            Err(HandleError::PunycodeLabel)
        );
    }

    #[test]
    fn rejects_punycode_byo_domain() {
        assert_eq!(
            Handle::try_new("xn--e1awd7f.com"),
            Err(HandleError::PunycodeLabel)
        );
    }

    #[test]
    fn rejects_punycode_anywhere_and_mixed_case() {
        // Not just the leftmost label.
        assert_eq!(
            Handle::try_new("good.xn--abc.com"),
            Err(HandleError::PunycodeLabel)
        );
        // Mixed-case `XN--` is normalized then caught.
        assert_eq!(
            Handle::try_new("XN--abc.com"),
            Err(HandleError::PunycodeLabel)
        );
    }

    // ---- Reserved labels (ZMVP-45) ----------------------------------------

    #[test]
    fn rejects_reserved_labels_in_zurfur_namespace() {
        for label in ["api", "admin", "www"] {
            assert_eq!(
                Handle::try_new(format!("{label}.zurfur.app")),
                Err(HandleError::ReservedLabel(label.into())),
                "{label}.zurfur.app should be reserved"
            );
        }
    }

    #[test]
    fn accepts_reserved_word_on_byo_domain() {
        // The reserved set guards only the *.zurfur.app namespace; a BYO domain
        // is the owner's to claim (Engineer disposition, 2026-06-30).
        assert_eq!(
            Handle::try_new("api.example.com").unwrap().as_str(),
            "api.example.com"
        );
    }

    // ---- Happy path --------------------------------------------------------

    #[test]
    fn accepts_well_formed_handles() {
        assert_eq!(
            Handle::try_new("alice.zurfur.app").unwrap().as_str(),
            "alice.zurfur.app"
        );
        assert_eq!(
            Handle::try_new("alice.example.com").unwrap().as_str(),
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

    // ---- RFC-9457 claim-site mapping — FULFILLED (ZMVP-44) ----
    //
    // The claim surface now exists: `POST /accounts` accepts a `handle`, validates
    // it through this newtype, and maps a [`HandleError`] to a 422
    // `urn:zurfur:error:invalid-request` problem+json (DD/23592962). Duplicate
    // handles map to a 409 `handle_taken`. That mapping is an api-layer concern
    // (the `domain` crate has no HTTP types), so its integration coverage lives in
    // `api/tests/accounts.rs` (`founding_rejects_a_punycode_handle`,
    // `founding_rejects_a_reserved_handle`, `founding_rejects_a_duplicate_handle`),
    // not here. This note records that the earlier handoff is closed.
}

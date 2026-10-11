use std::str::FromStr;

use super::HandleDomain;
use super::HandleError;
use super::reserved::{RESERVED_LABELS, RESERVED_TLDS, ZURFUR_NAMESPACE_SUFFIX};
use super::syntax;

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

impl FromStr for Handle {
    type Err = HandleError;

    /// Validate and wrap a handle, enforcing every rule in one pass: normalize
    /// (trim, lowercase ASCII, strip one trailing dot), then check length, segment
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
        // 1. NORMALIZE: trim, lowercase ASCII only (the spec's `A-Z` to `a-z`, so
        //    a non-ASCII letter stays foreign and fails the charset), strip a
        //    single trailing dot (FQDN root).
        let lowered = raw.trim().to_ascii_lowercase();
        let normalized = lowered.strip_suffix('.').unwrap_or(&lowered).to_owned();

        // 2–4. Length, segments, and each label's length, hyphen edges and
        //      charset — the syntax every handle shares, checked before the
        //      punycode reject so a malformed label reports its real fault.
        let labels = syntax::split_labels(&normalized)?;

        // 5. Punycode reject (ZMVP-48): any label beginning with `xn--`. The form
        //    is already lowercased, so a plain prefix check is case-insensitive.
        if labels.iter().any(|label| label.starts_with("xn--")) {
            return Err(HandleError::PunycodeLabel);
        }

        // 6–7. The rightmost (top-level) segment: no leading digit, not reserved.
        syntax::check_top_level(&labels, &RESERVED_TLDS)?;

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
mod proptests;
#[cfg(test)]
mod tests;

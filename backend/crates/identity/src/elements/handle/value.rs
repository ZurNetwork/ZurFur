use std::str::FromStr;

use super::HandleDomain;
use super::HandleError;
use super::reserved::{RESERVED_LABELS, RESERVED_TLDS};
use super::syntax;

/// A validated, normalized atproto-style Account handle. The stored value is
/// always lowercase, trimmed, and has no trailing dot.
///
/// ```
/// use identity::elements::handle::Handle;
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
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle(String);

impl FromStr for Handle {
    type Err = HandleError;

    /// Validate and wrap a handle, enforcing every context-free rule in one
    /// pass: normalize (trim, lowercase ASCII, strip one trailing dot), then
    /// check length, segment count and shape, charset, the `xn--` reject and the
    /// TLD rules. The reserved labels depend on the configured namespace, so
    /// [`Handle::ensure_unreserved`] checks them at the claim.
    ///
    /// ```
    /// use identity::elements::handle::{Handle, HandleError};
    ///
    /// assert_eq!("alice.zurfur.app".parse::<Handle>().unwrap().as_str(), "alice.zurfur.app");
    /// assert_eq!("XN--abc.com".parse::<Handle>(), Err(HandleError::PunycodeLabel)); // case-insensitive
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
    /// use identity::elements::handle::{Handle, HandleDomain};
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

    /// Refuse a handle `domain` withholds: its bare apex, or a member whose
    /// leftmost label is reserved. A BYO handle passes. A method, not a parse:
    /// the rule needs the configured namespace.
    ///
    /// ```
    /// use identity::elements::handle::{Handle, HandleDomain, HandleError};
    ///
    /// let staging: HandleDomain = "zurfur.foo".parse().unwrap();
    /// let admin: Handle = "admin.zurfur.foo".parse().unwrap();
    /// assert_eq!(admin.ensure_unreserved(&staging), Err(HandleError::ReservedLabel("admin".into())));
    /// // No one claims the namespace's root handle.
    /// let apex: Handle = "zurfur.foo".parse().unwrap();
    /// assert_eq!(apex.ensure_unreserved(&staging), Err(HandleError::NamespaceApex));
    /// // The same word is claimable on a BYO domain.
    /// let byo: Handle = "admin.example.com".parse().unwrap();
    /// assert_eq!(byo.ensure_unreserved(&staging), Ok(()));
    /// ```
    pub fn ensure_unreserved(&self, domain: &HandleDomain) -> Result<(), HandleError> {
        if self.0 == domain.as_str() {
            return Err(HandleError::NamespaceApex);
        }
        if !self.is_in_namespace(domain) {
            return Ok(());
        }
        let leftmost = self.0.split('.').next().unwrap_or_default();
        if RESERVED_LABELS.contains(&leftmost) {
            return Err(HandleError::ReservedLabel(leftmost.to_owned()));
        }
        Ok(())
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

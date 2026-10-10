//! DID → document: the two URL shapes the resolver builds, the strict parse,
//! the `id` check, and the claimed handle and PDS read from what passed.

use jacquard_common::types::{did_doc::DidDocument, value::Data};

use domain::elements::{
    did::Did,
    handle::{AtHandle, HandleError},
};
use domain::ports::ResolveError;

use super::client::AtprotoIdentityResolver;
use super::fetch::fetch_body;
use super::limits::{DID_MAX_LEN, PLC_DIRECTORY};
use crate::guarded_http::PublicHttpsUrl;

/// Where a DID's document is read from: the PLC directory for a `did:plc`,
/// the host's well-known file for a hostname-level `did:web`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DocumentUrl(PublicHttpsUrl);

/// A DID this resolver does not look up: too long, another method, a
/// malformed `did:plc`, or a `did:web` that is not a public hostname.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("DID not resolvable here")]
pub(super) struct UnresolvableDid;

impl TryFrom<&Did> for DocumentUrl {
    type Error = UnresolvableDid;

    /// Build the one URL `did`'s method allows, or refuse before any request.
    fn try_from(did: &Did) -> Result<Self, Self::Error> {
        let text = did.as_ref();
        if text.len() > DID_MAX_LEN {
            return Err(UnresolvableDid);
        }
        let address = if let Some(id) = text.strip_prefix("did:plc:") {
            plc_address(text, id)?
        } else if let Some(host) = text.strip_prefix("did:web:") {
            web_address(host)?
        } else {
            return Err(UnresolvableDid);
        };
        let url = PublicHttpsUrl::try_from(address.as_str()).map_err(|_| UnresolvableDid)?;
        Ok(Self(url))
    }
}

/// `https://plc.directory/<did>`, when `id` is 24 characters of base32.
fn plc_address(did: &str, id: &str) -> Result<String, UnresolvableDid> {
    let is_base32 = id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7'));
    if id.len() != 24 || !is_base32 {
        return Err(UnresolvableDid);
    }
    Ok(format!("{PLC_DIRECTORY}/{did}"))
}

/// `https://<host>/.well-known/did.json`, when `host` is a bare hostname: no
/// port (`%3A`), no path (`:`), no trailing dot.
fn web_address(host: &str) -> Result<String, UnresolvableDid> {
    let is_hostname = !host.is_empty()
        && !host.ends_with('.')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if !is_hostname {
        return Err(UnresolvableDid);
    }
    Ok(format!("https://{host}/.well-known/did.json"))
}

/// What a document's `alsoKnownAs` claims: its first syntactically valid
/// `at://` handle, which alone counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Claim {
    /// No entry is a valid `at://` handle.
    Absent,
    /// The first valid entry uses a disallowed TLD, so it can never confirm.
    Unusable,
    /// The first valid entry.
    Handle(AtHandle),
}

impl<'a> FromIterator<&'a str> for Claim {
    /// Skip entries that are not `at://` plus a valid handle; stop at the first that is.
    fn from_iter<I: IntoIterator<Item = &'a str>>(entries: I) -> Self {
        for entry in entries {
            let Some(raw) = entry.strip_prefix("at://") else {
                continue;
            };
            match raw.parse::<AtHandle>() {
                Ok(handle) => return Self::Handle(handle),
                Err(HandleError::ReservedTld(_)) => return Self::Unusable,
                Err(_) => continue,
            }
        }
        Self::Absent
    }
}

/// A DID document that parsed in full and whose `id` is the DID it was
/// fetched for.
#[derive(Debug)]
pub(crate) struct ResolvedDocument {
    document: DidDocument,
    /// The body exactly as fetched, for a caller that must parse it itself.
    body: Vec<u8>,
}

impl ResolvedDocument {
    /// Check `body` against `requested`: a full DID document (never a
    /// "mini-doc") naming exactly that DID, else `NotFound`. Inherent: it
    /// judges a body against the DID it was fetched for, two inputs.
    pub(super) fn parse(requested: &Did, body: &[u8]) -> Result<Self, ResolveError> {
        let document: DidDocument =
            serde_json::from_slice(body).map_err(|_| ResolveError::NotFound)?;
        if document.id.as_str() != requested.as_ref() {
            return Err(ResolveError::NotFound);
        }
        Ok(Self {
            document,
            body: body.to_vec(),
        })
    }

    /// The body that passed these checks, byte for byte.
    pub(crate) fn into_body(self) -> Vec<u8> {
        self.body
    }

    /// The handle this document claims.
    pub(crate) fn claim(&self) -> Claim {
        let also_known_as = self.document.also_known_as.as_deref().unwrap_or_default();
        also_known_as.iter().map(|entry| entry.as_str()).collect()
    }

    /// The PDS: the first service whose `id` ends `#atproto_pds` and whose type
    /// is `AtprotoPersonalDataServer`, admitted by the URL policy. `NotFound`
    /// when there is none; `Refused` when the policy refuses it.
    pub(crate) fn pds(&self) -> Result<PublicHttpsUrl, ResolveError> {
        let services = self.document.service.as_deref().unwrap_or_default();
        let endpoint = services
            .iter()
            .find(|service| {
                service.id.ends_with("#atproto_pds")
                    && service.r#type.as_str() == "AtprotoPersonalDataServer"
            })
            .and_then(|service| match &service.service_endpoint {
                Some(Data::String(text)) => Some(text.as_str()),
                _ => None,
            })
            .ok_or(ResolveError::NotFound)?;
        PublicHttpsUrl::try_from(endpoint)
            .map_err(|refusal| ResolveError::Refused(anyhow::Error::new(refusal)))
    }
}

impl AtprotoIdentityResolver {
    /// `did`'s document, fetched and checked here. A DID outside the two
    /// supported shapes is `NotFound` with no request made.
    pub(crate) async fn document(&self, did: &Did) -> Result<ResolvedDocument, ResolveError> {
        let DocumentUrl(url) = DocumentUrl::try_from(did).map_err(|_| ResolveError::NotFound)?;
        let body = fetch_body(&self.http, &url).await?;
        ResolvedDocument::parse(did, &body)
    }
}

#[cfg(test)]
mod tests;

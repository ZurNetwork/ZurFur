//! Handle → DID: the DNS TXT record first, then the HTTPS well-known file, one
//! after the other so a host an attacker picks sees as few requests as possible.

use std::str::FromStr;

use domain::elements::{did::Did, handle::AtHandle};
use domain::ports::ResolveError;

use super::client::AtprotoIdentityResolver;
use super::fetch::fetch_body;
use super::limits::TXT_HANDLE_MAX_LEN;
use crate::guarded_http::{LookupError, PublicHttpsUrl, is_public_domain};

/// What a handle's `_atproto` TXT records say.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TxtAnswer {
    /// No record names a valid DID, so the HTTPS file decides.
    Silent,
    /// Every record that names a valid DID names this one.
    Names(Did),
    /// Records name different DIDs, so resolution fails.
    Conflicting,
}

impl<'a> FromIterator<&'a [u8]> for TxtAnswer {
    /// Ignore values that do not start `did=` or do not hold a DID; the rest
    /// must agree.
    fn from_iter<I: IntoIterator<Item = &'a [u8]>>(records: I) -> Self {
        let mut named: Option<Did> = None;
        for did in records.into_iter().filter_map(record_did) {
            match &named {
                None => named = Some(did),
                Some(first) if *first == did => {}
                Some(_) => return Self::Conflicting,
            }
        }
        named.map_or(Self::Silent, Self::Names)
    }
}

/// The DID one TXT value names, when it is `did=` followed by a DID.
fn record_did(record: &[u8]) -> Option<Did> {
    let value = record.strip_prefix(b"did=")?;
    let text = std::str::from_utf8(value).ok()?;
    Did::from_str(text).ok()
}

/// The DID a well-known file names: its first line, with surrounding
/// whitespace stripped.
pub(super) fn well_known_did(body: &[u8]) -> Option<Did> {
    let text = std::str::from_utf8(body).ok()?;
    let first_line = text.lines().next()?;
    Did::from_str(first_line.trim()).ok()
}

/// The fully qualified TXT name for `handle`; `None` when it is too long for DNS.
pub(super) fn txt_name(handle: &AtHandle) -> Option<String> {
    let handle = handle.as_ref();
    (handle.len() <= TXT_HANDLE_MAX_LEN).then(|| format!("_atproto.{handle}."))
}

impl AtprotoIdentityResolver {
    /// The DID `handle` names, unconfirmed: the TXT record when there is one,
    /// otherwise the well-known file. A handle under a reserved TLD (`.test`)
    /// is not found, and nothing is asked.
    pub(super) async fn did_named_by(&self, handle: &AtHandle) -> Result<Did, ResolveError> {
        if !is_public_domain(handle.as_ref()) {
            return Err(ResolveError::NotFound);
        }
        match self.txt_answer(handle).await? {
            TxtAnswer::Names(did) => Ok(did),
            TxtAnswer::Conflicting => Err(ResolveError::NotFound),
            TxtAnswer::Silent => self.well_known(handle).await,
        }
    }

    /// The TXT step. No record, or a name too long for DNS, is `Silent`; any
    /// other DNS failure is `Unavailable`, with no fall-through to HTTPS.
    async fn txt_answer(&self, handle: &AtHandle) -> Result<TxtAnswer, ResolveError> {
        let Some(name) = txt_name(handle) else {
            return Ok(TxtAnswer::Silent);
        };
        let lookup = tokio::time::timeout(self.deadlines.dns, self.txt.lookup_txt(&name))
            .await
            .map_err(|elapsed| ResolveError::Unavailable(anyhow::Error::new(elapsed)))?;
        let records = match lookup {
            Ok(records) => records,
            Err(LookupError::NoRecords) => return Ok(TxtAnswer::Silent),
            Err(LookupError::Failed(cause)) => {
                return Err(ResolveError::Unavailable(anyhow::Error::from_boxed(cause)));
            }
        };
        let answer = records.iter().map(Vec::as_slice).collect();
        Ok(answer)
    }

    /// The HTTPS step: `https://<handle>/.well-known/atproto-did`.
    async fn well_known(&self, handle: &AtHandle) -> Result<Did, ResolveError> {
        let address = format!("https://{handle}/.well-known/atproto-did");
        let url = PublicHttpsUrl::try_from(address.as_str()).map_err(|_| ResolveError::NotFound)?;
        let body = fetch_body(&self.http, &url).await?;
        well_known_did(&body).ok_or(ResolveError::NotFound)
    }
}

#[cfg(test)]
mod tests;

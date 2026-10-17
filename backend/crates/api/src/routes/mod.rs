//! HTTP route groups, split along the domain/namespace seams (subdomain →
//! namespace → router). Each exposes
//! a `*_router()`; a namespace boundary is also a policy boundary — the
//! cookie surface is wrapped by [`require_first_party_origin`] (CSRF), while
//! `health`/`wellknown` and the not-found fallback stay outside it.

use axum::{
    extract::{Request, State},
    http::{
        HeaderMap, HeaderValue, Method,
        header::{GetAll, ORIGIN},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use composition::BareOrigin;
use url::Url;

use crate::problem::Problem;

mod accounts;
mod commissions;
mod den;
mod health;
mod session;
mod wellknown;

pub(crate) use accounts::accounts_router;
pub(crate) use commissions::commissions_router;
pub(crate) use den::den_router;
pub(crate) use health::health_router;
pub(crate) use session::session_router;
pub(crate) use wellknown::wellknown_router;

/// The fetch-metadata header a browser sends naming how the request's
/// initiator relates to its target.
const SEC_FETCH_SITE: &str = "sec-fetch-site";

/// The one `Sec-Fetch-Site` value that vouches for a request with no `Origin`.
const SAME_ORIGIN: &[u8] = b"same-origin";

/// CSRF guard on the cookie surface: refuses a write (any method [`is_safe`]
/// doesn't name), with `403 cross_origin` before its handler runs, unless its
/// `Origin` is exactly `public_origin`, or it sends no `Origin` and
/// `Sec-Fetch-Site: same-origin`.
pub(crate) async fn require_first_party_origin(
    State(public_origin): State<BareOrigin>,
    request: Request,
    next: Next,
) -> Response {
    if !is_safe(request.method()) && !is_first_party(request.headers(), public_origin.as_ref()) {
        return Problem::cross_origin().into_response();
    }
    next.run(request).await
}

/// Whether `method` is GET, HEAD or OPTIONS, exactly as sent. That holds only
/// while no cookie-surface GET changes the visitor's state but `/signin-callback`,
/// the API grants no CORS (so a cross-origin OPTIONS carries no cookie), and a
/// WebSocket upgrade on the cookie surface checks `Origin` itself.
fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Whether `headers` vouch for a first-party request: one `Origin` naming
/// `public_origin` byte for byte, whatever `Sec-Fetch-Site` says, or no
/// `Origin` and one `Sec-Fetch-Site` of exactly `same-origin`. Two of the
/// header that decides never pass.
fn is_first_party(headers: &HeaderMap, public_origin: &Url) -> bool {
    match Sole::from(headers.get_all(ORIGIN)) {
        Sole::One(origin) => names_origin(origin.as_bytes(), public_origin),
        Sole::Many => false,
        Sole::Absent => {
            let fetch_site = Sole::from(headers.get_all(SEC_FETCH_SITE));
            matches!(fetch_site, Sole::One(site) if site.as_bytes() == SAME_ORIGIN)
        }
    }
}

/// Whether an `Origin` value is `public_origin`'s ASCII serialisation. An
/// opaque origin (serialized `null`) matches nothing, so a sandboxed page's
/// `Origin: null` never passes, and neither does an empty value.
fn names_origin(origin: &[u8], public_origin: &Url) -> bool {
    let first_party = public_origin.origin();
    first_party.is_tuple() && origin == first_party.ascii_serialization().as_bytes()
}

/// How many values a request sent for one header: none, exactly one, or more.
enum Sole<'a> {
    Absent,
    One(&'a HeaderValue),
    Many,
}

impl<'a> From<GetAll<'a, HeaderValue>> for Sole<'a> {
    fn from(values: GetAll<'a, HeaderValue>) -> Self {
        let mut values = values.iter();
        match (values.next(), values.next()) {
            (None, _) => Self::Absent,
            (Some(value), None) => Self::One(value),
            (Some(_), Some(_)) => Self::Many,
        }
    }
}

#[cfg(test)]
mod tests;

use std::task::{Context, Poll};

use http::{HeaderMap, HeaderValue, Request, header::COOKIE};
use tower_layer::Layer;
use tower_service::Service;
use tower_sessions::cookie::Cookie;

/// Ahead of the session layer, drops every pair of the session cookie from a
/// request that carries two or more, across all its `Cookie` headers, so the
/// request is signed out. Other cookies pass through.
#[derive(Clone, Copy, Debug)]
pub struct SingleSessionCookieLayer {
    /// The session cookie's name.
    pub(crate) name: &'static str,
}

impl<S> Layer<S> for SingleSessionCookieLayer {
    type Service = SingleSessionCookie<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SingleSessionCookie {
            inner,
            name: self.name,
        }
    }
}

/// The service [`SingleSessionCookieLayer`] wraps around `S`.
#[derive(Clone, Debug)]
pub struct SingleSessionCookie<S> {
    inner: S,
    name: &'static str,
}

impl<S, B> Service<Request<B>> for SingleSessionCookie<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        drop_contested_pairs(request.headers_mut(), self.name);
        self.inner.call(request)
    }
}

/// Removes every `name` pair from `headers` when they carry two or more,
/// keeping each header's other pairs; a header left with none is removed.
fn drop_contested_pairs(headers: &mut HeaderMap, name: &str) {
    let pairs = headers
        .get_all(COOKIE)
        .iter()
        .flat_map(pieces)
        .filter(|piece| is_named(piece, name))
        .count();
    if pairs < 2 {
        return;
    }

    let kept: Vec<HeaderValue> = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| without_pairs(value, name))
        .collect();
    headers.remove(COOKIE);
    for value in kept {
        headers.append(COOKIE, value);
    }
}

/// The `;`-separated pieces of a `Cookie` header as the session layer's
/// cookie jar splits them: a header that isn't UTF-8 has none.
fn pieces(value: &HeaderValue) -> impl Iterator<Item = &str> {
    std::str::from_utf8(value.as_bytes())
        .into_iter()
        .flat_map(|text| text.split(';'))
}

/// Whether `piece` parses as a pair named `name` once percent-decoded, which
/// is how the session layer's cookie jar names it.
fn is_named(piece: &str, name: &str) -> bool {
    Cookie::parse_encoded(piece).is_ok_and(|cookie| cookie.name() == name)
}

/// `value` with every `name` pair taken out, or `None` when nothing is left.
/// A header that isn't UTF-8 holds no pair the jar reads, so it is kept as is.
fn without_pairs(value: &HeaderValue, name: &str) -> Option<HeaderValue> {
    let Ok(text) = std::str::from_utf8(value.as_bytes()) else {
        return Some(value.clone());
    };
    let kept: Vec<&str> = text
        .split(';')
        .filter(|piece| !is_named(piece, name))
        .collect();
    let joined = kept.join(";");
    if joined.trim().is_empty() {
        return None;
    }
    HeaderValue::from_str(&joined).ok()
}

#[cfg(test)]
mod tests;

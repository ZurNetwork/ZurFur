//! Every cookie the backend sets, by name, and the one place each one's
//! attributes are chosen: [`ZurfurCookie`] builds the cookies the routes set
//! and clear, and [`session_layer`] hands the same attributes to the session
//! middleware, which issues its own.

use std::borrow::Cow;

use axum::http::HeaderValue;
use tower_sessions::{
    Expiry, SessionManagerLayer, SessionStore,
    cookie::{Cookie, SameSite, time},
};

/// How long a session lives without a request before it expires.
const SESSION_INACTIVITY: time::Duration = time::Duration::days(7);

/// A cookie the backend sets. Its name and every attribute come from the variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::VariantArray)]
pub enum ZurfurCookie {
    /// The session id the session layer issues and rotates.
    Session,
    /// Binds a sign-in to the browser that started it, from `POST /signin` to
    /// the callback.
    SignInBinding,
}

impl ZurfurCookie {
    /// The cookie's name. Where cookies are `Secure` the sign-in binding's
    /// carries the `__Host-` prefix, which locks it to this host, `Path=/` and
    /// HTTPS, so no sibling host can plant one.
    pub fn name(self, secure: bool) -> &'static str {
        match (self, secure) {
            (Self::Session, _) => "zurfur.sid",
            (Self::SignInBinding, true) => "__Host-zurfur.signin",
            (Self::SignInBinding, false) => "zurfur.signin",
        }
    }

    /// Whether the cookie is kept from scripts: every Zurfur cookie is.
    fn http_only(self) -> bool {
        match self {
            Self::Session | Self::SignInBinding => true,
        }
    }

    /// `Lax` for both: the sign-in callback arrives as a top-level navigation
    /// from the PDS, which `Strict` would strip.
    fn same_site(self) -> SameSite {
        match self {
            Self::Session | Self::SignInBinding => SameSite::Lax,
        }
    }

    /// The path the cookie is sent on: the whole site.
    fn path(self) -> &'static str {
        match self {
            Self::Session | Self::SignInBinding => "/",
        }
    }

    /// The cookie carrying `value`, gone after `max_age`, with every attribute
    /// set from the variant.
    pub fn cookie<'c>(
        self,
        secure: bool,
        value: impl Into<Cow<'c, str>>,
        max_age: time::Duration,
    ) -> Cookie<'c> {
        Cookie::build((self.name(secure), value))
            .http_only(self.http_only())
            .same_site(self.same_site())
            .secure(secure)
            .path(self.path())
            .max_age(max_age)
            .build()
    }

    /// The cookie that clears this one: same name, path and flags, empty and
    /// already expired.
    pub fn cleared(self, secure: bool) -> Cookie<'static> {
        self.cookie(secure, "", time::Duration::ZERO)
    }
}

/// `cookie` as a `Set-Cookie` header value; `None` when its value cannot
/// travel in a header.
pub fn set_cookie_header(cookie: &Cookie<'_>) -> Option<HeaderValue> {
    HeaderValue::from_str(&cookie.to_string()).ok()
}

/// The session middleware over `store`, issuing [`ZurfurCookie::Session`]
/// with its name and attributes, renewed for [`SESSION_INACTIVITY`] on each
/// request that touches the session.
pub fn session_layer<S: SessionStore>(store: S, secure: bool) -> SessionManagerLayer<S> {
    let session = ZurfurCookie::Session;
    SessionManagerLayer::new(store)
        .with_name(session.name(secure))
        .with_http_only(session.http_only())
        .with_same_site(session.same_site())
        .with_secure(secure)
        .with_path(session.path())
        .with_expiry(Expiry::OnInactivity(SESSION_INACTIVITY))
}

#[cfg(test)]
mod tests;

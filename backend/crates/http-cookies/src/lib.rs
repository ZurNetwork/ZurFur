//! Every cookie the HTTP driver sets, by name, and the one place each one's
//! attributes are chosen: [`ZurfurCookie`] builds the cookies the routes set
//! and clear, and [`session_layer`] hands the same attributes to the session
//! middleware, which issues its own. [`SingleSessionCookieLayer`] runs ahead
//! of that middleware and signs out a request carrying two session cookies.

mod single_session_cookie;
mod zurfur_cookie;

pub use single_session_cookie::{SingleSessionCookie, SingleSessionCookieLayer};
pub use zurfur_cookie::{SessionLayer, ZurfurCookie, session_layer, set_cookie_header};

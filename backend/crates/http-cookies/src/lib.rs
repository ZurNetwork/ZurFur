//! Every cookie the HTTP driver sets, by name, and the one place each one's
//! attributes are chosen: [`ZurfurCookie`] builds the cookies the routes set
//! and clear, and [`session_layer`] hands the same attributes to the session
//! middleware, which issues its own.

mod zurfur_cookie;

pub use zurfur_cookie::{ZurfurCookie, session_layer, set_cookie_header};

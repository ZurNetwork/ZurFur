//! The [`Handle`] — a validated, normalized atproto-style Account handle — and
//! the [`HandleDomain`] it may live under.
//!
//! This module is the one shared validation gate every claim source funnels
//! through: constructing a [`Handle`] enforces normalization, the
//! charset/segment/length rules, an outright reject of any `xn--` punycode
//! label, and the Zurfur reserved-label reject in a single pass. Namespace membership is
//! [`Handle::is_in_namespace`], against a [`HandleDomain`] parsed once at config
//! load, so the claim checks and the resolver cannot disagree.

mod domain;
mod errors;
mod reserved;
mod value;

pub use domain::HandleDomain;
pub use errors::{HandleDomainError, HandleError};
pub use reserved::{HANDLE_MAX_LEN, LABEL_MAX_LEN};
pub use value::Handle;

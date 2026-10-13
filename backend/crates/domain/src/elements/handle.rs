//! Handles: [`Handle`] — a validated, normalized Account handle Zurfur has
//! claimed — the [`HandleDomain`] it may live under, and [`AtHandle`], any
//! valid atproto handle Zurfur looks up but does not own.
//!
//! [`Handle`] is the one shared validation gate every claim source funnels
//! through: constructing it enforces normalization, the
//! charset/segment/length rules and an outright reject of any `xn--` punycode
//! label in a single pass. The rules that depend on the configured namespace
//! take a [`HandleDomain`]: membership is [`Handle::is_in_namespace`], and the
//! reserved-label and apex reject is [`Handle::ensure_unreserved`]. A caller
//! that gates a claim or a lookup on the namespace applies both. [`AtHandle`]
//! shares the syntax rules and none of the claim rules.

mod at_handle;
mod domain;
mod errors;
mod reserved;
mod syntax;
mod value;

pub use at_handle::AtHandle;
pub use domain::HandleDomain;
pub use errors::{HandleDomainError, HandleError};
pub use reserved::{HANDLE_MAX_LEN, LABEL_MAX_LEN, RESERVED_TLDS};
pub use value::Handle;

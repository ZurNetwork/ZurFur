//! The composition root shared by every driving adapter (`api`, `cli`): the
//! figment-loaded [`Config`] (its load refuses a configuration its environment
//! may not boot with, [`LoadError::Refused`]), the boot-time custody guard
//! [`ensure_custody_hardened`], the [`logging`] filter every driver installs,
//! and [`Runtime`] — the bag of `Arc<dyn Port>`s
//! wired by [`Runtime::connect`]. HTTP-free by construction
//! (`tests/no_http_deps.rs`); migrations, background tasks, sessions and
//! cookies are the driver's.

mod config;
mod custody;
pub mod logging;
pub mod ports;
pub(crate) mod runtime;

pub use config::{
    BareOrigin, BareOriginError, CONFIG_DIR_ENV, Config, ConfigRefusal, DATABASE_URL_ENV,
    ENV_PREFIX, EXAMPLE_DEV_ROOT_KEY, Environment, LoadError, PROFILE_ENV, ROOT_KEY_ENV,
    ensure_custody_hardened,
};
pub use runtime::{ConnectError, Runtime};

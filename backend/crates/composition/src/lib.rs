//! The composition root shared by every driving adapter (`api`, `cli`): the
//! figment-loaded [`Config`] (its load refuses a configuration its environment
//! may not boot with, [`LoadError::Refused`]), the custody root [`RootSecret`]
//! and the one custody function behind [`Custody`], the [`logging`] filter every
//! driver installs, and [`Runtime`] — the bag of `Arc<dyn Port>`s wired by
//! [`Runtime::connect`]. HTTP-free by construction (`tests/no_http_deps.rs`).
//! Composition builds the migrator; the driver runs it. Background tasks,
//! sessions and cookies are the driver's.

mod config;
mod custody;
pub mod logging;
pub mod ports;
mod root;
pub(crate) mod runtime;

pub use config::{
    BareOrigin, BareOriginError, CONFIG_DIR_ENV, Config, ConfigRefusal, DATABASE_URL_ENV,
    ENV_PREFIX, EXAMPLE_DEV_ROOT_KEY, Environment, LoadError, PROFILE_ENV, ensure_custody_hardened,
};
pub use custody::Custody;
pub use root::{ROOT_KEY_ENV, RootSecret, RootSecretError};
pub use runtime::{ConnectError, Runtime};

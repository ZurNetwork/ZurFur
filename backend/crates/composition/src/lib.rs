//! The composition root shared by every driving adapter (`api`, `cli`): the
//! figment-loaded [`Config`], the boot-time custody guard
//! [`ensure_custody_hardened`], and [`Runtime`] — the bag of `Arc<dyn Port>`s
//! wired by [`Runtime::connect`]. HTTP-free by construction
//! (`tests/no_http_deps.rs`); migrations, background tasks, sessions and
//! cookies are the driver's.

mod config;
pub mod ports;
pub(crate) mod runtime;

pub use config::{
    CONFIG_DIR_ENV, Config, DATABASE_URL_ENV, ENV_PREFIX, EXAMPLE_DEV_ROOT_KEY, Environment,
    PROFILE_ENV, ROOT_KEY_ENV, ensure_custody_hardened,
};
pub use runtime::{ConnectError, Runtime};

//! The shared leaf crate: what every layer may consume, `domain` included.
//! No workspace dependencies, so it can never pull an adapter or a driver
//! into a lower layer.
//!
//! * [`settings`] — global build-time configuration: the numbers the design
//!   decisions leave to implementation (windows, limits, ceilings).
//!
//! Runtime configuration (env, profiles, ports) is `composition::Config`,
//! not this crate.

pub mod settings;

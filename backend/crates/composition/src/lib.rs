//! The composition root shared by every driving adapter of the Zurfur backend.
//!
//! Two driving adapters exist — the axum HTTP surface (`api`) and the terminal
//! (`cli`, epic ZMVP-199) — and both must boot the *same* runtime: the same
//! figment-loaded [`Config`], the same live adapters behind the same domain
//! ports. This crate is the one place that knows which adapters are live;
//! `api` and `cli` only choose how to drive them. It is HTTP-free by
//! construction (no axum/tower dependency; guarded by `tests/no_http_deps.rs`),
//! so a non-HTTP driver never links a web framework.
//!
//! What lives here: [`Config`] + [`Config::load`] (profile TOML → `DATABASE_URL`
//! → `ZURFUR_*` env, env wins), the boot-time custody guard
//! [`ensure_custody_hardened`], and [`Runtime`] — the bag of `Arc<dyn Port>`s —
//! with [`Runtime::connect`], the live pg + atproto wiring. What deliberately
//! does *not*: migrations (the caller runs [`adapter_pg::migrate`] explicitly),
//! background tasks, sessions, cookies — those are a driver's concern.
//!
//! References: DESIGN "Domains and Applications" (ports and adapters);
//! ZMVP-200; ZMVP-3 (the original composition root).

mod config;
mod runtime;

pub use config::{Config, EXAMPLE_DEV_ROOT_KEY, Environment, ensure_custody_hardened};
pub use runtime::{Runtime, transaction};

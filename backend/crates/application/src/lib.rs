//! The application layer: one function per use case, called by every driver.
//!
//! Ruling (Engineer, 2026-08-24, ZMVP-205): `api` is routing — decode, call,
//! encode — and `cli` is the same over argv. Everything between, the checks
//! and the orchestration, lives here so it is written once and reachable
//! from both.
//!
//! # The shape of a use case
//!
//! A plain `async fn` — no trait until something is generic over use cases —
//! with this signature structure:
//!
//! 1. **Input**: A `Query` or `Command` DTO carrying the operation's data
//!    (e.g., [`account::CreateAccountCommand`], [`user::MeQuery`]), already
//!    validated by its newtypes at the driver's boundary.
//! 2. **Ports**: a per-module struct of borrowed `&dyn` ports (e.g.
//!    [`account::AccountPorts`]) — the module's own stores plus `database`;
//!    a port from another module rides as an extra parameter on the one
//!    function that needs it. Bare `&dyn Port` parameters are fine for a
//!    read (a `Query` never sees `database`; `me` is the example).
//! 3. **Config & time**: runtime config values the use case needs
//!    (e.g. `handle_domain`) and `now: DateTimeUtc` as plain parameters —
//!    environment facts, never DTO fields. Build-time knobs (rate-limit
//!    windows, quarantine) come from `shared::settings`.
//! 4. **Output**: A `Result<OutputDTO, <Module>Error>` where the DTO holds
//!    domain VALUES — newtypes (`AccountId`, `Did`, `Handle`) or plain
//!    strings for facts the domain has no newtype for — never ENTITIES
//!    (e.g. [`account::CreateAccountResult`], [`user::MeResult`]).
//!
//! Error handling:
//! * One error enum per module — mapped by drivers to their own surface (API to
//!   problem+json, CLI to `{class, code}`) — so shared vocabulary is structural:
//!   the same variant codes to the same HTTP/CLI status everywhere.
//!
//! Transaction orchestration:
//! * Reads skip the unit of work entirely. Writes call [`transaction`] (or call
//!   it via ports passed to this crate), which handles `begin`/`commit`/`rollback`.
//!
//! Scope:
//! * No use case knows HTTP, sessions, argv, or exit codes. The driver is
//!   responsible for all I/O and interpretation.
//!
//! # Dependencies
//!
//! `api`, `cli` → `application` → `domain`. `composition` wires the ports
//! the drivers hand in; this crate never sees it. `domain` never depends on
//! this crate (`tests/dep_guard.rs`).
//!
//! Modules follow the domain's entities (work by module): [`user`], [`account`],
//! [`commission`] first; others follow as their handlers move. A use case need
//! not have an actor — [`commission::sweep_deadlines`] is the system acting on
//! an injected `now`, its timer left to the driver.

pub mod account;
pub mod commission;
mod transaction;
pub mod user;

pub use transaction::transaction;

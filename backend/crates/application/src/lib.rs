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
//! taking the **ports it needs** (as `&dyn Port`, never a whole runtime),
//! the acting user where one applies, and a typed input; returning
//! `Result<Output, <Module>Error>`.
//!
//! * One error enum per module. A driver maps it to its own surface — the
//!   API to problem+json, the CLI to `{class, code}` — so ruling 5's shared
//!   vocabulary is structural: the same variant is the same code everywhere.
//! * Steps, in order, where they apply: load → authorize → open the unit of
//!   work → write → changelog / outbox → commit. Reads skip what they don't
//!   need.
//! * No use case knows HTTP, sessions, argv, or exit codes.
//!
//! # Dependencies
//!
//! `api`, `cli` → `application` → `domain`. `composition` wires the ports
//! the drivers hand in; this crate never sees it. `domain` never depends on
//! this crate (`tests/dep_guard.rs`).
//!
//! Modules follow the domain's entities (work by module): [`user`] first;
//! `account`, `commission` follow as their handlers move.

pub mod user;

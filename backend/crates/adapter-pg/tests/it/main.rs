//! The single adapter-pg integration binary (ZMVP-134).
//!
//! One binary instead of one per file so every module here shares the same
//! test process — and therefore the same refcounted Postgres container from
//! `test_support::pg` (one boot + one migration replay for the whole crate,
//! then a template clone per test), with the modules' tests running in
//! parallel on libtest's threads. `cargo test` runs separate test binaries
//! sequentially, so per-file binaries would each pay their own boot.
//!
//! Filter as usual: `cargo test -p adapter-pg --test it account::`.

mod account;
mod actor_identity;
mod codegen_current;
mod commission;
mod commission_changelog;
mod commission_deadline;
mod commission_file;
mod commission_maturity;
mod commission_node;
mod commission_positioning;
mod commission_seat;
mod commission_seat_invitation;
mod commission_slot;
mod key_store;
mod no_bare_pool_writes;
mod plc_operation_log;
mod profile_cache;
mod session_store;
mod single_binary_guard;

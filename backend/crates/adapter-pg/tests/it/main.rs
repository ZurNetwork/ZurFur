//! The single adapter-pg integration binary.
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
mod actor_projection;
mod bug_report;
mod codegen_current;
mod commission;
mod commission_changelog;
mod commission_deadline;
mod commission_element;
mod commission_file;
mod commission_maturity;
mod commission_positioning;
mod commission_reads;
mod commission_seat;
mod commission_seat_invitation;
mod commission_slot;
mod commission_standing;
mod delete_stored_oauth_sessions;
mod key_store;
mod migration_steps;
mod no_bare_pool_writes;
mod node;
mod node_backfill;
mod plc_operation_log;
mod profile_cache;
mod schema_status;
mod seed;
mod session_store;
mod single_binary_guard;
mod unit_of_work;

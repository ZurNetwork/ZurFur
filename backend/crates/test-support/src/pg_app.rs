//! The PostgreSQL test setup: serves the app on the live PostgreSQL stores in a
//! private, migrated database, signed in through the in-process sign-in. It
//! swaps two of the live runtime's ports, sign-in and profile, for one
//! in-process PDS, so no test reaches the network; every store stays live.

mod den_world;
mod in_process_pds;
mod pairs;
mod served;
mod statements;

pub use den_world::{
    DenWorld, LEGACY_NAME, ProfileCacheState, SeededCommission, UNNAMED, absent_account,
    absent_commission, absent_file, absent_slot, absent_workflow, den_world, den_world_recording,
    stranger,
};
pub use in_process_pds::InProcessPds;
pub use pairs::{
    COMMISSION_NOT_FOUND, FILE_NOT_FOUND, MissPair, archived_participant_pairs,
    commission_read_live_pairs, commission_read_stranger_pairs, den_account_pair,
    den_character_pair, den_participant_pairs, den_stranger_pairs, den_stranger_targets,
    download_participant_pair, public_live_download_pair, stranger_targets,
};
pub use served::{
    PgRuntime, PgServed, SignedIn, pg_runtime, pg_runtime_recording, serve, serve_configured,
    serve_recording, serve_wrapped,
};
pub use statements::{Statement, StatementLog, record_statements};

//! The PostgreSQL test setup: serves the app on the live PostgreSQL stores in a
//! private, migrated database, signed in through the in-process sign-in. It
//! swaps two of the live runtime's ports, sign-in and profile, for one
//! in-process PDS, so no test reaches the network; every store stays live.

mod in_process_pds;
mod served;

pub use in_process_pds::InProcessPds;
pub use served::{PgServed, serve};

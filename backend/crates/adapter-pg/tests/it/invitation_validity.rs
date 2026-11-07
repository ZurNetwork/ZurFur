//! The shared invitation-validity suite (`test_support::invitation_validity_contract`), run
//! against PostgreSQL: one fresh database per case. Requires a container runtime socket.

use test_support::invitation_validity_contract::Runtime;

/// A fresh database's runtime, and the database kept alive.
async fn fresh() -> (Runtime, impl Sized) {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    (runtime, pg)
}

test_support::invitation_validity_contract_tests!(fresh);

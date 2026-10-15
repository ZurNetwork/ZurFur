use test_support::log_capture::CapturedLog;

use super::*;
use crate::account::Accounts;
use crate::test_world::world;

#[tokio::test]
async fn a_use_case_that_injects_nothing_is_logged_too() {
    let world = world().await;
    let accounts = Accounts::new(&world.ports, &*world.runtime.did_minter);
    let facts = Facts {
        accounts: &accounts,
    };
    let query = Query {
        account_id: adapter_mem::fixture::supreme_arts(),
    };
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let output = facts.exist(query).await;

    assert!(output.is_ok_and(|output| !output.has_facts));
    let text = log.text();
    let span = "use_case{name=\"application::account::facts::exist::exist\"}";
    let finished = text
        .lines()
        .find(|line| line.contains(span) && line.contains("use case finished"))
        .unwrap_or_else(|| panic!("no finish line: {text}"));
    assert!(finished.contains("outcome=\"ok\""), "{finished}");
    let started = text
        .lines()
        .any(|line| line.contains(span) && line.contains("use case started"));
    assert!(started, "{text}");
}

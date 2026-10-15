//! What `#[use_case]` logs: each call inside a `debug` span `use_case{name}` between a
//! `use case started` and a `use case finished` event carrying its outcome, an error by its
//! `Display` alone, never the command. Alone in its binary: tracing caches a callsite's
//! interest, so a sibling test running without the capture could blind it.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use application::account;
use chrono::Utc;
use domain::elements::{did::Did, handle::HandleDomain, user::UserId};
use domain::ports::{Database, UnitOfWork};
use test_support::log_capture::CapturedLog;
use tracing_subscriber::fmt::format::FmtSpan;

/// The name the founding command carries; it must never reach a use-case line.
const PLANTED_NAME: &str = "Plantedname Studio";

/// The handle the founding command carries; it must never reach a use-case line.
const PLANTED_HANDLE: &str = "plantedhandle.zurfur.app";

/// The span name the account founding runs in.
const FOUNDING: &str = "use_case{name=\"application::account::create::create\"}";

/// A debug subscriber printing span closes, writing plain text to `log`.
fn subscriber(log: &CapturedLog) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish()
}

fn handle_domain() -> HandleDomain {
    "zurfur.app".parse().expect("a valid handle domain")
}

fn command(actor_id: UserId) -> account::create::Command {
    account::create::Command {
        actor_id,
        name: PLANTED_NAME.parse().expect("a valid name"),
        handle: PLANTED_HANDLE.parse().expect("a valid handle"),
    }
}

/// The user recognized for `did`, provisioned straight through a unit.
async fn recognized(database: &dyn Database, did: &Did) -> UserId {
    let provisioned = did.clone();
    let user = application::transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&provisioned).await
    })
    .await
    .expect("provision");
    user.id
}

/// The lines of `text` that name the founding's span and `event`.
fn founding_lines<'t>(text: &'t str, event: &str) -> Vec<&'t str> {
    text.lines()
        .filter(|line| line.contains(FOUNDING) && line.contains(event))
        .collect()
}

#[tokio::test]
async fn a_use_case_logs_its_start_its_outcome_and_its_error_by_display_alone() {
    let did = Did::from("did:plc:usecaselog".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let user = recognized(&*runtime.database, &did).await;
    let app = runtime.app();
    let log = CapturedLog::default();
    let _guard = tracing::subscriber::set_default(subscriber(&log));

    let founded = app
        .accounts()
        .create(command(user.clone()), &handle_domain(), Utc::now())
        .await;
    let repeat = app
        .accounts()
        .create(command(user), &handle_domain(), Utc::now())
        .await;

    assert!(founded.is_ok(), "the first founding succeeds");
    let Err(taken) = repeat else {
        panic!("a repeat handle must not found a second account");
    };
    let text = log.text();
    assert_eq!(founding_lines(&text, "use case started").len(), 2, "{text}");
    let finished = founding_lines(&text, "use case finished");
    assert_eq!(finished.len(), 2, "{text}");
    assert!(finished[0].contains("DEBUG"), "{}", finished[0]);
    assert!(finished[0].contains("outcome=\"ok\""), "{}", finished[0]);
    assert!(!finished[0].contains("error="), "{}", finished[0]);
    assert!(finished[1].contains("outcome=\"err\""), "{}", finished[1]);
    let by_display = format!("error={taken}");
    assert!(finished[1].contains(&by_display), "{}", finished[1]);
    assert_eq!(
        founding_lines(&text, "}: application::account::create: close time.busy").len(),
        2,
        "{text}"
    );
    let use_case_lines = text
        .lines()
        .filter(|line| line.contains("use case ") || line.contains("close time.busy"));
    for line in use_case_lines {
        for planted in [PLANTED_NAME, "plantedhandle"] {
            assert!(
                !line.contains(planted),
                "a use-case line carries {planted}: {line}"
            );
        }
    }
}

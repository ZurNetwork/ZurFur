//! A leave that loses its race with a transfer, served over PostgreSQL: the pool still reads
//! the leaver as a Member, the unit finds them the Owner, and the route answers
//! `409 owner_cannot_leave` with one warning that names no DID. Requires a container runtime
//! socket.
//!
//! Its own test binary: the capturing subscriber is this binary's global one.

use std::sync::Arc;

use domain::elements::role::Role;
use serde_json::json;
use test_support::{
    log_capture::CapturedLog,
    pg_app::{self, DenWorld},
    store_doubles::{AccountsScript, ScriptedAccounts},
};
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

mod common;

/// The message the race warning carries.
const RACE_WARNING: &str = "an account act lost its race with a concurrent change";

/// `text` without its ANSI color escapes.
fn without_colors(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_escape = false;
    for character in text.chars() {
        match (in_escape, character) {
            (false, '\u{1b}') => in_escape = true,
            (false, _) => plain.push(character),
            (true, 'm') => in_escape = false,
            (true, _) => {}
        }
    }
    plain
}

/// The level of a formatted line: the first of its words that names one.
fn level_of(line: &str) -> Option<&str> {
    line.split_whitespace()
        .find(|word| ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].contains(word))
}

#[tokio::test]
async fn a_leave_losing_to_a_transfer_answers_409_and_warns_once_without_a_did() {
    let log = CapturedLog::default();
    api::logging::subscriber(&api::Environment::DEV, EnvFilter::new("trace"), log.clone())
        .try_init()
        .expect("the capture is this binary's only subscriber");
    let script = AccountsScript::default();
    let scripting = script.clone();
    let world: DenWorld = pg_app::den_world(move |mut runtime| {
        let scripted = ScriptedAccounts::new(Arc::clone(&runtime.accounts), scripting);
        runtime.accounts = Arc::new(scripted);
        api::app(runtime)
    })
    .await;
    world.add_bob_as_member(&world.supreme_arts).await;
    let bob = adapter_mem::fixture::bob();
    script.answer_role_of(bob.clone(), world.supreme_arts.clone(), Role::Member);
    let transfer = json!({ "new_owner": bob.to_string() });
    let transferred = world
        .alice
        .post(format!(
            "{}/accounts/{}/transfer",
            world.base_url(),
            world.supreme_arts
        ))
        .json(&transfer)
        .send()
        .await
        .expect("POST /accounts/{id}/transfer is answered");
    assert_eq!(transferred.status(), 200, "alice hands Supreme Arts to bob");

    let left = world
        .bob
        .delete(format!(
            "{}/accounts/{}/members/me",
            world.base_url(),
            world.supreme_arts
        ))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members/me is answered");

    common::assert_problem(left, 409, "owner_cannot_leave").await;
    let text = without_colors(&log.text());
    let warnings: Vec<&str> = text
        .lines()
        .filter(|line| line.contains(RACE_WARNING))
        .collect();
    assert_eq!(warnings.len(), 1, "one race warning: {warnings:#?}");
    let raised: Vec<&str> = text
        .lines()
        .filter(|line| matches!(level_of(line), Some("WARN" | "ERROR")))
        .collect();
    assert_eq!(raised, warnings, "the race warning is the only one raised");
    assert_eq!(level_of(warnings[0]), Some("WARN"), "{}", warnings[0]);
    assert!(
        !warnings[0].contains("did:"),
        "the warning names no DID: {}",
        warnings[0]
    );
}

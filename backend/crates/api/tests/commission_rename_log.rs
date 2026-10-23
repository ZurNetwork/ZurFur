//! `PATCH /commissions/{id}/title` logs no title, DID or handle at info or above, accepted or
//! refused.
//!
//! Its own test binary: the capturing subscriber is this binary's global one, so no other
//! test's thread can leave a callsite's interest cached without it.

use domain::elements::{did::Did, profile::Profile};
use serde_json::{Value, json};
use test_support::{
    http::{client, serve, sign_in},
    log_capture::CapturedLog,
};
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

/// The artist's DID: the one user the in-memory runtime signs in.
const ARTIST: &str = "did:plc:artist";

/// The artist's handle.
const ARTIST_HANDLE: &str = "artist.bsky.social";

/// The mark both titles carry, which no line at info or above may hold.
const TITLE_MARK: &str = "7f3a";

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

/// `PATCH /commissions/{id}/title` with `body` as `session`.
async fn rename(session: &reqwest::Client, base: &str, id: &str, body: &Value) -> u16 {
    let response = session
        .patch(format!("{base}/commissions/{id}/title"))
        .json(body)
        .send()
        .await
        .expect("PATCH /commissions/{id}/title");
    response.status().as_u16()
}

#[tokio::test]
async fn a_rename_logs_no_title_and_no_identity_at_info_or_above() {
    let log = CapturedLog::default();
    api::logging::subscriber(&api::Environment::DEV, EnvFilter::new("trace"), log.clone())
        .try_init()
        .expect("the capture is this binary's only subscriber");
    let artist = Did::from(ARTIST.to_owned());
    let profile = Profile::new(artist.clone(), ARTIST_HANDLE);
    let served = serve(
        test_support::runtime::mem(&artist).profile(profile),
        api::app,
    )
    .await;
    let base = served.base_url;
    let session = client();
    sign_in(&session, &base, ARTIST_HANDLE).await;
    let created: Value = session
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions")
        .json()
        .await
        .expect("a JSON body");
    let id = created["id"].as_str().expect("an id");
    let accepted_title = json!({ "title": format!("Quietly renamed {TITLE_MARK}") });
    let refused_title = json!({ "title": format!("Quietly/refused {TITLE_MARK}") });

    let accepted = rename(&session, &base, id, &accepted_title).await;
    let refused = rename(&session, &base, id, &refused_title).await;

    assert_eq!(accepted, 200);
    assert_eq!(refused, 422);
    let text = without_colors(&log.text());
    assert!(
        text.lines()
            .any(|line| line.contains("application::commission::rename")),
        "the capture saw no rename: {text}"
    );
    let title_at_info: Vec<&str> = text
        .lines()
        .filter(|line| matches!(level_of(line), Some("INFO" | "WARN" | "ERROR")))
        .filter(|line| line.contains(TITLE_MARK))
        .collect();
    assert!(
        title_at_info.is_empty(),
        "the log names a title at info or above: {title_at_info:#?}"
    );
    let identity_at_info: Vec<&str> = text
        .lines()
        .filter(|line| matches!(level_of(line), Some("INFO" | "WARN" | "ERROR")))
        .filter(|line| line.contains(ARTIST) || line.contains(ARTIST_HANDLE))
        .collect();
    assert!(
        identity_at_info.is_empty(),
        "the log names a DID or handle at info or above: {identity_at_info:#?}"
    );
}

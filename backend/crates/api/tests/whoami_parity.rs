//! `zurfur session whoami` must render exactly like `GET /me` (ZMVP-203 AC).
//! The CLI cannot name the generated `GetMeResponse` (it lives inside `api`,
//! behind axum), so its projection is a hand copy — this test is what keeps
//! the copy honest until the contract moves to a leaf crate (DD 40992770 D11).

use api::generated::GetMeResponse;
use cli::commands::session::Whoami;
use domain::elements::{did::Did, profile::Profile};

const DID: &str = "did:plc:parity";

/// The handler's own projection rule (`routes/session.rs::me`), applied to
/// the generated type.
fn http(profile: Option<Profile>) -> serde_json::Value {
    let did = DID.to_string();
    let body = match profile {
        Some(profile) => GetMeResponse {
            did,
            handle: Some(profile.handle),
            display_name: profile.display_name,
            avatar_url: profile.avatar_url,
        },
        None => GetMeResponse {
            did,
            handle: None,
            display_name: None,
            avatar_url: None,
        },
    };
    serde_json::to_value(body).unwrap()
}

fn terminal(profile: Option<Profile>) -> serde_json::Value {
    serde_json::to_value(Whoami::project(DID.to_string(), profile)).unwrap()
}

#[test]
fn whoami_renders_exactly_like_get_me() {
    let bare = Profile::new(Did::new(DID.to_string()), "parity.bsky.social");
    let cases = [
        None,
        Some(bare.clone()),
        Some(bare.clone().with_display_name("Parity")),
        Some(
            bare.with_display_name("Parity")
                .with_avatar_url("https://cdn/avatar.png"),
        ),
    ];
    for case in cases {
        assert_eq!(terminal(case.clone()), http(case));
    }
}

//! `zurfur session whoami` must render exactly like `GET /me` (ZMVP-203 AC).
//! Both drivers call the one use case (`application::user::me`, ZMVP-205)
//! and project its `Me` into their own response type — the CLI cannot name
//! the generated `GetMeResponse` (it lives inside `api`, behind axum), so
//! its `Whoami` is a hand copy. This test keeps the two projections
//! identical until the contract moves to a leaf crate (DD 40992770 D11).

use api::generated::GetMeResponse;
use application::user::Me;
use chrono::Utc;
use cli::commands::session::Whoami;
use domain::elements::{
    did::Did,
    profile::Profile,
    user::{User, UserId},
};
use uuid::Uuid;

fn me(profile: Option<Profile>) -> Me {
    let did = Did::new("did:plc:parity".to_string());
    let user = User {
        id: UserId::new(Uuid::now_v7()),
        did,
        created_at: Utc::now(),
    };
    Me { user, profile }
}

#[test]
fn whoami_renders_exactly_like_get_me() {
    let bare = Profile::new(Did::new("did:plc:parity".to_string()), "parity.bsky.social");
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
        let http = serde_json::to_value(GetMeResponse::from(me(case.clone()))).unwrap();
        let terminal = serde_json::to_value(Whoami::from(me(case))).unwrap();
        assert_eq!(terminal, http);
    }
}

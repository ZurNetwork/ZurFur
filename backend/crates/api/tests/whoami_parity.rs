//! `zurfur session whoami` must render exactly like `GET /me`.
//! Both drivers call the one use case (`application::user::me`)
//! and project its `MeResult` into their own response type — the CLI cannot
//! name the generated `GetMeResponse` (it lives inside `api`, behind axum),
//! so its `Whoami` is a hand copy. This test keeps the two projections
//! identical until the contract moves to a leaf crate both drivers can share.

use api::generated::GetMeResponse;
use application::user::me::{self, MeProfile};
use cli::commands::session::Whoami;
use domain::elements::{did::Did, user::UserId};

fn me(profile: Option<MeProfile>) -> me::Output {
    me::Output {
        id: UserId::new(Did::new("did:plc:parity".to_string())),
        profile,
    }
}

#[test]
fn whoami_renders_exactly_like_get_me() {
    let bare = MeProfile {
        handle: "parity.bsky.social".to_string(),
        display_name: None,
        avatar_url: None,
    };
    let named = MeProfile {
        display_name: Some("Parity".to_string()),
        ..bare.clone()
    };
    let pictured = MeProfile {
        avatar_url: Some("https://cdn/avatar.png".to_string()),
        ..named.clone()
    };
    let cases = [None, Some(bare), Some(named), Some(pictured)];
    for case in cases {
        let http = serde_json::to_value(GetMeResponse::from(me(case.clone()))).unwrap();
        let terminal = serde_json::to_value(Whoami::from(me(case))).unwrap();
        assert_eq!(terminal, http);
    }
}

//! `zurfur account create` must render exactly like `POST /accounts`.
//! Both drivers call the one use case
//! (`application::account::create_account`) and project its
//! `CreateAccountResult`; the CLI cannot name the generated
//! `CreateAccountResponse` (it lives inside `api`, behind axum), so its
//! `Founded` is a hand copy. This test keeps the two projections identical
//! until the contract moves to a leaf crate both drivers can share — the same
//! guard `whoami_parity.rs` gives `GET /me`.

use api::generated::CreateAccountResponse;
use application::account::create;
use cli::commands::account::Founded;
use domain::elements::{account::AccountId, did::Did};

fn founded() -> create::Output {
    create::Output {
        account_id: AccountId::new(Did::new("did:plc:parity".to_string())),
        handle: "parity.zurfur.app".parse().expect("a valid handle"),
        name: "Parity Studio".parse().expect("a valid name"),
    }
}

#[test]
fn account_create_renders_exactly_like_post_accounts() {
    // Built twice rather than cloned: `create::Output` derives none of DD
    // 55836674 D2's `Debug, Clone, PartialEq, Eq`, and the constructor is
    // deterministic, so the two calls carry identical values.
    let http = serde_json::to_value(CreateAccountResponse::from(founded())).unwrap();
    let terminal = serde_json::to_value(Founded::from(founded())).unwrap();
    assert_eq!(terminal, http);
}

//! `zurfur account create` must render exactly like `POST /accounts`
//! (ZMVP-205 slice 4). Both drivers call the one use case
//! (`application::account::create_account`) and project its
//! `CreateAccountResult`; the CLI cannot name the generated
//! `CreateAccountResponse` (it lives inside `api`, behind axum), so its
//! `Founded` is a hand copy. This test keeps the two projections identical
//! until the contract moves to a leaf crate (DD 40992770 D11) — the same
//! guard `whoami_parity.rs` gives `GET /me`.

use api::generated::CreateAccountResponse;
use application::account::CreateAccountResult;
use cli::commands::account::Founded;
use domain::elements::{account::AccountId, did::Did};
use uuid::Uuid;

fn founded() -> CreateAccountResult {
    CreateAccountResult {
        account_id: AccountId::new(Uuid::now_v7()),
        did: Did::new("did:plc:parity".to_string()),
        handle: "parity.zurfur.app".parse().expect("a valid handle"),
        name: "Parity Studio".parse().expect("a valid name"),
    }
}

#[test]
fn account_create_renders_exactly_like_post_accounts() {
    let result = founded();
    let http = serde_json::to_value(CreateAccountResponse::from(result.clone())).unwrap();
    let terminal = serde_json::to_value(Founded::from(result)).unwrap();
    assert_eq!(terminal, http);
}

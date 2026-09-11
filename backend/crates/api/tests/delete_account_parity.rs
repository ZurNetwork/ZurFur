//! `zurfur account delete` must render exactly like `DELETE /accounts/{id}`
//! (ZMVP-205 slice 5). Both drivers call the one use case
//! (`application::account::delete`) and project its
//! `delete::Output`; the CLI cannot name the generated
//! `DeleteAccountResponse` (it lives inside `api`, behind axum), so its
//! `Deleted` is a hand copy. This test keeps the two projections identical —
//! for BOTH outcomes, so neither driver can drift on the `soft`/`hard`
//! spelling — until the contract moves to a leaf crate (DD 40992770 D11); the
//! same guard `create_account_parity.rs` gives `POST /accounts`.

use api::generated::DeleteAccountResponse;
use application::account::delete::{self, DeleteOutcome};
use cli::commands::account::Deleted;

#[test]
fn account_delete_renders_exactly_like_delete_accounts() {
    // One outcome per projection rather than one shared value: `DeleteOutcome`
    // derives none of DD 55836674 D2's `Debug, Clone, PartialEq, Eq`, so it
    // can be neither copied into both nor named with `{:?}` — its `Display` is
    // the spelling under test anyway.
    let pairs = [
        [DeleteOutcome::Soft, DeleteOutcome::Soft],
        [DeleteOutcome::Hard, DeleteOutcome::Hard],
    ];
    for [rendered_http, rendered_terminal] in pairs {
        let outcome = rendered_http.to_string();
        let http = serde_json::to_value(DeleteAccountResponse::from(delete::Output {
            outcome: rendered_http,
        }))
        .unwrap();
        let terminal = serde_json::to_value(Deleted::from(delete::Output {
            outcome: rendered_terminal,
        }))
        .unwrap();
        assert_eq!(terminal, http, "the two projections of {outcome} differ");
    }
}

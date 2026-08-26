//! The `session` namespace: who the CLI acts as. `whoami` and `logout`
//! (ZMVP-203) read and clear the identity file; `login` (ZMVP-204) is blocked
//! on the Engineer's client-model ruling and answers an honest
//! `not_implemented` problem until then — never a fake success.

use std::path::Path;

use application::user::{MeError, MeQuery, MeResult};
use clap::Subcommand;
use composition::Runtime;
use serde::Serialize;

use crate::{CliError, identity, principal::Principal};

/// The session operations.
#[derive(Debug, Subcommand)]
pub enum SessionOp {
    /// Sign in through the browser and record the acting identity.
    Login,
    /// Forget the acting identity (the local record only).
    Logout,
    /// Show the acting identity, the way `GET /me` reports it.
    Whoami,
}

/// `whoami`'s projection — the same keys, spelling, and omissions as the
/// HTTP `GetMeResponse`: the DID always; handle/displayName/avatarUrl only
/// when the profile resolved. A hand copy of the generated contract type
/// (which lives inside `api`, behind axum); `api/tests/whoami_parity.rs`
/// asserts the two render identically until the contract moves to a leaf
/// crate (DD 40992770 D11).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Whoami {
    did: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

impl From<MeResult> for Whoami {
    /// The `GET /me` projection rule: a resolved profile contributes its
    /// handle + optionals; no profile degrades to the bare DID.
    fn from(me: MeResult) -> Self {
        let did = me.did.to_string();
        match me.profile {
            Some(profile) => Whoami {
                did,
                handle: Some(profile.handle),
                display_name: profile.display_name,
                avatar_url: profile.avatar_url,
            },
            None => Whoami {
                did,
                handle: None,
                display_name: None,
                avatar_url: None,
            },
        }
    }
}

/// `logout`: forget the local identity. Needs no runtime — `run` in `lib.rs`
/// answers it before any config or database is touched, so "forget my
/// credential" can never be blocked by a broken stack.
pub fn logout(identity_path: &Path) -> Result<serde_json::Value, CliError> {
    let removed = identity::delete(identity_path)?;
    Ok(serde_json::json!({ "loggedOut": true, "hadIdentity": removed }))
}

/// Run one session op over the runtime, with the identity file at `identity_path`.
pub async fn run(
    runtime: &Runtime,
    identity_path: &Path,
    op: SessionOp,
) -> Result<serde_json::Value, CliError> {
    match op {
        SessionOp::Login => Err(CliError::infra(
            "not_implemented",
            "`session login` is blocked on the client-model ruling (ZMVP-204)",
        )),
        SessionOp::Logout => logout(identity_path),
        SessionOp::Whoami => {
            let principal = Principal::resolve(runtime, identity_path).await?;
            let query = MeQuery {
                user_id: principal.user.id,
            };
            let me = application::user::me(
                query,
                &*runtime.users,
                &*runtime.profile_cache,
                &*runtime.profile_source,
            )
            .await
            .map_err(|e| match e {
                MeError::UnknownUser(_) => CliError::domain("not_authenticated", e),
                MeError::Store(_) => CliError::infra("internal_error", e),
            })?;
            let body = Whoami::from(me);
            Ok(serde_json::to_value(body).expect("Whoami serializes"))
        }
    }
}

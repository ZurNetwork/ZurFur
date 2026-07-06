//! The commissions route group: the commission JSON API, split per area
//! (ZMVP-65/87) so each later commission ticket adds a file here rather than
//! growing one hotspot — the same seam-splitting move as [`super`] itself:
//!
//! - [`create`] — `POST /commissions` (ZMVP-65 + the creation changelog entry;
//!   ZMVP-71 mints the root surface in the same unit of work).
//! - [`changelog`] — `GET /commissions/{id}/changelog` (the ordered read).
//! - [`notes`] — `POST /commissions/{id}/notes` (free text into the record).
//! - [`channel`] — `PUT`/`DELETE /commissions/{id}/channel` (the linked-channel
//!   pointer).
//! - [`delete`] — `DELETE /commissions/{id}` (the fact-free hard delete,
//!   ZMVP-66).
//! - [`archive`] — `POST /commissions/{id}/archive` / `POST
//!   /commissions/{id}/unarchive` (the soft archive/un-archive acts, ZMVP-68).
//! - [`maturity`] — `PUT /commissions/{id}/maturity` (ZMVP-31: the owner rates
//!   the commission; replace-only, no clear).
//! - [`surfaces`] — `POST /commissions/{id}/surfaces` (ZMVP-71: the owner grows
//!   the content tree).
//! - [`components`] — `POST /commissions/{id}/components` (ZMVP-72: the owner
//!   adds a leaf under a surface).
//! - [`remove`] — `DELETE /commissions/{id}/nodes/{node}` (ZMVP-73: the owner
//!   prunes a node and its subtree; the root refuses).
//! - [`status`] — `PUT`/`DELETE /commissions/{id}/status/direction` (the
//!   direction-axis Status, ZMVP-85).
//! - [`deadline`] — `PUT`/`DELETE /commissions/{id}/deadline` and
//!   `PUT`/`DELETE /commissions/{id}/status/deadline` (the deadline envelope
//!   field and the manual Delayed flag, ZMVP-86; the system half — the Late
//!   sweeper — lives in [`crate::sweep_deadlines`], not on a route).
//!
//! Commissions are user-scoped (no Account required — ZMVP-47, DD 26247170) and
//! entirely Index-side. Like the rest of the JSON API the group returns status
//! codes, not redirects: an unrecognized caller gets a `401`. It is part of the
//! cookie surface, so [`crate::app`] mounts the group under the
//! first-party-`Origin` (CSRF) layer.
//!
//! **The closed door.** Whether a commission *exists* is participant-only
//! knowledge: every handler here answers a non-participant — and a truly absent
//! id — with the one uniform [`Problem::commission_not_found`] 404 via
//! [`require_participant`], never a 403 (an existence oracle). The changelog is
//! Total-tier: this holds at every future root mode.
//!
//! References: ZMVP-65/87; DESIGN/Commission (`3276807`), the Changelog DD
//! (`30408741`).

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
};
use domain::elements::{
    commission::{Commission, CommissionId},
    user::{User, UserId},
};
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, SESSION_USER_KEY, problem::Problem};

mod archive;
mod changelog;
mod channel;
mod components;
mod create;
mod deadline;
mod delete;
mod files;
mod maturity;
mod notes;
mod positioning;
mod remove;
mod status;
mod surfaces;

/// Slack, in bytes, added above [`Config::max_upload_bytes`](crate::Config::max_upload_bytes)
/// for the request body-size limit on the upload route: a `multipart/form-data`
/// envelope (boundary + part headers) adds a little to the file's own size, so the
/// framework backstop must sit above the file cap. The exact per-file cap is still
/// enforced on the read bytes (a `413`); this only keeps the envelope overhead from
/// tripping the backstop for a file that is itself within the cap.
const UPLOAD_BODY_SLACK_BYTES: usize = 1024 * 1024;

/// The commissions route group. On the cookie surface; the composition root
/// wraps the group with the CSRF
/// [`require_first_party_origin`](super::require_first_party_origin) layer.
///
/// The changelog surface is deliberately **append-and-read only** (ZMVP-87 AC4):
/// `GET` is the stream's single method — no route updates or removes an entry,
/// so editing history is unrepresentable at the HTTP layer too.
///
/// `max_upload_bytes` is [`Config::max_upload_bytes`](crate::Config::max_upload_bytes),
/// used to size the request body-size limit on the file-upload route (the exact
/// per-file cap is enforced in the handler).
pub(crate) fn commissions_router(max_upload_bytes: usize) -> Router<AppState> {
    let upload_body_limit = max_upload_bytes.saturating_add(UPLOAD_BODY_SLACK_BYTES);
    Router::new()
        .route("/commissions", post(create::create_commission))
        .route(
            "/commissions/{id}",
            axum::routing::delete(delete::delete_commission),
        )
        .route(
            "/commissions/{id}/changelog",
            get(changelog::read_changelog),
        )
        .route("/commissions/{id}/notes", post(notes::write_note))
        .route(
            "/commissions/{id}/channel",
            put(channel::link_channel).delete(channel::clear_channel),
        )
        .route(
            "/commissions/{id}/archive",
            post(archive::archive_commission),
        )
        .route(
            "/commissions/{id}/unarchive",
            post(archive::unarchive_commission),
        )
        .route(
            "/commissions/{id}/placements",
            post(positioning::place_commission),
        )
        .route("/commissions/{id}/grants", post(positioning::grant_view))
        .route(
            "/commissions/{id}/grants/{account_id}",
            delete(positioning::revoke_view),
        )
        .route("/commissions/{id}/maturity", put(maturity::set_maturity))
        .route("/commissions/{id}/surfaces", post(surfaces::add_surface))
        .route(
            "/commissions/{id}/components",
            post(components::add_component),
        )
        .route(
            "/commissions/{id}/nodes/{node}",
            delete(remove::remove_node),
        )
        .route(
            "/commissions/{id}/status/direction",
            put(status::set_direction_status).delete(status::clear_direction_status),
        )
        .route(
            "/commissions/{id}/deadline",
            put(deadline::set_deadline).delete(deadline::clear_deadline),
        )
        .route(
            "/commissions/{id}/status/deadline",
            put(deadline::set_deadline_status).delete(deadline::clear_deadline_status),
        )
        .route(
            // The upload raises the request body limit above the default (2 MiB) to
            // the configured cap plus envelope slack; the download is a plain GET.
            "/commissions/{id}/files",
            post(files::upload_file).layer(DefaultBodyLimit::max(upload_body_limit)),
        )
        .route(
            "/commissions/{id}/files/{file_id}",
            get(files::download_file),
        )
}

/// Resolve the session to the acting [`User`] — the shared authentication step
/// of every commission handler. An absent/unreadable session or a vanished User
/// is a `401`, never a redirect, because the frontend *calls* these endpoints.
async fn current_user(state: &AppState, session: &Session) -> Result<User, Problem> {
    let id = session
        .get::<Uuid>(SESSION_USER_KEY)
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)?;
    state
        .users
        .find(UserId::new(id))
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)
}

/// The closed-door gate (ZMVP-87 AC5; DESIGN/Commission): admit `user` only if
/// they are a Participant of `commission`, answering **everyone else with the
/// one uniform [`Problem::commission_not_found`] 404** — the same body whether
/// the commission is hidden from them or does not exist at all
/// ([`CommissionStore::is_participant`](domain::ports::CommissionStore::is_participant)
/// answers `false` for both), so no response distinguishes the cases. Never a
/// 403: a 403 would confirm existence.
async fn require_participant(
    state: &AppState,
    commission: CommissionId,
    user: UserId,
) -> Result<(), Problem> {
    if state.commissions.is_participant(commission, user).await? {
        Ok(())
    } else {
        Err(Problem::commission_not_found())
    }
}

/// The shared **owner-authority gate** — owner-only in v1, shaped so the future
/// Commission Admin (ZMVP-83) extends *this one match* rather than growing a
/// second path (one seam, swept once when the Admin arm activates): resolve the
/// commission, then rank the caller. A non-participant (who may not learn the
/// commission exists) gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404; a participant
/// who is not the owner already knows it exists, so refusing them managing
/// authority is an honest `403` — today that arm is unreachable (the owner is
/// the only participant until ZMVP-79 seats more). Consumed by every
/// owner-gated commission handler ([`channel`], [`delete`], [`archive`],
/// [`surfaces`]). Returns the resolved [`Commission`] so callers needn't
/// re-read it.
async fn require_owner(
    state: &AppState,
    commission: CommissionId,
    user: &User,
) -> Result<Commission, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    if found.owner_id == user.id {
        return Ok(found);
    }
    Err(
        if state
            .commissions
            .is_participant(commission, user.id)
            .await?
        {
            Problem::forbidden()
        } else {
            Problem::commission_not_found()
        },
    )
}

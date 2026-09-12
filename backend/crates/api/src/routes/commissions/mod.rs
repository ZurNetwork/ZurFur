//! The commissions route group: the commission JSON API, split per area
//! (create, list, changelog, notes, channel, delete, archive, maturity,
//! elements, slots, seats, invitations, status, deadline, files, markup,
//! positioning). Mounted under the first-party-`Origin` (CSRF) layer.

use application::commission::CommissionError;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{MethodRouter, delete, get, post, put},
};
use domain::elements::{
    commission::{Commission, CommissionId},
    user::UserId,
};

use crate::{AppState, problem::Problem};

/// Maps a commission use-case error onto the wire problem: `404` for a missing
/// commission (or one that hides a non-participant, the closed-door policy),
/// `403` for insufficient standing, `401` when the session's user no longer
/// exists, `422` for a malformed request, `500` for the store.
impl From<CommissionError> for Problem {
    fn from(err: CommissionError) -> Self {
        match err {
            CommissionError::Infrastructure(err) => Problem::from(err),
            CommissionError::UserNotFound => Problem::not_authenticated(),
            CommissionError::CommissionNotFound => Problem::commission_not_found(),
            CommissionError::CommissionAlreadyAtState => {
                Problem::invalid_request("This commission is already in this state.")
            }
            CommissionError::InsufficientPermissions => Problem::forbidden(),
            // Closed-door: a non-participant answers exactly like an absent commission.
            CommissionError::NotAMember => Problem::commission_not_found(),
            CommissionError::InvalidStateRequested => Problem::invalid_request(err.to_string()),
            CommissionError::InvalidFileName(e) => {
                Problem::invalid_request(format!("Invalid filename: {e}."))
            }
            // The exact {max}-byte message lives at the upload call site; this is the fallback.
            CommissionError::FileTooLarge => {
                Problem::invalid_request("The file exceeds the upload limit.")
            }
            CommissionError::FileEmpty => Problem::invalid_request("The uploaded file is empty."),
            CommissionError::FileNotFound => Problem::file_not_found(),
            CommissionError::InvalidMarkup(e) => {
                Problem::invalid_request(format!("Invalid markup: {e}."))
            }
            // Bytes lost server-side, never the caller's doing.
            CommissionError::FileBlobMissing => {
                Problem::internal_error("The file's contents could not be retrieved.")
            }
            // A Seat is an Element, so an absent seat answers as element_not_found.
            CommissionError::SeatNotFound => Problem::element_not_found(),
            CommissionError::SeatFilled => Problem::seat_filled(),
            // Fabricated/foreign tab → 404; undeclared surface under a real tab → 422.
            CommissionError::TabNotFound => Problem::tab_not_found(),
            CommissionError::UnknownSurface => Problem::unknown_surface(),
            CommissionError::ElementNotFound => Problem::element_not_found(),
            CommissionError::NoDeadline => Problem::no_deadline(),
            CommissionError::CommissionLate => Problem::commission_late(),
            CommissionError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            CommissionError::IncorrectContent => {
                Problem::invalid_request("The submitted content is empty.")
            }
            CommissionError::AccountNotFound => Problem::account_not_found(),
        }
    }
}

/// Domain time → the contract's wire timestamp type.
pub(super) fn wire_timestamp(at: domain::datetime::DateTimeUtc) -> crate::wire_time::WireTimestamp {
    crate::wire_time::WireTimestamp::from(at)
}

/// Wire timestamp → domain time, or `None` if outside the protobuf
/// `Timestamp` range (years 0001–9999).
pub(super) fn from_wire_timestamp(
    at: crate::wire_time::WireTimestamp,
) -> Option<domain::datetime::DateTimeUtc> {
    at.as_datetime()
}

mod archive;
mod changelog;
mod channel;
mod create;
mod deadline;
mod delete;
mod elements;
mod files;
mod invitations;
mod list;
mod markup;
mod maturity;
mod notes;
mod positioning;
mod seats;
mod slots;
mod status;

/// Slack, in bytes, added above [`Config::max_upload_bytes`](crate::Config::max_upload_bytes)
/// for the upload route's request body-size limit, to cover the
/// `multipart/form-data` envelope overhead.
const UPLOAD_BODY_SLACK_BYTES: usize = 1024 * 1024;

/// The commissions route group; mounted under the CSRF
/// [`require_first_party_origin`](super::require_first_party_origin) layer.
/// `max_upload_bytes` sizes the upload route's body-size limit.
pub(crate) fn commissions_router(max_upload_bytes: usize) -> Router<AppState> {
    let upload_body_limit = max_upload_bytes.saturating_add(UPLOAD_BODY_SLACK_BYTES);
    Router::new()
        .route(
            "/commissions",
            get(list::list_commissions).post(create::create_commission),
        )
        .route(
            "/commissions/{id}",
            axum::routing::delete(delete::delete_commission),
        )
        .route(
            "/commissions/{id}/changelog",
            get(changelog::read_changelog),
        )
        .route("/commissions/{id}/notes", post(notes::write_note))
        .route("/commissions/{id}/channel", channel_methods())
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
        .route("/commissions/{id}/elements", post(elements::add_element))
        .route(
            "/commissions/{id}/elements/{element}",
            delete(elements::remove_element),
        )
        .route("/commissions/{id}/slots", post(slots::declare_slots))
        .route("/commissions/{id}/seats", post(seats::declare_seat))
        .route(
            "/commissions/{id}/invitations",
            post(invitations::invite_to_seat).delete(invitations::revoke_seat_invitation),
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
            "/commissions/{id}/files",
            post(files::upload_file).layer(DefaultBodyLimit::max(upload_body_limit)),
        )
        .route(
            "/commissions/{id}/files/{file_id}",
            get(files::download_file),
        )
        .route(
            "/commissions/{id}/files/{file_id}/markup",
            post(markup::add_markup),
        )
}

/// The linked-channel pointer's methods, pulled out so `#[allow(deprecated)]`
/// covers only [`link_channel`](channel::link_channel). (DD 6848513)
#[allow(deprecated)]
fn channel_methods() -> MethodRouter<AppState> {
    put(channel::link_channel).delete(channel::clear_channel)
}

/// Admits only the commission's owner, returning the resolved [`Commission`].
/// `404` for a non-participant, `403` for a non-owner participant.
///
/// ⚠️ Driver-side authorization (DD 55836674 D7 places this in the
/// application layer); survives only for `channel`/`elements`.
async fn require_owner(
    state: &AppState,
    commission: &CommissionId,
    user: &UserId,
) -> Result<Commission, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    if found.owner_id == *user {
        return Ok(found);
    }
    Err(
        if state.commissions.is_participant(commission, user).await? {
            Problem::forbidden()
        } else {
            Problem::commission_not_found()
        },
    )
}

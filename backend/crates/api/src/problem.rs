//! The RFC 9457 problem-document type served as `application/problem+json` for
//! every JSON API error; success responses stay bare resources with the status
//! line as outcome. Construct via the named registry below, never field-by-field.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};

/// The generated RFC 9457 problem type (DD 40992770). Construct only via the
/// named registry methods below — never field by field — so `type`/`code`/
/// `title`/`status` stay fixed per kind; `detail` is the one per-call part and
/// must be non-empty.
pub use crate::generated::Problem;

impl Problem {
    /// Shared constructor every registry entry funnels through.
    fn new(
        kind: &'static str,
        code: &'static str,
        title: &'static str,
        status: u16,
        detail: impl Into<String>,
    ) -> Self {
        let detail = detail.into();
        // detail must be non-empty — an empty one is a registry bug.
        debug_assert!(
            !detail.is_empty(),
            "a Problem's detail must be non-empty (detail-required ruling)"
        );
        Self {
            r#type: kind.to_owned(),
            code: code.to_owned(),
            title: title.to_owned(),
            detail,
            status: i32::from(status),
        }
    }

    /// `401` — no (or unreadable) session on an endpoint that requires one.
    pub fn not_authenticated() -> Self {
        Self::new(
            "urn:zurfur:error:not-authenticated",
            "not_authenticated",
            "Not authenticated",
            401,
            "You must be signed in to do that.",
        )
    }

    /// `403` — an authenticated caller lacking authority for the action (shared
    /// role floor). Never used to avoid revealing a resource exists — use the 404 for that.
    pub fn forbidden() -> Self {
        Self::new(
            "urn:zurfur:error:forbidden",
            "forbidden",
            "Forbidden",
            403,
            "You don't have permission to perform this action.",
        )
    }

    /// `403` — a state-changing request arrived with a non-first-party `Origin`
    /// (CSRF defense-in-depth on top of `SameSite=Lax`). Non-browser callers (no
    /// `Origin` header) are never rejected here.
    pub fn cross_origin() -> Self {
        Self::new(
            "urn:zurfur:error:cross-origin",
            "cross_origin",
            "Cross-origin request blocked",
            403,
            "This state-changing request came from an untrusted origin.",
        )
    }

    /// `404` — the addressed account doesn't exist (or is soft-deleted).
    pub fn account_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:account-not-found",
            "account_not_found",
            "Account not found",
            404,
            "No such account.",
        )
    }

    /// `404` — closed-door answer for a hidden or absent commission: identical
    /// either way, so it can never be used as an existence oracle. Fixed detail
    /// text by construction.
    pub fn commission_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:commission-not-found",
            "commission_not_found",
            "Commission not found",
            404,
            "No such commission.",
        )
    }

    /// `404` — no such board (an ordinary not-found; the owning account's
    /// membership has already been checked by this point).
    pub fn workflow_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:workflow-not-found",
            "workflow_not_found",
            "Workflow not found",
            404,
            "No such workflow.",
        )
    }

    /// `404` — no such column, raised before the account membership check can run.
    pub fn column_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:column-not-found",
            "column_not_found",
            "Column not found",
            404,
            "No such column.",
        )
    }

    /// `404` — no such element in this commission. Answers identically whether
    /// the id is absent or belongs to another commission, so it can't probe
    /// other commissions.
    pub fn element_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:element-not-found",
            "element_not_found",
            "Element not found",
            404,
            "No such element in this commission.",
        )
    }

    /// `404` — no such tab in this commission; same cross-commission collapse as
    /// [`element_not_found`](Problem::element_not_found).
    pub fn tab_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:tab-not-found",
            "tab_not_found",
            "Tab not found",
            404,
            "No such tab in this commission.",
        )
    }

    /// `404` — no such file entry in this commission; scoped to the commission
    /// so it can't confirm a file exists elsewhere.
    pub fn file_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:file-not-found",
            "file_not_found",
            "File not found",
            404,
            "No such file entry on this commission.",
        )
    }

    /// `413` — an uploaded file exceeds the configured size cap
    /// ([`Config::max_upload_bytes`](crate::Config::max_upload_bytes)).
    pub fn payload_too_large(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:payload-too-large",
            "payload_too_large",
            "Payload too large",
            413,
            detail,
        )
    }

    /// `404` — the addressed user holds no membership in the account.
    pub fn member_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:member-not-found",
            "member_not_found",
            "Member not found",
            404,
            "That user is not a member of this account.",
        )
    }

    /// `404` — the caller has no pending invitation to act on for this account
    /// (the account itself may still exist).
    pub fn no_pending_invitation() -> Self {
        Self::new(
            "urn:zurfur:error:no-pending-invitation",
            "no_pending_invitation",
            "No pending invitation",
            404,
            "You have no pending invitation for this account.",
        )
    }

    /// `409` — inviting a user who is already a member. `detail` names the collision.
    pub fn already_member(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:already-member",
            "already_member",
            "Already a member",
            409,
            detail,
        )
    }

    /// `409` — the DID is already interned as a different actor kind (one DID =
    /// one actor). (DD 34013187)
    pub fn did_belongs_to_another_actor() -> Self {
        Self::new(
            "urn:zurfur:error:did-belongs-to-another-actor",
            "did_belongs_to_another_actor",
            "DID belongs to another actor",
            409,
            "That DID is already interned as a different actor kind.",
        )
    }

    /// `409` — the seat already holds an occupant, so no invitation can target
    /// it. Fixed text by construction: naming the occupant would leak the other party.
    pub fn seat_filled() -> Self {
        Self::new(
            "urn:zurfur:error:seat-filled",
            "seat_filled",
            "Seat already filled",
            409,
            "That seat is already occupied, so no one can be invited to it.",
        )
    }

    /// `409` — the commission bears facts, so hard delete is no longer
    /// possible; points the caller at Archive instead. (DD 3014657)
    pub fn commission_has_facts() -> Self {
        Self::new(
            "urn:zurfur:error:commission-has-facts",
            "commission_has_facts",
            "Commission has facts",
            409,
            "This commission bears facts and can no longer be deleted. Archive it instead.",
        )
    }

    /// `409` — the handle is already claimed, including by a tombstoned
    /// account (the handle index is global). (DD 23003138)
    pub fn handle_taken() -> Self {
        Self::new(
            "urn:zurfur:error:handle-taken",
            "handle_taken",
            "Handle already taken",
            409,
            "That handle is already in use. Please choose another.",
        )
    }

    /// `422` — the `(tab, surface)` pair isn't declared by the composition
    /// skeleton. A 422, not a 404: the skeleton is code-declared and global, so
    /// this is a fact about the program, not about any commission.
    pub fn unknown_surface() -> Self {
        Self::new(
            "urn:zurfur:error:unknown-surface",
            "unknown_surface",
            "Unknown surface",
            422,
            "No such surface under that tab: a commission's surfaces are a fixed, declared set per tab.",
        )
    }

    /// `409` — a deadline-axis action on a commission with no deadline; set one first.
    pub fn no_deadline() -> Self {
        Self::new(
            "urn:zurfur:error:no-deadline",
            "no_deadline",
            "No deadline",
            409,
            "This commission has no deadline, so it can't carry a deadline status.",
        )
    }

    /// `409` — the commission is Late, and Late is system-set: resolve it
    /// through the deadline itself (extend or clear), never overwrite or clear by hand.
    pub fn commission_late() -> Self {
        Self::new(
            "urn:zurfur:error:commission-late",
            "commission_late",
            "Commission is Late",
            409,
            "The commission is Late; extend or clear the deadline to resolve it.",
        )
    }

    /// `409` — the sole Owner tried to leave; transfer ownership or delete the
    /// account first.
    pub fn owner_cannot_leave() -> Self {
        Self::new(
            "urn:zurfur:error:owner-cannot-leave",
            "owner_cannot_leave",
            "Owner cannot leave",
            409,
            "You can't leave an account you own. Transfer ownership or delete the account first.",
        )
    }

    /// `422` — the request is understood but its data won't do; `detail` says why.
    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:invalid-request",
            "invalid_request",
            "Invalid request",
            422,
            detail,
        )
    }

    /// `422`, code `unknown_role` — a role discriminant that isn't one we grant.
    pub fn unknown_role(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:invalid-request",
            "unknown_role",
            "Invalid request",
            422,
            detail,
        )
    }

    /// `422`, code `unknown_maturity_rating` — a maturity token outside the
    /// fixed four-tier vocabulary (Safe/Suggestive/Nudity/Adult). (DD 29982722)
    pub fn unknown_maturity_rating(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:invalid-request",
            "unknown_maturity_rating",
            "Invalid request",
            422,
            detail,
        )
    }

    /// `422`, code `unsupported_handle` — handle change isn't supported yet for
    /// this namespace (v1: `*.zurfur.app` only). (DD 27852802)
    pub fn unsupported_handle(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:invalid-request",
            "unsupported_handle",
            "Invalid request",
            422,
            detail,
        )
    }

    /// `429` — the caller hit the anti-abuse rate limit; retry after the window.
    pub fn rate_limited(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:rate-limited",
            "rate_limited",
            "Too many requests",
            429,
            detail,
        )
    }

    /// `500` — a dependency (store, recognizer) failed; the request was fine.
    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:internal",
            "internal_error",
            "Internal error",
            500,
            detail,
        )
    }

    /// `503` — a dependency is unavailable; the caller may retry.
    pub fn service_unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:service-unavailable",
            "service_unavailable",
            "Service unavailable",
            503,
            detail,
        )
    }
}

impl From<anyhow::Error> for Problem {
    /// Any port failure becomes a `500`; lets handlers use `?`.
    fn from(_: anyhow::Error) -> Self {
        Problem::internal_error("The request couldn't be completed. Please try again.")
    }
}

impl IntoResponse for Problem {
    /// Renders as JSON with the content type overridden to `application/problem+json`.
    fn into_response(self) -> Response {
        let status = u16::try_from(self.status)
            .ok()
            .and_then(|code| StatusCode::from_u16(code).ok())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status, Json(self)).into_response();
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // AC1/AC3 — a Problem serializes to exactly the five RFC 9457 members we
    // promise, with our URN `type` and terse `code`.
    #[test]
    fn serializes_to_the_rfc9457_members() {
        let value = serde_json::to_value(Problem::already_member(
            "did:plc:abc already holds a role on account 0192.",
        ))
        .expect("serializes");

        assert_eq!(value["type"], "urn:zurfur:error:already-member");
        assert_eq!(value["code"], "already_member");
        assert_eq!(value["title"], "Already a member");
        assert_eq!(
            value["detail"],
            "did:plc:abc already holds a role on account 0192."
        );
        assert_eq!(value["status"], 409);
        // No stray `error` key from the old shape.
        assert!(value.get("error").is_none(), "the old shape is gone");
    }

    // AC4 — the response sets the problem+json content type (not application/json)
    // and the HTTP status matching the body's `status`.
    #[test]
    fn into_response_sets_problem_json_content_type_and_status() {
        let response = Problem::forbidden().into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .expect("content-type is set"),
            "application/problem+json"
        );
    }

    // ZMVP-66 AC3 — the fact-bearing delete refusal is a 409 whose detail points
    // the caller at Archive (the path that remains once facts exist).
    #[test]
    fn commission_has_facts_is_a_409_pointing_at_archive() {
        let problem = Problem::commission_has_facts();
        assert_eq!(problem.r#type, "urn:zurfur:error:commission-has-facts");
        assert_eq!(problem.code, "commission_has_facts");
        assert_eq!(problem.status, 409);
        assert!(
            problem.detail.to_lowercase().contains("archive"),
            "the detail points at Archive, got {:?}",
            problem.detail
        );
    }

    // The 422 specifics share the invalid-request type but carry their own code.
    #[test]
    fn invalid_request_specifics_share_the_type_but_vary_the_code() {
        assert_eq!(Problem::invalid_request("x").code, "invalid_request");
        assert_eq!(Problem::unknown_role("bad").code, "unknown_role");
        assert_eq!(
            Problem::unknown_role("bad").r#type,
            "urn:zurfur:error:invalid-request"
        );
        assert_eq!(Problem::unknown_role("bad").status, 422);
    }
}

//! The JSON API's one error shape: an [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457.html)
//! problem document, served as `application/problem+json` (ZMVP-35; DESIGN "API
//! Response Shape & Error Model").
//!
//! Success responses stay bare resources and the HTTP status line carries the
//! outcome; this type standardizes the *error* half. Every [`Problem`] carries a
//! stable [`type`](Problem::kind) URN (`urn:zurfur:error:<slug>` — an identifier,
//! not a docs URL), our own terse [`code`](Problem::code) for machine branching, a
//! stable [`title`], a specific [`detail`], and the [`status`]. Handlers return
//! `Result<_, Problem>` and lean on `?`; [`Problem`]'s [`IntoResponse`] renders the
//! body and sets the `application/problem+json` content type.
//!
//! [`title`]: Problem::title
//! [`detail`]: Problem::detail
//! [`status`]: Problem::status

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// An RFC 9457 problem document — the JSON API's single error representation.
///
/// Constructed through the named constructors (one per entry in the error
/// registry, e.g. [`Problem::forbidden`], [`Problem::already_member`]), never field
/// by field, so the `type`/`code`/`title`/`status` of a given kind stay consistent
/// across every call-site. `detail` is the only per-occurrence part.
#[derive(Debug, Serialize)]
pub struct Problem {
    /// Stable problem-type identifier: a non-dereferenceable `urn:zurfur:error:*`
    /// URN. An identity to look up in our error docs, **not** a live URL — nothing
    /// to host, nothing to 404. Serialized as `type` (the RFC 9457 member name).
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Our own terse, machine-branchable code (e.g. `already_member`) — never a
    /// leaked PostgreSQL or internal error code. An RFC 9457 extension member.
    pub code: &'static str,
    /// Short, stable, human-readable summary of the problem *type* (same for every
    /// occurrence of a given `code`).
    pub title: &'static str,
    /// Specific, human-readable explanation of *this* occurrence.
    pub detail: String,
    /// The HTTP status code, duplicated in the body per RFC 9457 so it survives
    /// proxies and logs that drop the status line.
    pub status: u16,
}

impl Problem {
    /// The shared constructor — every registry entry funnels through here so the
    /// shape stays uniform.
    fn new(
        kind: &'static str,
        code: &'static str,
        title: &'static str,
        status: u16,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            code,
            title,
            detail: detail.into(),
            status,
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

    /// `403` — a recognized caller who lacks the authority for the action (the
    /// shared role floor; DESIGN/Roles). Action- and resource-neutral: it serves
    /// the account seams and the commission owner-only seams alike. **Never** the
    /// answer for a caller who shouldn't learn the resource exists — that is the
    /// uniform 404 (e.g. [`commission_not_found`](Problem::commission_not_found)).
    pub fn forbidden() -> Self {
        Self::new(
            "urn:zurfur:error:forbidden",
            "forbidden",
            "Forbidden",
            403,
            "You don't have permission to perform this action.",
        )
    }

    /// `403` — a state-changing request arrived with an `Origin` that isn't our
    /// first-party origin: defense-in-depth CSRF, layered on the session cookie's
    /// `SameSite=Lax` (ZMVP-23; DESIGN "Auth Surfaces, the Plugin Trust Boundary &
    /// CSRF"). A non-browser client (no `Origin`) is never rejected here.
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

    /// `404` — the addressed commission "doesn't exist" **as far as this caller
    /// may know**: the shared existence-hiding answer of the closed-door policy
    /// (DESIGN/Commission; ZMVP-75/87). Returned identically whether the
    /// commission is truly absent **or** exists but is hidden from the caller
    /// (a non-participant), so the response can never be used as an existence
    /// oracle — which is also why a non-participant is **never** answered
    /// [`forbidden`](Problem::forbidden) (a 403 would confirm there is something
    /// to be forbidden from). `detail` is a fixed string by construction: any
    /// per-occurrence wording could leak which case produced it.
    pub fn commission_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:commission-not-found",
            "commission_not_found",
            "Commission not found",
            404,
            "No such commission.",
        )
    }

    /// `404` — the addressed tree node doesn't exist in this commission
    /// (ZMVP-71). Reached only past the commission's own gate (the caller is
    /// already its owner), so unlike
    /// [`commission_not_found`](Problem::commission_not_found) it hides
    /// nothing *about this commission* — but it deliberately answers a node id
    /// that exists in **someone else's** tree identically to one that exists
    /// nowhere (the store refuses both as one case), so node ids can't be used
    /// to probe other commissions' trees.
    pub fn node_not_found() -> Self {
        Self::new(
            "urn:zurfur:error:node-not-found",
            "node_not_found",
            "Node not found",
            404,
            "No such node in this commission.",
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

    /// `404` — the signed-in User has no pending invitation to act on (accept or
    /// decline) for this account. Distinct from `account_not_found`: the account
    /// exists, there's just no live offer for them.
    pub fn no_pending_invitation() -> Self {
        Self::new(
            "urn:zurfur:error:no-pending-invitation",
            "no_pending_invitation",
            "No pending invitation",
            404,
            "You have no pending invitation for this account.",
        )
    }

    /// `409` — inviting a user who is already a member (a state conflict, not an
    /// authority failure). `detail` names the specific collision.
    pub fn already_member(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:already-member",
            "already_member",
            "Already a member",
            409,
            detail,
        )
    }

    /// `409` — the commission bears facts, so hard-deleting it is no longer
    /// possible (ZMVP-66; Deletion DD `3014657`: "Delete = hard delete, possible
    /// only while fact-free"). A state conflict, not an authority failure — the
    /// caller is the owner, the commission just crossed the point of no return.
    /// The detail points at **Archive** (ZMVP-68), the path that remains once
    /// facts exist. Fixed text by construction: naming *which* facts would leak
    /// the other party's activity to no benefit.
    pub fn commission_has_facts() -> Self {
        Self::new(
            "urn:zurfur:error:commission-has-facts",
            "commission_has_facts",
            "Commission has facts",
            409,
            "This commission bears facts and can no longer be deleted. Archive it instead.",
        )
    }

    /// `409` — the chosen account handle is already taken. The handle index is
    /// global — a soft-deleted (tombstoned) account still reserves its handle (DD
    /// 23003138 "Account Deletion, Tombstoning & Handle Reuse"; DD "The Account
    /// Handle" 24870914) — so founding with a claimed handle is a state conflict, not
    /// an authority failure, whether the holder is live or tombstoned.
    pub fn handle_taken() -> Self {
        Self::new(
            "urn:zurfur:error:handle-taken",
            "handle_taken",
            "Handle already taken",
            409,
            "That handle is already in use. Please choose another.",
        )
    }

    /// `409` — the named parent node exists (in the caller's own commission)
    /// but is a component, and components never have children (ZMVP-72:
    /// "always the child of a surface, never with children"). A state
    /// conflict, not an authority failure — and honest by construction: it is
    /// only ever reachable past the owner gate *and* past the absent/foreign
    /// parent check ([`node_not_found`](Problem::node_not_found)), so it can
    /// never reveal anything about another commission's tree.
    pub fn parent_not_a_surface() -> Self {
        Self::new(
            "urn:zurfur:error:parent-not-a-surface",
            "parent_not_a_surface",
            "Parent is not a surface",
            409,
            "Components are leaves: nothing can be added under a component.",
        )
    }

    /// `409` — the addressed node is the commission's root surface, which is
    /// the fixed skeleton and cannot be removed (ZMVP-73 AC3; the Title is not
    /// a tree node, so no node id even addresses it). A state conflict, not an
    /// authority failure — and honest by construction: like
    /// [`parent_not_a_surface`](Problem::parent_not_a_surface) it is only ever
    /// reachable past the owner gate *and* past the absent/foreign target
    /// check ([`node_not_found`](Problem::node_not_found)), so it can never
    /// confirm that a foreign node is a root.
    pub fn cannot_remove_root() -> Self {
        Self::new(
            "urn:zurfur:error:cannot-remove-root",
            "cannot_remove_root",
            "The root surface cannot be removed",
            409,
            "Every commission keeps its root surface; remove its children instead.",
        )
    }

    /// `409` — the Owner tried to leave while still Owner. The sole-Owner root has
    /// nowhere to re-home its members, so leaving is refused as a state conflict (not
    /// an authority failure): transfer ownership (ZMVP-33) or delete the account first.
    pub fn owner_cannot_leave() -> Self {
        Self::new(
            "urn:zurfur:error:owner-cannot-leave",
            "owner_cannot_leave",
            "Owner cannot leave",
            409,
            "You can't leave an account you own. Transfer ownership or delete the account first.",
        )
    }

    /// `422` — the request is understood but its data won't do. `detail` says why.
    /// Specific cases get their own `code` via [`name_required`](Problem::name_required)
    /// / [`unknown_role`](Problem::unknown_role) under the same `type`.
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

    /// `422`, code `unknown_maturity_rating` — a maturity token outside the four-tier
    /// vocabulary the Maturity Vocabulary DD (`29982722`) fixes (Safe / Suggestive /
    /// Nudity / Adult). The server-side half of ZMVP-31's "values from the enum only":
    /// the superseded Safe/Questionable/Explicit tokens, case variants, and the derived
    /// *label* values all land here. Shares the invalid-request `type` but carries its
    /// own `code`, like [`unknown_role`](Problem::unknown_role).
    pub fn unknown_maturity_rating(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:invalid-request",
            "unknown_maturity_rating",
            "Invalid request",
            422,
            detail,
        )
    }

    /// `422`, code `unsupported_handle` — a well-formed handle whose *namespace* isn't
    /// supported for this operation yet: v1 ships the handle-*change* flow for the
    /// Zurfur-issued `*.zurfur.app` namespace only, since changing to a brought (BYO)
    /// domain needs bidirectional verify-before-commit that isn't built (DD "Account
    /// Handle Change Flow" `27852802` §6; deferred to a follow-up). Shares the
    /// invalid-request `type` but carries its own `code`, like [`unknown_role`](Problem::unknown_role).
    pub fn unsupported_handle(detail: impl Into<String>) -> Self {
        Self::new(
            "urn:zurfur:error:invalid-request",
            "unsupported_handle",
            "Invalid request",
            422,
            detail,
        )
    }

    /// `429` — the caller has hit the light anti-abuse rate limit for an action (the
    /// handle-change throttle; DD `27852802` §3). The request was valid; the caller may
    /// retry once the window passes.
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

    /// `503` — a dependency is unavailable (e.g. the DID minter), so the request
    /// can't be served right now; the caller may retry.
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
    /// Any failure bubbling up from a port (the store, the recognizer) is an
    /// internal error: the request was well-formed, our side couldn't complete it.
    /// This lets handlers lean on `?` instead of mapping every port call by hand.
    fn from(_: anyhow::Error) -> Self {
        Problem::internal_error("The request couldn't be completed. Please try again.")
    }
}

impl IntoResponse for Problem {
    /// Renders the document as JSON and **overrides** the content type to
    /// `application/problem+json` (axum's [`Json`] sets `application/json`, so the
    /// header is replaced after the body is built), with the matching HTTP status.
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
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
        assert_eq!(problem.kind, "urn:zurfur:error:commission-has-facts");
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
            Problem::unknown_role("bad").kind,
            "urn:zurfur:error:invalid-request"
        );
        assert_eq!(Problem::unknown_role("bad").status, 422);
    }
}

//! `POST /commissions/{id}/files` and `GET /commissions/{id}/files/{file_id}` — a
//! Participant uploads a work-in-progress file entry into the review loop, and a
//! Participant retrieves one (ZMVP-88; DESIGN/Commission — "File entries and
//! Markup").
//!
//! A file entry is **not** a Product — no fact-lock, no atproto: it is private,
//! Index-side, **Total-tier** content. So both endpoints are participant-gated
//! behind [`require_participant`](super::require_participant) (the uniform 404
//! closed door — a non-participant learns nothing, not even existence), and the
//! upload does **not** trip fact-lock (the `commission_file` row is bookkeeping,
//! not a [`Fact`](domain::elements::commission::Fact), so
//! [`commission_has_facts`](domain::ports::CommissionWrites::commission_has_facts)
//! stays `false`).
//!
//! **Three homes, one entry.** The bytes go to the
//! [`FileStore`](domain::ports::FileStore) (pool-backed, **before** the unit of
//! work — bytes cannot ride a transaction; orphan-on-rollback accepted). Then, in
//! one unit of work, the `commission_file` link and the `file_added` changelog
//! entry commit atomically (Changelog DD D4).
//!
//! **No coupled status write (ZMVP-89; Engineer rulings 2026-07-01 and
//! 2026-07-05).** Uploading never mutates any status — not the direction axis,
//! not the deadline axis, not the Lifecycle; anything status-shaped smuggled
//! alongside the upload (an extra multipart field, a query parameter) is
//! ignored, never applied. The future submission form's "optional Status choice"
//! is frontend orchestration over **two explicit calls** — this upload plus
//! `PUT /commissions/{id}/status/direction` (ZMVP-85) — each landing its own
//! changelog entry; no coupled backend write exists, by design. The contract is
//! pinned by `tests/commission_submission_contract.rs`.
//!
//! **Download hardening.** Stored content is user-controlled and may be an SVG or
//! HTML file; served naively it could execute in the app origin (stored XSS). The
//! response therefore always carries `Content-Disposition: attachment` (never
//! inline) and `X-Content-Type-Options: nosniff` (the browser won't sniff a
//! script type out of the declared `Content-Type`), and the filename is validated
//! control-free and emitted RFC 5987-encoded so it can never inject a header.

use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{
        ChangelogEntryKind, CommissionFile, CommissionId, FileKey, FileMetadata, FileName,
        NewChangelogEntry,
    },
    ports::transaction,
};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// `POST /commissions/{id}/files` — a Participant uploads a file entry (ZMVP-88).
///
/// Any-Participant-gated behind [`require_participant`](super::require_participant)
/// (uniform 404 for everyone else — the closed door). The body is
/// `multipart/form-data` with a `file` part carrying the bytes, filename, and
/// content type. The filename is validated ([`FileName`]) and the content type
/// normalized ([`FileMetadata::new`]); an oversize file is `413`, a malformed body
/// or a bad filename is `422`. The bytes are stored through the
/// [`FileStore`](domain::ports::FileStore) first, then the `commission_file` link
/// and the `file_added` changelog entry commit in **one unit of work**. Returns
/// `201 Created` with `{ "id": "<uuid>" }` — the key the download path uses.
pub(super) async fn upload_file(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    multipart: Multipart,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let upload = read_file_part(multipart).await?;
    let max = state.config.max_upload_bytes;
    if upload.bytes.len() as u64 > max {
        return Err(Problem::payload_too_large(format!(
            "The file exceeds the {max}-byte upload limit."
        )));
    }
    if upload.bytes.is_empty() {
        return Err(Problem::invalid_request("The uploaded file is empty."));
    }

    let filename = FileName::try_new(upload.filename.unwrap_or_default())
        .map_err(|e| Problem::invalid_request(format!("Invalid filename: {e}.")))?;
    let metadata = FileMetadata::new(
        filename,
        upload.content_type.unwrap_or_default(),
        upload.bytes.len() as i64,
    );

    let key = FileKey::generate();
    // The blob write precedes the unit of work (bytes can't ride a transaction);
    // a later rollback orphans it, which is accepted for v1 (nothing points at it).
    state.files.put(key, &metadata, &upload.bytes).await?;

    let now = Utc::now();
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::FileAdded,
        user.id,
        json!({
            "file_id": *key,
            "filename": metadata.filename.as_str(),
            "content_type": metadata.content_type,
            "byte_size": metadata.byte_size,
        }),
        now,
    );
    let file = CommissionFile {
        id: key,
        commission_id: commission,
        uploaded_by: user.id,
        created_at: now,
    };
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.commissions().add_file(&file).await?;
            uow.changelog().append(&entry).await
        })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(json!({ "id": *key }))).into_response())
}

/// `GET /commissions/{id}/files/{file_id}` — a Participant retrieves a file entry
/// (ZMVP-88).
///
/// Any-Participant-gated behind [`require_participant`](super::require_participant):
/// a non-participant gets the uniform commission-not-found 404, never the bytes. A
/// key that names no entry **within this commission** (including one belonging to a
/// different commission) is [`file_not_found`](Problem::file_not_found) — a 404 that
/// is no cross-commission oracle. The response streams the stored bytes with
/// `Content-Type` from the stored metadata and — always — `Content-Disposition:
/// attachment` plus `X-Content-Type-Options: nosniff`, so a stored SVG/HTML can
/// never execute in the app origin.
pub(super) async fn download_file(
    State(state): State<AppState>,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let key = FileKey::new(file_id);
    // Scoped to the commission: a key from another commission answers None here.
    state
        .commissions
        .find_file(commission, key)
        .await?
        .ok_or_else(Problem::file_not_found)?;

    let stored = state
        .files
        .get(key)
        .await?
        // The link exists but the blob is gone — an internal inconsistency, not an
        // authorization outcome (never leaks as a 404 for a file the caller may see).
        .ok_or_else(|| Problem::internal_error("The file's contents are unavailable."))?;

    let content_type = HeaderValue::from_str(&stored.metadata.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static(FileMetadata::DEFAULT_CONTENT_TYPE));
    let disposition =
        HeaderValue::from_str(&content_disposition(stored.metadata.filename.as_str()))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment"));

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        Body::from(stored.bytes),
    )
        .into_response())
}

/// The `file` part of a multipart upload: its declared filename and content type
/// (both optional at the wire level) and its bytes.
struct UploadPart {
    filename: Option<String>,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

/// Pull the `file` part out of a `multipart/form-data` body. A body that isn't
/// multipart, is malformed, or carries no `file` part is a `422` (a client error).
/// The bytes are read fully into memory — bounded by the route's body-size limit
/// (the framework backstop) and re-checked against the exact cap by the caller.
async fn read_file_part(mut multipart: Multipart) -> Result<UploadPart, Problem> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| Problem::invalid_request("Malformed multipart body."))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().map(str::to_owned);
        let content_type = field.content_type().map(str::to_owned);
        let bytes = field
            .bytes()
            .await
            .map_err(|_| Problem::invalid_request("Could not read the uploaded file."))?
            .to_vec();
        return Ok(UploadPart {
            filename,
            content_type,
            bytes,
        });
    }
    Err(Problem::invalid_request(
        "Expected a 'file' part in the multipart body.",
    ))
}

/// Build a `Content-Disposition: attachment` header value that carries the filename
/// safely across the ASCII-only header boundary (RFC 6266 + RFC 5987).
///
/// Two forms are emitted: an ASCII `filename="…"` fallback (any non-ASCII or quote
/// replaced with `_`) for legacy clients, and `filename*=UTF-8''…` with the true
/// name percent-encoded per RFC 5987 for modern ones. The value is always
/// `attachment`, so even a client that ignores both still downloads rather than
/// renders.
fn content_disposition(filename: &str) -> String {
    let fallback: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        rfc5987_encode(filename)
    )
}

/// Percent-encode `s` per RFC 5987's `attr-char` set (the safe characters an
/// `ext-value` may carry unescaped); everything else becomes `%XX`. Pure ASCII out,
/// so the result is always a valid header value.
fn rfc5987_encode(s: &str) -> String {
    const ATTR_CHAR_EXTRA: &[u8] = b"!#$&+-.^_`|~";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || ATTR_CHAR_EXTRA.contains(&b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The Content-Disposition is always `attachment`, carries an ASCII fallback and
    // an RFC 5987 encoding, and neutralizes header-hostile characters.
    #[test]
    fn content_disposition_is_attachment_and_encodes_safely() {
        let value = content_disposition("réf sheet.png");
        assert!(
            value.starts_with("attachment; "),
            "always attachment: {value}"
        );
        // Non-ASCII and space are percent-encoded in the RFC 5987 form.
        assert!(
            value.contains("filename*=UTF-8''r%C3%A9f%20sheet.png"),
            "{value}"
        );
        // The ASCII fallback replaces the non-ASCII byte, keeps the extension.
        assert!(value.contains("filename=\"r_f sheet.png\""), "{value}");
    }

    #[test]
    fn rfc5987_leaves_attr_chars_and_escapes_the_rest() {
        assert_eq!(rfc5987_encode("a-b_c.png"), "a-b_c.png");
        assert_eq!(rfc5987_encode("a b"), "a%20b");
        assert_eq!(rfc5987_encode("\""), "%22");
    }
}

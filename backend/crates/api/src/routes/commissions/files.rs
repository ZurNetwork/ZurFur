//! `POST /commissions/{id}/files` and `GET /commissions/{id}/files/{file_id}` —
//! a Participant uploads a work-in-progress file, and retrieves one. Routing
//! only; the use case lives in `application::commission::files`, this file
//! adapts axum's multipart/body types to the streaming `FileStore` seam.

use application::commission::{
    CommissionError,
    files::{download, upload},
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State, multipart::Field},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::{CommissionId, FileKey, FileMetadata};
use futures_util::TryStreamExt;
use serde::Serialize;
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// `POST /commissions/{id}/files`'s `201` body: the uploaded entry's key — see
/// [`upload_file`].
#[derive(Serialize)]
struct UploadFileResponse {
    id: Uuid,
}

/// Uploads a file entry as multipart form data. Any-Participant-gated;
/// `413` if oversize, `422` for a malformed body or bad filename. Returns
/// `201 Created` with `{ "id": "<uuid>" }`.
pub(super) async fn upload_file(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    mut multipart: Multipart,
) -> Result<Response, Problem> {
    let part: UploadPart<'_> = loop {
        let Some(field) = multipart
            .next_field()
            .await
            .map_err(|_| Problem::invalid_request("Malformed multipart body."))?
        else {
            return Err(Problem::invalid_request(
                "Expected a 'file' part in the multipart body.",
            ));
        };
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().map(str::to_owned);
        let content_type = field.content_type().map(str::to_owned);
        break UploadPart {
            filename,
            content_type,
            field,
        };
    };
    let max = state.config.max_upload_bytes;

    let command = upload::Command {
        actor_id,
        commission_id,
        filename: part.filename,
        content_type: part.content_type,
    };
    // Lazy reader — nothing is pulled from the wire until the use case authorizes.
    let reader = StreamReader::new(part.field.map_err(std::io::Error::other));

    let outcome = state
        .app()
        .commissions()
        .files()
        .upload(command, reader, max, Utc::now())
        .await;
    let result = match outcome {
        Ok(result) => result,
        // Exact byte count lives here (the use case only knows the cap was exceeded).
        Err(CommissionError::FileTooLarge) => {
            return Err(Problem::payload_too_large(format!(
                "The file exceeds the {max}-byte upload limit."
            )));
        }
        Err(err) => return Err(Problem::from(err)),
    };

    let body = UploadFileResponse { id: *result.id };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

/// Retrieves a file entry's bytes. Any-Participant-gated; `404
/// file_not_found` for a key not in this commission. Always serves
/// `Content-Disposition: attachment` and `X-Content-Type-Options: nosniff` so
/// a stored SVG/HTML can never execute in the app origin.
pub(super) async fn download_file(
    State(state): State<AppState>,
    Path((commission_id, file_key)): Path<(CommissionId, FileKey)>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let query = download::Query {
        actor_id,
        commission_id,
        file_id: file_key,
    };

    let download::Output { result: download } =
        state.app().commissions().files().download(query).await?;

    let content_type = HeaderValue::from_str(&download.metadata.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static(FileMetadata::DEFAULT_CONTENT_TYPE));
    let disposition =
        HeaderValue::from_str(&content_disposition(download.metadata.filename.as_str()))
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
        Body::from_stream(ReaderStream::new(download.content)),
    )
        .into_response())
}

/// The `file` part of a multipart upload, as scanned for in [`upload_file`]:
/// its declared filename and content type (both optional at the wire level)
/// and the unconsumed field stream.
struct UploadPart<'a> {
    filename: Option<String>,
    content_type: Option<String>,
    field: Field<'a>,
}

/// Builds a `Content-Disposition: attachment` header value carrying the
/// filename safely across the ASCII-only header boundary (RFC 6266 + 5987).
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

/// Percent-encodes `s` per RFC 5987's `attr-char` set; everything else
/// becomes `%XX`.
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

    #[test]
    fn upload_file_response_serializes_to_a_bare_id_object() {
        let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000006").unwrap();
        let body = UploadFileResponse { id };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"id\":\"{id}\"}}")
        );
    }

    #[test]
    fn content_disposition_is_attachment_and_encodes_safely() {
        let value = content_disposition("réf sheet.png");
        assert!(
            value.starts_with("attachment; "),
            "always attachment: {value}"
        );
        assert!(
            value.contains("filename*=UTF-8''r%C3%A9f%20sheet.png"),
            "{value}"
        );
        assert!(value.contains("filename=\"r_f sheet.png\""), "{value}");
    }

    #[test]
    fn rfc5987_leaves_attr_chars_and_escapes_the_rest() {
        assert_eq!(rfc5987_encode("a-b_c.png"), "a-b_c.png");
        assert_eq!(rfc5987_encode("a b"), "a%20b");
        assert_eq!(rfc5987_encode("\""), "%22");
    }
}

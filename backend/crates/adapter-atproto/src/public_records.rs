//! The real [`PublicRecords`] adapter — authenticated `com.atproto.repo.*` writes
//! (and `uploadBlob`) into an actor's repo on its PDS (ZMVP-105).
//!
//! This is the **write half** of Zurfur's public data boundary and the first
//! authenticated write to a PDS repo in the codebase (DID minting writes to a PLC
//! directory; profile reads are unauthenticated). Every AT-Protocol type stays
//! **quarantined here**: the [`PublicRecords`] port speaks domain records + [`Did`]
//! only, and the `jacquard` wire types + the PDS credential never cross the crate
//! boundary (DESIGN/"Domains and Applications"; the plugin-trust containment,
//! DD `24543244`).
//!
//! # Auth (ZMVP-105 = Bearer)
//!
//! The adapter is **constructed with** its acting identity's Bearer access token
//! (the `access_jwt` the ZMVP-103 seam vends as `ActingCredential::PdsSession`) and
//! its PDS endpoint; it sends every request as `Authorization: Bearer <jwt>` to
//! that endpoint. The port never takes a credential per call, so swapping to a
//! DPoP-bound OAuth session later (ZMVP-107) is an internal change here — the
//! error mapping keys on the **XRPC** outcome (status + atproto error name), not
//! the auth transport, so it survives that swap.
//!
//! Reads and writes both target the constructed endpoint directly (no DID→PDS
//! resolution), which keeps the boundary hermetic against a throwaway PDS whose
//! `did:plc` lives only in a local stub directory. That scopes reads to the
//! acting identity's own repo — exactly ZMVP-105's subject; cross-repo reads
//! (which need resolution) are a later concern.

use std::str::FromStr;

use async_trait::async_trait;
use jacquard::common::AuthorizationToken;
use jacquard::common::deps::fluent_uri::Uri;
use jacquard::common::types::blob::{Blob as JacBlob, MimeType};
use jacquard::common::types::cid::CidLink;
use jacquard::common::types::ident::AtIdentifier;
use jacquard::common::types::nsid::Nsid as JacNsid;
use jacquard::common::types::recordkey::RecordKey as JacRecordKey;
use jacquard::common::types::string::Did as JacDid;
use jacquard::common::types::value::{from_data, to_data};
use jacquard::common::xrpc::{XrpcError, XrpcExt};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use domain::elements::did::Did;
use domain::elements::public_record::{
    AspectRatio, AtUri, BlobRef, Credit, Embed, FeedPost, PublicRecord, RecordKey, RecordRef,
    ReplyRef, ReplySubject, SelfLabels, StrongRef,
};
use domain::ports::{PublicRecords, PublicRecordsError};

/// The real public-boundary record store, bound to one acting identity's PDS
/// session (Bearer). See the module docs for the auth model.
pub struct AtprotoPublicRecords {
    http: reqwest::Client,
    /// The PDS base URL every request targets.
    endpoint: Uri<String>,
    /// The Bearer access token, held inside the adapter and never surfaced.
    access_jwt: SmolStr,
}

impl AtprotoPublicRecords {
    /// Build the adapter for the identity authenticated by `access_jwt` on the
    /// PDS at `endpoint`.
    ///
    /// `access_jwt` is the `ActingCredential::PdsSession` access token from the
    /// ZMVP-103 fixture seam (or, in production later, an OAuth-issued token). It
    /// is a secret; it lives only inside this value and appears in no port
    /// signature. Errors only if `endpoint` is not a valid URI.
    pub fn new(endpoint: &str, access_jwt: impl Into<String>) -> anyhow::Result<Self> {
        let endpoint = Uri::parse(endpoint.to_string())
            .map_err(|e| anyhow::anyhow!("invalid PDS endpoint {endpoint:?}: {e:?}"))?;
        Ok(Self {
            http: reqwest::Client::new(),
            endpoint,
            access_jwt: SmolStr::from(access_jwt.into()),
        })
    }

    fn bearer(&self) -> AuthorizationToken<SmolStr> {
        AuthorizationToken::Bearer(self.access_jwt.clone())
    }

    fn bearer_header(&self) -> String {
        format!("Bearer {}", self.access_jwt)
    }
}

#[async_trait]
impl PublicRecords for AtprotoPublicRecords {
    async fn create_record(
        &self,
        repo: &Did,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError> {
        use jacquard::api::com_atproto::repo::create_record::CreateRecord;

        let collection = record.collection();
        let wire = wire_record_for(record)?;
        let data = to_data(&wire).map_err(|e| {
            PublicRecordsError::Unexpected(anyhow::anyhow!("serialize record: {e}"))
        })?;

        let request = CreateRecord::new()
            .repo(AtIdentifier::Did(jac_did(repo)?))
            .collection(jac_nsid(collection.as_str())?)
            .record(data)
            .build();

        let output = self
            .send(request)
            .await?
            .into_output()
            .map_err(map_output_err)?;
        record_ref_from(output.uri.as_str(), output.cid.as_str())
    }

    async fn put_record(
        &self,
        uri: &AtUri,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError> {
        use jacquard::api::com_atproto::repo::put_record::PutRecord;

        let wire = wire_record_for(record)?;
        let data = to_data(&wire).map_err(|e| {
            PublicRecordsError::Unexpected(anyhow::anyhow!("serialize record: {e}"))
        })?;

        let request = PutRecord::new()
            .repo(AtIdentifier::Did(jac_did(&uri.did)?))
            .collection(jac_nsid(uri.collection.as_str())?)
            .rkey(jac_rkey(&uri.rkey)?)
            .record(data)
            .build();

        let output = self
            .send(request)
            .await?
            .into_output()
            .map_err(map_output_err)?;
        record_ref_from(output.uri.as_str(), output.cid.as_str())
    }

    async fn delete_record(&self, uri: &AtUri) -> Result<(), PublicRecordsError> {
        use jacquard::api::com_atproto::repo::delete_record::DeleteRecord;

        let request = DeleteRecord::new()
            .repo(AtIdentifier::Did(jac_did(&uri.did)?))
            .collection(jac_nsid(uri.collection.as_str())?)
            .rkey(jac_rkey(&uri.rkey)?)
            .build();

        self.send(request)
            .await?
            .into_output()
            .map_err(map_output_err)?;
        Ok(())
    }

    async fn get_record(&self, uri: &AtUri) -> Result<PublicRecord, PublicRecordsError> {
        use jacquard::api::com_atproto::repo::get_record::GetRecord;

        let request = GetRecord::new()
            .repo(AtIdentifier::Did(jac_did(&uri.did)?))
            .collection(jac_nsid(uri.collection.as_str())?)
            .rkey(jac_rkey(&uri.rkey)?)
            .build();

        let output = self
            .send(request)
            .await?
            .into_output()
            .map_err(map_output_err)?;
        let wire: WireFeedPost = from_data(&output.value).map_err(|e| {
            PublicRecordsError::Unexpected(anyhow::anyhow!("deserialize record: {e}"))
        })?;
        Ok(PublicRecord::FeedPost(feed_post_from_wire(wire)?))
    }

    async fn upload_blob(
        &self,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<BlobRef, PublicRecordsError> {
        // `com.atproto.repo.uploadBlob` is a raw binary POST. We issue it directly
        // rather than through jacquard's typed request: jacquard 0.12's generated
        // `UploadBlob::encode_body` is `buffer.copy_from_slice(..)` into an empty
        // buffer (it should `extend_from_slice`), which panics on the stateless
        // client path. A plain POST — same Bearer auth, same endpoint — sidesteps
        // that bug while keeping the wire format identical.
        let size = bytes.len() as u64;
        let response = self
            .http
            .post(format!(
                "{}/xrpc/com.atproto.repo.uploadBlob",
                self.endpoint.as_str().trim_end_matches('/')
            ))
            .header(http::header::AUTHORIZATION, self.bearer_header())
            .header(http::header::CONTENT_TYPE, mime_type)
            .body(bytes)
            .send()
            .await
            .map_err(|e| {
                // A connect/timeout failure is an unreachable PDS; anything else the
                // request layer refuses is surfaced, never swallowed.
                if e.is_connect() || e.is_timeout() {
                    PublicRecordsError::Unreachable(anyhow::anyhow!("uploadBlob transport: {e}"))
                } else {
                    PublicRecordsError::Unexpected(anyhow::anyhow!("uploadBlob request: {e}"))
                }
            })?;

        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|e| PublicRecordsError::Unexpected(anyhow::anyhow!("uploadBlob body: {e}")))?;
        if !status.is_success() {
            return Err(rejected_from_body(status.as_u16(), &body));
        }

        // { "blob": { "$type":"blob", "ref": {"$link": "<cid>"}, "mimeType": "...", "size": N } }
        let parsed: serde_json::Value = serde_json::from_slice(&body).map_err(|e| {
            PublicRecordsError::Unexpected(anyhow::anyhow!("uploadBlob response JSON: {e}"))
        })?;
        let blob = &parsed["blob"];
        let cid = blob["ref"]["$link"].as_str().ok_or_else(|| {
            PublicRecordsError::Unexpected(anyhow::anyhow!(
                "uploadBlob response missing blob.ref.$link: {parsed}"
            ))
        })?;
        let mime = blob["mimeType"].as_str().unwrap_or(mime_type).to_string();
        Ok(BlobRef {
            cid: parse_cid(cid)?,
            mime_type: mime,
            size,
        })
    }
}

impl AtprotoPublicRecords {
    /// Send an authenticated XRPC request to the bound endpoint. A thin wrapper so
    /// every write shares one auth + transport-error path.
    async fn send<R>(
        &self,
        request: R,
    ) -> Result<
        jacquard::common::xrpc::Response<<R as jacquard::common::xrpc::XrpcRequest>::Response>,
        PublicRecordsError,
    >
    where
        R: jacquard::common::xrpc::XrpcRequest + Serialize,
        <R as jacquard::common::xrpc::XrpcRequest>::Response: Send + Sync,
    {
        self.http
            .xrpc(self.endpoint.borrow())
            .auth(self.bearer())
            .send(&request)
            .await
            .map_err(map_send_err)
    }
}

// --- error mapping (XRPC outcome → domain error; never the auth transport) ---

/// Map a send-layer [`ClientError`](jacquard::common::error::ClientError): a
/// transport failure is [`PublicRecordsError::Unreachable`]; an HTTP error status
/// the transport layer surfaced (5xx / 403 / …) is a [`PublicRecordsError::Rejected`].
fn map_send_err(err: jacquard::common::error::ClientError) -> PublicRecordsError {
    use jacquard::common::error::ClientErrorKind;
    match err.kind() {
        ClientErrorKind::Transport | ClientErrorKind::IdentityResolution => {
            PublicRecordsError::Unreachable(anyhow::anyhow!("{err}"))
        }
        ClientErrorKind::Http { status } => {
            let status = status.as_u16();
            if status == 404 {
                PublicRecordsError::NotFound
            } else {
                PublicRecordsError::Rejected {
                    status,
                    error: "HttpError".to_string(),
                    message: Some(err.to_string()),
                }
            }
        }
        _ => PublicRecordsError::Unexpected(anyhow::anyhow!("{err}")),
    }
}

/// Map a response-layer [`XrpcError`] (a PDS that answered but refused) to the
/// domain error, keying on the atproto error **name** and HTTP status.
fn map_output_err<E: std::error::Error>(err: XrpcError<E>) -> PublicRecordsError {
    match err {
        XrpcError::Xrpc(typed) => {
            let rendered = typed.to_string();
            if is_not_found(&rendered) {
                PublicRecordsError::NotFound
            } else {
                PublicRecordsError::Rejected {
                    status: 400,
                    error: rendered.clone(),
                    message: Some(rendered),
                }
            }
        }
        XrpcError::Generic(g) => {
            if is_not_found(g.error.as_str()) {
                PublicRecordsError::NotFound
            } else {
                PublicRecordsError::Rejected {
                    status: g.http_status.as_u16(),
                    error: g.error.to_string(),
                    message: g.message.map(|m| m.to_string()),
                }
            }
        }
        XrpcError::Auth(auth) => PublicRecordsError::Rejected {
            status: 401,
            error: "AuthError".to_string(),
            message: Some(auth.to_string()),
        },
        XrpcError::Decode(d) => {
            PublicRecordsError::Unexpected(anyhow::anyhow!("decode response: {d}"))
        }
        // `XrpcError` is `#[non_exhaustive]`: surface any future variant rather
        // than swallowing it (never a silent success on failure — AC4).
        other => PublicRecordsError::Unexpected(anyhow::anyhow!("XRPC error: {other}")),
    }
}

/// Build a [`PublicRecordsError::Rejected`] from a non-success atproto error body
/// (`{"error": "...", "message": "..."}`), keeping the HTTP status and error name.
fn rejected_from_body(status: u16, body: &[u8]) -> PublicRecordsError {
    let parsed: Option<serde_json::Value> = serde_json::from_slice(body).ok();
    let error = parsed
        .as_ref()
        .and_then(|v| v["error"].as_str())
        .unwrap_or("Unknown")
        .to_string();
    if is_not_found(&error) {
        return PublicRecordsError::NotFound;
    }
    let message = parsed
        .as_ref()
        .and_then(|v| v["message"].as_str())
        .map(|s| s.to_string());
    PublicRecordsError::Rejected {
        status,
        error,
        message,
    }
}

/// Whether an atproto error name/render denotes a missing record.
fn is_not_found(s: &str) -> bool {
    s.contains("RecordNotFound") || s.contains("NotFound") || s.contains("Could not locate record")
}

// --- domain ↔ jacquard string-type construction ---

fn jac_did(did: &Did) -> Result<JacDid<SmolStr>, PublicRecordsError> {
    JacDid::new_owned(did.as_str()).map_err(|e| {
        PublicRecordsError::InvalidRecord(format!("invalid DID {:?}: {e:?}", did.as_str()))
    })
}

fn jac_nsid(nsid: &str) -> Result<JacNsid<SmolStr>, PublicRecordsError> {
    JacNsid::new(SmolStr::from(nsid))
        .map_err(|e| PublicRecordsError::InvalidRecord(format!("invalid NSID {nsid:?}: {e:?}")))
}

fn jac_rkey(
    rkey: &RecordKey,
) -> Result<JacRecordKey<jacquard::common::types::string::Rkey<SmolStr>>, PublicRecordsError> {
    JacRecordKey::any_owned(rkey.as_str()).map_err(|e| {
        PublicRecordsError::InvalidRecord(format!("invalid rkey {:?}: {e:?}", rkey.as_str()))
    })
}

fn parse_cid(s: &str) -> Result<cid::Cid, PublicRecordsError> {
    cid::Cid::from_str(s)
        .map_err(|e| PublicRecordsError::Unexpected(anyhow::anyhow!("invalid CID {s:?}: {e}")))
}

fn record_ref_from(uri: &str, cid: &str) -> Result<RecordRef, PublicRecordsError> {
    Ok(RecordRef {
        uri: AtUri::parse(uri).map_err(|e| {
            PublicRecordsError::Unexpected(anyhow::anyhow!(
                "PDS returned malformed AT-URI {uri:?}: {e}"
            ))
        })?,
        cid: parse_cid(cid)?,
    })
}

// --- wire (`app.zurfur.feed.post`) serde types, quarantined in this crate ---

fn feed_post_type() -> String {
    "app.zurfur.feed.post".to_string()
}
fn self_labels_type() -> String {
    "com.atproto.label.defs#selfLabels".to_string()
}

/// The `app.zurfur.feed.post` record on the wire.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireFeedPost {
    #[serde(rename = "$type", default = "feed_post_type")]
    ty: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    embed: Option<WireEmbed>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    reply: Option<WireReplyRef>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    credits: Vec<WireCredit>,
    labels: WireSelfLabels,
    created_at: String,
}

/// `app.zurfur.embed.media` (a single-ref embed — no `$type` needed on the ref).
///
/// The `blob` field is jacquard's own [`JacBlob`], **not** a hand-rolled struct:
/// an atproto blob's `ref` is a first-class CID-link node in the `Data`/CBOR
/// model (not a plain `{$link}` map), so only jacquard's `Blob` serialises to a
/// real, PDS-recognised blob reference and reads one back.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireEmbed {
    blob: JacBlob<SmolStr>,
    alt: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    aspect_ratio: Option<WireAspectRatio>,
}

#[derive(Serialize, Deserialize)]
struct WireAspectRatio {
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize)]
struct WireReplyRef {
    root: WireReplySubject,
    parent: WireReplySubject,
}

/// The reply-subject union, tagged by `$type` exactly as the lexicon's
/// `strongRef | #didSubject` union serialises.
#[derive(Serialize, Deserialize)]
#[serde(tag = "$type")]
enum WireReplySubject {
    #[serde(rename = "com.atproto.repo.strongRef")]
    Record { uri: String, cid: String },
    #[serde(rename = "app.zurfur.feed.post#didSubject")]
    Profile { did: String },
}

#[derive(Serialize, Deserialize)]
struct WireCredit {
    role: String,
    did: String,
}

#[derive(Serialize, Deserialize)]
struct WireSelfLabels {
    #[serde(rename = "$type", default = "self_labels_type")]
    ty: String,
    values: Vec<WireSelfLabel>,
}

#[derive(Serialize, Deserialize)]
struct WireSelfLabel {
    val: String,
}

// --- domain FeedPost ↔ wire mapping ---

/// Serialise the domain record to its wire form. Fails only if the record body
/// is structurally impossible to render (never, today — one infallible variant).
fn wire_record_for(record: &PublicRecord) -> Result<WireFeedPost, PublicRecordsError> {
    match record {
        PublicRecord::FeedPost(post) => Ok(wire_feed_post(post)),
    }
}

fn wire_feed_post(post: &FeedPost) -> WireFeedPost {
    WireFeedPost {
        ty: feed_post_type(),
        text: post.text.clone(),
        embed: post.embed.as_ref().map(wire_embed),
        reply: post.reply.as_ref().map(wire_reply),
        credits: post.credits.iter().map(wire_credit).collect(),
        labels: WireSelfLabels {
            ty: self_labels_type(),
            values: post
                .labels
                .0
                .iter()
                .map(|val| WireSelfLabel { val: val.clone() })
                .collect(),
        },
        created_at: post
            .created_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }
}

fn wire_embed(embed: &Embed) -> WireEmbed {
    WireEmbed {
        blob: JacBlob {
            // `ipld` keeps the CID as a real link node, so `to_data` emits a
            // genuine atproto blob reference the PDS associates and serves.
            r#ref: CidLink::ipld(embed.blob.cid),
            mime_type: MimeType::new_owned(&embed.blob.mime_type),
            size: embed.blob.size as usize,
        },
        alt: embed.alt.clone(),
        aspect_ratio: embed.aspect_ratio.as_ref().map(|a| WireAspectRatio {
            width: a.width,
            height: a.height,
        }),
    }
}

fn wire_reply(reply: &ReplyRef) -> WireReplyRef {
    WireReplyRef {
        root: wire_subject(&reply.root),
        parent: wire_subject(&reply.parent),
    }
}

fn wire_subject(subject: &ReplySubject) -> WireReplySubject {
    match subject {
        ReplySubject::Record(strong) => WireReplySubject::Record {
            uri: strong.uri.to_string(),
            cid: strong.cid.to_string(),
        },
        ReplySubject::Profile(did) => WireReplySubject::Profile {
            did: did.as_str().to_string(),
        },
    }
}

fn wire_credit(credit: &Credit) -> WireCredit {
    WireCredit {
        role: credit.role.clone(),
        did: credit.did.as_str().to_string(),
    }
}

/// Parse the wire record back into a domain [`FeedPost`]. Fails
/// ([`PublicRecordsError::Unexpected`]) if the repo returned a structurally
/// malformed record (a bad CID, AT-URI, or timestamp) — never a silent default.
fn feed_post_from_wire(wire: WireFeedPost) -> Result<FeedPost, PublicRecordsError> {
    Ok(FeedPost {
        text: wire.text,
        embed: wire.embed.map(embed_from_wire).transpose()?,
        reply: wire.reply.map(reply_from_wire).transpose()?,
        credits: wire
            .credits
            .into_iter()
            .map(|c| Credit {
                role: c.role,
                did: Did::new(c.did),
            })
            .collect(),
        labels: SelfLabels(wire.labels.values.into_iter().map(|v| v.val).collect()),
        created_at: chrono::DateTime::parse_from_rfc3339(&wire.created_at)
            .map_err(|e| {
                PublicRecordsError::Unexpected(anyhow::anyhow!(
                    "record has malformed createdAt {:?}: {e}",
                    wire.created_at
                ))
            })?
            .with_timezone(&chrono::Utc),
    })
}

fn embed_from_wire(wire: WireEmbed) -> Result<Embed, PublicRecordsError> {
    Ok(Embed {
        blob: BlobRef {
            cid: parse_cid(wire.blob.cid().as_str())?,
            mime_type: wire.blob.mime_type.as_str().to_string(),
            size: wire.blob.size as u64,
        },
        alt: wire.alt,
        aspect_ratio: wire.aspect_ratio.map(|a| AspectRatio {
            width: a.width,
            height: a.height,
        }),
    })
}

fn reply_from_wire(wire: WireReplyRef) -> Result<ReplyRef, PublicRecordsError> {
    Ok(ReplyRef {
        root: subject_from_wire(wire.root)?,
        parent: subject_from_wire(wire.parent)?,
    })
}

fn subject_from_wire(wire: WireReplySubject) -> Result<ReplySubject, PublicRecordsError> {
    match wire {
        WireReplySubject::Record { uri, cid } => Ok(ReplySubject::Record(StrongRef {
            uri: AtUri::parse(&uri).map_err(|e| {
                PublicRecordsError::Unexpected(anyhow::anyhow!(
                    "malformed reply AT-URI {uri:?}: {e}"
                ))
            })?,
            cid: parse_cid(&cid)?,
        })),
        WireReplySubject::Profile { did } => Ok(ReplySubject::Profile(Did::new(did))),
    }
}

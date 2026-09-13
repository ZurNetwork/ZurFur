//! The account `did:plc` minter — real ([`RealDidMinter`]) and stub
//! ([`StubDidMinter`]).
//!
//! The real minter generates per-account secp256k1 rotation keys, signs an
//! identity-only genesis operation, custodies the keys through a [`KeyStore`],
//! and submits to a [`PlcDirectory`]. The stub mints a synthetic DID only.

use async_trait::async_trait;
use atrium_crypto::keypair::{Did as _, Export as _, Secp256k1Keypair};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    elements::{
        account_keys::{AccountKeys, SecretKey},
        did::Did,
        handle::Handle,
        plc_operation::PlcOperationRecord,
    },
    ports::{DidMinter, KeyStore, PlcOperationLog},
};
use rand::Rng;
use std::sync::Arc;

use crate::plc::{PlcOperation, TombstoneOperation};
use crate::plc_directory::PlcDirectory;

/// The real [`DidMinter`]: mints a genuine, custody-backed `did:plc` over an
/// injected [`KeyStore`], [`PlcOperationLog`] and `PlcDirectory`.
pub struct RealDidMinter {
    key_store: Arc<dyn KeyStore>,
    op_log: Arc<dyn PlcOperationLog>,
    directory: Box<dyn PlcDirectory>,
}

impl RealDidMinter {
    /// Build the real minter over custody, the operation log (so the next op
    /// knows its `prev`), and the submission directory.
    pub fn new(
        key_store: Arc<dyn KeyStore>,
        op_log: Arc<dyn PlcOperationLog>,
        directory: Box<dyn PlcDirectory>,
    ) -> Self {
        Self {
            key_store,
            op_log,
            directory,
        }
    }

    /// Sign `op` with the operational key, derive the DID, custody the three
    /// keypairs, log the genesis op, then submit to the directory. Shared by
    /// [`mint`](DidMinter::mint) and
    /// [`mint_handleless`](DidMinter::mint_handleless) — the built `op` is the
    /// only difference between them. Keys are stored before submission, so a
    /// submission retry never orphans them.
    async fn sign_custody_and_submit(
        &self,
        op: PlcOperation,
        cold: Secp256k1Keypair,
        operational: Secp256k1Keypair,
        signing: Secp256k1Keypair,
    ) -> anyhow::Result<Did> {
        // Signing with the operational key keeps cold-recovery off the signing
        // path; atrium-crypto already emits atproto's canonical low-S form.
        let signing_bytes = op.signing_bytes()?;
        let sig_bytes = operational.sign(&signing_bytes)?;
        let sig = URL_SAFE_NO_PAD.encode(&sig_bytes);

        let signed = op.into_signed(sig);
        let did = Did::from(signed.did()?);
        // The genesis op's CID — the `prev` a future operation chains onto.
        let genesis_cid = signed.cid()?;
        let op_json = signed.to_json()?;

        let keys = AccountKeys {
            cold_recovery: SecretKey::new(cold.export()),
            operational: SecretKey::new(operational.export()),
            signing: SecretKey::new(signing.export()),
        };

        // Private writes first: custody, then the genesis op.
        self.key_store.put(&did, &keys).await?;
        self.op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: genesis_cid,
                op_type: "plc_operation".to_string(),
                prev: None,
                operation_json: op_json.to_string(),
            })
            .await?;
        // Public dual-write — a separate retryable step, never a shared transaction.
        self.directory
            .submit(AsRef::<str>::as_ref(&did), &op_json)
            .await?;

        Ok(did)
    }
}

/// Generate the three per-mint secp256k1 keypairs (cold-recovery, operational,
/// signing) in a block so the non-`Send` `ThreadRng` is dropped before any
/// `.await` the caller runs next (the keypairs themselves are `Send`).
fn generate_keypairs() -> (Secp256k1Keypair, Secp256k1Keypair, Secp256k1Keypair) {
    let mut rng = rand::thread_rng();
    (
        Secp256k1Keypair::create(&mut rng),
        Secp256k1Keypair::create(&mut rng),
        Secp256k1Keypair::create(&mut rng),
    )
}

#[async_trait]
impl DidMinter for RealDidMinter {
    /// Mint an identity-only `did:plc` bound to `handle`: generate the three
    /// keypairs, sign the genesis operation with the operational key, derive the
    /// DID, custody the keys and log the op, then submit to the directory.
    /// Keys are stored before submission, so a submission retry never orphans them.
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did> {
        let (cold, operational, signing) = generate_keypairs();

        // rotationKeys in DESCENDING authority: cold-recovery, then operational.
        let rotation_keys = vec![cold.did(), operational.did()];
        let op = PlcOperation::identity_only(rotation_keys, signing.did(), handle.as_str());

        self.sign_custody_and_submit(op, cold, operational, signing)
            .await
    }

    /// Mint an identity-only `did:plc` with no alias, chaining the same
    /// key-generation, custody and submission path as [`mint`](Self::mint).
    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        let (cold, operational, signing) = generate_keypairs();

        let rotation_keys = vec![cold.did(), operational.did()];
        let op = PlcOperation::identity_only_handleless(rotation_keys, signing.did());

        self.sign_custody_and_submit(op, cold, operational, signing)
            .await
    }

    /// Tombstone `did`: sign a `plc_tombstone` with the operational rotation
    /// key, chaining onto the DID's latest logged operation, then submit before
    /// recording — so a failed submit never advances the local chain. Fails
    /// retryably if the DID has no custody keys or no op to chain onto.
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()> {
        let keys = self.key_store.get(did).await?.ok_or_else(|| {
            anyhow::anyhow!("no custody keys to tombstone {}", AsRef::<str>::as_ref(did))
        })?;
        let prev = self.op_log.latest_cid(did).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "no prior PLC operation to chain a tombstone onto for {}",
                AsRef::<str>::as_ref(did)
            )
        })?;

        let operational = Secp256k1Keypair::import(keys.operational.expose())?;
        let op = TombstoneOperation::new(prev.clone());
        let sig_bytes = operational.sign(&op.signing_bytes()?)?;
        let signed = op.into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes));
        let cid = signed.cid()?;
        let op_json = signed.to_json()?;

        // Submission first, so a failed submit never advances the local chain.
        self.directory
            .submit(AsRef::<str>::as_ref(did), &op_json)
            .await?;
        self.op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid,
                op_type: "plc_tombstone".to_string(),
                prev: Some(prev),
                operation_json: op_json.to_string(),
            })
            .await?;

        Ok(())
    }

    /// Re-point `did`'s `alsoKnownAs` to `handle`: sign a `plc_operation` with
    /// the operational rotation key, chaining onto the DID's latest logged op,
    /// which also supplies the carried-forward public document fields; a
    /// non-identity-only prior op is rejected, never rewritten. Submits before
    /// recording; an identical replay is idempotent and a competing update on the
    /// same `prev` errors rather than forking the chain.
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()> {
        // The latest op's `cid` is our `prev`; its JSON holds the public document
        // fields we preserve, never re-derived from the custodied private keys.
        let prior = self.op_log.latest_op(did).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "no prior PLC operation to chain an update onto for {}",
                AsRef::<str>::as_ref(did)
            )
        })?;
        // An update rebuilds an identity-only op, so a prior op of any other
        // shape must fail loud rather than silently drop its fields.
        if prior.op_type != "plc_operation" {
            anyhow::bail!(
                "cannot update {}: its latest op is `{}`, not a chainable plc_operation",
                AsRef::<str>::as_ref(did),
                prior.op_type
            );
        }
        let prior_json: serde_json::Value = serde_json::from_str(&prior.operation_json)?;
        let rotation_keys = string_array(&prior_json, "rotationKeys")?;
        let services_empty = prior_json["services"]
            .as_object()
            .is_none_or(serde_json::Map::is_empty);
        let verification_only_atproto = prior_json["verificationMethods"]
            .as_object()
            .is_some_and(|vm| vm.len() == 1 && vm.contains_key("atproto"));
        if !(services_empty && verification_only_atproto) {
            anyhow::bail!(
                "cannot update {}: its latest op is not identity-only (unexpected services or \
                 verification methods); carrying those forward is not implemented",
                AsRef::<str>::as_ref(did)
            );
        }
        let atproto_signing_did = prior_json["verificationMethods"]["atproto"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "prior op for {} has no verificationMethods.atproto",
                    AsRef::<str>::as_ref(did)
                )
            })?
            .to_string();

        // Only the operational key is decrypted; the others stay sealed.
        let keys = self.key_store.get(did).await?.ok_or_else(|| {
            anyhow::anyhow!("no custody keys to update {}", AsRef::<str>::as_ref(did))
        })?;
        let operational = Secp256k1Keypair::import(keys.operational.expose())?;

        let op = PlcOperation::update_handle(
            rotation_keys,
            atproto_signing_did,
            handle.as_str(),
            prior.cid.clone(),
        );
        let sig_bytes = operational.sign(&op.signing_bytes()?)?;
        let signed = op.into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes));
        let cid = signed.cid()?;
        let op_json = signed.to_json()?;

        // Submission first, so a failed submit never advances the local chain.
        self.directory
            .submit(AsRef::<str>::as_ref(did), &op_json)
            .await?;
        let append = self
            .op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: cid.clone(),
                op_type: "plc_operation".to_string(),
                prev: Some(prior.cid),
                operation_json: op_json.to_string(),
            })
            .await;
        if let Err(err) = append {
            // Benign only if the log's tip already IS our exact op (an identical
            // writer landed it); otherwise propagate so the caller retries onto
            // the new tip rather than forking.
            if self.op_log.latest_cid(did).await?.as_deref() == Some(cid.as_str()) {
                return Ok(());
            }
            return Err(err);
        }
        Ok(())
    }
}

/// Extract a JSON string array as `Vec<String>`, erroring if the field is
/// missing, not an array, or holds a non-string element.
fn string_array(value: &serde_json::Value, field: &str) -> anyhow::Result<Vec<String>> {
    value[field]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("prior op field `{field}` is missing or not an array"))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("prior op field `{field}` has a non-string element"))
        })
        .collect()
}

/// `did:plc` base32 alphabet (RFC 4648, lowercase, no padding).
const PLC_BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// A synthetic floor stub for [`DidMinter`]: a well-formed but entirely
/// synthetic `did:plc`, with no keypair, operation, or directory write. It
/// registers nowhere, for dev and tests that only need a DID-shaped value.
#[derive(Debug, Default, Clone)]
pub struct StubDidMinter;

impl StubDidMinter {
    /// Construct the stub; it is stateless.
    pub fn new() -> Self {
        Self
    }
}

/// A well-formed synthetic `did:plc:` suffix: 24 random lowercase base32
/// chars, shared by every [`StubDidMinter`] mint regardless of alias.
fn random_suffix() -> String {
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| PLC_BASE32[rng.gen_range(0..PLC_BASE32.len())] as char)
        .collect()
}

#[async_trait]
impl DidMinter for StubDidMinter {
    /// Return `did:plc:` + 24 random lowercase base32 chars. `handle` is ignored
    /// and the DID resolves nowhere; purely local, so it never fails.
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        Ok(Did::from(format!("did:plc:{}", random_suffix())))
    }

    /// Return `did:plc:` + 24 random lowercase base32 chars, same as
    /// [`mint`](Self::mint); the DID resolves nowhere either way.
    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        Ok(Did::from(format!("did:plc:{}", random_suffix())))
    }

    /// No-op: the stub registers no operation, so there is nothing to tombstone.
    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
        Ok(())
    }

    /// No-op: the stub custodies no keys and has no `alsoKnownAs` to re-point.
    async fn update_handle(&self, _did: &Did, _handle: &Handle) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

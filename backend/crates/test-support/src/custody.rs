//! Custody fixtures: three real 0.1 custody rows and the root they open under, a 0.1 store
//! on a test database, the test root and a test custodian's scoped handles over it, a lying
//! custodian for the self-check tests, and the log canary's printings of what a planted
//! root derives, computed apart from the custodian's code.

mod legacy_store;

pub use legacy_store::{
    CUSTODY_ENVELOPE, StoreSnapshot, before_the_envelope, legacy_genesis_record,
    seed_alice_and_bob, seed_legacy, store_snapshot,
};

use std::sync::Arc;

use adapter_atproto::AtprotoAuthStore;
use adapter_hkdf::HkdfKeyCustodian;
use adapter_pg::PgPool;
use async_trait::async_trait;
use composition::RootSecret;
use data_encoding::{BASE64, HEXLOWER};
use domain::elements::account_keys::{AccountKeys, SecretKey};
use domain::elements::custody::{
    CustodyError, CustodySubject, ForDidCustody, ForSigninSecrets, MacMessage, MacTag, MacUse,
    OpenedSecret, Purpose, SealPurpose, SealedEnvelope,
};
use domain::ports::{KeyCustodian, ScopedMac, ScopedSealer};
use fluent_uri::Uri;
use hmac::{Hmac, KeyInit, Mac};
use jacquard_common::types::did::Did;
use jacquard_oauth::{
    authstore::ClientAuthStore,
    scopes::Scopes,
    session::{AuthRequestData, ClientSessionData, DpopClientData, DpopReqData},
    types::{OAuthTokenType, TokenSet},
    utils::generate_key,
};
use sha2_0_11::Sha256;
use smol_str::SmolStr;
use strum::VariantArray as _;
use zeroize::Zeroizing;

/// The 0.1 root every legacy fixture below is sealed under.
pub const LEGACY_ROOT: [u8; 32] = [9u8; 32];

/// The DID [`LEGACY_ALICE_BLOB_HEX`] and [`LEGACY_ALICE_BLOB_STARTING_0X01_HEX`] are sealed for.
pub const LEGACY_ALICE_DID: &str = "did:plc:alice";

/// The DID [`LEGACY_BOB_BLOB_HEX`] is sealed for.
pub const LEGACY_BOB_DID: &str = "did:plc:bob";

/// A 0.1 blob: root `[9; 32]`, `did:plc:alice`, keys 0xAA/0xBB/0xCC.
pub const LEGACY_ALICE_BLOB_HEX: &str = "8d64dbe72a9240dd603161404dde03a52f2ab5f09d01c2a73c407435224dd5a5e90b4a81e254fbfd2e90bb7ce0b5e9c538a7264805af7e4dfae728cdda82b3a41d5bae953729453b5c051a9492e899424528f87209964566cdceb06afdb612fef8044e0bf9799fd4286f9992dba79223546f5658cec25e6d3139731b2c932f53bef06a6ebb0d403f";

/// A 0.1 blob: root `[9; 32]`, `did:plc:bob`, keys 0xAA/0xBB/0xCC.
pub const LEGACY_BOB_BLOB_HEX: &str = "ac5871adc37fc19049ccf5d3073dfe6fededfd9b34c141e9757645fded3eed3fa9db90a84dd5f243bef7871349ac7e0a322850734beeed04f3b7081706ca0fb0894be0114d46ccecca099f807e1e8f04c2eb56b1693f68b92060e84d01334ba3fc385c25915807e36c79765293b9bb05d0085015e369625bc9b00c18ffffef71521c6b9c64ec95d3";

/// A 0.1 blob: root `[9; 32]`, `did:plc:alice`, keys 0xAA/0xBB/0xCC; its first byte is 0x01.
pub const LEGACY_ALICE_BLOB_STARTING_0X01_HEX: &str = "017c34555bb478e165076720364596499e1ab8c224c4a65ec7907f78b40ec4dc48043e5ed67aa6fad1053c4eba0306264a360d7492f45c95252c44fb91401ee504803dc33c351d65a2948edaec168ac913e8aacd09cfdc1ec3e86fcf28ecd37771cd31fe3f52054a6daa4dc49049dc6fdcaf0e61391320a9d4fab3551254e1ae2068ecdc45ba2def";

/// The root every test runtime and the test custodian derive their keys from.
pub const TEST_ROOT: [u8; 32] = [0x5e; 32];

/// The byte the lying custodian masks a secret with.
const LYING_MASK: u8 = 0x5c;

/// The HKDF salt every custody derivation extracts under.
const CUSTODY_SALT: &[u8] = b"zurfur/custody/v1";

/// The HKDF info of a purpose key, before the purpose's token.
const KEY_INFO_PREFIX: &str = "zurfur/custody/v1/key/";

/// The HKDF info of a key id's fingerprint, before the purpose's token.
const KEY_ID_INFO_PREFIX: &str = "zurfur/custody/v1/key-id/";

/// The length of a custody key and of the PRK, in bytes.
const KEY_BYTES: usize = 32;

/// The length of a key id's fingerprint, in bytes.
const FINGERPRINT_BYTES: usize = 8;

/// The DID the canary's sign-in session belongs to: a log may print it at debug only.
pub const SIGNIN_CANARY_DID: &str = "did:plc:canarysigninsecrets7d";

/// The canary sign-in's secrets, each named: what [`signin_secrets_round_trip`] seals,
/// and no log may print at any level.
pub const SIGNIN_CANARY_SECRETS: [(&str, &str); 5] = [
    ("the session id", "canarysessionid4b8e2f"),
    ("the OAuth state", "canarystate6a3c9d1e"),
    ("the access token", "canaryaccesstoken2f7b5e"),
    ("the refresh token", "canaryrefreshtoken9c1d4a"),
    ("the PKCE verifier", "canarypkceverifier5e8f3b"),
];

/// The keys every legacy fixture holds: 32 bytes each of 0xAA, 0xBB and 0xCC, in role order.
pub fn legacy_keys() -> AccountKeys {
    AccountKeys {
        cold_recovery: SecretKey::new(vec![0xAA; 32]),
        operational: SecretKey::new(vec![0xBB; 32]),
        signing: SecretKey::new(vec![0xCC; 32]),
    }
}

/// Decodes a lowercase hex fixture into bytes; panics on malformed hex.
pub fn hex_bytes(hex: &str) -> Vec<u8> {
    data_encoding::HEXLOWER
        .decode(hex.as_bytes())
        .expect("a hex fixture holds only lowercase hex digit pairs")
}

/// [`TEST_ROOT`] as the custody root a runtime is connected with.
pub fn test_root() -> RootSecret {
    root_of(TEST_ROOT)
}

/// `bytes` as a custody root: for a test that plants its own.
pub fn root_of(bytes: [u8; 32]) -> RootSecret {
    RootSecret::from(Zeroizing::new(bytes))
}

/// A test custodian's handles, each purpose's key derived from one fixed test root.
pub struct TestCustody {
    /// Seals and opens the did:plc custody keys.
    pub did_custody: ScopedSealer<ForDidCustody>,
    /// Seals and opens sign-in's secrets.
    pub signin_secrets: ScopedSealer<ForSigninSecrets>,
    /// Tags and verifies the Den's records.
    pub den_mac: ScopedMac,
}

/// An `HkdfKeyCustodian` holding every purpose over [`TEST_ROOT`], whole: for a
/// test that builds a handle the scoped ones can't.
pub fn test_custodian() -> Arc<dyn KeyCustodian> {
    custodian_of(TEST_ROOT)
}

/// An `HkdfKeyCustodian` holding every purpose over `root`, whole.
pub fn custodian_of(root: [u8; KEY_BYTES]) -> Arc<dyn KeyCustodian> {
    let derived = HkdfKeyCustodian::derive(Zeroizing::new(root), Purpose::VARIANTS);
    Arc::new(derived)
}

/// The handles of [`test_custodian`].
pub fn test_custody() -> TestCustody {
    custody_of(TEST_ROOT)
}

/// The handles of [`custodian_of`] `root`: for a test that plants its own root.
pub fn custody_of(root: [u8; KEY_BYTES]) -> TestCustody {
    let custodian = custodian_of(root);
    TestCustody {
        did_custody: ScopedSealer::from(Arc::clone(&custodian)),
        signin_secrets: ScopedSealer::from(Arc::clone(&custodian)),
        den_mac: ScopedMac::from(custodian),
    }
}

/// A custodian whose `open` hands back other bytes than were sealed: the secret
/// with its first byte flipped. It seals by masking, and holds no Den MAC key.
pub struct LyingCustodian;

#[async_trait]
impl KeyCustodian for LyingCustodian {
    async fn seal(
        &self,
        _purpose: SealPurpose,
        _subject: &CustodySubject,
        plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError> {
        let masked: Vec<u8> = plaintext.iter().map(|byte| byte ^ LYING_MASK).collect();
        Ok(SealedEnvelope::from(masked))
    }

    async fn open(
        &self,
        _purpose: SealPurpose,
        _subject: &CustodySubject,
        envelope: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError> {
        let mut altered: Vec<u8> = envelope
            .as_ref()
            .iter()
            .map(|byte| byte ^ LYING_MASK)
            .collect();
        if let Some(first) = altered.first_mut() {
            *first ^= 0xFF;
        }
        Ok(OpenedSecret::from(altered))
    }

    async fn mac(&self, _mac_use: MacUse, _message: &MacMessage) -> Result<MacTag, CustodyError> {
        Err(CustodyError::PurposeNotHeld {
            purpose: Purpose::DenMac,
        })
    }

    async fn verify(
        &self,
        _mac_use: MacUse,
        _message: &MacMessage,
        _tag: &MacTag,
    ) -> Result<(), CustodyError> {
        Err(CustodyError::PurposeNotHeld {
            purpose: Purpose::DenMac,
        })
    }
}

/// A DID-custody handle over the [`LyingCustodian`].
pub fn lying_did_custody() -> ScopedSealer<ForDidCustody> {
    let custodian: Arc<dyn KeyCustodian> = Arc::new(LyingCustodian);
    ScopedSealer::from(custodian)
}

/// HMAC-SHA256 under `key` of `parts`, one after another.
fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; KEY_BYTES] {
    let mut hmac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes a key of any length");
    for part in parts {
        hmac.update(part);
    }
    hmac.finalize().into_bytes().into()
}

/// RFC 5869 Expand: the first `length` bytes of `T(1) | T(2) | …` from `prk` over `info`.
fn expand(prk: &[u8; KEY_BYTES], info: &[u8], length: usize) -> Vec<u8> {
    let mut okm = Vec::with_capacity(length);
    let mut previous: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while okm.len() < length {
        let block = hmac_sha256(prk, &[&previous, info, &[counter]]);
        okm.extend_from_slice(&block);
        previous = block.to_vec();
        counter += 1;
    }
    okm.truncate(length);
    okm
}

/// The PRK `root` extracts to: RFC 5869 Extract, HMAC-SHA256 keyed by the custody salt.
pub fn prk_of(root: &[u8; KEY_BYTES]) -> [u8; KEY_BYTES] {
    hmac_sha256(CUSTODY_SALT, &[root])
}

/// The key `root` derives for `purpose`, by RFC 5869 alone.
pub fn purpose_key_of(root: &[u8; KEY_BYTES], purpose: Purpose) -> [u8; KEY_BYTES] {
    let token: &'static str = purpose.into();
    let info = format!("{KEY_INFO_PREFIX}{token}");
    let okm = expand(&prk_of(root), info.as_bytes(), KEY_BYTES);
    <[u8; KEY_BYTES]>::try_from(okm.as_slice()).expect("Expand gives the length asked for")
}

/// The id `root` names `purpose`'s key by: `hkdf:<purpose>:<16 lowercase hex>`, by RFC
/// 5869 alone.
pub fn key_id_of(root: &[u8; KEY_BYTES], purpose: Purpose) -> String {
    let token: &'static str = purpose.into();
    let info = format!("{KEY_ID_INFO_PREFIX}{token}");
    let fingerprint = expand(&prk_of(root), info.as_bytes(), FINGERPRINT_BYTES);
    let fingerprint_hex = HEXLOWER.encode(&fingerprint);
    format!("hkdf:{token}:{fingerprint_hex}")
}

/// The key id a stored envelope's header names; panics on an envelope too short for one.
pub fn envelope_key_id(envelope: &[u8]) -> String {
    let [_version, key_id_length, rest @ ..] = envelope else {
        panic!("a stored envelope has a header");
    };
    let key_id = &rest[..usize::from(*key_id_length)];
    String::from_utf8(key_id.to_vec()).expect("a key id is text")
}

/// `bytes` as a log could print them: lowercase hex, standard base64, and the `Debug` of
/// a byte slice (`[1, 2, …]`).
pub fn printed_forms(bytes: &[u8]) -> [String; 3] {
    [
        HEXLOWER.encode(bytes),
        BASE64.encode(bytes),
        format!("{bytes:?}"),
    ]
}

/// Each of [`printed_forms`] of `bytes`, named after `what`.
pub fn named_printings(what: &str, bytes: &[u8]) -> [(String, String); 3] {
    let [hex, base64, list] = printed_forms(bytes);
    [
        (format!("{what} as hex"), hex),
        (format!("{what} as base64"), base64),
        (format!("{what} as a byte list"), list),
    ]
}

/// Every printing of what `root` derives, each named: its PRK and each purpose's key, in
/// each of [`printed_forms`].
pub fn derived_printings(root: &[u8; KEY_BYTES]) -> Vec<(String, String)> {
    let prk = ("the PRK".to_owned(), prk_of(root));
    let keys = Purpose::VARIANTS
        .iter()
        .map(|purpose| (format!("the {purpose} key"), purpose_key_of(root, *purpose)));
    std::iter::once(prk)
        .chain(keys)
        .flat_map(|(what, bytes)| named_printings(&what, &bytes))
        .collect()
}

/// The canary sign-in's secret named `what` in [`SIGNIN_CANARY_SECRETS`].
fn signin_canary(what: &str) -> &'static str {
    SIGNIN_CANARY_SECRETS
        .iter()
        .find(|(name, _)| *name == what)
        .map(|(_, value)| *value)
        .expect("a named canary secret")
}

/// The canary sign-in's session: [`SIGNIN_CANARY_DID`]'s, granting exactly `atproto`.
fn canary_session() -> ClientSessionData {
    let account_did = Did::new_static(SIGNIN_CANARY_DID).expect("a valid DID");
    let host_url = Uri::parse("https://pds.canary.example")
        .expect("a valid URI")
        .to_owned();
    let dpop_data = DpopClientData {
        dpop_key: generate_key(&[SmolStr::new_static("ES256")]).expect("a DPoP key"),
        dpop_authserver_nonce: SmolStr::default(),
        dpop_host_nonce: SmolStr::default(),
    };
    let token_set = TokenSet {
        iss: SmolStr::new_static("https://issuer.canary.example"),
        sub: account_did.clone(),
        aud: SmolStr::new_static("https://pds.canary.example"),
        scope: Some(SmolStr::new_static("atproto")),
        refresh_token: Some(SmolStr::new_static(signin_canary("the refresh token"))),
        access_token: SmolStr::new_static(signin_canary("the access token")),
        token_type: OAuthTokenType::DPoP,
        expires_at: None,
    };
    ClientSessionData {
        account_did,
        session_id: SmolStr::new_static(signin_canary("the session id")),
        host_url,
        authserver_url: SmolStr::new_static("https://issuer.canary.example"),
        authserver_token_endpoint: SmolStr::new_static("https://issuer.canary.example/token"),
        authserver_revocation_endpoint: None,
        scopes: Scopes::empty(),
        dpop_data,
        token_set,
        resolved_scopes: None,
    }
}

/// The canary sign-in's auth request, under its OAuth state.
fn canary_auth_request() -> AuthRequestData {
    let dpop_data = DpopReqData {
        dpop_key: generate_key(&[SmolStr::new_static("ES256")]).expect("a DPoP key"),
        dpop_authserver_nonce: None,
    };
    AuthRequestData {
        state: SmolStr::new_static(signin_canary("the OAuth state")),
        authserver_url: SmolStr::new_static("https://issuer.canary.example"),
        account_did: Some(Did::new_static(SIGNIN_CANARY_DID).expect("a valid DID")),
        scopes: Scopes::empty(),
        request_uri: SmolStr::new_static("urn:ietf:params:oauth:request_uri:canary"),
        authserver_token_endpoint: SmolStr::new_static("https://issuer.canary.example/token"),
        authserver_revocation_endpoint: None,
        pkce_verifier: SmolStr::new_static(signin_canary("the PKCE verifier")),
        dpop_data,
    }
}

/// Saves the canary sign-in's session and auth request through an [`AtprotoAuthStore`]
/// over `pool`, sealed with `sealer`, and reads each back; panics unless both round-trip.
pub async fn signin_secrets_round_trip(pool: PgPool, sealer: ScopedSealer<ForSigninSecrets>) {
    let store = AtprotoAuthStore::new(pool, sealer);
    let session = canary_session();
    let request = canary_auth_request();
    let did: Did = Did::new_static(SIGNIN_CANARY_DID).expect("a valid DID");

    store
        .upsert_session(session.clone())
        .await
        .expect("the session is sealed and saved");
    let reopened_session = store
        .get_session(&did, signin_canary("the session id"))
        .await
        .expect("the session opens")
        .expect("the session is stored");
    store
        .save_auth_req_info(&request)
        .await
        .expect("the auth request is sealed and saved");
    let reopened_request = store
        .get_auth_req_info(signin_canary("the OAuth state"))
        .await
        .expect("the auth request opens")
        .expect("the auth request is stored");

    assert_eq!(reopened_session, session, "the session round-trips");
    assert_eq!(reopened_request, request, "the auth request round-trips");
}

#[cfg(test)]
mod tests;

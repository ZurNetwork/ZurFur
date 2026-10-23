//! Custody fixtures: three real 0.1 custody rows and the root they open under, the test
//! root and a test custodian's scoped handles over it, and a lying custodian for the
//! self-check tests.

use std::sync::Arc;

use adapter_hkdf::HkdfKeyCustodian;
use async_trait::async_trait;
use composition::RootSecret;
use domain::elements::account_keys::{AccountKeys, SecretKey};
use domain::elements::custody::{
    CustodyError, CustodySubject, ForDidCustody, ForSigninSecrets, MacMessage, MacTag, MacUse,
    OpenedSecret, Purpose, SealPurpose, SealedEnvelope,
};
use domain::ports::{KeyCustodian, ScopedMac, ScopedSealer};
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
    let root = Zeroizing::new(TEST_ROOT);
    let derived = HkdfKeyCustodian::derive(root, Purpose::VARIANTS);
    Arc::new(derived)
}

/// The handles of [`test_custodian`].
pub fn test_custody() -> TestCustody {
    let custodian = test_custodian();
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

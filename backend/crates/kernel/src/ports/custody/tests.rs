use std::future::Future;
use std::pin::pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use crate::elements::custody::{ForDidCustody, ForSigninSecrets, KeyId};

use super::*;

/// The key id the fake tags under.
const FAKE_KEY_ID: &str = "fake:den_mac:0000000000000000";

/// A call the fake saw, with the purpose or use it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Seal(SealPurpose),
    Open(SealPurpose),
    Mac(MacUse),
    Verify(MacUse),
}

/// A custodian that records each call and answers with fixed values.
#[derive(Default)]
struct RecordingCustodian {
    calls: Mutex<Vec<Call>>,
}

impl RecordingCustodian {
    fn record(&self, call: Call) {
        self.calls.lock().expect("the call log").push(call);
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("the call log").clone()
    }
}

#[async_trait]
impl KeyCustodian for RecordingCustodian {
    async fn seal(
        &self,
        purpose: SealPurpose,
        _subject: &CustodySubject,
        plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError> {
        self.record(Call::Seal(purpose));
        Ok(SealedEnvelope::from(plaintext.to_vec()))
    }

    async fn open(
        &self,
        purpose: SealPurpose,
        _subject: &CustodySubject,
        envelope: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError> {
        self.record(Call::Open(purpose));
        Ok(OpenedSecret::from(envelope.as_ref().to_vec()))
    }

    async fn mac(&self, mac_use: MacUse, _message: &MacMessage) -> Result<MacTag, CustodyError> {
        self.record(Call::Mac(mac_use));
        let key_id = KeyId::try_from(FAKE_KEY_ID).expect("a valid key id");
        let tag = MacTag::try_from((key_id, [0u8; 32].as_slice())).expect("32 bytes");
        Ok(tag)
    }

    async fn verify(
        &self,
        mac_use: MacUse,
        _message: &MacMessage,
        _tag: &MacTag,
    ) -> Result<(), CustodyError> {
        self.record(Call::Verify(mac_use));
        Ok(())
    }
}

/// The output of `future`, which must finish on its first poll: the fake never waits.
fn finished<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(output) = pin!(future).poll(&mut context) else {
        panic!("the fake custodian never waits");
    };
    output
}

fn subject() -> CustodySubject {
    let parts: &[&[u8]] = &[b"did:plc:subject"];
    CustodySubject::try_from(parts).expect("small parts encode")
}

fn message() -> MacMessage {
    let parts: &[&[u8]] = &[b"node"];
    MacMessage::try_from(parts).expect("small parts encode")
}

#[test]
fn a_scoped_sealer_only_seals_its_purpose() {
    let custodian = Arc::new(RecordingCustodian::default());
    let shared: Arc<dyn KeyCustodian> = custodian.clone();
    let did_custody = ScopedSealer::<ForDidCustody>::from(Arc::clone(&shared));
    let signin_secrets = ScopedSealer::<ForSigninSecrets>::from(shared);

    let did_envelope = finished(did_custody.seal(&subject(), b"did key")).expect("sealed");
    finished(did_custody.open(&subject(), &did_envelope)).expect("opened");
    let signin_envelope = finished(signin_secrets.seal(&subject(), b"state")).expect("sealed");
    finished(signin_secrets.open(&subject(), &signin_envelope)).expect("opened");

    let expected = [
        Call::Seal(SealPurpose::DidCustody),
        Call::Open(SealPurpose::DidCustody),
        Call::Seal(SealPurpose::SigninSecrets),
        Call::Open(SealPurpose::SigninSecrets),
    ];
    assert_eq!(custodian.calls(), expected);
}

#[test]
fn a_scoped_mac_passes_each_use_through() {
    let custodian = Arc::new(RecordingCustodian::default());
    let shared: Arc<dyn KeyCustodian> = custodian.clone();
    let den_mac = ScopedMac::from(shared);

    let tag = finished(den_mac.mac(MacUse::DenNode, &message())).expect("tagged");
    finished(den_mac.verify(MacUse::DenChangelogEntry, &message(), &tag)).expect("verified");

    let expected = [
        Call::Mac(MacUse::DenNode),
        Call::Verify(MacUse::DenChangelogEntry),
    ];
    assert_eq!(custodian.calls(), expected);
}

#[test]
fn a_cloned_handle_shares_its_custodian() {
    let custodian = Arc::new(RecordingCustodian::default());
    let shared: Arc<dyn KeyCustodian> = custodian.clone();
    let sealer = ScopedSealer::<ForDidCustody>::from(Arc::clone(&shared));
    let den_mac = ScopedMac::from(shared);

    let sealer_copy = sealer.clone();
    let den_mac_copy = den_mac.clone();

    assert_eq!(Arc::strong_count(&custodian), 5);
    finished(sealer_copy.seal(&subject(), b"x")).expect("sealed");
    finished(den_mac_copy.mac(MacUse::DenNode, &message())).expect("tagged");
    let expected = [
        Call::Seal(SealPurpose::DidCustody),
        Call::Mac(MacUse::DenNode),
    ];
    assert_eq!(custodian.calls(), expected);
}

#[test]
fn a_handle_prints_its_purpose_only() {
    let shared: Arc<dyn KeyCustodian> = Arc::new(RecordingCustodian::default());
    let did_custody = ScopedSealer::<ForDidCustody>::from(Arc::clone(&shared));
    let signin_secrets = ScopedSealer::<ForSigninSecrets>::from(Arc::clone(&shared));
    let den_mac = ScopedMac::from(shared);

    let printed = [
        format!("{did_custody:?}"),
        format!("{signin_secrets:?}"),
        format!("{den_mac:?}"),
    ];

    let expected = [
        "ScopedSealer { purpose: DidCustody }",
        "ScopedSealer { purpose: SigninSecrets }",
        "ScopedMac { purpose: DenMac }",
    ];
    assert_eq!(printed, expected);
}

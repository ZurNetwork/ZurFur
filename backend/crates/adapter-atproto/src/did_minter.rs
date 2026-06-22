use async_trait::async_trait;
use domain::{elements::did::Did, ports::DidMinter};
use rand::Rng;

/// `did:plc` base32 alphabet (RFC 4648, lowercase, no padding). A real account
/// DID is `did:plc:` followed by 24 of these characters.
const PLC_BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// The ZMVP-14 floor stub for [`DidMinter`]: it returns a structurally
/// valid-looking but entirely **synthetic** `did:plc` value — `did:plc:` plus 24
/// random lowercase base32 characters. DIDs are opaque, so a random suffix is
/// indistinguishable in shape from a real one; no marker substring is needed.
///
/// The real minter is deliberately deferred — keypair generation, building and
/// signing the PLC genesis operation, submitting it to the PLC directory,
/// allocating the account's PDS slot, and taking custody of the signing key are
/// all out of scope here. Dress when The Who closes.
#[derive(Debug, Default, Clone)]
pub struct StubDidMinter;

impl StubDidMinter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DidMinter for StubDidMinter {
    async fn mint(&self) -> anyhow::Result<Did> {
        let mut rng = rand::thread_rng();
        let suffix: String = (0..24)
            .map(|_| PLC_BASE32[rng.gen_range(0..PLC_BASE32.len())] as char)
            .collect();
        Ok(Did::new(format!("did:plc:{suffix}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mint_produces_synthetic_did_plc() {
        let did = StubDidMinter::new().mint().await.unwrap();
        let value = did.as_str();
        assert!(
            value.starts_with("did:plc:"),
            "expected did:plc prefix, got {value}"
        );
        // "did:plc:" (8) + 24 base32 chars = 32 characters total.
        assert_eq!(value.len(), 32, "unexpected DID length: {value}");
        let suffix = &value["did:plc:".len()..];
        assert_eq!(suffix.len(), 24);
        assert!(
            suffix.bytes().all(|b| PLC_BASE32.contains(&b)),
            "suffix has non-base32 chars: {suffix}"
        );
    }
}

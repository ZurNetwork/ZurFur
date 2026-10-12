//! The address table: which IP addresses the guarded client may connect to.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// An IP address the guarded client may connect to: none of the private,
/// loopback, link-local, metadata, documentation or reserved blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Into)]
pub(crate) struct PublicAddress(IpAddr);

/// The address is private, internal or reserved, so nothing connects to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("forbidden address")]
pub(crate) struct ForbiddenAddress;

impl TryFrom<IpAddr> for PublicAddress {
    type Error = ForbiddenAddress;

    /// Admit `address` only when the address table allows it.
    fn try_from(address: IpAddr) -> Result<Self, Self::Error> {
        let public = match address {
            IpAddr::V4(v4) => is_public_v4(v4),
            IpAddr::V6(v6) => is_public_v6(v6),
        };
        if !public {
            return Err(ForbiddenAddress);
        }
        Ok(Self(address))
    }
}

/// An IPv4 CIDR block.
struct V4Block {
    network: Ipv4Addr,
    prefix: u32,
}

impl V4Block {
    const fn new(network: Ipv4Addr, prefix: u32) -> Self {
        Self { network, prefix }
    }

    fn contains(&self, address: Ipv4Addr) -> bool {
        let mask = u32::MAX.checked_shl(32 - self.prefix).unwrap_or(0);
        address.to_bits() & mask == self.network.to_bits() & mask
    }
}

/// An IPv6 CIDR block.
struct V6Block {
    network: Ipv6Addr,
    prefix: u32,
}

impl V6Block {
    const fn new(network: Ipv6Addr, prefix: u32) -> Self {
        Self { network, prefix }
    }

    fn contains(&self, address: Ipv6Addr) -> bool {
        let mask = u128::MAX.checked_shl(128 - self.prefix).unwrap_or(0);
        address.to_bits() & mask == self.network.to_bits() & mask
    }
}

/// Every refused IPv4 block. Anything outside them is allowed.
const V4_REFUSED: [V4Block; 16] = [
    // "This network".
    V4Block::new(Ipv4Addr::new(0, 0, 0, 0), 8),
    // Private-Use.
    V4Block::new(Ipv4Addr::new(10, 0, 0, 0), 8),
    // Shared Address Space (CGNAT), Alibaba Cloud's metadata service included.
    V4Block::new(Ipv4Addr::new(100, 64, 0, 0), 10),
    // Loopback.
    V4Block::new(Ipv4Addr::new(127, 0, 0, 0), 8),
    // Link Local, the AWS/GCP/Azure metadata service included.
    V4Block::new(Ipv4Addr::new(169, 254, 0, 0), 16),
    // Private-Use.
    V4Block::new(Ipv4Addr::new(172, 16, 0, 0), 12),
    // IETF Protocol Assignments, the whole block.
    V4Block::new(Ipv4Addr::new(192, 0, 0, 0), 24),
    // Documentation (TEST-NET-1).
    V4Block::new(Ipv4Addr::new(192, 0, 2, 0), 24),
    // Deprecated 6to4 relay anycast.
    V4Block::new(Ipv4Addr::new(192, 88, 99, 0), 24),
    // Private-Use.
    V4Block::new(Ipv4Addr::new(192, 168, 0, 0), 16),
    // Benchmarking.
    V4Block::new(Ipv4Addr::new(198, 18, 0, 0), 15),
    // Documentation (TEST-NET-2).
    V4Block::new(Ipv4Addr::new(198, 51, 100, 0), 24),
    // Documentation (TEST-NET-3).
    V4Block::new(Ipv4Addr::new(203, 0, 113, 0), 24),
    // Multicast.
    V4Block::new(Ipv4Addr::new(224, 0, 0, 0), 4),
    // Reserved, limited broadcast included.
    V4Block::new(Ipv4Addr::new(240, 0, 0, 0), 4),
    // Azure's platform address (WireServer, platform DNS): public-looking.
    V4Block::new(Ipv4Addr::new(168, 63, 129, 16), 32),
];

/// IPv4-mapped IPv6 addresses are unwrapped through `to_ipv4_mapped`; this is
/// the other embedding that gets unwrapped: the NAT64 well-known prefix.
const NAT64_WELL_KNOWN: V6Block = V6Block::new(Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0), 96);

/// Global Unicast: the only IPv6 space allowed at all.
const GLOBAL_UNICAST: V6Block = V6Block::new(Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3);

/// The refused blocks inside Global Unicast.
const V6_REFUSED: [V6Block; 4] = [
    // IETF Protocol Assignments, Teredo included.
    V6Block::new(Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23),
    // Documentation.
    V6Block::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    // 6to4, which embeds an IPv4 address.
    V6Block::new(Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16),
    // Documentation.
    V6Block::new(Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20),
];

fn is_public_v4(address: Ipv4Addr) -> bool {
    !V4_REFUSED.iter().any(|block| block.contains(address))
}

/// Unwraps mapped and NAT64 addresses to the IPv4 table; never the deprecated
/// IPv4-compatible form, which falls outside Global Unicast and is refused.
fn is_public_v6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_v4(mapped);
    }
    if NAT64_WELL_KNOWN.contains(address) {
        let [.., a, b, c, d] = address.octets();
        return is_public_v4(Ipv4Addr::new(a, b, c, d));
    }
    GLOBAL_UNICAST.contains(address) && !V6_REFUSED.iter().any(|block| block.contains(address))
}

#[cfg(test)]
mod tests;

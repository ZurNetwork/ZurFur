use super::*;

/// Every listed address, parsed, paired with whether the table admits it.
fn verdicts(addresses: &[&str]) -> Vec<(String, bool)> {
    addresses
        .iter()
        .map(|text| {
            let address: IpAddr = text.parse().expect("test address parses");
            let admitted = PublicAddress::try_from(address).is_ok();
            (text.to_string(), admitted)
        })
        .collect()
}

/// The addresses in `addresses` the table admits; empty when it refuses all.
fn admitted(addresses: &[&str]) -> Vec<String> {
    verdicts(addresses)
        .into_iter()
        .filter(|(_, admitted)| *admitted)
        .map(|(text, _)| text)
        .collect()
}

/// The addresses in `addresses` the table refuses; empty when it admits all.
fn refused(addresses: &[&str]) -> Vec<String> {
    verdicts(addresses)
        .into_iter()
        .filter(|(_, admitted)| !*admitted)
        .map(|(text, _)| text)
        .collect()
}

#[test]
fn every_refused_ipv4_block_is_refused_at_its_first_last_and_an_inside_address() {
    let refused_blocks = [
        // 0.0.0.0/8
        "0.0.0.0",
        "0.1.2.3",
        "0.255.255.255",
        // 10.0.0.0/8
        "10.0.0.0",
        "10.0.0.1",
        "10.255.255.255",
        // 100.64.0.0/10, Alibaba Cloud's metadata included
        "100.64.0.0",
        "100.100.100.200",
        "100.127.255.255",
        // 127.0.0.0/8
        "127.0.0.0",
        "127.0.0.1",
        "127.255.255.255",
        // 169.254.0.0/16, the cloud metadata services included
        "169.254.0.0",
        "169.254.169.254",
        "169.254.169.253",
        "169.254.170.2",
        "169.254.255.255",
        // 172.16.0.0/12
        "172.16.0.0",
        "172.20.1.1",
        "172.31.255.255",
        // 192.0.0.0/24
        "192.0.0.0",
        "192.0.0.9",
        "192.0.0.255",
        // 192.0.2.0/24
        "192.0.2.0",
        "192.0.2.1",
        "192.0.2.255",
        // 192.88.99.0/24
        "192.88.99.0",
        "192.88.99.1",
        "192.88.99.255",
        // 192.168.0.0/16
        "192.168.0.0",
        "192.168.1.1",
        "192.168.255.255",
        // 198.18.0.0/15
        "198.18.0.0",
        "198.18.0.1",
        "198.19.255.255",
        // 198.51.100.0/24
        "198.51.100.0",
        "198.51.100.7",
        "198.51.100.255",
        // 203.0.113.0/24
        "203.0.113.0",
        "203.0.113.5",
        "203.0.113.255",
        // 224.0.0.0/4
        "224.0.0.0",
        "224.0.0.251",
        "239.255.255.255",
        // 240.0.0.0/4, limited broadcast included
        "240.0.0.0",
        "250.1.2.3",
        "255.255.255.255",
        // 168.63.129.16/32
        "168.63.129.16",
    ];

    let wrongly_admitted = admitted(&refused_blocks);

    let none: Vec<String> = Vec::new();
    assert_eq!(wrongly_admitted, none, "these must be refused");
}

#[test]
fn the_allowed_neighbour_on_each_side_of_each_ipv4_block_is_admitted() {
    let neighbours = [
        "1.0.0.0",
        "9.255.255.255",
        "11.0.0.0",
        "100.63.255.255",
        "100.128.0.0",
        "126.255.255.255",
        "128.0.0.0",
        "169.253.255.255",
        "169.255.0.0",
        "172.15.255.255",
        "172.32.0.0",
        "191.255.255.255",
        "192.0.1.0",
        "192.0.1.255",
        "192.0.3.0",
        "192.88.98.255",
        "192.88.100.0",
        "192.167.255.255",
        "192.169.0.0",
        "198.17.255.255",
        "198.20.0.0",
        "198.51.99.255",
        "198.51.101.0",
        "203.0.112.255",
        "203.0.114.0",
        "223.255.255.255",
        "168.63.129.15",
        "168.63.129.17",
        "8.8.8.8",
        "93.184.216.34",
    ];

    let wrongly_refused = refused(&neighbours);

    let none: Vec<String> = Vec::new();
    assert_eq!(wrongly_refused, none, "these must be admitted");
}

#[test]
fn ipv6_outside_global_unicast_and_the_refused_blocks_inside_it_are_refused() {
    let refused_v6 = [
        "::",
        "::1",
        // IPv4-compatible: never unwrapped, so a public IPv4 inside is no excuse.
        "::127.0.0.1",
        "::8.8.8.8",
        // IPv4-mapped, unwrapped to a refused IPv4 address.
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::ffff:169.254.169.254",
        // NAT64 well-known prefix, unwrapped to a refused IPv4 address.
        "64:ff9b::a00:1",
        "64:ff9b::a9fe:a9fe",
        // Local-use NAT64 is not unwrapped and lies outside Global Unicast.
        "64:ff9b:1::1",
        "100::1",
        "1fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
        // 2001::/23, first and last.
        "2001::1",
        "2001:1ff:ffff:ffff:ffff:ffff:ffff:ffff",
        "2001:db8::1",
        "2002:a00:1::1",
        // 3fff::/20, first and last.
        "3fff::1",
        "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
        "4000::1",
        "5f00::1",
        "fd00:ec2::254",
        "fd20:ce::254",
        "fe80::1",
        "fec0::1",
        "ff02::1",
    ];

    let wrongly_admitted = admitted(&refused_v6);

    let none: Vec<String> = Vec::new();
    assert_eq!(wrongly_admitted, none, "these must be refused");
}

#[test]
fn public_ipv6_and_public_ipv4_behind_mapped_or_nat64_addresses_are_admitted() {
    let allowed_v6 = [
        "2606:4700:4700::1111",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
        "2000::1",
        "2001:200::1",
        "2001:db7:ffff::1",
        "2001:db9::1",
        "2003::1",
        "3ffe:ffff::1",
        "3fff:1000::1",
    ];

    let wrongly_refused = refused(&allowed_v6);

    let none: Vec<String> = Vec::new();
    assert_eq!(wrongly_refused, none, "these must be admitted");
}

#[test]
fn an_admitted_address_converts_back_to_itself() {
    let address: IpAddr = "8.8.8.8".parse().expect("parses");

    let public = PublicAddress::try_from(address).expect("public");

    assert_eq!(IpAddr::from(public), address);
}

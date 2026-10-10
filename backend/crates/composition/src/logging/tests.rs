use super::*;

#[test]
fn the_dns_client_crates_are_never_loggable() {
    for target in [
        "hickory_resolver",
        "hickory_resolver::name_server_pool",
        "hickory_net::xfer",
        "hickory_proto::rr",
    ] {
        assert!(!is_loggable_target(target), "{target}");
    }
}

#[test]
fn every_other_target_is_loggable() {
    for target in [
        "api",
        "cli",
        "adapter_atproto::resolver::client",
        "hickory_resolverish",
        "tower_http",
    ] {
        assert!(is_loggable_target(target), "{target}");
    }
}

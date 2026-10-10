use test_support::log_capture::CapturedLog;

use super::*;

#[test]
fn a_dns_client_error_naming_a_handle_never_reaches_the_server_log() {
    let log = CapturedLog::default();
    let wide_open =
        EnvFilter::new("trace,hickory_resolver=trace,hickory_resolver::name_server_pool=trace");
    let subscriber = subscriber(wide_open, log.clone());

    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(target: "hickory_resolver::name_server_pool", "malformed answer for alice.example.com");
        tracing::error!(target: "hickory_proto::op", "bad record for bob.example.com");
        tracing::info!(target: "api", "server started");
    });

    let text = log.text();
    assert!(
        text.contains("server started"),
        "the rest of the log still flows: {text}"
    );
    assert!(!text.contains("alice.example.com"), "{text}");
    assert!(!text.contains("bob.example.com"), "{text}");
}

use test_support::log_capture::CapturedLog;

use super::*;

#[test]
fn a_dns_client_error_naming_a_handle_never_reaches_the_cli_log() {
    let log = CapturedLog::default();
    let wide_open = EnvFilter::new("trace,hickory_resolver=trace,hickory_net=trace");
    let subscriber = subscriber(wide_open, log.clone());

    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(target: "hickory_resolver::name_server_pool", "malformed answer for alice.example.com");
        tracing::error!(target: "hickory_net::xfer", "truncated reply for bob.example.com");
        tracing::warn!(target: "cli", "a warning still shows");
    });

    let text = log.text();
    assert!(
        text.contains("a warning still shows"),
        "the rest of the log still flows: {text}"
    );
    assert!(!text.contains("alice.example.com"), "{text}");
    assert!(!text.contains("bob.example.com"), "{text}");
}

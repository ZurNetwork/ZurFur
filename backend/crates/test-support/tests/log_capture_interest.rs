//! Pins a scoped log capture seeing an event whose callsite another thread, with no
//! subscriber, reached first. Alone in its binary, so no other test's dispatcher masks it.

use test_support::log_capture::{self, CapturedLog};

/// The line the shared callsite logs.
const ANNOUNCEMENT: &str = "the shared callsite speaks";

/// One callsite, reached from both threads.
fn announce() {
    tracing::info!("{ANNOUNCEMENT}");
}

#[test]
fn a_capture_sees_a_callsite_another_thread_reached_first() {
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _guard = log_capture::set_default(subscriber);

    let bystander = std::thread::spawn(announce);
    bystander.join().expect("the bystander thread finishes");
    announce();

    let text = log.text();
    assert!(
        text.contains(ANNOUNCEMENT),
        "the capture missed the line: {text:?}"
    );
}

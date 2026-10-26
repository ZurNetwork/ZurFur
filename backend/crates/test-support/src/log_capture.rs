//! [`CapturedLog`]: a `tracing-subscriber` writer that keeps what a subscriber
//! wrote, so a driver's test can read its own log back; [`set_default`] and
//! [`with_default`], the one way a test installs its capture for its thread.

use std::sync::{Arc, LazyLock, Mutex};

use tracing::{
    Dispatch, Event, Metadata, Subscriber,
    level_filters::LevelFilter,
    span::{Attributes, Id, Record},
    subscriber::{DefaultGuard, Interest},
};
use tracing_subscriber::fmt::MakeWriter;

/// Everything a subscriber wrote through it; clones share one buffer.
#[derive(Clone, Default)]
pub struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl CapturedLog {
    /// What has been written so far, as text.
    pub fn text(&self) -> String {
        let bytes = self.0.lock().expect("captured log lock").clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl std::io::Write for CapturedLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("captured log lock")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for CapturedLog {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

/// Makes `subscriber` this thread's default until the guard drops, as tracing's own does.
/// A second dispatcher stays alive beside it, so a parallel thread with no subscriber
/// can't cache a callsite as never while this capture is the only one.
pub fn set_default<S>(subscriber: S) -> DefaultGuard
where
    S: Subscriber + Send + Sync + 'static,
{
    LazyLock::force(&KEEPER);
    tracing::subscriber::set_default(subscriber)
}

/// Runs `work` with `subscriber` as this thread's default, as tracing's own does.
/// A second dispatcher stays alive beside it, so a parallel thread with no subscriber
/// can't cache a callsite as never while this capture is the only one.
pub fn with_default<S, T>(subscriber: S, work: impl FnOnce() -> T) -> T
where
    S: Subscriber + Send + Sync + 'static,
{
    LazyLock::force(&KEEPER);
    tracing::subscriber::with_default(subscriber, work)
}

/// The process-wide dispatcher that keeps every capture from being the only one; never
/// anyone's default and never dropped.
static KEEPER: LazyLock<Dispatch> = LazyLock::new(|| Dispatch::new(Keeper));

/// A subscriber that enables nothing and answers every callsite "sometimes".
struct Keeper;

impl Subscriber for Keeper {
    fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        false
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::OFF)
    }

    fn new_span(&self, _attributes: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

//! A test-only log recorder shared by every test in this crate: one global
//! subscriber that keeps the events of whichever thread asks for them.

use std::future::Future;

pub(crate) type Recorded = Vec<(tracing::Level, String)>;

thread_local! {
    /// The events this thread emitted while [`record_events`] runs it.
    static RECORDED: std::cell::RefCell<Option<Recorded>> = const { std::cell::RefCell::new(None) };
}

/// A global subscriber that keeps only the events of a thread inside
/// [`record_events`]. Global and per-thread rather than a scoped default: a
/// lone scoped dispatcher lets parallel tests cache callsites as uninteresting.
struct ThreadRecorder;

/// Renders an event's fields as `name=value` pairs.
struct FieldLine<'a>(&'a mut String);

impl tracing::field::Visit for FieldLine<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;
        let _ = write!(self.0, "{}={value:?} ", field.name());
    }
}

impl tracing::Subscriber for ThreadRecorder {
    fn register_callsite(
        &self,
        _: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }

    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        RECORDED.with(|recorded| recorded.borrow().is_some())
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut line = String::new();
        event.record(&mut FieldLine(&mut line));
        let level = *event.metadata().level();
        RECORDED.with(|recorded| {
            if let Some(events) = recorded.borrow_mut().as_mut() {
                events.push((level, line));
            }
        });
    }

    fn enter(&self, _: &tracing::span::Id) {}

    fn exit(&self, _: &tracing::span::Id) {}
}

/// Run `work` on this thread and return what it logged, as `name=value` lines.
pub(crate) async fn record_events<T>(work: impl Future<Output = T>) -> (T, Recorded) {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(ThreadRecorder)
            .expect("no other global subscriber in this test binary");
    });
    RECORDED.with(|recorded| *recorded.borrow_mut() = Some(Vec::new()));
    let output = work.await;
    let events = RECORDED.with(|recorded| recorded.borrow_mut().take().unwrap_or_default());
    (output, events)
}

/// The lines of `events` at `level`, or more severe when `and_above`.
pub(crate) fn lines_at(events: &Recorded, level: tracing::Level, and_above: bool) -> Vec<String> {
    events
        .iter()
        .filter(|(event_level, _)| *event_level == level || (and_above && *event_level < level))
        .map(|(_, line)| line.clone())
        .collect()
}

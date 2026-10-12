//! [`CapturedLog`]: a `tracing-subscriber` writer that keeps what a subscriber
//! wrote, so a driver's test can read its own log back.

use std::sync::{Arc, Mutex};

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

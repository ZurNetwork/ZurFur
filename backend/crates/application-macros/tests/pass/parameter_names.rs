//! A use case whose parameters share the names the wrapper binds still gets its own values.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use application_macros::use_case;

struct Probe;

impl Probe {
    #[use_case]
    async fn run(
        &self,
        span: u8,
        logged: u8,
        outcome: u8,
        error: u8,
    ) -> Result<[u8; 4], std::fmt::Error> {
        Ok([span, logged, outcome, error])
    }
}

/// Polls `future` once; the use case above never waits.
fn ready<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(output) = pin!(future).poll(&mut context) else {
        panic!("the use case never waits");
    };
    output
}

fn main() {
    let answer = ready(Probe.run(1, 2, 3, 4));
    assert_eq!(answer, Ok([1, 2, 3, 4]));
}

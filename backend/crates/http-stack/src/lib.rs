//! The one HTTP stack every mount serves through: [`stack`] puts the session
//! layer inside [`Detach`], which runs each request to its end in its own task,
//! with the request log outside both, and [`serve`] serves only such a stack,
//! shutting down gracefully.

mod detach;
mod request_log;
mod served;

pub use detach::{Detach, Detached, REQUEST_PERMITS, Unfinished, panic_answer};
pub use request_log::{RequestLog, UNMATCHED_ROUTE, request_log};
pub use served::{BODY_IDLE_TIMEOUT, SHUTDOWN_DEADLINE, Stacked, serve, stack};

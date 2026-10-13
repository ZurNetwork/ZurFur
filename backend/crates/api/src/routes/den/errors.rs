use application::den::DenError;

use super::{params::DenParamsError, pieces::NotDenPieces};
use crate::problem::Problem;

impl From<DenParamsError> for Problem {
    /// An unknown parameter is the fixed `400`; a repeated or unusable one the fixed `422`.
    fn from(error: DenParamsError) -> Self {
        match error {
            DenParamsError::Unknown => Problem::unknown_parameter(),
            DenParamsError::Repeated | DenParamsError::Malformed => Problem::invalid_query(),
        }
    }
}

impl From<NotDenPieces> for Problem {
    /// A path that doesn't decode is the one not-found.
    fn from(_: NotDenPieces) -> Self {
        Problem::node_not_found()
    }
}

impl From<DenError> for Problem {
    /// Every miss is the one not-found; a store failure is the house's `500`, its cause never
    /// rendered.
    fn from(error: DenError) -> Self {
        match error {
            DenError::NotFound => Problem::node_not_found(),
            DenError::Store(cause) => Problem::from(cause),
        }
    }
}

/// A Den read's error as its problem. A store failure logs one fixed line naming only the
/// route template, never its cause, a path, a piece, a token or a viewer.
pub(super) fn read_problem(error: DenError, route: &str) -> Problem {
    if matches!(error, DenError::Store(_)) {
        tracing::error!(route, "a den read failed in a store");
    }
    Problem::from(error)
}

#[cfg(test)]
mod tests;

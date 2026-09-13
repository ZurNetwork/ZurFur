//! The confirmation gate in front of an irreversible operation: a person at
//! a terminal is asked and only a typed `y`/`yes` proceeds; a pipe, redirect
//! or CI session is refused, not defaulted (`--yes` is the explicit opt-in).
//! The question goes to stderr — stdout stays the data channel.

use std::io::{BufRead, IsTerminal, Write};

use crate::CliError;

/// Whether there is anybody to answer a question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    /// A person at a terminal: the question can be asked and answered.
    Interactive,
    /// A pipe, a redirect, a CI runner: no question can be asked.
    Detached,
}

impl Session {
    /// Interactive only when both stdin and stderr are terminals. Untestable
    /// directly (needs a pty); tests drive [`confirm`] with the variant they
    /// mean to exercise.
    pub fn detect() -> Self {
        if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
            Session::Interactive
        } else {
            Session::Detached
        }
    }
}

/// Ask before an irreversible operation, over the process's real streams.
/// `Ok(())` is the only outcome that lets the caller proceed; anything else
/// — a detached session, a declined prompt, an unreadable answer — becomes
/// a [`CliError`], and the operation never runs.
pub fn confirm_destructive(prompt: &str) -> Result<(), CliError> {
    let stdin = std::io::stdin();
    confirm(
        prompt,
        Session::detect(),
        stdin.lock(),
        std::io::stderr().lock(),
    )
}

/// The gate itself, over injected streams: write `prompt` to `question`, read
/// one line from `answer`, and proceed only on `y`/`yes` (case-insensitive,
/// trimmed). A [`Session::Detached`] session is refused before either stream
/// is touched.
pub fn confirm(
    prompt: &str,
    session: Session,
    mut answer: impl BufRead,
    mut question: impl Write,
) -> Result<(), CliError> {
    if session == Session::Detached {
        return Err(CliError::domain(
            "confirmation_required",
            "refusing to delete without --yes when not attached to a terminal",
        ));
    }

    write!(question, "{prompt}")
        .and_then(|()| question.flush())
        .map_err(|err| {
            CliError::infra("internal_error", format!("could not ask to confirm: {err}"))
        })?;

    let mut reply = String::new();
    answer.read_line(&mut reply).map_err(|err| {
        CliError::infra(
            "internal_error",
            format!("could not read the confirmation: {err}"),
        )
    })?;

    // Anything but an explicit y/yes means no — including a bare Enter or EOF.
    let reply = reply.trim().to_ascii_lowercase();
    if matches!(reply.as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(CliError::domain("cancelled", "deletion cancelled"))
    }
}

#[cfg(test)]
mod tests;

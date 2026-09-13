//! The stdout data channel: exactly one JSON value (or one raw byte payload,
//! for `completions`) per successful run.

use std::io::Write as _;

/// How JSON reaches stdout: pretty (the default, for eyes) or compact (`--json`,
/// for pipes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pretty,
    Compact,
}

impl Format {
    /// `--json` → [`Compact`](Format::Compact); otherwise [`Pretty`](Format::Pretty).
    pub fn from_flag(json: bool) -> Self {
        if json {
            Format::Compact
        } else {
            Format::Pretty
        }
    }
}

/// A successful run's stdout payload.
#[derive(Debug)]
pub enum Output {
    /// A JSON value rendered per its format, newline-terminated.
    Json(serde_json::Value, Format),
    /// Bytes written verbatim (a completion script).
    Raw(Vec<u8>),
}

impl Output {
    pub fn json(value: serde_json::Value, format: Format) -> Self {
        Output::Json(value, format)
    }

    pub fn raw(bytes: Vec<u8>) -> Self {
        Output::Raw(bytes)
    }

    /// Render to bytes exactly as they will hit stdout.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Output::Json(value, Format::Pretty) => {
                let mut rendered = serde_json::to_vec_pretty(value).expect("Value serializes");
                rendered.push(b'\n');
                rendered
            }
            Output::Json(value, Format::Compact) => {
                let mut rendered = serde_json::to_vec(value).expect("Value serializes");
                rendered.push(b'\n');
                rendered
            }
            Output::Raw(bytes) => bytes.clone(),
        }
    }

    /// Write to stdout. A closed pipe (`head`, `jq -e` exiting early) is not an
    /// error worth reporting — the write is best-effort by design.
    pub fn write_stdout(&self) {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(&self.to_bytes());
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests;

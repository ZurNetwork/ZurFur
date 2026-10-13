use std::str::FromStr;

/// Text percent-decoded exactly once: each `%XX` becomes its byte and every other character
/// stays as it is, `+` included.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Into)]
pub(super) struct PercentDecoded(String);

/// Why text can't be percent-decoded: a `%` without two hex digits, or bytes that aren't UTF-8
/// once decoded. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("text is not percent-encoded UTF-8")]
pub(super) struct NotPercentEncoded;

/// The value of one ASCII hex digit.
fn hex_value(digit: u8) -> Option<u8> {
    char::from(digit)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

impl FromStr for PercentDecoded {
    type Err = NotPercentEncoded;

    /// Decodes each `%XX` once; a decoded `%` is never decoded again.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut decoded = Vec::with_capacity(text.len());
        let mut bytes = text.bytes();
        while let Some(byte) = bytes.next() {
            if byte != b'%' {
                decoded.push(byte);
                continue;
            }
            let high = bytes.next().and_then(hex_value).ok_or(NotPercentEncoded)?;
            let low = bytes.next().and_then(hex_value).ok_or(NotPercentEncoded)?;
            decoded.push(high << 4 | low);
        }
        let text = String::from_utf8(decoded).map_err(|_| NotPercentEncoded)?;
        Ok(Self(text))
    }
}

#[cfg(test)]
mod tests;

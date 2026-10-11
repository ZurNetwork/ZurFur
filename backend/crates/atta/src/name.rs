use std::str::FromStr;

/// The most bytes a node's name may hold.
pub const MAX_NAME_BYTES: usize = 2048;

/// U+200C ZERO WIDTH NON-JOINER, the one format character besides the joiner a name may hold.
const ZERO_WIDTH_NON_JOINER: char = '\u{200C}';

/// U+200D ZERO WIDTH JOINER, the other format character a name may hold.
const ZERO_WIDTH_JOINER: char = '\u{200D}';

/// U+2028 LINE SEPARATOR, refused anywhere in a name.
const LINE_SEPARATOR: char = '\u{2028}';

/// U+2029 PARAGRAPH SEPARATOR, refused anywhere in a name.
const PARAGRAPH_SEPARATOR: char = '\u{2029}';

/// The code points with Unicode's `Default_Ignorable_Code_Point` property, as inclusive ranges,
/// from the Unicode Character Database 16.0.0 (`DerivedCoreProperties.txt`). They render as
/// nothing on their own.
const DEFAULT_IGNORABLE: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF0, 0xFFF8),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE0FFF),
];

/// Characters that show an empty cell yet are neither white space nor default-ignorable: the
/// blank Braille pattern, the null notehead, the Khitan filler and the two Egyptian blanks.
const VISIBLE_FILLERS: &[char] = &[
    '\u{2800}',
    '\u{1D159}',
    '\u{16FE4}',
    '\u{13441}',
    '\u{13442}',
];

/// The code points of Unicode's general category Cf (format), as inclusive ranges, from the
/// Unicode Character Database 16.0.0.
const FORMAT_CHARACTERS: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

/// A node's display name, taken from what the node holds and never its key.
///
/// Parsed by hand because the parse is the rule: at most [`MAX_NAME_BYTES`] bytes, no `/`, no
/// control character (Cc), no line or paragraph separator, no format character (Cf) except the
/// zero-width joiner and non-joiner, and at least one character that is not blank: not white
/// space, not default-ignorable and not a visible filler.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct NodeName(String);

/// Why a piece of text is not a [`NodeName`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NodeNameError {
    /// The name is empty, or holds only white space, default-ignorable characters and visible
    /// fillers.
    #[error("a node name is empty")]
    Empty,
    /// The name is longer than [`MAX_NAME_BYTES`].
    #[error("a node name is longer than {MAX_NAME_BYTES} bytes")]
    TooLong,
    /// The name holds a `/`.
    #[error("a node name holds a `/`")]
    Slash,
    /// The name holds a control character or a format character other than the two joiners.
    #[error("a node name holds a control or format character")]
    Invisible,
    /// The name holds a line or paragraph separator.
    #[error("a node name holds a line or paragraph separator")]
    Separator,
}

/// Whether `character` is in Unicode's general category Cf.
fn is_format(character: char) -> bool {
    let code = u32::from(character);
    FORMAT_CHARACTERS
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
}

/// Whether `character` has Unicode's `Default_Ignorable_Code_Point` property.
fn is_default_ignorable(character: char) -> bool {
    let code = u32::from(character);
    DEFAULT_IGNORABLE
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
}

/// Whether `character` shows nothing a reader could tell apart: white space, a default-ignorable
/// character (the joiners and the Hangul fillers among them) or a visible filler.
fn is_blank(character: char) -> bool {
    character.is_whitespace()
        || is_default_ignorable(character)
        || VISIBLE_FILLERS.contains(&character)
}

/// Whether `character` is one of the two joiners a name may hold.
fn is_joiner(character: char) -> bool {
    character == ZERO_WIDTH_JOINER || character == ZERO_WIDTH_NON_JOINER
}

/// Checks `text` against the name rule, before anything is copied.
fn check(text: &str) -> Result<(), NodeNameError> {
    if text.len() > MAX_NAME_BYTES {
        return Err(NodeNameError::TooLong);
    }
    if text.contains('/') {
        return Err(NodeNameError::Slash);
    }
    if text.contains([LINE_SEPARATOR, PARAGRAPH_SEPARATOR]) {
        return Err(NodeNameError::Separator);
    }
    if text.chars().all(is_blank) {
        return Err(NodeNameError::Empty);
    }
    let invisible =
        |character: char| character.is_control() || (is_format(character) && !is_joiner(character));
    if text.chars().any(invisible) {
        return Err(NodeNameError::Invisible);
    }
    Ok(())
}

impl TryFrom<String> for NodeName {
    type Error = NodeNameError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        check(&text)?;
        Ok(Self(text))
    }
}

impl TryFrom<&str> for NodeName {
    type Error = NodeNameError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        check(text)?;
        Ok(Self(text.to_owned()))
    }
}

impl FromStr for NodeName {
    type Err = NodeNameError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

#[cfg(test)]
mod tests;

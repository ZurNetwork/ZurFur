use std::{ops::RangeInclusive, str::FromStr};

use combining_marks::is_combining_mark;

mod combining_marks;

/// The most bytes a name may hold.
pub const MAX_NAME_BYTES: usize = 2048;

/// U+200C ZERO WIDTH NON-JOINER, the one format character besides the joiner a name may hold.
const ZERO_WIDTH_NON_JOINER: char = '\u{200C}';

/// U+200D ZERO WIDTH JOINER, the other format character a name may hold.
const ZERO_WIDTH_JOINER: char = '\u{200D}';

/// U+2028 LINE SEPARATOR, refused anywhere in a name.
const LINE_SEPARATOR: char = '\u{2028}';

/// U+FFFC OBJECT REPLACEMENT CHARACTER, refused anywhere in a name: it stands for an object, not
/// text.
const OBJECT_REPLACEMENT: char = '\u{FFFC}';

/// The noncharacters of the Arabic Presentation Forms-A block, refused anywhere in a name.
const ARABIC_NONCHARACTERS: RangeInclusive<char> = '\u{FDD0}'..='\u{FDEF}';

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

/// The Hangul fillers. They are default-ignorable, and stripped wherever they stand.
const HANGUL_FILLERS: &[char] = &['\u{115F}', '\u{1160}', '\u{3164}', '\u{FFA0}'];

/// Characters that show an empty cell yet are neither white space nor default-ignorable: the
/// blank Braille pattern, the null notehead, the Khitan filler and the two Egyptian blanks.
const VISIBLE_FILLERS: &[char] = &[
    '\u{2800}',
    '\u{1D159}',
    '\u{16FE4}',
    '\u{13441}',
    '\u{13442}',
];

/// The zero-width spaces, stripped wherever they stand: U+200B ZERO WIDTH SPACE, U+2060 WORD
/// JOINER and U+FEFF ZERO WIDTH NO-BREAK SPACE.
const ZERO_WIDTH_SPACES: &[char] = &['\u{200B}', '\u{2060}', '\u{FEFF}'];

/// The variation selectors, as inclusive ranges: the Mongolian free variation selectors, VS1 to
/// VS16 and the variation selectors supplement.
const VARIATION_SELECTORS: &[(u32, u32)] = &[
    (0x180B, 0x180D),
    (0x180F, 0x180F),
    (0xFE00, 0xFE0F),
    (0xE0100, 0xE01EF),
];

/// The code points that are `Default_Ignorable_Code_Point` but not yet assigned, as inclusive
/// ranges, from the Unicode Character Database 16.0.0. Refused inside a name, so a format
/// character a later Unicode version assigns there can't get in before the tables are regenerated.
const UNASSIGNED_DEFAULT_IGNORABLE: &[(u32, u32)] = &[
    (0x2065, 0x2065),
    (0xFFF0, 0xFFF8),
    (0xE0000, 0xE0000),
    (0xE0002, 0xE001F),
    (0xE0080, 0xE00FF),
    (0xE01F0, 0xE0FFF),
];

/// U+1F3F4 WAVING BLACK FLAG, which a tag sequence after it makes a subdivision flag.
const BLACK_FLAG: char = '\u{1F3F4}';

/// The tag digits, U+E0030 to U+E0039. With the tag lowercase letters they are the only tags an
/// emoji tag sequence spells a subdivision id with (Unicode Technical Standard #51, Annex C).
const TAG_DIGITS: RangeInclusive<char> = '\u{E0030}'..='\u{E0039}';

/// The tag lowercase letters, U+E0061 to U+E007A.
const TAG_LOWERCASE_LETTERS: RangeInclusive<char> = '\u{E0061}'..='\u{E007A}';

/// How many tags a subdivision id takes: a two-letter region or three-digit code, then one to
/// four letters or digits (Unicode Technical Standard #35, `unicode_subdivision_id`).
const SUBDIVISION_ID_TAGS: RangeInclusive<usize> = 3..=7;

/// U+E007F CANCEL TAG, which ends a subdivision flag's tag sequence.
const CANCEL_TAG: char = '\u{E007F}';

/// U+180E MONGOLIAN VOWEL SEPARATOR, kept inside a Mongolian word.
const MONGOLIAN_VOWEL_SEPARATOR: char = '\u{180E}';

/// The assigned Mongolian letters (Lo and Lm), Todo, Sibe, Manchu and Ali Gali included, as
/// inclusive ranges, from the Unicode Character Database 16.0.0 (unchanged in 17.0.0 and 18.0.0).
/// The combining marks U+1885, U+1886 and U+18A9 and the unassigned U+1879 to U+187F are not
/// letters.
const MONGOLIAN_LETTERS: &[(u32, u32)] = &[
    (0x1820, 0x1878),
    (0x1880, 0x1884),
    (0x1887, 0x18A8),
    (0x18AA, 0x18AA),
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

/// A name safe to show: text with nothing in it that hides, fakes or reorders what a reader
/// sees, and that may hold a `/`. Parsed by hand because the parse is the rule: refused when too
/// long or holding a separator or noncharacter, then stripped of blank and invisible characters,
/// and refused when nothing is left or it holds a control or unkept format character.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct ShowableName(String);

/// Why a piece of text is not a [`ShowableName`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShowableNameError {
    /// The name is empty once its blank characters are stripped. The length and separator rules
    /// are checked first, and win.
    #[error("a name is empty")]
    Empty,
    /// The name is longer than [`MAX_NAME_BYTES`].
    #[error("a name is longer than {MAX_NAME_BYTES} bytes")]
    TooLong,
    /// Once stripped, the name holds a control character, an unassigned default-ignorable, or a
    /// format character the rule doesn't keep.
    #[error("a name holds a control or format character")]
    Invisible,
    /// The name holds a line or paragraph separator.
    #[error("a name holds a line or paragraph separator")]
    Separator,
    /// The name holds a noncharacter or U+FFFC OBJECT REPLACEMENT CHARACTER, which stand for no
    /// text of their own.
    #[error("a name holds a noncharacter or an object replacement character")]
    Reserved,
}

/// A node's display name, taken from what the node holds and never its key: a
/// [`ShowableName`] with no `/`, parsed by hand because the `/` clause is checked after the
/// length rule and before every other.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct NodeName(ShowableName);

/// Why a piece of text is not a [`NodeName`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NodeNameError {
    /// The name is empty once its blank characters are stripped. The length, `/` and separator
    /// rules are checked first, and win.
    #[error("a node name is empty")]
    Empty,
    /// The name is longer than [`MAX_NAME_BYTES`].
    #[error("a node name is longer than {MAX_NAME_BYTES} bytes")]
    TooLong,
    /// The name holds a `/`.
    #[error("a node name holds a `/`")]
    Slash,
    /// Once stripped, the name holds a control character, an unassigned default-ignorable, or a
    /// format character the name rule doesn't keep.
    #[error("a node name holds a control or format character")]
    Invisible,
    /// The name holds a line or paragraph separator.
    #[error("a node name holds a line or paragraph separator")]
    Separator,
    /// The name holds a noncharacter or U+FFFC OBJECT REPLACEMENT CHARACTER, which stand for no
    /// text of their own.
    #[error("a node name holds a noncharacter or an object replacement character")]
    Reserved,
}

impl From<ShowableNameError> for NodeNameError {
    /// The same refusal, as a node name's.
    fn from(error: ShowableNameError) -> Self {
        match error {
            ShowableNameError::Empty => Self::Empty,
            ShowableNameError::TooLong => Self::TooLong,
            ShowableNameError::Invisible => Self::Invisible,
            ShowableNameError::Separator => Self::Separator,
            ShowableNameError::Reserved => Self::Reserved,
        }
    }
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

/// Whether `character` is a variation selector.
fn is_variation_selector(character: char) -> bool {
    let code = u32::from(character);
    VARIATION_SELECTORS
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
}

/// Whether `character` is a joiner or a variation selector, which acts on the character before it.
fn acts_on_previous(character: char) -> bool {
    is_joiner(character) || is_variation_selector(character)
}

/// U+034F COMBINING GRAPHEME JOINER and the Khmer inherent vowels U+17B4 and U+17B5: invisible,
/// and neither a joiner nor a selector, so stripped wherever they stand.
const INVISIBLE_MARKS: &[char] = &['\u{034F}', '\u{17B4}', '\u{17B5}'];

/// Whether `character` is stripped even from inside a name: a filler, a zero-width space or an
/// invisible mark.
fn is_stripped_inside(character: char) -> bool {
    HANGUL_FILLERS.contains(&character)
        || VISIBLE_FILLERS.contains(&character)
        || ZERO_WIDTH_SPACES.contains(&character)
        || INVISIBLE_MARKS.contains(&character)
}

/// Whether `character` is default-ignorable but not yet assigned.
fn is_unassigned_default_ignorable(character: char) -> bool {
    let code = u32::from(character);
    UNASSIGNED_DEFAULT_IGNORABLE
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
}

/// Whether `character` is one of the Mongolian free variation selectors.
fn is_mongolian_selector(character: char) -> bool {
    matches!(character, '\u{180B}'..='\u{180D}' | '\u{180F}')
}

/// Whether `character` is a tag a subdivision id is spelled with: a tag digit or a tag
/// lowercase letter.
fn is_subdivision_tag(character: char) -> bool {
    TAG_DIGITS.contains(&character) || TAG_LOWERCASE_LETTERS.contains(&character)
}

/// The length in bytes of the subdivision tag sequence `rest` starts with: three to seven tag
/// digits or lowercase letters, then the cancel tag. Zero when `rest` starts with none.
fn tag_sequence_len(rest: &str) -> usize {
    let count = rest
        .chars()
        .take_while(|&character| is_subdivision_tag(character))
        .count();
    let letters = rest.chars().take(count).map(char::len_utf8).sum::<usize>();
    let terminated = rest[letters..].starts_with(CANCEL_TAG);
    if terminated && SUBDIVISION_ID_TAGS.contains(&count) {
        letters + CANCEL_TAG.len_utf8()
    } else {
        0
    }
}

/// Whether `character` is an assigned Mongolian letter.
fn is_mongolian_letter(character: char) -> bool {
    let code = u32::from(character);
    MONGOLIAN_LETTERS
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
}

/// Splits `text` into the units the name rule reads: a subdivision flag whole, with its tag
/// sequence, and every other character alone.
fn units(text: &str) -> impl Iterator<Item = &str> {
    let mut offset = 0;
    std::iter::from_fn(move || {
        let rest = &text[offset..];
        let first = rest.chars().next()?;
        let mut len = first.len_utf8();
        if first == BLACK_FLAG {
            len += tag_sequence_len(&rest[len..]);
        }
        offset += len;
        Some(&rest[..len])
    })
}

/// The one character `unit` is, or `None` for a subdivision flag.
fn single(unit: &str) -> Option<char> {
    let mut characters = unit.chars();
    let first = characters.next()?;
    characters.next().is_none().then_some(first)
}

/// Whether `character` is a noncharacter: U+FDD0 to U+FDEF, or the last two code points of a
/// plane.
fn is_noncharacter(character: char) -> bool {
    let code = u32::from(character);
    ARABIC_NONCHARACTERS.contains(&character) || code & 0xFFFE == 0xFFFE
}

/// Whether `character` is refused anywhere in a name as standing for no text: a noncharacter or
/// the object replacement character.
fn is_reserved(character: char) -> bool {
    is_noncharacter(character) || character == OBJECT_REPLACEMENT
}

/// The units of `text` left once blank units are stripped from both ends: from the first unit
/// that is neither blank nor a combining mark with no base before it, to the last unit that isn't
/// blank, with the joiners and selectors right after it. `None` when nothing is left.
fn trim_blank(text: &str) -> Option<Vec<&str>> {
    let all: Vec<&str> = units(text).collect();
    let is_blank_unit = |unit: &&str| single(unit).is_some_and(is_blank);
    let is_lone_mark = |unit: &&str| single(unit).is_some_and(is_combining_mark);
    let first = all
        .iter()
        .position(|unit| !is_blank_unit(unit) && !is_lone_mark(unit))?;
    let last = all.iter().rposition(|unit| !is_blank_unit(unit))?;
    let kept_after = all[last + 1..]
        .iter()
        .take_while(|unit| single(unit).is_some_and(acts_on_previous))
        .count();
    Some(all[first..=last + kept_after].to_vec())
}

/// Whether the vowel separator at `index` in `units` stands inside a Mongolian word: after a
/// Mongolian letter or selector, and before a Mongolian letter.
fn inside_a_mongolian_word(units: &[&str], index: usize) -> bool {
    let previous = index
        .checked_sub(1)
        .and_then(|before| units.get(before))
        .and_then(|unit| single(unit));
    let next = units.get(index + 1).and_then(|unit| single(unit));
    let after_mongolian = previous.is_some_and(|character| {
        is_mongolian_letter(character) || is_mongolian_selector(character)
    });
    let before_a_letter = next.is_some_and(is_mongolian_letter);
    after_mongolian && before_a_letter
}

/// Whether the unit at `index` in the stripped `units` is one a name may not hold.
fn is_invisible(units: &[&str], index: usize) -> bool {
    let Some(character) = single(units[index]) else {
        return false;
    };
    if character == MONGOLIAN_VOWEL_SEPARATOR {
        return !inside_a_mongolian_word(units, index);
    }
    character.is_control()
        || (is_format(character) && !is_joiner(character))
        || is_unassigned_default_ignorable(character)
}

/// What a unit is to a run of joiners and selectors after a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    /// A zero-width joiner or non-joiner.
    Joiner,
    /// A variation selector.
    Selector,
}

/// The kind of modifier `unit` is, or `None` when it is not one.
fn modifier(unit: &str) -> Option<Modifier> {
    let character = single(unit)?;
    if is_joiner(character) {
        return Some(Modifier::Joiner);
    }
    is_variation_selector(character).then_some(Modifier::Selector)
}

/// `units` with the fillers, zero-width spaces and invisible marks stripped, and each gap between
/// two characters that aren't blank cut to its first joiner and its first selector: an emoji or a
/// script needs no more, and more would only hide bytes. Spaces in the gap stay.
fn strip_inside(units: Vec<&str>) -> Vec<&str> {
    let mut kept: Vec<&str> = Vec::with_capacity(units.len());
    let mut joiner_in_gap = false;
    let mut selector_in_gap = false;
    for unit in units {
        if single(unit).is_some_and(is_stripped_inside) {
            continue;
        }
        let seen = match modifier(unit) {
            Some(Modifier::Joiner) => std::mem::replace(&mut joiner_in_gap, true),
            Some(Modifier::Selector) => std::mem::replace(&mut selector_in_gap, true),
            None => false,
        };
        if seen {
            continue;
        }
        if !single(unit).is_some_and(is_blank) {
            joiner_in_gap = false;
            selector_in_gap = false;
        }
        kept.push(unit);
    }
    kept
}

/// Checks `text` against the rule for a name safe to show and returns the stripped form the
/// name holds.
fn normalize(text: &str) -> Result<String, ShowableNameError> {
    if text.len() > MAX_NAME_BYTES {
        return Err(ShowableNameError::TooLong);
    }
    if text.contains([LINE_SEPARATOR, PARAGRAPH_SEPARATOR]) {
        return Err(ShowableNameError::Separator);
    }
    if text.chars().any(is_reserved) {
        return Err(ShowableNameError::Reserved);
    }
    let trimmed = trim_blank(text).ok_or(ShowableNameError::Empty)?;
    let stripped = strip_inside(trimmed);
    if (0..stripped.len()).any(|index| is_invisible(&stripped, index)) {
        return Err(ShowableNameError::Invisible);
    }
    Ok(stripped.concat())
}

/// Checks `text` against the node name rule: the `/` clause, which only the length rule
/// outranks, then the rule for a name safe to show.
fn node_name(text: &str) -> Result<ShowableName, NodeNameError> {
    if text.len() <= MAX_NAME_BYTES && text.contains('/') {
        return Err(NodeNameError::Slash);
    }
    let name = ShowableName::try_from(text)?;
    Ok(name)
}

impl TryFrom<String> for ShowableName {
    type Error = ShowableNameError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        normalize(&text).map(Self)
    }
}

impl TryFrom<&str> for ShowableName {
    type Error = ShowableNameError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        normalize(text).map(Self)
    }
}

impl FromStr for ShowableName {
    type Err = ShowableNameError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

impl TryFrom<String> for NodeName {
    type Error = NodeNameError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        node_name(&text).map(Self)
    }
}

impl TryFrom<&str> for NodeName {
    type Error = NodeNameError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        node_name(text).map(Self)
    }
}

impl FromStr for NodeName {
    type Err = NodeNameError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;

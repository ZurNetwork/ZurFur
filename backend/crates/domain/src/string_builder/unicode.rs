/// The code points of Unicode's general category Cf (format), as inclusive ranges, from the
/// Unicode Character Database 16.0.0.
pub const FORMAT_CHARACTERS: &[(u32, u32)] = &[
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

/// The code points with Unicode's `Default_Ignorable_Code_Point` property, as inclusive ranges,
/// from the Unicode Character Database 16.0.0 (`DerivedCoreProperties.txt`). They render as
/// nothing on their own.
pub const DEFAULT_IGNORABLE: &[(u32, u32)] = &[
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

/// U+2028 LINE SEPARATOR.
pub(super) const LINE_SEPARATOR: char = '\u{2028}';

/// U+2029 PARAGRAPH SEPARATOR.
pub(super) const PARAGRAPH_SEPARATOR: char = '\u{2029}';

/// Whether `code` falls in one of `ranges`.
fn in_ranges(ranges: &[(u32, u32)], code: u32) -> bool {
    ranges
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
}

/// Whether `character` is in Unicode's general category Cf.
pub fn is_format(character: char) -> bool {
    in_ranges(FORMAT_CHARACTERS, u32::from(character))
}

/// Whether `character` has Unicode's `Default_Ignorable_Code_Point` property.
pub fn is_default_ignorable(character: char) -> bool {
    in_ranges(DEFAULT_IGNORABLE, u32::from(character))
}

/// Whether `character` is one of the nine explicit bidirectional formatting characters: the
/// embeddings, overrides and their pop, U+202A to U+202E, and the isolates and their pop, U+2066
/// to U+2069.
pub fn is_bidi_control(character: char) -> bool {
    matches!(character, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Whether printing `character` raw could hide, fake or reorder what a reader sees: a control
/// character (Cc), a format character (Cf), a default-ignorable, or a line or paragraph separator.
pub fn is_unsafe_to_print(character: char) -> bool {
    character.is_control()
        || is_format(character)
        || is_default_ignorable(character)
        || character == LINE_SEPARATOR
        || character == PARAGRAPH_SEPARATOR
}

#[cfg(test)]
mod tests;

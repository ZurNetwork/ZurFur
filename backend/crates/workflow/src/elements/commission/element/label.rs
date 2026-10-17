use std::str::FromStr;

use crate::string_builder::{StringBuilder, StringBuilderViolation};

use super::CompositionLabelError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct CompositionLabel(String);
impl TryFrom<String> for CompositionLabel {
    type Error = CompositionLabelError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let label = StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(LABEL_MAX_CHARS)
            .no_control()
            .build()
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => CompositionLabelError::Empty,
                StringBuilderViolation::TooLong { .. } => CompositionLabelError::TooLong,
                StringBuilderViolation::ControlCharacter => CompositionLabelError::ControlCharacter,
            })?;

        Ok(Self(label))
    }
}
impl FromStr for CompositionLabel {
    type Err = CompositionLabelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The shared length cap of every composition label, in characters.
pub const LABEL_MAX_CHARS: usize = 64;

/// One declared tab's stable name — the `commission_tab.tab` token, e.g.
/// `"main"`. A name, not a key: the row's key is [`TabId`], and which names are
/// legal is the [`SKELETON`]'s to say.
///
/// ```
/// use domain::elements::commission::TabName;
///
/// let tab = "  main  ".parse::<TabName>().unwrap();
/// assert_eq!(tab.as_ref(), "main"); // trimmed
///
/// assert!("   ".parse::<TabName>().is_err()); // empty after trim
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct TabName(CompositionLabel);

/// One declared surface's stable name — the `commission_element.surface` token,
/// e.g. `"content"`. Surfaces have no rows; this type validates the label's
/// shape, while [`SKELETON`] owns the vocabulary.
///
/// ```
/// use domain::elements::commission::SurfaceName;
///
/// let surface = "  content  ".parse::<SurfaceName>().unwrap();
/// assert_eq!(surface.as_ref(), "content"); // trimmed
///
/// assert!("a\nb".parse::<SurfaceName>().is_err()); // control character
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct SurfaceName(CompositionLabel);

/// An element's type tag — what the element is. An open vocabulary in v1: the
/// core stores and returns the tag and never interprets it. [`slot`](Self::slot)
/// and [`seat`](Self::seat) are the two tags already spoken for.
///
/// ```
/// use domain::elements::commission::ElementType;
///
/// assert_eq!("  note ".parse::<ElementType>().unwrap().as_ref(), "note");
/// assert_eq!(ElementType::seat().as_ref(), "seat");
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct ElementType(CompositionLabel);

impl ElementType {
    /// The tag of an element carrying a declared Slot; the `commission_slot`
    /// satellite shares the element's id.
    pub const SLOT_TAG: &'static str = "slot";
    /// The tag of an element carrying a declared Seat; the `commission_seat`
    /// satellite shares the element's id.
    pub const SEAT_TAG: &'static str = "seat";

    /// The type tag of a Slot-carrying element.
    pub fn slot() -> Self {
        Self::SLOT_TAG.parse().expect("a valid label")
    }

    /// The type tag of a Seat-carrying element.
    pub fn seat() -> Self {
        Self::SEAT_TAG.parse().expect("a valid label")
    }
}

/// An element's ordering band within a surface: the run its `position` is
/// counted in, so one surface can hold several independently-ordered sequences.
///
/// ⚠️ Placeholder vocabulary — everything is born [`Band::default`] (`"body"`).
/// Do not add a vocabulary check here ahead of the type-catalog decision.
///
/// ```
/// use domain::elements::commission::Band;
///
/// assert_eq!(Band::default().as_ref(), "body"); // the placeholder default
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct Band(CompositionLabel);

impl Band {
    /// The one band that exists today — the placeholder every element is born into.
    pub const BODY: &'static str = "body";
}

impl Default for Band {
    /// The placeholder band ([`BODY`](Self::BODY)), matching the
    /// `commission_element.band` column default.
    fn default() -> Self {
        Self::BODY.parse().expect("a valid label")
    }
}

#[cfg(test)]
mod tests;

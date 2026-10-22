//! The one registry through which a [`Problem`] names the request fields it
//! refused. A refusal list cannot be empty, and a field's detail is fixed per
//! code, so it can never echo what the caller sent.

use domain::elements::name::NodeNameError;

use super::Problem;
use crate::generated::FieldError;

/// A JSON Pointer in URI-fragment form that points at one member of a request
/// body: `#/title`.
///
/// The member name is the field's ProtoJSON (lowerCamelCase) name as it appears
/// on the wire, such as `title` or `linkedChannel`. A member name becomes a
/// pointer by this rule, not by a conversion, so there is no `From<&str>`.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::AsRef, derive_more::Display)]
#[as_ref(str)]
pub struct FieldPointer(String);

impl FieldPointer {
    /// Points at the request body's member with this ProtoJSON name.
    pub fn member(name: &'static str) -> Self {
        debug_assert!(
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric()),
            "a member name is non-empty ASCII letters and digits, got {name:?}"
        );
        Self(format!("#/{name}"))
    }
}

/// Why one field was refused. Each code has one fixed, plain-words detail.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::IntoStaticStr, strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
#[expect(
    clippy::enum_variant_names,
    reason = "each variant spells its wire token; codes for fields other than names join later"
)]
pub enum FieldCode {
    /// A name that is empty once trimmed, or once the name rule strips what it strips.
    NameEmpty,
    /// A name longer than the name rule allows.
    NameTooLong,
    /// A name that holds a `/`.
    NameSlash,
    /// A name that holds an invisible or direction-changing character.
    NameInvisible,
    /// A name that holds a line or paragraph separator.
    NameSeparator,
    /// A name that holds a character that can't be shown.
    NameReserved,
}

impl FieldCode {
    /// The plain words shown under the field. Fixed per code; never the value sent.
    pub fn detail(self) -> &'static str {
        match self {
            Self::NameEmpty => "This can't be blank.",
            Self::NameTooLong => "This is too long. Shorten it.",
            Self::NameSlash => "Leave out “/”.",
            Self::NameInvisible => "Leave out invisible and direction-changing characters.",
            Self::NameSeparator => "Keep it on one line.",
            Self::NameReserved => "Leave out characters that can't be shown.",
        }
    }
}

impl From<NodeNameError> for FieldCode {
    /// Why the name rule refused a name, whichever field it was typed in.
    fn from(rule: NodeNameError) -> Self {
        match rule {
            NodeNameError::Empty => Self::NameEmpty,
            NodeNameError::TooLong => Self::NameTooLong,
            NodeNameError::Slash => Self::NameSlash,
            NodeNameError::Invisible => Self::NameInvisible,
            NodeNameError::Separator => Self::NameSeparator,
            NodeNameError::Reserved => Self::NameReserved,
        }
    }
}

/// One refused field: where it sits in the request and why it was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRefusal {
    /// The request member that was refused.
    pub pointer: FieldPointer,
    /// Why it was refused.
    pub code: FieldCode,
}

impl From<FieldRefusal> for FieldError {
    /// The wire form: the pointer text, the code's token and the code's fixed detail.
    fn from(refusal: FieldRefusal) -> Self {
        Self {
            pointer: refusal.pointer.to_string(),
            code: refusal.code.to_string(),
            detail: refusal.code.detail().to_owned(),
        }
    }
}

/// The detail of every field refusal problem; each field error says why itself.
const INVALID_FIELDS_DETAIL: &str = "Some fields need changing; each one says why.";

impl Problem {
    /// `422`, code `invalid_request` — the request's fields need changing.
    /// It is [`Problem::invalid_request`] with a fixed detail, plus `errors`,
    /// which lists every refusal in order and is never empty.
    pub fn invalid_fields(
        first: FieldRefusal,
        rest: impl IntoIterator<Item = FieldRefusal>,
    ) -> Self {
        let errors = std::iter::once(first)
            .chain(rest)
            .map(FieldError::from)
            .collect();
        let invalid_request = Self::invalid_request(INVALID_FIELDS_DETAIL);
        Self {
            errors,
            ..invalid_request
        }
    }
}

#[cfg(test)]
mod tests;

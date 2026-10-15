use std::str::FromStr;

use application::den::PageToken;

use super::percent::PercentDecoded;

/// The Den routes' query string: `includeDeleted` and `pageToken`, each at most once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DenParams {
    /// Whether listings also show soft-deleted nodes the viewer may open.
    pub(super) include_deleted: bool,
    /// Where a listing starts; `None` for the first page.
    pub(super) page: Option<PageToken>,
}

/// Why a query string isn't [`DenParams`]. Carries no part of the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(super) enum DenParamsError {
    /// A key that isn't one of the known parameters, after decoding.
    #[error("the query names an unknown parameter")]
    Unknown,
    /// A known parameter given more than once.
    #[error("the query repeats a parameter")]
    Repeated,
    /// A known parameter whose value the route can't use.
    #[error("the query carries a value the route can't use")]
    Malformed,
}

/// The query parameters the Den routes know, spelled as their JSON names.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::IntoStaticStr, strum::VariantArray,
)]
enum Parameter {
    /// Also list soft-deleted nodes the viewer may open.
    #[strum(serialize = "includeDeleted")]
    IncludeDeleted,
    /// Start a listing at an earlier answer's next-page token.
    #[strum(serialize = "pageToken")]
    PageToken,
}

/// One `key=value` pair of the raw query, the value empty when there is no `=`.
fn pair(raw: &str) -> (&str, &str) {
    raw.split_once('=').unwrap_or((raw, ""))
}

/// A raw key, decoded once, as a known parameter.
fn parameter(raw_key: &str) -> Result<Parameter, DenParamsError> {
    let key = raw_key
        .parse::<PercentDecoded>()
        .map_err(|_| DenParamsError::Unknown)?;
    Parameter::from_str(&String::from(key)).map_err(|_| DenParamsError::Unknown)
}

/// A raw value, decoded once.
fn value(raw_value: &str) -> Result<String, DenParamsError> {
    let value = raw_value
        .parse::<PercentDecoded>()
        .map_err(|_| DenParamsError::Malformed)?;
    Ok(String::from(value))
}

impl FromStr for DenParams {
    type Err = DenParamsError;

    /// Parses the raw query: pairs split on `&` (empty ones skipped), each key and value
    /// percent-decoded once. Every key is checked before any value, so an unknown parameter
    /// wins over a bad value wherever it sits.
    fn from_str(query: &str) -> Result<Self, Self::Err> {
        let known = query
            .split('&')
            .filter(|raw| !raw.is_empty())
            .map(pair)
            .map(|(raw_key, raw_value)| Ok((parameter(raw_key)?, raw_value)))
            .collect::<Result<Vec<_>, DenParamsError>>()?;

        let mut include_deleted = None;
        let mut page = None;
        for (parameter, raw_value) in known {
            let value = value(raw_value)?;
            let repeated = match parameter {
                Parameter::IncludeDeleted => {
                    let flag = value
                        .parse::<bool>()
                        .map_err(|_| DenParamsError::Malformed)?;
                    include_deleted.replace(flag).is_some()
                }
                Parameter::PageToken => {
                    let token = value
                        .parse::<PageToken>()
                        .map_err(|_| DenParamsError::Malformed)?;
                    page.replace(token).is_some()
                }
            };
            if repeated {
                return Err(DenParamsError::Repeated);
            }
        }

        let params = Self {
            include_deleted: include_deleted.unwrap_or_default(),
            page,
        };
        Ok(params)
    }
}

#[cfg(test)]
mod tests;

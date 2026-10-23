use axum::{
    extract::{FromRequestParts, RawPathParams},
    http::request::Parts,
};
use domain::elements::{commission::CommissionId, did::Did, user::UserId};
use tower_sessions::Session;

use crate::{SESSION_USER_KEY, problem::Problem};

pub struct CallingUser(pub UserId);
impl<S: Send + Sync> FromRequestParts<S> for CallingUser {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| Problem::not_authenticated())?;

        let did = session
            .get::<Did>(SESSION_USER_KEY)
            .await
            .ok()
            .flatten()
            .ok_or_else(Problem::not_authenticated)?;

        Ok(CallingUser(UserId::from(did)))
    }
}

/// The commission id in the route's `{id}` segment. A missing or unparsable id
/// answers the same 404 as an id that names no commission, and never echoes the
/// input. A segment that isn't UTF-8 gets that 404 only where no other `Path`
/// extractor runs first; one that does answers axum's 400 instead.
pub struct CommissionPath(pub CommissionId);
impl<S: Send + Sync> FromRequestParts<S> for CommissionPath {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(|_| Problem::commission_not_found())?;

        let raw_id = params
            .iter()
            .find_map(|(key, value)| (key == "id").then_some(value))
            .ok_or_else(Problem::commission_not_found)?;

        let commission_id = raw_id
            .parse::<CommissionId>()
            .map_err(|_| Problem::commission_not_found())?;

        Ok(CommissionPath(commission_id))
    }
}

#[cfg(test)]
mod tests;

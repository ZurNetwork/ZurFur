use axum::{extract::FromRequestParts, http::request::Parts};
use domain::elements::{did::Did, user::UserId};
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

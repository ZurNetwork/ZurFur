//! One guarded GET as the resolver sees it: a fetch failure keeps its class,
//! and the status decides between a body, "not found" and "try again".

use jacquard_common::http_client::HttpClient as _;

use domain::ports::ResolveError;

use crate::guarded_http::{FetchError, GuardedHttp, PublicHttpsUrl};

impl From<FetchError> for ResolveError {
    /// A broken fetch rule is `Refused`, a host with no address `NotFound`,
    /// anything that did not finish `Unavailable`.
    fn from(error: FetchError) -> Self {
        match error {
            FetchError::NotFound => Self::NotFound,
            refused @ FetchError::Refused(_) => Self::Refused(anyhow::Error::new(refused)),
            unavailable @ FetchError::Unavailable(_) => {
                Self::Unavailable(anyhow::Error::new(unavailable))
            }
        }
    }
}

/// GET `url` through `http` and return the body of a 2xx answer. A timeout,
/// a rate limit or a 5xx is `Unavailable`; any other answer is `NotFound`.
pub(super) async fn fetch_body(
    http: &GuardedHttp,
    url: &PublicHttpsUrl,
) -> Result<Vec<u8>, ResolveError> {
    let request = http::Request::get(url.as_ref())
        .body(Vec::new())
        .map_err(|error| ResolveError::Refused(anyhow::Error::new(error)))?;
    let response = http.send_http(request).await?;
    successful_body(response)
}

/// The body of a 2xx response, or the error its status means.
fn successful_body(response: http::Response<Vec<u8>>) -> Result<Vec<u8>, ResolveError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response.into_body());
    }
    let worth_retrying = status.is_server_error()
        || status == http::StatusCode::REQUEST_TIMEOUT
        || status == http::StatusCode::TOO_MANY_REQUESTS;
    if worth_retrying {
        let unanswered = anyhow::anyhow!("the host answered {status}");
        return Err(ResolveError::Unavailable(unanswered));
    }
    Err(ResolveError::NotFound)
}

#[cfg(test)]
mod tests;

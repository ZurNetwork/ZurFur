//! [`GuardedHttp`]: the one HTTP client for hosts a visitor or a document names.

use std::{
    error::Error,
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use anyhow::Context as _;
use jacquard_common::http_client::HttpClient;

use super::{
    dns::{AddressLookup, DnsError, FilteringDns, SystemDns},
    errors::{FetchError, Refusal},
    limits::{BodyCap, Timeouts},
    policy::PublicHttpsUrl,
};

/// Lets a host's operator tell who is fetching.
const USER_AGENT: &str = concat!("zurfur-identity/", env!("CARGO_PKG_VERSION"));

/// The guarded client: every URL through the policy, every name through the
/// DNS filter, no redirects, no proxy, no decompression, a capped body and a
/// total timeout. Certificate checks stay reqwest's rustls defaults.
#[derive(Clone)]
pub(crate) struct GuardedHttp {
    client: reqwest::Client,
    #[cfg(test)]
    route: TestRoute,
}

impl GuardedHttp {
    /// The production client over the system's DNS configuration. Errors when
    /// that configuration cannot be read or TLS cannot start. Inherent: it
    /// reads the system, it converts nothing.
    pub(crate) fn new() -> anyhow::Result<Self> {
        let system =
            SystemDns::from_system_conf().context("reading the system DNS configuration")?;
        let client = client_builder(Arc::new(system), Timeouts::default())
            .build()
            .context("building the guarded HTTP client")?;
        Ok(Self {
            client,
            #[cfg(test)]
            route: TestRoute::Strict,
        })
    }

    /// Send `request` under the policy, refusing a body over `cap`. A 3xx is
    /// refused unread; any other status is returned for the caller to judge.
    pub(crate) async fn send_capped(
        &self,
        request: http::Request<Vec<u8>>,
        cap: BodyCap,
    ) -> Result<http::Response<Vec<u8>>, FetchError> {
        let (mut parts, body) = request.into_parts();
        // The checked URL decides where the request goes and the configured
        // identity says who asks; a caller's Host or User-Agent never does.
        parts.headers.remove(http::header::HOST);
        parts.headers.remove(http::header::USER_AGENT);
        let url = self.admit(&parts.uri)?;
        #[cfg(test)]
        let url = self.route.reroute(url, &mut parts.headers).await?;
        self.dispatch(url, parts, body, cap).await
    }

    /// Parse the request's URI once, into the URL that will be dialled.
    fn admit(&self, uri: &http::Uri) -> Result<PublicHttpsUrl, FetchError> {
        let text = uri.to_string();
        #[cfg(test)]
        if let TestRoute::Relaxed = self.route {
            return relaxed_admit(&text);
        }
        let url = PublicHttpsUrl::try_from(text.as_str()).map_err(Refusal::from)?;
        Ok(url)
    }

    /// Send to exactly `url` (no re-parse), then read the body under `cap`.
    async fn dispatch(
        &self,
        url: PublicHttpsUrl,
        parts: http::request::Parts,
        body: Vec<u8>,
        cap: BodyCap,
    ) -> Result<http::Response<Vec<u8>>, FetchError> {
        let mut request = reqwest::Request::new(parts.method, url.into());
        *request.headers_mut() = parts.headers;
        if !body.is_empty() {
            *request.body_mut() = Some(reqwest::Body::from(body));
        }
        let response = self.client.execute(request).await.map_err(classify)?;
        if response.status().is_redirection() {
            return Err(Refusal::Redirect.into());
        }
        read_capped(response, cap).await
    }
}

impl HttpClient for GuardedHttp {
    type Error = FetchError;

    /// Send under the default body cap.
    async fn send_http(
        &self,
        request: http::Request<Vec<u8>>,
    ) -> Result<http::Response<Vec<u8>>, Self::Error> {
        self.send_capped(request, BodyCap::DEFAULT).await
    }
}

/// The reqwest configuration every guarded client shares.
fn client_builder(lookup: Arc<dyn AddressLookup>, timeouts: Timeouts) -> reqwest::ClientBuilder {
    let dns = FilteringDns::new(lookup, timeouts.dns);
    reqwest::Client::builder()
        .dns_resolver(Arc::new(dns))
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .timeout(timeouts.fetch)
        .user_agent(USER_AGENT)
}

impl reqwest::dns::Resolve for FilteringDns {
    /// Hand reqwest only the filtered addresses; the address checked is the
    /// address dialled.
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let filter = self.clone();
        Box::pin(async move {
            let addresses = filter.public_addresses(name.as_str()).await?;
            let socket_addresses: reqwest::dns::Addrs = Box::new(
                addresses
                    .into_iter()
                    .map(|address| SocketAddr::new(IpAddr::from(address), 0)),
            );
            Ok(socket_addresses)
        })
    }
}

/// Read the body chunk by chunk, refusing once it passes `cap`; a declared
/// length over the cap is refused before any byte is read.
async fn read_capped(
    mut response: reqwest::Response,
    cap: BodyCap,
) -> Result<http::Response<Vec<u8>>, FetchError> {
    if response
        .content_length()
        .is_some_and(|length| cap.is_exceeded_by(length))
    {
        return Err(Refusal::BodyTooLarge.into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify)? {
        let total = body.len().saturating_add(chunk.len());
        if cap.is_exceeded_by(u64::try_from(total).unwrap_or(u64::MAX)) {
            return Err(Refusal::BodyTooLarge.into());
        }
        body.extend_from_slice(&chunk);
    }
    let mut out = http::Response::new(body);
    *out.status_mut() = response.status();
    *out.version_mut() = response.version();
    *out.headers_mut() = response.headers().clone();
    Ok(out)
}

/// Sort a transport error into its fetch class. The URL is stripped first.
fn classify(error: reqwest::Error) -> FetchError {
    let error = error.without_url();
    if let Some(dns) = find_in_chain::<DnsError>(&error) {
        return match dns {
            DnsError::Refused(forbidden) => Refusal::Address(*forbidden).into(),
            DnsError::NotFound => FetchError::NotFound,
            DnsError::Unavailable(_) => FetchError::Unavailable(Box::new(error)),
        };
    }
    if error.is_timeout() {
        return FetchError::Unavailable(Box::new(error));
    }
    if error.is_builder() {
        return Refusal::Request(Box::new(error)).into();
    }
    if error.is_connect() && is_tls_failure(&error) {
        return Refusal::Tls(Box::new(error)).into();
    }
    FetchError::Unavailable(Box::new(error))
}

/// `error`, then each cause in turn. An `io::Error` is followed into its
/// payload (`get_ref`), which its own `source()` would skip.
fn causes<'a>(error: &'a (dyn Error + 'static)) -> impl Iterator<Item = &'a (dyn Error + 'static)> {
    std::iter::successors(Some(error), |&current| {
        let payload = current
            .downcast_ref::<io::Error>()
            .and_then(io::Error::get_ref)
            .map(|inner| inner as &(dyn Error + 'static));
        payload.or_else(|| current.source())
    })
}

/// The first `T` among `error`'s causes, `error` itself included.
fn find_in_chain<'a, T: Error + 'static>(error: &'a (dyn Error + 'static)) -> Option<&'a T> {
    causes(error).find_map(|current| current.downcast_ref::<T>())
}

/// The TLS stack reports a failed handshake (a bad certificate included) as an
/// `io::Error` of kind `InvalidData`; only meaningful on a connect error.
fn is_tls_failure(error: &(dyn Error + 'static)) -> bool {
    causes(error)
        .filter_map(|current| current.downcast_ref::<io::Error>())
        .any(|io_error| io_error.kind() == io::ErrorKind::InvalidData)
}

/// Where a test client sends what the policy admitted. Exists only in tests.
#[cfg(test)]
#[derive(Clone)]
enum TestRoute {
    /// Production behaviour.
    Strict,
    /// Any http(s) URL, loopback and any port included, sent as given.
    Relaxed,
    /// The strict policy and the DNS filter run, then the request goes in
    /// plaintext to `upstream`, its `Host` header naming the original host.
    Upstream {
        upstream: SocketAddr,
        dns: FilteringDns,
    },
}

#[cfg(test)]
impl TestRoute {
    /// Point an upstream route's request at the plaintext upstream; any other
    /// route passes `url` through.
    async fn reroute(
        &self,
        url: PublicHttpsUrl,
        headers: &mut http::HeaderMap,
    ) -> Result<PublicHttpsUrl, FetchError> {
        let Self::Upstream { upstream, dns } = self else {
            return Ok(url);
        };
        let mut target = url::Url::from(url);
        let host = target.host_str().unwrap_or_default().to_owned();
        dns.public_addresses(&host).await?;
        let host_header =
            http::HeaderValue::from_str(&host).expect("a policy-checked host is a header value");
        headers.insert(http::header::HOST, host_header);
        target
            .set_scheme("http")
            .expect("https to http keeps a special scheme");
        target
            .set_ip_host(upstream.ip())
            .expect("an http URL takes an IP host");
        target
            .set_port(Some(upstream.port()))
            .expect("an http URL takes a port");
        Ok(PublicHttpsUrl::unchecked(target))
    }
}

/// The relaxed route's admission: any http(s) URL.
#[cfg(test)]
fn relaxed_admit(text: &str) -> Result<PublicHttpsUrl, FetchError> {
    use super::policy::UrlPolicyError;

    let url = url::Url::parse(text).map_err(|_| Refusal::Url(UrlPolicyError::NotUrl))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Refusal::Url(UrlPolicyError::NotHttps).into());
    }
    Ok(PublicHttpsUrl::unchecked(url))
}

#[cfg(test)]
impl GuardedHttp {
    /// The production policy over scripted DNS.
    pub(crate) fn scripted(lookup: Arc<dyn AddressLookup>, timeouts: Timeouts) -> Self {
        let client = client_builder(lookup, timeouts)
            .build()
            .expect("the test client builds");
        Self {
            client,
            route: TestRoute::Strict,
        }
    }

    /// Loopback, plain http and any port allowed; names resolve to nothing.
    pub(crate) fn relaxed(timeouts: Timeouts) -> Self {
        let lookup = Arc::new(super::scripted::ScriptedLookup::default());
        let client = client_builder(lookup, timeouts)
            .https_only(false)
            .build()
            .expect("the test client builds");
        Self {
            client,
            route: TestRoute::Relaxed,
        }
    }

    /// The production policy and DNS filter over scripted DNS, every admitted
    /// request then sent in plaintext to `upstream` with its `Host` kept.
    pub(crate) fn upstream(
        upstream: SocketAddr,
        lookup: Arc<dyn AddressLookup>,
        timeouts: Timeouts,
    ) -> Self {
        let dns = FilteringDns::new(lookup.clone(), timeouts.dns);
        let client = client_builder(lookup, timeouts)
            .https_only(false)
            .build()
            .expect("the test client builds");
        Self {
            client,
            route: TestRoute::Upstream { upstream, dns },
        }
    }
}

#[cfg(test)]
mod tests;

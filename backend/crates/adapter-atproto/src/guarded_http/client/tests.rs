use std::{net::SocketAddr, time::Duration};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::guarded_http::{
    limits::FETCH_TIMEOUT,
    policy::UrlPolicyError,
    scripted::{Script, ScriptedLookup},
};

/// A body-less GET for `url`.
fn get(url: &str) -> http::Request<Vec<u8>> {
    http::Request::get(url)
        .body(Vec::new())
        .expect("test request builds")
}

/// Timeouts short enough that a hang fails the test quickly.
fn short_timeouts() -> Timeouts {
    Timeouts {
        fetch: Duration::from_secs(2),
        dns: Duration::from_millis(200),
    }
}

/// How many requests `server` has seen for `request_path`.
async fn hits(server: &MockServer, request_path: &str) -> usize {
    let requests = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| request.url.path() == request_path)
        .count()
}

/// What a raw server sends after its response head.
enum RawBody {
    /// Nothing, while holding the connection open.
    Withheld,
    /// Chunked encoding, 16 KiB chunks, until the client hangs up.
    EndlessChunks,
    /// These exact bytes, then the connection held open.
    Bytes(String),
}

/// A one-connection plain-http server answering with `head`, then `body`.
async fn raw_server(head: &'static str, body: RawBody) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut request = [0u8; 4096];
        let _ = stream.read(&mut request).await;
        if stream.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        match body {
            RawBody::Withheld => std::future::pending::<()>().await,
            RawBody::EndlessChunks => {
                let chunk = format!("4000\r\n{}\r\n", "a".repeat(0x4000));
                while stream.write_all(chunk.as_bytes()).await.is_ok() {}
            }
            RawBody::Bytes(bytes) => {
                if stream.write_all(bytes.as_bytes()).await.is_ok() {
                    std::future::pending::<()>().await;
                }
            }
        }
    });
    address
}

#[tokio::test]
async fn a_small_200_passes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
        .mount(&server)
        .await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let response = http
        .send_http(get(&format!("{}/ok", server.uri())))
        .await
        .expect("a small 200 passes");

    let expected_body = b"hello".to_vec();
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.into_body(), expected_body);
}

#[tokio::test]
async fn any_other_status_is_passed_back_for_the_caller() {
    let server = MockServer::start().await;
    Mock::given(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(path("/broken"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let missing = http
        .send_http(get(&format!("{}/missing", server.uri())))
        .await
        .expect("a 404 is a response");
    let broken = http
        .send_http(get(&format!("{}/broken", server.uri())))
        .await
        .expect("a 500 is a response");

    assert_eq!(missing.status(), http::StatusCode::NOT_FOUND);
    assert_eq!(broken.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn no_redirect_is_ever_followed() {
    let server = MockServer::start().await;
    let elsewhere = MockServer::start().await;
    Mock::given(path("/target"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(path("/target"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&elsewhere)
        .await;
    let locations = [
        format!("{}/target", elsewhere.uri()),
        "https://169.254.169.254/latest/meta-data/".to_string(),
        format!("{}/target", server.uri()),
    ];
    let http = GuardedHttp::relaxed(short_timeouts());

    let mut followed = Vec::new();
    for status in [301u16, 302, 307, 308] {
        for location in &locations {
            server.reset().await;
            Mock::given(path("/start"))
                .respond_with(
                    ResponseTemplate::new(status).insert_header("location", location.as_str()),
                )
                .mount(&server)
                .await;

            let result = http
                .send_http(get(&format!("{}/start", server.uri())))
                .await;

            if !matches!(result, Err(FetchError::Refused(Refusal::Redirect))) {
                followed.push(format!("{status} -> {location}: {result:?}"));
            }
        }
    }

    let none: Vec<String> = Vec::new();
    assert_eq!(followed, none, "every redirect must be refused");
    assert_eq!(hits(&server, "/target").await, 0);
    assert_eq!(hits(&elsewhere, "/target").await, 0);
}

#[tokio::test]
async fn a_declared_length_over_the_cap_is_refused_before_reading() {
    // The server declares 10 MiB and then sends nothing: reading would hang
    // until the timeout, so a refusal proves nothing was read.
    let head = "HTTP/1.1 200 OK\r\ncontent-length: 10485760\r\n\r\n";
    let address = raw_server(head, RawBody::Withheld).await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let result = http.send_http(get(&format!("http://{address}/"))).await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::BodyTooLarge))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn a_chunked_body_over_the_cap_is_refused_at_the_cap() {
    let head = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n";
    let address = raw_server(head, RawBody::EndlessChunks).await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let result = http.send_http(get(&format!("http://{address}/"))).await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::BodyTooLarge))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn a_raised_cap_accepts_a_body_the_default_refuses() {
    let server = MockServer::start().await;
    let body = vec![b'a'; 700 * 1024];
    Mock::given(path("/status-list"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .mount(&server)
        .await;
    let http = GuardedHttp::relaxed(short_timeouts());
    let url = format!("{}/status-list", server.uri());
    let one_mebibyte = BodyCap::from(1024 * 1024);

    let raised = http
        .send_capped(get(&url), one_mebibyte)
        .await
        .expect("1 MiB accepts 700 KiB");
    let default = http.send_http(get(&url)).await;

    assert_eq!(raised.into_body(), body);
    assert!(
        matches!(default, Err(FetchError::Refused(Refusal::BodyTooLarge))),
        "got: {default:?}"
    );
}

#[tokio::test]
async fn a_response_slower_than_the_timeout_is_unavailable() {
    let server = MockServer::start().await;
    Mock::given(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let timeouts = Timeouts {
        fetch: Duration::from_millis(200),
        dns: Duration::from_millis(200),
    };
    let http = GuardedHttp::relaxed(timeouts);

    let result = http.send_http(get(&format!("{}/slow", server.uri()))).await;

    assert!(
        matches!(result, Err(FetchError::Unavailable(_))),
        "got: {result:?}"
    );
}

#[test]
fn the_production_fetch_timeout_is_ten_seconds() {
    let ten_seconds = Duration::from_secs(10);

    assert_eq!(FETCH_TIMEOUT, ten_seconds);
}

#[test]
fn the_default_cap_is_512_kib() {
    let half_mebibyte = BodyCap::from(512 * 1024);

    assert_eq!(BodyCap::DEFAULT, half_mebibyte);
}

#[tokio::test]
async fn the_production_policy_refuses_the_mock_and_sends_nothing() {
    let server = MockServer::start().await;
    Mock::given(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let http = GuardedHttp::scripted(Arc::new(ScriptedLookup::default()), short_timeouts());

    let result = http.send_http(get(&format!("{}/ok", server.uri()))).await;

    assert!(
        matches!(
            result,
            Err(FetchError::Refused(Refusal::Url(UrlPolicyError::NotHttps)))
        ),
        "got: {result:?}"
    );
    assert_eq!(hits(&server, "/ok").await, 0);
}

#[tokio::test]
async fn the_production_policy_refuses_every_ip_literal_before_any_dns() {
    let lookup = Arc::new(ScriptedLookup::default());
    let http = GuardedHttp::scripted(lookup.clone(), short_timeouts());

    let result = http.send_http(get("https://0x7f.1/")).await;

    assert!(
        matches!(
            result,
            Err(FetchError::Refused(Refusal::Url(UrlPolicyError::IpLiteral)))
        ),
        "got: {result:?}"
    );
    assert_eq!(lookup.asked(), Vec::<String>::new());
}

#[tokio::test]
async fn a_gzip_encoded_response_is_not_decoded() {
    let server = MockServer::start().await;
    // Not valid gzip: a client that tried to decode it would fail.
    let raw = b"raw bytes, never inflated".to_vec();
    Mock::given(path("/encoded"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_bytes(raw.clone()),
        )
        .mount(&server)
        .await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let response = http
        .send_http(get(&format!("{}/encoded", server.uri())))
        .await
        .expect("served as is");

    assert_eq!(response.into_body(), raw);
    let requests = server.received_requests().await.unwrap_or_default();
    let asked_for_encoding = requests
        .iter()
        .any(|request| request.headers.contains_key("accept-encoding"));
    assert!(
        !asked_for_encoding,
        "the client must not ask for compression"
    );
}

#[tokio::test]
async fn requests_carry_the_identity_user_agent_and_never_the_callers_host_or_agent() {
    let server = MockServer::start().await;
    Mock::given(path("/ua"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let http = GuardedHttp::relaxed(short_timeouts());
    let request = http::Request::get(format!("{}/ua", server.uri()))
        .header(http::header::HOST, "internal.example")
        .header(http::header::USER_AGENT, "someone-else/1.0")
        .body(Vec::new())
        .expect("test request builds");

    http.send_http(request).await.expect("sent");

    let requests = server.received_requests().await.unwrap_or_default();
    let sent = requests.first().expect("one request");
    let user_agent = sent.headers.get("user-agent").expect("a user agent");
    let host = sent.headers.get("host").expect("a host");
    let expected_user_agent = format!("zurfur-identity/{}", env!("CARGO_PKG_VERSION"));
    assert_eq!(user_agent.to_str().ok(), Some(expected_user_agent.as_str()));
    assert_ne!(host.to_str().ok(), Some("internal.example"));
}

// --- the DNS filter, wired into the client (scripted answers, no network) ---

/// The strict client over a script answering `pds.example.com.` with `script`.
fn strict_over(script: Script) -> (GuardedHttp, Arc<ScriptedLookup>) {
    let lookup = Arc::new(ScriptedLookup::default().with("pds.example.com.", script));
    let http = GuardedHttp::scripted(lookup.clone(), short_timeouts());
    (http, lookup)
}

fn answer(texts: &[&str]) -> Script {
    let addresses = texts
        .iter()
        .map(|text| text.parse().expect("test address parses"))
        .collect();
    Script::Answer(addresses)
}

#[tokio::test]
async fn a_host_resolving_to_a_private_address_is_refused_at_connect() {
    let (http, lookup) = strict_over(answer(&["10.0.0.1"]));

    let result = http.send_http(get("https://pds.example.com/")).await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::Address(_)))),
        "got: {result:?}"
    );
    let expected_asked = vec!["pds.example.com.".to_string()];
    assert_eq!(lookup.asked(), expected_asked);
}

#[tokio::test]
async fn a_host_with_one_private_answer_among_public_ones_is_refused() {
    let (http, _) = strict_over(answer(&["93.184.216.34", "127.0.0.1"]));

    let result = http.send_http(get("https://pds.example.com/")).await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::Address(_)))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn a_host_with_no_address_is_not_found() {
    let (http, _) = strict_over(Script::Answer(Vec::new()));

    let result = http.send_http(get("https://pds.example.com/")).await;

    assert!(
        matches!(result, Err(FetchError::NotFound)),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn a_failed_or_slow_lookup_is_unavailable() {
    let (failing, _) = strict_over(Script::Fail);
    let (hanging, _) = strict_over(Script::Hang);

    let failed = failing.send_http(get("https://pds.example.com/")).await;
    let slow = hanging.send_http(get("https://pds.example.com/")).await;

    assert!(
        matches!(failed, Err(FetchError::Unavailable(_))),
        "got: {failed:?}"
    );
    assert!(
        matches!(slow, Err(FetchError::Unavailable(_))),
        "got: {slow:?}"
    );
}

// --- the plaintext test upstream ---

#[tokio::test]
async fn the_upstream_route_delivers_an_admitted_request_with_its_host() {
    let server = MockServer::start().await;
    Mock::given(path("/.well-known/atproto-did"))
        .respond_with(ResponseTemplate::new(200).set_body_string("did:plc:abc"))
        .mount(&server)
        .await;
    let upstream = *server.address();
    let lookup =
        Arc::new(ScriptedLookup::default().with("alice.example.com.", answer(&["93.184.216.34"])));
    let http = GuardedHttp::upstream(upstream, lookup, short_timeouts());

    let response = http
        .send_http(get("https://alice.example.com/.well-known/atproto-did"))
        .await
        .expect("delivered");

    assert_eq!(response.into_body(), b"did:plc:abc".to_vec());
    let requests = server.received_requests().await.unwrap_or_default();
    let host = requests
        .first()
        .and_then(|request| request.headers.get("host"))
        .and_then(|value| value.to_str().ok());
    assert_eq!(host, Some("alice.example.com"));
}

#[tokio::test]
async fn the_upstream_route_still_refuses_a_private_address_and_sends_nothing() {
    let server = MockServer::start().await;
    Mock::given(path("/.well-known/atproto-did"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let upstream = *server.address();
    let lookup =
        Arc::new(ScriptedLookup::default().with("alice.example.com.", answer(&["10.0.0.1"])));
    let http = GuardedHttp::upstream(upstream, lookup, short_timeouts());

    let result = http
        .send_http(get("https://alice.example.com/.well-known/atproto-did"))
        .await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::Address(_)))),
        "got: {result:?}"
    );
    assert_eq!(hits(&server, "/.well-known/atproto-did").await, 0);
}

// --- the body cap at its boundary ---

#[tokio::test]
async fn a_declared_length_of_exactly_the_cap_passes_and_one_more_byte_is_refused() {
    let server = MockServer::start().await;
    Mock::given(path("/ten"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 10]))
        .mount(&server)
        .await;
    Mock::given(path("/eleven"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 11]))
        .mount(&server)
        .await;
    let http = GuardedHttp::relaxed(short_timeouts());
    let ten_bytes = BodyCap::from(10);

    let at_cap = http
        .send_capped(get(&format!("{}/ten", server.uri())), ten_bytes)
        .await;
    let over_cap = http
        .send_capped(get(&format!("{}/eleven", server.uri())), ten_bytes)
        .await;

    assert!(at_cap.is_ok(), "got: {at_cap:?}");
    assert!(
        matches!(over_cap, Err(FetchError::Refused(Refusal::BodyTooLarge))),
        "got: {over_cap:?}"
    );
}

/// A server sending `payload` as one chunk, with no declared length.
async fn chunked_server(payload: &str) -> SocketAddr {
    let head = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n";
    let body = format!("{:x}\r\n{payload}\r\n0\r\n\r\n", payload.len());
    raw_server(head, RawBody::Bytes(body)).await
}

#[tokio::test]
async fn an_undeclared_body_of_exactly_the_cap_passes_and_one_more_byte_is_refused() {
    let at_cap_server = chunked_server("0123456789").await;
    let over_cap_server = chunked_server("0123456789x").await;
    let http = GuardedHttp::relaxed(short_timeouts());
    let ten_bytes = BodyCap::from(10);

    let at_cap = http
        .send_capped(get(&format!("http://{at_cap_server}/")), ten_bytes)
        .await;
    let over_cap = http
        .send_capped(get(&format!("http://{over_cap_server}/")), ten_bytes)
        .await;

    let expected_body = b"0123456789".to_vec();
    assert_eq!(
        at_cap.expect("exactly the cap passes").into_body(),
        expected_body
    );
    assert!(
        matches!(over_cap, Err(FetchError::Refused(Refusal::BodyTooLarge))),
        "got: {over_cap:?}"
    );
}

// --- the transport locks behind the policy ---

#[tokio::test]
async fn the_transport_itself_refuses_plain_http() {
    let server = MockServer::start().await;
    Mock::given(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let http = GuardedHttp::scripted(Arc::new(ScriptedLookup::default()), short_timeouts());
    let target = url::Url::parse(&format!("{}/ok", server.uri())).expect("mock url parses");
    // Straight to the reqwest client, past the URL policy: the second lock alone.
    let request = reqwest::Request::new(http::Method::GET, target);

    let result = http.client.execute(request).await;

    assert!(result.is_err(), "got: {result:?}");
    assert_eq!(hits(&server, "/ok").await, 0);
}

#[test]
fn the_transport_carries_no_proxy_and_the_no_redirect_policy() {
    let guarded = GuardedHttp::scripted(Arc::new(ScriptedLookup::default()), short_timeouts());
    // A default client always carries the system-proxy matcher, so the
    // field's absence is `no_proxy` at work, not an empty environment.
    let control = reqwest::Client::new();

    let guarded_shape = format!("{:?}", guarded.client);
    let control_shape = format!("{control:?}");

    assert!(
        control_shape.contains("proxies"),
        "control: {control_shape}"
    );
    assert!(
        !guarded_shape.contains("proxies"),
        "guarded: {guarded_shape}"
    );
    // reqwest prints the policy only when it is not the default, so this
    // pins `Policy::none()` whatever spelling built it.
    assert!(
        guarded_shape.contains("redirect_policy: \"Policy(None)\""),
        "guarded: {guarded_shape}"
    );
}

// --- TLS failures, end to end ---

#[tokio::test]
async fn a_failed_tls_handshake_is_refused() {
    // A plain-HTTP server answers the ClientHello with an HTTP response.
    let head = "HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n";
    let address = raw_server(head, RawBody::Withheld).await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let result = http.send_http(get(&format!("https://{address}/"))).await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::Tls(_)))),
        "got: {result:?}"
    );
}

/// A one-connection TLS server presenting the untrusted fixture certificate.
async fn untrusted_tls_server() -> SocketAddr {
    use tokio_rustls::rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject as _},
    };

    let certificate = CertificateDer::from_pem_slice(include_bytes!("untrusted_cert.pem"))
        .expect("fixture certificate parses");
    let key = PrivateKeyDer::from_pem_slice(include_bytes!("untrusted_key.pem"))
        .expect("fixture key parses");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .expect("server config");
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let _ = acceptor.accept(stream).await;
    });
    address
}

#[tokio::test]
async fn an_untrusted_certificate_is_refused() {
    let address = untrusted_tls_server().await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let result = http.send_http(get(&format!("https://{address}/"))).await;

    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::Tls(_)))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn a_garbled_body_is_not_a_tls_failure() {
    // A newline inside a chunk extension: hyper rejects it as InvalidData.
    let head = "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n";
    let body = RawBody::Bytes("5;a\nhello\r\n0\r\n\r\n".to_string());
    let address = raw_server(head, body).await;
    let http = GuardedHttp::relaxed(short_timeouts());

    let result = http.send_http(get(&format!("http://{address}/"))).await;

    assert!(
        matches!(result, Err(FetchError::Unavailable(_))),
        "got: {result:?}"
    );
}

#[test]
fn a_fetch_error_never_echoes_the_host() {
    let errors = [
        FetchError::from(Refusal::Redirect),
        FetchError::NotFound,
        FetchError::Unavailable("timed out".into()),
    ];

    let echoed: Vec<String> = errors
        .iter()
        .map(ToString::to_string)
        .filter(|text| text.contains("example"))
        .collect();

    assert_eq!(echoed, Vec::<String>::new());
}

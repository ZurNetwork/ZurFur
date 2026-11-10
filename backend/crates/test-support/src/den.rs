//! What the den route suites read an answer by, shared by the in-memory and the PostgreSQL
//! setups: a request helper, the cache header every answer carries, a JSON body's entry
//! names, and an answer's every client-visible byte.

use serde_json::Value;

/// The cache header every Den answer carries.
pub const PRIVATE_NO_STORE: &str = "private, no-store";

/// `GET {base}{path}` as `client`.
pub async fn get(client: &reqwest::Client, base: &str, path: &str) -> reqwest::Response {
    client
        .get(format!("{base}{path}"))
        .send()
        .await
        .expect("the request is answered")
}

/// `PATCH {base}{path}` as `client`, with `title` as the body's `title`.
pub async fn patch_title(
    client: &reqwest::Client,
    base: &str,
    path: &str,
    title: &str,
) -> reqwest::Response {
    let request = serde_json::json!({ "title": title });
    client
        .patch(format!("{base}{path}"))
        .json(&request)
        .send()
        .await
        .expect("the request is answered")
}

/// A `200` answer's JSON body, after checking its cache header.
pub async fn ok_json(response: reqwest::Response) -> Value {
    assert_eq!(response.status(), 200, "{}", response.url());
    assert_eq!(cache_control(&response), PRIVATE_NO_STORE);
    response.json().await.expect("a JSON body")
}

/// The answer's `Cache-Control` header.
pub fn cache_control(response: &reqwest::Response) -> String {
    let header = response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .expect("a Cache-Control header");
    header.to_str().expect("an ASCII header").to_owned()
}

/// The names of a listing's entries, in order.
pub fn entry_names(listing: &Value) -> Vec<String> {
    let entries = listing["entries"].as_array().expect("an entries array");
    entries
        .iter()
        .map(|entry| entry["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// Everything a client can tell an answer by: its status, every header but `date`, sorted,
/// and its body's bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

impl Answer {
    /// Reads `response` whole.
    pub async fn read(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let mut headers: Vec<(String, Vec<u8>)> = response
            .headers()
            .iter()
            .filter(|(name, _)| *name != reqwest::header::DATE)
            .map(|(name, value)| (name.to_string(), value.as_bytes().to_vec()))
            .collect();
        headers.sort();
        let body = response.bytes().await.expect("a body").to_vec();
        Self {
            status,
            headers,
            body,
        }
    }
}

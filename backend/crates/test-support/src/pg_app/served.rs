use std::sync::Arc;

use adapter_pg::PgSessionStore;
use axum::Router;
use base64::Engine as _;
use composition::{Config, EXAMPLE_DEV_ROOT_KEY, Environment, Runtime};
use domain::elements::profile::Profile;
use tower_sessions::cookie::{SameSite, time::Duration};
use tower_sessions::{Expiry, SessionManagerLayer};

use super::InProcessPds;
use crate::http::client;
use crate::pg::{TestDb, fresh_db};

/// A running app on its own migrated PostgreSQL database, plus what a test
/// seeds and inspects it through.
pub struct PgServed {
    pub base_url: String,
    /// The live runtime the app serves: PostgreSQL stores over `runtime.pool`,
    /// for seeding through use cases and reading rows back.
    pub runtime: Runtime,
    /// The in-process PDS behind the sign-in and profile ports.
    pub pds: Arc<InProcessPds>,
    /// Keeps the private database, and the shared container, alive.
    _db: TestDb,
}

/// Boots `app` on the composition root's live wiring over a fresh migrated
/// database, with the sign-in and profile ports answered by an
/// [`InProcessPds`] and sessions kept in PostgreSQL.
pub async fn serve(app: impl FnOnce(Runtime) -> Router) -> PgServed {
    let db = fresh_db().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));

    let pds = Arc::new(InProcessPds::default());
    let live = Runtime::connect(config(&base_url, db.url()))
        .await
        .expect("the live runtime connects to the test database");
    let runtime = Runtime {
        auth: pds.clone(),
        profile_source: pds.clone(),
        ..live
    };

    let sessions = SessionManagerLayer::new(PgSessionStore::new(runtime.pool.clone()))
        .with_name("zurfur.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(7)));
    let router = app(runtime.clone()).layer(sessions);
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    PgServed {
        base_url,
        runtime,
        pds,
        _db: db,
    }
}

impl PgServed {
    /// Registers `profile` with the in-process PDS, then signs it in through
    /// the app's own sign-in routes. Returns a client holding the session.
    pub async fn sign_in(&self, profile: Profile) -> reqwest::Client {
        let handle = profile.handle.to_string();
        self.pds.register(profile);
        let browser = client();

        let started = browser
            .post(format!("{}/signin", self.base_url))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("handle={handle}"))
            .send()
            .await
            .expect("POST /signin");
        assert_eq!(started.status(), 303, "signin should redirect to the PDS");
        let authorization_url = started
            .headers()
            .get("location")
            .expect("signin redirects somewhere")
            .to_str()
            .expect("the redirect is text")
            .to_string();

        let callback = browser
            .get(format!("{}{authorization_url}", self.base_url))
            .send()
            .await
            .expect("GET the sign-in callback");
        assert_eq!(
            callback.status(),
            303,
            "callback should redirect on success"
        );
        let landing = callback
            .headers()
            .get("location")
            .expect("the callback redirects somewhere");
        assert_eq!(landing, "/", "sign-in lands on the home page, not an error");

        browser
    }
}

/// A dev-profile [`Config`] on `database_url`, served at `public_url`: the
/// shipped example root key with PLC submission off, so minting stays local.
fn config(public_url: &str, database_url: &str) -> Config {
    let root_key = base64::engine::general_purpose::STANDARD.encode(EXAMPLE_DEV_ROOT_KEY);
    Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().expect("loopback socket address"),
        public_url: public_url.to_string(),
        database_url: database_url.to_string(),
        log_level: "info".to_string(),
        handle_domain: "zurfur.app".parse().expect("a valid handle domain"),
        did_key_root_key: root_key,
        plc_directory_endpoint: "https://plc.directory".to_string(),
        plc_directory_submit: false,
        deadline_sweep_interval_secs: 60,
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
    }
}

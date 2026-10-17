use std::sync::Arc;

use adapter_pg::PgSessionStore;
use axum::Router;
use base64::Engine as _;
use composition::{Config, EXAMPLE_DEV_ROOT_KEY, Environment, Runtime};
use domain::elements::profile::Profile;
use http_cookies::ZurfurCookie;
use http_stack::{Detach, Stacked};
use reqwest::header::SET_COOKIE;

use super::{InProcessPds, StatementLog, record_statements};
use crate::http::client_with_origin;
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
    /// The detach layer the server runs its requests in.
    pub detach: Detach,
    /// Keeps the private database, and the shared container, alive.
    db: TestDb,
}

/// A user signed in through the app's routes: the client holding the session, and the raw
/// `zurfur.sid` value, for a caller that hands the session on as a browser user would.
pub struct SignedIn {
    /// A client whose cookie jar carries the session.
    pub client: reqwest::Client,
    /// The `zurfur.sid` cookie's value, as devtools shows it.
    pub session_cookie: String,
}

/// Boots `app` on the composition root's live wiring over a fresh migrated
/// database, with the sign-in and profile ports answered by an
/// [`InProcessPds`] and sessions kept in PostgreSQL.
pub async fn serve(app: impl FnOnce(Runtime) -> Router) -> PgServed {
    serve_through(app, None, |stacked| stacked, |_| {}).await
}

/// As [`serve`], with `configure` changing the dev [`Config`] before the runtime connects:
/// a test that plants its own root key, say.
pub async fn serve_configured(
    app: impl FnOnce(Runtime) -> Router,
    configure: impl FnOnce(&mut Config),
) -> PgServed {
    serve_through(app, None, |stacked| stacked, configure).await
}

/// As [`serve`], with `wrap` adding layers outside the whole stack.
pub async fn serve_wrapped(
    app: impl FnOnce(Runtime) -> Router,
    wrap: impl FnOnce(Stacked) -> Stacked,
) -> PgServed {
    serve_through(app, None, wrap, |_| {}).await
}

/// As [`serve`], with the app reaching its database through a proxy that logs into
/// `statements` every statement PostgreSQL is asked to execute, seeding included.
pub async fn serve_recording(
    app: impl FnOnce(Runtime) -> Router,
    statements: &StatementLog,
) -> PgServed {
    serve_through(app, Some(statements), |stacked| stacked, |_| {}).await
}

/// Boots `app` as [`serve`] says, through the recording proxy when `statements` is given,
/// with `wrap` adding layers outside the whole stack and `configure` changing the config.
async fn serve_through(
    app: impl FnOnce(Runtime) -> Router,
    statements: Option<&StatementLog>,
    wrap: impl FnOnce(Stacked) -> Stacked,
    configure: impl FnOnce(&mut Config),
) -> PgServed {
    let db = fresh_db().await;
    let database_url = match statements {
        Some(log) => record_statements(db.url(), log.clone()).await,
        None => db.url().to_owned(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));

    let pds = Arc::new(InProcessPds::default());
    let mut config = config(&base_url, &database_url);
    configure(&mut config);
    let live = Runtime::connect(config)
        .await
        .expect("the live runtime connects to the test database");
    let runtime = Runtime {
        auth: pds.clone(),
        profile_source: pds.clone(),
        ..live
    };

    let sessions = http_cookies::session_layer(PgSessionStore::new(runtime.pool.clone()), false);
    let stacked = wrap(http_stack::stack(
        app(runtime.clone()),
        sessions,
        Detach::default(),
    ));
    let detach = stacked.detach().clone();
    tokio::spawn(async move {
        http_stack::serve(listener, stacked, std::future::pending())
            .await
            .unwrap();
    });

    PgServed {
        base_url,
        runtime,
        pds,
        detach,
        db,
    }
}

/// The composition root's live wiring over a fresh migrated database, served
/// nowhere: for a test that drives use cases on PostgreSQL directly.
pub struct PgRuntime {
    /// The live runtime: PostgreSQL stores over `runtime.pool`.
    pub runtime: Runtime,
    /// Keeps the private database, and the shared container, alive while held.
    _db: TestDb,
}

/// A [`PgRuntime`] with the sign-in and profile ports answered by an [`InProcessPds`].
pub async fn pg_runtime() -> PgRuntime {
    let db = fresh_db().await;
    let pds = Arc::new(InProcessPds::default());
    let live = Runtime::connect(config("http://localhost", db.url()))
        .await
        .expect("the live runtime connects to the test database");
    let runtime = Runtime {
        auth: pds.clone(),
        profile_source: pds,
        ..live
    };
    PgRuntime { runtime, _db: db }
}

impl PgServed {
    /// Registers `profile` with the in-process PDS, then signs it in through
    /// the app's own sign-in routes. Returns a client holding the session,
    /// which sends `Origin: {base_url}` on every request.
    pub async fn sign_in(&self, profile: Profile) -> reqwest::Client {
        let signed_in = self.sign_in_with_cookie(profile).await;
        signed_in.client
    }

    /// The command line that runs `psql` on this app's database inside its container.
    pub fn psql_command(&self) -> String {
        self.db.psql_command()
    }

    /// As [`PgServed::sign_in`], also returning the `zurfur.sid` value the callback set.
    pub async fn sign_in_with_cookie(&self, profile: Profile) -> SignedIn {
        let handle = profile.handle.to_string();
        self.pds.register(profile);
        let browser = client_with_origin(&self.base_url);

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
        let home = format!("{}/", self.base_url);
        assert_eq!(
            landing,
            home.as_str(),
            "sign-in lands on the home page, not an error"
        );
        let session_cookie =
            set_session_cookie(&callback).expect("the sign-in callback sets the session cookie");

        SignedIn {
            client: browser,
            session_cookie,
        }
    }
}

/// A dev-profile [`Config`] on `database_url`, served at `public_url`: the
/// shipped example root key with PLC submission off, so minting stays local.
/// Panics when `public_url` is not a bare origin.
fn config(public_url: &str, database_url: &str) -> Config {
    let root_key = base64::engine::general_purpose::STANDARD.encode(EXAMPLE_DEV_ROOT_KEY);
    Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().expect("loopback socket address"),
        public_url: public_url.parse().expect("a bare origin"),
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

/// The value of the `zurfur.sid` cookie `response` sets, if it sets a non-empty one.
fn set_session_cookie(response: &reqwest::Response) -> Option<String> {
    let prefix = format!("{}=", ZurfurCookie::Session.name(false));
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .filter_map(|header| header.split(';').next())
        .filter_map(|pair| pair.strip_prefix(prefix.as_str()))
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

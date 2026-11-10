use std::sync::Arc;

use adapter_pg::PgSessionStore;
use axum::Router;
use composition::{Config, Custody, Environment, RootSecret, Runtime};
use domain::elements::custody::Purpose;
use domain::elements::profile::Profile;
use http_cookies::ZurfurCookie;
use http_stack::{Detach, Stacked};
use reqwest::header::SET_COOKIE;
use strum::VariantArray as _;

use super::{InProcessPds, StatementLog, record_statements};
use crate::custody::test_root;
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
/// [`InProcessPds`] and sessions kept in PostgreSQL. Every custody purpose is
/// derived from [`TEST_ROOT`](crate::custody::TEST_ROOT).
pub async fn serve(app: impl FnOnce(Runtime) -> Router) -> PgServed {
    serve_through(
        fresh_db().await,
        app,
        None,
        |stacked| stacked,
        test_root(),
        |_| {},
    )
    .await
}

/// As [`serve`], over `db` as it stands, migrated by no one here, with every purpose derived
/// from `root`: for a test that brings its database to a schema itself.
pub async fn serve_over(
    db: TestDb,
    app: impl FnOnce(Runtime) -> Router,
    root: RootSecret,
) -> PgServed {
    serve_through(db, app, None, |stacked| stacked, root, |_| {}).await
}

/// As [`serve`], over `root` and with `configure` changing the dev [`Config`] before the
/// runtime connects.
pub async fn serve_configured(
    app: impl FnOnce(Runtime) -> Router,
    root: RootSecret,
    configure: impl FnOnce(&mut Config),
) -> PgServed {
    serve_through(
        fresh_db().await,
        app,
        None,
        |stacked| stacked,
        root,
        configure,
    )
    .await
}

/// As [`serve`], with `wrap` adding layers outside the whole stack.
pub async fn serve_wrapped(
    app: impl FnOnce(Runtime) -> Router,
    wrap: impl FnOnce(Stacked) -> Stacked,
) -> PgServed {
    serve_through(fresh_db().await, app, None, wrap, test_root(), |_| {}).await
}

/// As [`serve`], with the app reaching its database through a proxy that logs into
/// `statements` every statement PostgreSQL is asked to execute, seeding included.
pub async fn serve_recording(
    app: impl FnOnce(Runtime) -> Router,
    statements: &StatementLog,
) -> PgServed {
    serve_through(
        fresh_db().await,
        app,
        Some(statements),
        |stacked| stacked,
        test_root(),
        |_| {},
    )
    .await
}

/// Boots `app` as [`serve`] says over `db`, through the recording proxy when `statements`
/// is given, with `wrap` adding layers outside the whole stack, every purpose derived from
/// `root`, and `configure` changing the config.
async fn serve_through(
    db: TestDb,
    app: impl FnOnce(Runtime) -> Router,
    statements: Option<&StatementLog>,
    wrap: impl FnOnce(Stacked) -> Stacked,
    root: RootSecret,
    configure: impl FnOnce(&mut Config),
) -> PgServed {
    let database_url = match statements {
        Some(log) => record_statements(db.url(), log.clone()).await,
        None => db.url().to_owned(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));

    let pds = Arc::new(InProcessPds::default());
    let mut config = dev_config(&base_url, &database_url);
    configure(&mut config);
    let custody = every_purpose(root, &config);
    let live = Runtime::connect(config, custody)
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
    runtime_through(None).await
}

/// As [`pg_runtime`], with the runtime reaching its database through a proxy that logs into
/// `statements` every statement PostgreSQL is asked to execute, seeding included.
pub async fn pg_runtime_recording(statements: &StatementLog) -> PgRuntime {
    runtime_through(Some(statements)).await
}

/// Builds the [`PgRuntime`] as [`pg_runtime`] says, through the recording proxy when
/// `statements` is given.
async fn runtime_through(statements: Option<&StatementLog>) -> PgRuntime {
    let db = fresh_db().await;
    let database_url = match statements {
        Some(log) => record_statements(db.url(), log.clone()).await,
        None => db.url().to_owned(),
    };
    let pds = Arc::new(InProcessPds::default());
    let config = dev_config("http://localhost", &database_url);
    let custody = every_purpose(test_root(), &config);
    let live = Runtime::connect(config, custody)
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

/// Every custody purpose's handles, derived from `root` behind `config`'s guard.
fn every_purpose(root: RootSecret, config: &Config) -> Custody {
    Custody::derive(
        root,
        &config.env,
        config.plc_directory_submit,
        Purpose::VARIANTS,
    )
    .expect("the custody guard accepts a dev root with PLC submission off")
}

/// A dev-profile [`Config`] on `database_url`, served at `public_url`, with PLC
/// submission off, so minting stays local. Panics when `public_url` is not a bare origin.
pub fn dev_config(public_url: &str, database_url: &str) -> Config {
    Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().expect("loopback socket address"),
        public_url: public_url.parse().expect("a bare origin"),
        database_url: database_url.to_string(),
        log_level: "info".to_string(),
        handle_domain: "zurfur.app".parse().expect("a valid handle domain"),
        plc_directory_endpoint: "https://plc.directory".to_string(),
        plc_directory_submit: false,
        deadline_sweep_interval_secs: 60,
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        build_id: "unknown".try_into().expect("a build id"),
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

//! The dev profile's two locks and its sessions. Every build runs the guards
//! that keep the profile out of a default binary; a build with the
//! `dev-profile` feature also runs the profile itself, in process and as the
//! real binary.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

/// The feature that builds the dev profile into the binary.
const FEATURE: &str = "dev-profile";

/// A database URL that never resolves, so a server that reaches for it fails.
const UNREACHABLE_DATABASE_URL: &str = "postgres://dev-profile.invalid/none";

/// The shipped example root key, base64: what `.env.example` carries.
const EXAMPLE_ROOT_KEY_BASE64: &str = "ZGV2LW9ubHktcm9vdC1rZXktZG8tbm90LXNoaXAhISE=";

/// How long a spawned server may take to start or to give up.
const BOOT_TIMEOUT: Duration = Duration::from_secs(60);

/// The `api` binary, in a scratch directory (so no `.env` is found) with only
/// the variables a boot needs plus `vars`; ports are ephemeral, logs plain.
fn server_command(scratch: &tempfile::TempDir, vars: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_api"));
    command
        .env_clear()
        .current_dir(scratch.path())
        .env("DATABASE_URL", UNREACHABLE_DATABASE_URL)
        .env("ZURFUR_DID_KEY_ROOT_KEY", EXAMPLE_ROOT_KEY_BASE64)
        .env("ZURFUR_HTTP_ADDR", "127.0.0.1:0")
        .env("ZURFUR_PUBLIC_URL", "http://127.0.0.1:8180")
        .env("ZURFUR_LOG_LEVEL", "info")
        .env("NO_COLOR", "1")
        .envs(vars.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

/// `line` without ANSI escape sequences, in case colour slipped through.
fn plain(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            chars.by_ref().find(|c| c.is_ascii_alphabetic());
        } else {
            plain.push(c);
        }
    }
    plain
}

/// The output of a server that is expected to exit on its own.
struct Exited {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Runs the binary under `vars` and waits for it to exit.
async fn run_to_exit(vars: &[(&str, &str)]) -> Exited {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let running = server_command(&scratch, vars).output();
    let output = tokio::time::timeout(BOOT_TIMEOUT, running)
        .await
        .expect("the server exits in time")
        .expect("the server binary runs");
    Exited {
        success: output.status.success(),
        stdout: plain(&String::from_utf8_lossy(&output.stdout)),
        stderr: plain(&String::from_utf8_lossy(&output.stderr)),
    }
}

/// Feature enablement is unified across a build, so any of these would turn
/// the profile on in a default binary: `api`'s own default set, another
/// workspace member asking for the feature, or a feature forwarding to it.
#[test]
fn no_default_build_enables_the_dev_profile() {
    let output = std::process::Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(output.status.success(), "cargo metadata succeeds");
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON");
    let packages = metadata["packages"].as_array().expect("a package list");

    let api = packages
        .iter()
        .find(|package| package["name"] == "api")
        .expect("the api package is in the workspace");
    let api_features = api["features"].as_object().expect("api's feature table");
    assert!(
        api_features.contains_key(FEATURE),
        "the guard watches a feature that exists"
    );

    let mut default_closure = Vec::new();
    let mut pending = vec!["default".to_string()];
    while let Some(feature) = pending.pop() {
        if default_closure.contains(&feature) {
            continue;
        }
        let enables = api_features
            .get(&feature)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string);
        pending.extend(enables);
        default_closure.push(feature);
    }
    assert!(
        !default_closure.iter().any(|feature| feature == FEATURE),
        "api's default features never include {FEATURE}: {default_closure:?}"
    );

    for package in packages {
        let name = &package["name"];
        let dependencies = package["dependencies"]
            .as_array()
            .expect("a dependency list");
        // `name` is the package depended on; `rename` is the key it goes by
        // in this manifest, which is what a forwarding feature spells.
        let on_api: Vec<&serde_json::Value> = dependencies
            .iter()
            .filter(|dependency| dependency["name"] == "api")
            .collect();
        for dependency in &on_api {
            let requested = dependency["features"].as_array().expect("a feature list");
            assert!(
                !requested.iter().any(|feature| feature == FEATURE),
                "{name} depends on api with {FEATURE}"
            );
        }
        let forwarded: Vec<String> = on_api
            .iter()
            .map(|dependency| dependency["rename"].as_str().unwrap_or("api"))
            .flat_map(|key| [format!("{key}/{FEATURE}"), format!("{key}?/{FEATURE}")])
            .collect();
        let features = package["features"].as_object().expect("a feature table");
        for (feature, enables) in features {
            let enables = enables.as_array().expect("a feature's list");
            assert!(
                !enables
                    .iter()
                    .any(|enabled| forwarded.iter().any(|f| enabled == f.as_str())),
                "{name}'s feature {feature} turns on api's {FEATURE}"
            );
        }
    }
}

/// Without the feature, even `ZURFUR_ENV=dev` boots the everyday stack: it
/// reaches for PostgreSQL and never serves the fixture world.
#[cfg(not(feature = "dev-profile"))]
#[tokio::test]
async fn the_default_binary_is_the_everyday_stack_even_in_dev() {
    let exited = run_to_exit(&[("ZURFUR_ENV", "dev")]).await;

    assert!(!exited.success, "no database, no server");
    assert!(
        exited.stderr.contains("database unreachable"),
        "the everyday stack went to PostgreSQL: {}",
        exited.stderr
    );
    let everything = format!("{}{}", exited.stdout, exited.stderr);
    assert!(
        !everything.contains("dev profile"),
        "the default binary has no dev profile: {everything}"
    );
}

#[cfg(feature = "dev-profile")]
mod with_the_feature {
    use super::*;

    use api::{Config, Environment};
    use domain::elements::profile::Profile;
    use reqwest::StatusCode;
    use reqwest::header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
    use test_support::http::client;
    use test_support::pg_app::{self, PgServed};
    use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, BufReader};

    /// The session cookie both stacks issue.
    const SESSION_COOKIE: &str = "zurfur.sid";

    /// A dev-profile [`Config`] in `env`, served at `public_url`.
    fn config(env: Environment, public_url: &str) -> Config {
        Config {
            env,
            http_addr: "127.0.0.1:0".parse().expect("loopback socket address"),
            public_url: public_url.parse().expect("a bare origin"),
            database_url: UNREACHABLE_DATABASE_URL.to_string(),
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".parse().expect("a valid handle domain"),
            did_key_root_key: EXAMPLE_ROOT_KEY_BASE64.to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
            build_id: "unknown".try_into().expect("a build id"),
        }
    }

    /// Serves the dev profile in process on an ephemeral port; its base URL.
    async fn serve_dev_profile() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let app = api::dev_profile::app(config(Environment::DEV, &base_url))
            .await
            .expect("the dev profile starts in dev");
        tokio::spawn(async move {
            http_stack::serve(listener, app, std::future::pending())
                .await
                .unwrap();
        });
        base_url
    }

    /// The `zurfur.sid=<id>` pair `response` sets, if it sets one.
    fn session_cookie(response: &reqwest::Response) -> Option<String> {
        let prefix = format!("{SESSION_COOKIE}=");
        response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .filter(|value| value.starts_with(&prefix))
            .filter_map(|value| value.split(';').next())
            .find(|pair| pair.len() > prefix.len())
            .map(str::to_string)
    }

    /// `response`'s `Location`, as text.
    fn location(response: &reqwest::Response) -> String {
        response
            .headers()
            .get(LOCATION)
            .expect("a redirect")
            .to_str()
            .expect("the redirect is text")
            .to_string()
    }

    /// `POST {base}/signin` typing `handle`, from `browser`.
    async fn start_sign_in(browser: &reqwest::Client, base: &str, handle: &str) -> String {
        let started = browser
            .post(format!("{base}/signin"))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(format!("handle={handle}"))
            .send()
            .await
            .expect("POST /signin");
        assert_eq!(started.status(), StatusCode::SEE_OTHER, "signin redirects");
        location(&started)
    }

    /// Signs a fresh browser in to the dev profile at `base` through the
    /// ordinary routes; the browser and the session cookie it was given.
    async fn sign_in_to_dev_profile(base: &str) -> (reqwest::Client, String) {
        let browser = client();
        let authorization_url = start_sign_in(&browser, base, "bob.test").await;
        let callback_prefix = format!("{base}/signin-callback?");
        assert!(
            authorization_url.starts_with(&callback_prefix),
            "the fixed sign-in sends the browser straight back: {authorization_url}"
        );

        let callback = browser
            .get(&authorization_url)
            .send()
            .await
            .expect("GET the sign-in callback");
        assert_eq!(callback.status(), StatusCode::SEE_OTHER);
        let home = format!("{base}/");
        assert_eq!(location(&callback), home, "sign-in lands home");
        let cookie = session_cookie(&callback).expect("the callback sets the session cookie");
        (browser, cookie)
    }

    /// Signs alice in to the PostgreSQL stack through its ordinary routes,
    /// under the fixture DID and handle the dev profile signs in; the session
    /// cookie she was given. Alice then exists on both stacks, so only the
    /// stores' separation can refuse a cookie from the other one.
    async fn sign_alice_in_to_postgres_stack(pg: &PgServed) -> String {
        let alice = Profile::new(adapter_mem::fixture::alice().did().clone(), "alice.test");
        pg.pds.register(alice);
        let browser = client();
        let authorization_url = start_sign_in(&browser, &pg.base_url, "alice.test").await;

        let callback = browser
            .get(format!("{}{authorization_url}", pg.base_url))
            .send()
            .await
            .expect("GET the sign-in callback");
        assert_eq!(callback.status(), StatusCode::SEE_OTHER);
        session_cookie(&callback).expect("the callback sets the session cookie")
    }

    /// `GET {base}/me` presenting only `cookie`.
    async fn me_status(base: &str, cookie: &str) -> StatusCode {
        reqwest::Client::new()
            .get(format!("{base}/me"))
            .header(COOKIE, cookie)
            .send()
            .await
            .expect("GET /me")
            .status()
    }

    #[tokio::test]
    async fn every_environment_but_dev_is_refused_before_anything_starts() {
        for env in [Environment::STG, Environment::PROD] {
            let refused = api::dev_profile::app(config(env, "http://127.0.0.1:8180"))
                .await
                .expect_err("only dev starts");

            assert!(
                refused
                    .downcast_ref::<api::dev_profile::OutsideDev>()
                    .is_some(),
                "refused by the environment lock: {refused:#}"
            );
        }
    }

    #[tokio::test]
    async fn the_ordinary_sign_in_signs_the_browser_in_as_alice() {
        let base = serve_dev_profile().await;
        let (browser, _cookie) = sign_in_to_dev_profile(&base).await;

        let me = browser
            .get(format!("{base}/me"))
            .send()
            .await
            .expect("GET /me");

        assert_eq!(me.status(), StatusCode::OK);
        let body: serde_json::Value = me.json().await.expect("a JSON whoami");
        let alice = adapter_mem::fixture::alice();
        assert_eq!(
            body["did"],
            alice.did().to_string(),
            "bob.test was typed; alice is who signs in"
        );
        assert_eq!(
            body["handle"], "alice.test",
            "her fixture profile is served"
        );
    }

    #[tokio::test]
    async fn alice_files_a_bug_report() {
        let base = serve_dev_profile().await;
        let (browser, _cookie) = sign_in_to_dev_profile(&base).await;
        let report = serde_json::json!({ "description": "The dev board froze." });

        let filed = browser
            .post(format!("{base}/bug-reports"))
            .json(&report)
            .send()
            .await
            .expect("POST /bug-reports");

        assert_eq!(filed.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn a_callback_without_the_browser_binding_signs_no_one_in() {
        let base = serve_dev_profile().await;
        let browser = client();
        let authorization_url = start_sign_in(&browser, &base, "alice.test").await;

        let stranger = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("client builds");
        let callback = stranger
            .get(&authorization_url)
            .send()
            .await
            .expect("GET the sign-in callback");

        assert_eq!(callback.status(), StatusCode::SEE_OTHER);
        let refused = format!("{base}/login?error=browser_mismatch");
        assert_eq!(location(&callback), refused);
        assert_eq!(
            session_cookie(&callback),
            None,
            "no session without the binding"
        );
    }

    #[tokio::test]
    async fn a_postgres_stack_session_is_no_dev_profile_session() {
        let pg = pg_app::serve(api::app).await;
        let dev_profile = serve_dev_profile().await;
        let postgres_cookie = sign_alice_in_to_postgres_stack(&pg).await;

        assert_eq!(
            me_status(&pg.base_url, &postgres_cookie).await,
            StatusCode::OK,
            "the cookie is live on its own stack"
        );
        assert_eq!(
            me_status(&dev_profile, &postgres_cookie).await,
            StatusCode::UNAUTHORIZED,
            "the dev profile does not accept the PostgreSQL stack's session"
        );
    }

    #[tokio::test]
    async fn a_dev_profile_session_is_no_postgres_stack_session() {
        let pg = pg_app::serve(api::app).await;
        let dev_profile = serve_dev_profile().await;
        sign_alice_in_to_postgres_stack(&pg).await;
        let (_browser, dev_cookie) = sign_in_to_dev_profile(&dev_profile).await;

        assert_eq!(
            me_status(&dev_profile, &dev_cookie).await,
            StatusCode::OK,
            "the cookie is live on its own stack"
        );
        assert_eq!(
            me_status(&pg.base_url, &dev_cookie).await,
            StatusCode::UNAUTHORIZED,
            "the PostgreSQL stack does not accept the dev profile's session"
        );
    }

    #[tokio::test]
    async fn the_binary_refuses_to_start_outside_dev() {
        // The deployed environments get a configuration their own boot checks
        // accept, so the refusal each meets is the dev profile's.
        let deployed = [
            ("ZURFUR_PUBLIC_URL", "https://zurfur.foo"),
            ("ZURFUR_HANDLE_DOMAIN", "zurfur.foo"),
        ];
        let staging = [("ZURFUR_ENV", "stg"), deployed[0], deployed[1]];
        let production = [("ZURFUR_ENV", "prod"), deployed[0], deployed[1]];
        let cases: [(&[(&str, &str)], &str); 3] = [
            (&[], "ZURFUR_ENV unset"),
            (&staging, "ZURFUR_ENV=\"stg\""),
            (&production, "ZURFUR_ENV=\"prod\""),
        ];
        for (vars, why) in cases {
            let exited = run_to_exit(vars).await;

            assert!(!exited.success, "{why}: the binary refuses to start");
            assert!(
                exited.stderr.contains("refusing to start the dev profile"),
                "{why}: the refusal is the environment lock: {}",
                exited.stderr
            );
            assert!(
                exited.stderr.contains(why),
                "{why}: the refusal says which: {}",
                exited.stderr
            );
            assert!(
                exited.stderr.contains("fixture user"),
                "{why}: the refusal names its reason: {}",
                exited.stderr
            );
            let everything = format!("{}{}", exited.stdout, exited.stderr);
            assert!(
                !everything.contains("dev profile:"),
                "{why}: refused before the profile started: {everything}"
            );
            assert!(
                !everything.contains("starting HTTP server"),
                "{why}: nothing was served"
            );
        }
    }

    /// Asserts the binary refused to boot DEV off this machine, naming
    /// `setting`, before anything was served.
    fn assert_refused_off_this_machine(exited: &Exited, setting: &str) {
        assert!(!exited.success, "{setting}: the binary refuses to start");
        assert!(
            exited
                .stderr
                .contains("refusing to boot DEV off this machine"),
            "{setting}: the refusal is the loopback check: {}",
            exited.stderr
        );
        assert!(
            exited.stderr.contains(setting),
            "{setting}: the refusal names the setting: {}",
            exited.stderr
        );
        let everything = format!("{}{}", exited.stdout, exited.stderr);
        assert!(
            !everything.contains("dev profile:"),
            "{setting}: refused before the profile started: {everything}"
        );
        assert!(
            !everything.contains("starting HTTP server"),
            "{setting}: nothing was served"
        );
    }

    #[tokio::test]
    async fn the_binary_refuses_to_listen_beyond_loopback() {
        let vars = [("ZURFUR_ENV", "dev"), ("ZURFUR_HTTP_ADDR", "0.0.0.0:0")];

        let exited = run_to_exit(&vars).await;

        assert_refused_off_this_machine(&exited, "`http_addr`");
    }

    #[tokio::test]
    async fn the_binary_refuses_a_public_url_beyond_loopback() {
        let vars = [
            ("ZURFUR_ENV", "dev"),
            ("ZURFUR_PUBLIC_URL", "http://192.0.2.1:8180"),
        ];

        let exited = run_to_exit(&vars).await;

        assert_refused_off_this_machine(&exited, "`public_url`");
    }

    #[tokio::test]
    async fn the_binary_starts_in_dev_and_its_log_names_no_user() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let mut server = server_command(&scratch, &[("ZURFUR_ENV", "dev")])
            .spawn()
            .expect("the server binary runs");
        let stdout = server.stdout.take().expect("piped stdout");
        let mut lines = BufReader::new(stdout).lines();
        let mut log = Vec::new();

        let started = tokio::time::timeout(BOOT_TIMEOUT, async {
            while let Some(line) = lines.next_line().await.expect("a log line") {
                let line = plain(&line);
                let addr = line
                    .contains("starting HTTP server")
                    .then(|| line.split_once("addr=").map(|(_, rest)| rest.to_string()))
                    .flatten()
                    .and_then(|rest| rest.split_whitespace().next().map(str::to_string));
                log.push(line);
                if let Some(addr) = addr {
                    return addr;
                }
            }
            panic!("the server exited before serving: {log:?}");
        });
        let addr = started.await.expect("the server starts in time");
        let base = format!("http://{addr}");

        let browser = client();
        let authorization_url = start_sign_in(&browser, &base, "alice.test").await;
        let callback_path = authorization_url
            .strip_prefix("http://127.0.0.1:8180")
            .expect("redirected on the public origin");
        let callback = browser
            .get(format!("{base}{callback_path}"))
            .send()
            .await
            .expect("GET the sign-in callback");
        assert_eq!(callback.status(), StatusCode::SEE_OTHER);
        let me = browser
            .get(format!("{base}/me"))
            .send()
            .await
            .expect("GET /me");
        assert_eq!(
            me.status(),
            StatusCode::OK,
            "the binary signed the browser in"
        );

        server.start_kill().expect("the server stops");
        while let Ok(Some(line)) = lines.next_line().await {
            log.push(plain(&line));
        }
        let mut stderr = String::new();
        if let Some(mut pipe) = server.stderr.take() {
            pipe.read_to_string(&mut stderr)
                .await
                .expect("stderr reads");
        }
        server.wait().await.expect("the server is reaped");
        log.push(plain(&stderr));
        let log = log.join("\n");

        assert!(
            log.lines()
                .any(|line| line.contains("WARN") && line.contains("dev profile:")),
            "the start says it is the dev profile: {log}"
        );
        let people = [
            adapter_mem::fixture::alice().did().to_string(),
            adapter_mem::fixture::bob().did().to_string(),
            adapter_mem::fixture::supreme_arts().did().to_string(),
            adapter_mem::fixture::abco().did().to_string(),
            adapter_mem::fixture::ember().did().to_string(),
            adapter_mem::fixture::kael_sona().did().to_string(),
            "alice.test".to_string(),
            "bob.test".to_string(),
            "supreme-arts.zurfur.app".to_string(),
        ];
        for person in people {
            assert!(!log.contains(&person), "the log names {person}: {log}");
        }
    }
}

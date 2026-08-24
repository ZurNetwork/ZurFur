//! The runtime [`Config`], its figment loader, and the boot-time custody guard.
//!
//! Moved from `api` (ZMVP-200) so the CLI boots from the same source, with
//! two additions: [`Environment`] accepts the lowercase spellings (closing the
//! `.env.example` catch-22) and [`Config::load_from`] takes the directory.
//! References: CLAUDE.md "Configuration"; the repo memory `config-and-runtime`.

use std::net::SocketAddr;
use std::path::PathBuf;

use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;

/// The environment variable naming the config profile (`dev`/`stg`/`prod`).
pub const PROFILE_ENV: &str = "ZURFUR_ENV";
/// The environment variable overriding the config directory.
pub const CONFIG_DIR_ENV: &str = "ZURFUR_CONFIG_DIR";
/// The one un-prefixed variable, shared with sqlx tooling.
pub const DATABASE_URL_ENV: &str = "DATABASE_URL";
/// The prefix every other `Config` field answers to (`ZURFUR_<FIELD>`).
pub const ENV_PREFIX: &str = "ZURFUR_";
/// `ENV_PREFIX` + [`Config::did_key_root_key`] — named because every harness
/// that boots a runtime has to set it.
pub const ROOT_KEY_ENV: &str = "ZURFUR_DID_KEY_ROOT_KEY";

/// The deployment profile, selected by `ZURFUR_ENV` (`dev` → [`DEV`]). The only
/// behavioral fork it drives today is cookie security: [`STG`] and [`PROD`] set
/// the session cookie `Secure` (HTTPS-only) in `main`, while [`DEV`] leaves it
/// off so loopback HTTP doesn't drop the cookie.
///
/// Caveats: deserialized from config/env as `DEV`/`STG`/`PROD`, with the
/// lowercase spellings accepted as aliases — `ZURFUR_ENV=dev` both selects the
/// `dev.toml` profile AND lands on this field through the `ZURFUR_` env layer,
/// so the two spellings must agree (the `.env.example` catch-22 closed in
/// ZMVP-200). New environments are an enum change, not config.
///
/// [`DEV`]: Environment::DEV
/// [`STG`]: Environment::STG
/// [`PROD`]: Environment::PROD
#[derive(Clone, Debug, Deserialize)]
pub enum Environment {
    /// Local development: plain HTTP on loopback, non-`Secure` cookies.
    #[serde(alias = "dev")]
    DEV,
    /// Staging: HTTPS, `Secure` cookies — a production-shaped environment.
    #[serde(alias = "stg")]
    STG,
    /// Production: HTTPS, `Secure` cookies.
    #[serde(alias = "prod")]
    PROD,
}

/// The fully-resolved runtime configuration, produced by [`Config::load`] and
/// then handed to [`Runtime::connect`](crate::Runtime::connect). Every field is required at boot except
/// [`http_addr`], which defaults to `127.0.0.1:3621`, and [`handle_domain`], which
/// defaults to `zurfur.app`.
///
/// Caveats: figment layers config/{profile}.toml first, then `DATABASE_URL`,
/// then `ZURFUR_*` env (env wins); a missing required key fails the load.
/// [`database_url`] is read from the unprefixed `DATABASE_URL` on purpose — sqlx
/// tooling reads that exact name. [`public_url`] is the externally-visible
/// origin and must be a parseable URI: [`Runtime::connect`](crate::Runtime::connect) builds the OAuth
/// redirect URI from it and aborts boot if it can't.
///
/// References: CLAUDE.md "Configuration"; [`Config::load`].
///
/// [`http_addr`]: Config::http_addr
/// [`database_url`]: Config::database_url
/// [`public_url`]: Config::public_url
/// [`handle_domain`]: Config::handle_domain
#[derive(Clone, Deserialize)]
pub struct Config {
    /// The deployment profile; see [`Environment`].
    pub env: Environment,
    /// The socket the HTTP server binds. Defaults to `127.0.0.1:3621`
    /// (`default_http_addr`); dev.toml overrides to `127.0.0.1:8080`.
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    /// Externally-visible origin (scheme + host + port) used to build OAuth redirect URIs.
    pub public_url: String,
    /// Postgres connection string for the pool built at boot. Read from the
    /// unprefixed `DATABASE_URL` (the name sqlx tooling expects), not `ZURFUR_*`.
    pub database_url: String,
    /// Default tracing filter, applied when `RUST_LOG` is unset (see `main`).
    pub log_level: String,
    /// The DNS suffix Zurfur issues Account handles under, e.g. `zurfur.app`
    /// (default `default_handle_domain`). The `/.well-known/atproto-did` resolver
    /// only answers for a `Host` that is a subdomain of this domain — a request for
    /// any other authority is not ours to resolve (ZMVP-44, DD/26607618).
    #[serde(default = "default_handle_domain")]
    pub handle_domain: String,
    /// **DEV-ONLY root key** (base64, 32 bytes) that envelope-encrypts every
    /// account's minted `did:plc` custody keys at rest (ZMVP-49). A config/env
    /// secret is *not* a hardware boundary: this is acceptable only pre-alpha.
    /// Hardening it into a cloud KMS/HSM is the URGENT follow-up **ZMVP-53**, which
    /// must land before any real account is minted. Read from
    /// `ZURFUR_DID_KEY_ROOT_KEY`; never committed to a profile TOML.
    pub did_key_root_key: String,
    /// PLC directory base URL used **only** when [`plc_directory_submit`] is on.
    /// Defaults to a **local placeholder** (`http://localhost:2582`, the local
    /// `@did-plc/server` port) — deliberately **not** the canonical
    /// `https://plc.directory`. The canonical directory is a permanent, public,
    /// append-only log; a stray `plc_directory_submit = true` must never register
    /// against it by accident, so canonical must be set **explicitly** at launch.
    ///
    /// [`plc_directory_submit`]: Config::plc_directory_submit
    #[serde(default = "default_plc_directory_endpoint")]
    pub plc_directory_endpoint: String,
    /// Whether the minter actually submits genesis operations to the directory.
    /// **Defaults to `false`** (ZMVP-49 C2): the minter uses a no-op directory and
    /// registers nothing. Flip on at launch — and only alongside an explicit,
    /// intentional [`plc_directory_endpoint`](Config::plc_directory_endpoint).
    #[serde(default)]
    pub plc_directory_submit: bool,
    /// How often the deadline sweep runs, in seconds (ZMVP-86, ruling E12).
    /// Defaults to `300` (`default_deadline_sweep_interval_secs`); override via
    /// `ZURFUR_DEADLINE_SWEEP_INTERVAL_SECS`. `main` spawns
    /// [`run_deadline_sweeper`] on this cadence; the loop clamps the value to
    /// at least one second. Late **state** is derived on every read, so
    /// correctness never depends on this — the sweep only bounds how *promptly*
    /// the system `late` **changelog entry** is appended (each sweep is one
    /// atomic unit of work over whatever has lapsed by then).
    #[serde(default = "default_deadline_sweep_interval_secs")]
    pub deadline_sweep_interval_secs: u64,
    /// The maximum size, in bytes, of a single uploaded commission file entry
    /// (ZMVP-88, ruling E13). Defaults to [`Config::DEFAULT_MAX_UPLOAD_BYTES`]
    /// (50 MiB — Bluesky PDS blob-cap parity, Engineer ruling 2026-07-25);
    /// override via `ZURFUR_MAX_UPLOAD_BYTES`. The
    /// upload route enforces this two ways: a body-size limit on the request (a
    /// hard framework backstop, set a margin above this for the multipart
    /// envelope) and an exact check on the file bytes that answers `413`
    /// problem+json. The real limit/format policy is the future blob-architecture
    /// walkthrough's; v1 only needs a cap so nothing ships uncapped.
    #[serde(default = "default_max_upload_bytes")]
    pub max_upload_bytes: u64,
}

/// Serde default for [`Config::max_upload_bytes`]:
/// [`Config::DEFAULT_MAX_UPLOAD_BYTES`].
fn default_max_upload_bytes() -> u64 {
    Config::DEFAULT_MAX_UPLOAD_BYTES
}

/// Serde default for [`Config::deadline_sweep_interval_secs`]: every five
/// minutes. The derived Late *state* is instant on read, so this only paces the
/// changelog `late` entry, and the scan rides the partial `deadline` index.
/// (Cadence vs. sweep cost at scale is a further-optimization axis — ZMVP-86
/// review 2026-07-09.)
fn default_deadline_sweep_interval_secs() -> u64 {
    300
}

/// Serde default for [`Config::handle_domain`]: `zurfur.app`, the production
/// Zurfur-issued handle namespace.
fn default_handle_domain() -> String {
    "zurfur.app".to_string()
}

/// Serde default for [`Config::plc_directory_endpoint`]: a **local placeholder**,
/// never the canonical public log (see the field docs for why).
fn default_plc_directory_endpoint() -> String {
    "http://localhost:2582".to_string()
}

/// The raw bytes of the example dev root key shipped in `.env.example`
/// (`ZURFUR_DID_KEY_ROOT_KEY`, base64 of these 32 ASCII bytes). Its private value
/// is public, so minting real identities under it would be catastrophic — the boot
/// guard refuses it wherever real minting could happen.
pub const EXAMPLE_DEV_ROOT_KEY: &[u8] = b"dev-only-root-key-do-not-ship!!!";

/// Boot-time custody guard (ZMVP-49): refuse to run any configuration that would
/// mint **real** account identities under **dev-only** key custody, so the
/// "harden before real accounts" rule is *enforced*, not documentation.
///
/// `root_key` is the decoded `did:plc` custody root key; `submit` is whether the
/// minter registers operations to a PLC directory. Two refusals:
///
/// 1. **Production-like environment (`PROD`/`STG`).** v1 custody is always
///    config/env-root-backed — there is no KMS-backed [`KeyStore`](domain::ports::KeyStore)
///    adapter yet (that is the URGENT follow-up **ZMVP-53**). So a production-like
///    boot with today's custody is refused outright: it must wait for KMS.
/// 2. **Submitting under the shipped example key.** Registering an operation with
///    the public example root key would publish a DID whose keys everyone knows —
///    refused in any environment.
///
/// Returns `Ok(())` for the dev/test configurations that are actually safe (dev
/// env, and — unless it is the example key — submission off).
pub fn ensure_custody_hardened(
    env: &Environment,
    root_key: &[u8],
    submit: bool,
) -> anyhow::Result<()> {
    let prod_like = matches!(env, Environment::PROD | Environment::STG);
    let is_example_key = root_key == EXAMPLE_DEV_ROOT_KEY;
    // v1 has no KMS-backed KeyStore; custody is always config/env-root-backed.
    let config_root_backed = true;

    if prod_like && (config_root_backed || is_example_key) {
        anyhow::bail!(
            "refusing to boot in {env:?}: did:plc key custody is config/env-root-backed, \
             which is DEV-ONLY (a config secret is not a hardware boundary). Cloud-KMS-backed \
             custody must land before any real account is minted — ZMVP-53."
        );
    }
    if submit && is_example_key {
        anyhow::bail!(
            "refusing PLC directory submission: the did:plc custody root key is the shipped \
             example key (its private value is public). Set a real ZURFUR_DID_KEY_ROOT_KEY and \
             use KMS-backed custody — ZMVP-53."
        );
    }
    Ok(())
}

/// Serde default for [`Config::http_addr`]: `127.0.0.1:3621`. The literal is a
/// known-valid socket, so the parse can't fail.
fn default_http_addr() -> SocketAddr {
    "127.0.0.1:3621".parse().unwrap()
}

impl Config {
    /// Default for [`Config::max_upload_bytes`]: **50 MiB (52,428,800 bytes) —
    /// Bluesky PDS blob-cap parity** (Engineer ruling 2026-07-25, MVP; "we can
    /// increase eventually"). Bounded so no upload is uncapped (ZMVP-88). The
    /// one home for the number; the serde default and every test fixture
    /// reference it.
    ///
    /// Raising it later is fine **up to `i32::MAX` bytes** (2,147,483,647 —
    /// one byte under 2 GiB): past that the
    /// wire's `byte_size` field can no longer be an `int32` JSON number and
    /// must become the canonical int64 decimal **string** (the minimum-range
    /// ruling, `contract/VERSIONING.md` §7.2) — a breaking change to plan,
    /// not stumble into.
    pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;

    /// Loads and validates the runtime [`Config`] from the layered figment
    /// sources, selecting the profile from `ZURFUR_ENV` (default `dev`).
    ///
    /// Layering, lowest precedence first: `config/{profile}.toml`, then the
    /// unprefixed `DATABASE_URL`, then all `ZURFUR_*` env vars — so environment
    /// always wins over the file. The config directory is anchored to
    /// `CARGO_MANIFEST_DIR` (overridable via `ZURFUR_CONFIG_DIR`) because cargo,
    /// cargo-watch, and `just` each run from a different CWD.
    ///
    /// Caveats: returns a boxed [`figment::Error`] if a required key is missing
    /// or a value fails to deserialize (e.g. a malformed `http_addr`, or an
    /// `env` that isn't one of [`Environment`]'s variants). The TOML file is
    /// optional — env alone can satisfy every required key — but the keys
    /// themselves are not.
    ///
    /// References: CLAUDE.md "Configuration".
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Self::load_from(None)
    }

    /// [`load`](Config::load) with the config directory chosen by the caller
    /// (the CLI's `--config-dir`). `None` falls back to `ZURFUR_CONFIG_DIR`,
    /// then the repo's `backend/config`.
    ///
    /// Anchoring: the default is relative to this crate's `CARGO_MANIFEST_DIR`
    /// (`backend/crates/composition` → `backend/config`) rather than the
    /// current working directory, because cargo, cargo-watch, and `just` each
    /// run from a different CWD. A deployed binary points elsewhere via
    /// `ZURFUR_CONFIG_DIR` or the explicit argument.
    pub fn load_from(config_dir: Option<PathBuf>) -> Result<Self, Box<figment::Error>> {
        let profile = std::env::var(PROFILE_ENV).unwrap_or_else(|_| "dev".into());
        // The profile names a file; keep it a bare name so `ZURFUR_ENV=../x`
        // can never walk out of the config dir (today the `Environment` enum
        // happens to reject it too — this guard is where the risk is).
        if profile.is_empty()
            || !profile
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(Box::new(figment::Error::from(format!(
                "{PROFILE_ENV} must be a bare profile name (letters, digits, '-'), got {profile:?}"
            ))));
        }

        let config_dir = config_dir
            .or_else(|| std::env::var_os(CONFIG_DIR_ENV).map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../config")));
        let profile_file = config_dir.join(format!("{profile}.toml"));

        Figment::new()
            .merge(Toml::file(profile_file))
            .merge(Env::raw().only(&[DATABASE_URL_ENV]))
            .merge(Env::prefixed(ENV_PREFIX))
            .extract()
            .map_err(Box::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A production-like boot with today's config-root-backed custody is REFUSED —
    // it must wait for KMS (ZMVP-53). True regardless of which root key is set.
    #[test]
    fn prod_like_boot_is_refused_under_config_root_custody() {
        let real_key = [0xABu8; 32];
        assert!(ensure_custody_hardened(&Environment::PROD, &real_key, false).is_err());
        assert!(ensure_custody_hardened(&Environment::STG, &real_key, false).is_err());
        assert!(ensure_custody_hardened(&Environment::PROD, EXAMPLE_DEV_ROOT_KEY, false).is_err());
    }

    // Submitting to a directory under the shipped example key is REFUSED in any env.
    #[test]
    fn submitting_with_the_example_key_is_refused() {
        assert!(ensure_custody_hardened(&Environment::DEV, EXAMPLE_DEV_ROOT_KEY, true).is_err());
    }

    // The safe dev configurations pass: dev env, and dev submission only when the
    // root key is a real (non-example) one.
    #[test]
    fn dev_configurations_are_allowed() {
        let real_key = [0xABu8; 32];
        assert!(ensure_custody_hardened(&Environment::DEV, &real_key, false).is_ok());
        assert!(ensure_custody_hardened(&Environment::DEV, &real_key, true).is_ok());
        // Dev with the example key but NOT submitting is fine (the common local case).
        assert!(ensure_custody_hardened(&Environment::DEV, EXAMPLE_DEV_ROOT_KEY, false).is_ok());
    }

    // Precedence, lowest first: profile TOML < `DATABASE_URL` < `ZURFUR_*` env.
    // Env always wins over the file; the bare `DATABASE_URL` name is honored;
    // and `ZURFUR_ENV=dev` (the `.env.example` spelling) is a valid profile.
    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's closure signature
    fn env_wins_over_the_profile_file() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "dev.toml",
                r#"
                    env = "DEV"
                    public_url = "http://from-file"
                    database_url = "postgres://from-file"
                    log_level = "info"
                    did_key_root_key = "ZmlsZQ=="
                "#,
            )?;
            // Jail restores what it mutates but inherits the process env —
            // clear it so a developer's `.env` (via `just`) can't leak in.
            jail.clear_env();
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", "dev");
            jail.set_env("DATABASE_URL", "postgres://from-env");
            jail.set_env("ZURFUR_PUBLIC_URL", "http://from-env");

            let config = Config::load().map_err(|e| *e)?;
            assert!(matches!(config.env, Environment::DEV));
            assert_eq!(config.database_url, "postgres://from-env");
            assert_eq!(config.public_url, "http://from-env");
            assert_eq!(config.log_level, "info");
            assert_eq!(config.handle_domain, "zurfur.app");
            assert_eq!(config.max_upload_bytes, Config::DEFAULT_MAX_UPLOAD_BYTES);
            Ok(())
        });
    }

    // The profile selector is a file name: no path components allowed.
    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's closure signature
    fn a_traversing_profile_is_refused() {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", "../etc/passwd");
            jail.set_env("DATABASE_URL", "postgres://x");
            let Err(error) = Config::load() else {
                panic!("a traversing profile must be refused");
            };
            assert!(error.to_string().contains("bare profile name"), "{error}");
            Ok(())
        });
    }

    // A required key missing from every layer fails the load — never a
    // default. A valid `dev` profile with an empty file, so the only thing
    // wrong is the absent `public_url`/`log_level`/`did_key_root_key`.
    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's closure signature
    fn a_missing_required_key_fails_the_load() {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("dev.toml", "env = \"DEV\"\n")?;
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", "dev");
            jail.set_env("DATABASE_URL", "postgres://x");
            let Err(error) = Config::load() else {
                panic!("a config with no public_url must not load");
            };
            assert!(error.to_string().contains("public_url"), "{error}");
            Ok(())
        });
    }
}

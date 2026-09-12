//! The runtime [`Config`], its figment loader, and the boot-time custody guard.

use std::net::SocketAddr;
use std::path::PathBuf;

use domain::elements::handle::HandleDomain;
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::{Deserialize, Deserializer, de};

/// The environment variable naming the config profile (`dev`/`stg`/`prod`).
pub const PROFILE_ENV: &str = "ZURFUR_ENV";
/// The environment variable overriding the config directory.
pub const CONFIG_DIR_ENV: &str = "ZURFUR_CONFIG_DIR";
/// The one un-prefixed variable, shared with sqlx tooling.
pub const DATABASE_URL_ENV: &str = "DATABASE_URL";
/// The prefix every other `Config` field answers to (`ZURFUR_<FIELD>`).
pub const ENV_PREFIX: &str = "ZURFUR_";
/// `ENV_PREFIX` + [`Config::did_key_root_key`]; every harness that boots a
/// runtime has to set it.
pub const ROOT_KEY_ENV: &str = "ZURFUR_DID_KEY_ROOT_KEY";

/// The deployment profile, selected by `ZURFUR_ENV`. Spelled `DEV`/`STG`/`PROD`
/// with the lowercase forms accepted as aliases, since `ZURFUR_ENV` both picks
/// the profile TOML and lands on this field. A new environment is an enum
/// change, not config.
#[derive(Clone, Debug, Deserialize)]
pub enum Environment {
    /// Local development: plain HTTP on loopback, non-`Secure` cookies.
    #[serde(alias = "dev")]
    DEV,
    /// Staging: HTTPS, `Secure` cookies.
    #[serde(alias = "stg")]
    STG,
    /// Production: HTTPS, `Secure` cookies.
    #[serde(alias = "prod")]
    PROD,
}

/// The fully-resolved runtime configuration, produced by [`Config::load`] and
/// handed to [`Runtime::connect`](crate::Runtime::connect). Every field is
/// required at boot except [`http_addr`](Config::http_addr) and
/// [`handle_domain`](Config::handle_domain), which default.
#[derive(Clone, Deserialize)]
pub struct Config {
    /// The deployment profile; see [`Environment`].
    pub env: Environment,
    /// The socket the HTTP server binds. Defaults to `127.0.0.1:3621`.
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    /// Externally-visible origin (scheme + host + port). Must parse as a URI —
    /// the OAuth redirect URI is built from it, and boot aborts if it can't be.
    pub public_url: String,
    /// Postgres connection string for the pool built at boot. Read from the
    /// unprefixed `DATABASE_URL`, the name sqlx tooling expects.
    pub database_url: String,
    /// Default tracing filter, applied when `RUST_LOG` is unset.
    pub log_level: String,
    /// The DNS suffix Zurfur issues Account handles under, e.g. `zurfur.app`.
    /// Parsed once here, so an invalid namespace fails the boot and the claim
    /// checks and the well-known resolver cannot disagree.
    #[serde(
        default = "default_handle_domain",
        deserialize_with = "deserialize_handle_domain"
    )]
    pub handle_domain: HandleDomain,
    /// **DEV-ONLY root key** (base64, 32 bytes) that envelope-encrypts every
    /// account's minted `did:plc` custody keys at rest. Read from
    /// `ZURFUR_DID_KEY_ROOT_KEY`, never committed to a profile TOML; refused in
    /// production-like environments by [`ensure_custody_hardened`].
    pub did_key_root_key: String,
    /// PLC directory base URL, used only when
    /// [`plc_directory_submit`](Config::plc_directory_submit) is on. Defaults
    /// to a local placeholder — never the canonical public append-only log,
    /// which must be set explicitly.
    #[serde(default = "default_plc_directory_endpoint")]
    pub plc_directory_endpoint: String,
    /// Whether the minter submits genesis operations to the directory.
    /// Defaults to `false`; flip on only alongside an intentional
    /// [`plc_directory_endpoint`](Config::plc_directory_endpoint).
    #[serde(default)]
    pub plc_directory_submit: bool,
    /// How often the deadline sweep runs, in seconds (default 300). Late state
    /// is derived on read, so this only paces the `late` changelog entry.
    #[serde(default = "default_deadline_sweep_interval_secs")]
    pub deadline_sweep_interval_secs: u64,
    /// Maximum size in bytes of a single uploaded commission file entry.
    /// Defaults to [`Config::DEFAULT_MAX_UPLOAD_BYTES`].
    #[serde(default = "default_max_upload_bytes")]
    pub max_upload_bytes: u64,
}

/// Serde default for [`Config::max_upload_bytes`].
fn default_max_upload_bytes() -> u64 {
    Config::DEFAULT_MAX_UPLOAD_BYTES
}

/// Serde default for [`Config::deadline_sweep_interval_secs`]: five minutes.
fn default_deadline_sweep_interval_secs() -> u64 {
    300
}

/// The production Zurfur-issued handle namespace, already normalized — the
/// default for [`Config::handle_domain`].
pub const DEFAULT_HANDLE_DOMAIN: &str = "zurfur.app";

/// Serde default for [`Config::handle_domain`]: [`DEFAULT_HANDLE_DOMAIN`],
/// a known-valid namespace, so the parse can't fail.
fn default_handle_domain() -> HandleDomain {
    DEFAULT_HANDLE_DOMAIN
        .parse()
        .expect("the default handle domain is valid")
}

/// Deserialize [`Config::handle_domain`] through [`HandleDomain`]'s validating
/// `FromStr`: normalized once here, and a value that is no namespace at all
/// fails the load rather than silently matching nothing.
fn deserialize_handle_domain<'de, D>(deserializer: D) -> Result<HandleDomain, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    raw.parse::<HandleDomain>().map_err(de::Error::custom)
}

/// Serde default for [`Config::plc_directory_endpoint`]: a local placeholder,
/// never the canonical public log.
fn default_plc_directory_endpoint() -> String {
    "http://localhost:2582".to_string()
}

/// The raw bytes of the example dev root key shipped in `.env.example`. Its
/// private value is public, so the boot guard refuses it wherever real minting
/// could happen.
pub const EXAMPLE_DEV_ROOT_KEY: &[u8] = b"dev-only-root-key-do-not-ship!!!";

/// Refuse to boot a configuration that could mint real account identities under
/// dev-only key custody. Errors in `PROD`/`STG` (v1 custody is always
/// config/env-root-backed) and whenever `submit` is on under
/// [`EXAMPLE_DEV_ROOT_KEY`]; `Ok(())` for the safe dev configurations.
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

/// Serde default for [`Config::http_addr`]: `127.0.0.1:3621`, a known-valid
/// socket, so the parse can't fail.
fn default_http_addr() -> SocketAddr {
    "127.0.0.1:3621".parse().unwrap()
}

impl Config {
    /// Default for [`Config::max_upload_bytes`]: 50 MiB, the one home for the
    /// number. Raising it past `i32::MAX` bytes is a wire break — the contract's
    /// `byte_size` must then become an int64 decimal string
    /// (`contract/VERSIONING.md` §7.2).
    pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;

    /// Load and validate the runtime [`Config`], selecting the profile from
    /// `ZURFUR_ENV` (default `dev`). Layered lowest-first:
    /// `config/{profile}.toml`, the unprefixed `DATABASE_URL`, then `ZURFUR_*`
    /// env. The TOML file is optional; a missing required key fails the load.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Self::load_from(None)
    }

    /// [`load`](Config::load) with the config directory chosen by the caller.
    /// `None` falls back to `ZURFUR_CONFIG_DIR`, then to this crate's
    /// `CARGO_MANIFEST_DIR`-anchored `backend/config` — never the CWD, since
    /// cargo, cargo-watch and `just` each run from a different one.
    pub fn load_from(config_dir: Option<PathBuf>) -> Result<Self, Box<figment::Error>> {
        let profile = std::env::var(PROFILE_ENV).unwrap_or_else(|_| "dev".into());
        // The profile names a file; keep it a bare name so `ZURFUR_ENV=../x`
        // can never walk out of the config dir.
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

    // A production-like boot is refused whichever root key is set.
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

    // The safe dev configurations pass.
    #[test]
    fn dev_configurations_are_allowed() {
        let real_key = [0xABu8; 32];
        assert!(ensure_custody_hardened(&Environment::DEV, &real_key, false).is_ok());
        assert!(ensure_custody_hardened(&Environment::DEV, &real_key, true).is_ok());
        // Dev with the example key but NOT submitting is fine (the common local case).
        assert!(ensure_custody_hardened(&Environment::DEV, EXAMPLE_DEV_ROOT_KEY, false).is_ok());
    }

    // Precedence, lowest first: profile TOML < `DATABASE_URL` < `ZURFUR_*` env.
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
            assert_eq!(config.handle_domain.as_str(), "zurfur.app");
            assert_eq!(config.max_upload_bytes, Config::DEFAULT_MAX_UPLOAD_BYTES);
            Ok(())
        });
    }

    // The handle namespace normalizes at load, so no call site re-normalizes it.
    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's closure signature
    fn the_handle_domain_is_normalized_at_load() {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", "dev");
            jail.set_env("DATABASE_URL", "postgres://x");
            jail.set_env("ZURFUR_PUBLIC_URL", "http://x");
            jail.set_env("ZURFUR_LOG_LEVEL", "info");
            jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");
            jail.set_env("ZURFUR_HANDLE_DOMAIN", " .Zurfur.App. ");

            let config = Config::load().map_err(|e| *e)?;
            assert_eq!(config.handle_domain.as_str(), "zurfur.app");
            Ok(())
        });
    }

    // ...and a value that is no namespace at all fails the LOAD.
    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's closure signature
    fn an_empty_handle_domain_fails_the_load() {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", "dev");
            jail.set_env("DATABASE_URL", "postgres://x");
            jail.set_env("ZURFUR_PUBLIC_URL", "http://x");
            jail.set_env("ZURFUR_LOG_LEVEL", "info");
            jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");
            jail.set_env("ZURFUR_HANDLE_DOMAIN", " . ");

            let Err(error) = Config::load() else {
                panic!("an empty handle domain must not load");
            };
            assert!(error.to_string().contains("must not be empty"), "{error}");
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

    // A required key missing from every layer fails the load — never a default.
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

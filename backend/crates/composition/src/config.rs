//! The runtime [`Config`], its figment loader with the environment guard
//! ([`ConfigRefusal`]), the [`BareOrigin`] its `public_url` must be, and the
//! boot-time custody guard.

mod origin;

use std::net::SocketAddr;
use std::path::PathBuf;

use domain::elements::handle::HandleDomain;
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::{Deserialize, Deserializer, de};

pub use origin::{BareOrigin, BareOriginError};

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
    /// Local development: plain HTTP on loopback, non-`Secure` cookies. Boots
    /// only on this machine; see [`ConfigRefusal`].
    #[serde(alias = "dev")]
    DEV,
    /// Staging: HTTPS, `Secure` cookies.
    #[serde(alias = "stg")]
    STG,
    /// Production: HTTPS, `Secure` cookies.
    #[serde(alias = "prod")]
    PROD,
}

impl Environment {
    /// Whether this environment serves HTTPS, so its cookies are `Secure`.
    pub fn secure_cookies(&self) -> bool {
        matches!(self, Self::STG | Self::PROD)
    }
}

/// The fully-resolved runtime configuration, produced by [`Config::load`] and
/// handed to [`Runtime::connect`](crate::Runtime::connect). Every field is
/// required at boot except [`http_addr`](Config::http_addr) and
/// [`handle_domain`](Config::handle_domain), which default.
#[derive(Clone, Deserialize)]
pub struct Config {
    /// The deployment profile; see [`Environment`].
    pub env: Environment,
    /// The socket the HTTP server binds. Defaults to `127.0.0.1:3621`; `DEV`
    /// refuses a non-loopback address.
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    /// Externally-visible origin (scheme + host + port). Every redirect and the
    /// OAuth redirect URI are built on it. The load refuses anything but a
    /// [`BareOrigin`], and one that does not fit [`env`](Config::env); see
    /// [`ConfigRefusal`].
    #[serde(deserialize_with = "deserialize_public_url")]
    pub public_url: BareOrigin,
    /// Postgres connection string for the pool built at boot. Read from the
    /// unprefixed `DATABASE_URL`, the name sqlx tooling expects.
    pub database_url: String,
    /// Default tracing filter, applied when `RUST_LOG` is unset.
    pub log_level: String,
    /// The DNS suffix Zurfur issues Account handles under, e.g. `zurfur.app`.
    /// Parsed once here, so an invalid namespace fails the boot and the claim
    /// checks and the well-known resolver cannot disagree. `STG` refuses
    /// production's namespace, the default, and every subdomain of it.
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

/// Deserialize [`Config::public_url`] as a [`BareOrigin`] without echoing the
/// value: a URL can carry credentials, so the error names only the rule that
/// failed.
fn deserialize_public_url<'de, D>(deserializer: D) -> Result<BareOrigin, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    raw.parse::<BareOrigin>().map_err(|refusal| {
        de::Error::custom(format!(
            "`public_url` must be a bare origin (a scheme, a host and an optional port): {refusal}"
        ))
    })
}

/// The origins `DEV` may boot on, any port: the loopback return addresses an
/// atproto authorization server accepts.
const LOOPBACK: [&str; 2] = ["http://127.0.0.1", "http://[::1]"];

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
             custody must land before any real account is minted."
        );
    }
    if submit && is_example_key {
        anyhow::bail!(
            "refusing PLC directory submission: the did:plc custody root key is the shipped \
             example key (its private value is public). Set a real ZURFUR_DID_KEY_ROOT_KEY and \
             use KMS-backed custody."
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
    /// env. The TOML file is optional; a missing required key fails the load,
    /// and so does a configuration its environment refuses
    /// ([`LoadError::Refused`]).
    pub fn load() -> Result<Self, LoadError> {
        Self::load_from(None)
    }

    /// [`load`](Config::load) with the config directory chosen by the caller.
    /// `None` falls back to `ZURFUR_CONFIG_DIR`, then to this crate's
    /// `CARGO_MANIFEST_DIR`-anchored `backend/config` — never the CWD, since
    /// cargo, cargo-watch and `just` each run from a different one.
    pub fn load_from(config_dir: Option<PathBuf>) -> Result<Self, LoadError> {
        let profile = std::env::var(PROFILE_ENV).unwrap_or_else(|_| "dev".into());
        // The profile names a file; keep it a bare name so `ZURFUR_ENV=../x`
        // can never walk out of the config dir.
        if profile.is_empty()
            || !profile
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            let profile_error = figment::Error::from(format!(
                "{PROFILE_ENV} must be a bare profile name (letters, digits, '-'), got {profile:?}"
            ));
            return Err(LoadError::from(Box::new(profile_error)));
        }

        let config_dir = config_dir
            .or_else(|| std::env::var_os(CONFIG_DIR_ENV).map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../config")));
        let profile_file = config_dir.join(format!("{profile}.toml"));

        let config: Self = Figment::new()
            .merge(Toml::file(profile_file))
            .merge(Env::raw().only(&[DATABASE_URL_ENV]))
            .merge(Env::prefixed(ENV_PREFIX))
            .extract()
            .map_err(Box::new)?;
        config.ensure_fits_environment()?;
        Ok(config)
    }

    /// Refuse a configuration its [`env`](Config::env) may not boot with:
    /// `DEV` only on this machine, `STG` and `PROD` only on an `https` origin
    /// named by a public domain, and `STG` never on or under production's
    /// handle namespace.
    fn ensure_fits_environment(&self) -> Result<(), ConfigRefusal> {
        match self.env {
            Environment::DEV => {
                if !self.public_url.is_of_origin(&LOOPBACK) {
                    return Err(ConfigRefusal::DevPublicUrlNotLoopback);
                }
                if !self.http_addr.ip().to_canonical().is_loopback() {
                    return Err(ConfigRefusal::DevHttpAddrNotLoopback);
                }
            }
            Environment::STG | Environment::PROD => {
                if !self.public_url.is_public_https() {
                    let not_https_error = ConfigRefusal::PublicUrlNotHttpsOrigin {
                        env: self.env.clone(),
                    };
                    return Err(not_https_error);
                }
            }
        }
        let staging_on_production_namespace = matches!(self.env, Environment::STG)
            && is_on_or_under_production_namespace(&self.handle_domain);
        if staging_on_production_namespace {
            return Err(ConfigRefusal::StagingOnProductionHandleDomain);
        }
        Ok(())
    }
}

/// Why [`Config::load`] failed: the layered sources would not extract into a
/// [`Config`], or its environment refused what they gave.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// A source could not be read, or did not deserialize into a [`Config`].
    #[error(transparent)]
    Figment(#[from] Box<figment::Error>),
    /// The configuration loaded, but its environment may not boot with it.
    #[error(transparent)]
    Refused(#[from] ConfigRefusal),
}

/// Why [`Config::load`] refused a configuration for its environment. Each
/// message names the setting at fault and never echoes its value.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigRefusal {
    /// `DEV`'s `public_url` is not a loopback origin a sign-in can return to.
    #[error(
        "refusing to boot DEV off this machine: `public_url` must be `http://127.0.0.1` or \
         `http://[::1]` (any port); `localhost` is refused because Bluesky refuses it as a \
         sign-in return address"
    )]
    DevPublicUrlNotLoopback,
    /// `DEV`'s `http_addr` listens beyond loopback.
    #[error("refusing to boot DEV off this machine: `http_addr` must be a loopback address")]
    DevHttpAddrNotLoopback,
    /// A deployed environment's `public_url` is not an `https` origin named by
    /// a public domain.
    #[error(
        "refusing to boot {env:?}: `public_url` must be an `https` origin on a domain name \
         of two or more labels without a trailing dot, with an optional non-zero port; \
         no `localhost` and no IP address"
    )]
    PublicUrlNotHttpsOrigin {
        /// The environment that demanded HTTPS.
        env: Environment,
    },
    /// `STG`'s `handle_domain` is production's namespace or a subdomain of it.
    #[error(
        "refusing to boot STG: `handle_domain` is production's namespace \
         `{DEFAULT_HANDLE_DOMAIN}` or a subdomain of it; set staging's own"
    )]
    StagingOnProductionHandleDomain,
}

impl ConfigRefusal {
    /// The configuration key the refusal is about.
    pub fn setting(&self) -> &'static str {
        match self {
            Self::DevPublicUrlNotLoopback | Self::PublicUrlNotHttpsOrigin { .. } => "public_url",
            Self::DevHttpAddrNotLoopback => "http_addr",
            Self::StagingOnProductionHandleDomain => "handle_domain",
        }
    }
}

/// Whether `handle_domain` is production's namespace, [`DEFAULT_HANDLE_DOMAIN`],
/// or a subdomain of it.
fn is_on_or_under_production_namespace(handle_domain: &HandleDomain) -> bool {
    let namespace = handle_domain.as_str();
    namespace
        .strip_suffix(DEFAULT_HANDLE_DOMAIN)
        .is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('.'))
}

#[cfg(test)]
mod tests;

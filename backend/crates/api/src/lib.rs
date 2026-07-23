//! The composition root and HTTP surface of the Zurfur backend.
//!
//! This crate is the one place that knows which adapters are live. It owns
//! [`Config`] (figment-loaded), the shared [`AppState`] (a bag of trait
//! objects, one per port), and the axum [`app`] router. Domain logic lives in
//! `domain`; persistence and the PDS live behind the `adapter-*` crates; this
//! crate only wires them together and translates between HTTP and those ports.
//!
//! The HTTP surface is split into per-domain route groups under [`mod@routes`]
//! (`health`, `session`, `accounts`, `commissions`), each exposing a `*_router()` builder;
//! [`app`] is pure composition that merges them. Two shapes of endpoint coexist.
//! The browser-facing sign-in flow (`/signin`, `/signin-callback`, `/me`,
//! `/logout`) redirects or speaks JSON; the human-facing HTML lives in the
//! SvelteKit frontend (ZMVP-151). The account/membership API (`POST /accounts`,
//! `.../members`, `.../invitations`) speaks JSON and returns status codes — an
//! unrecognized caller gets a `401`, never a redirect, because the frontend calls
//! these rather than browsing to them.
//!
//! References: DESIGN "Domains and Applications" (ports and adapters);
//! DESIGN/Account, DESIGN/Roles; ZMVP-8 through ZMVP-16; ZMVP-151.

use std::net::SocketAddr;
use std::sync::Arc;

use adapter_pg::PgPool;
use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
};
use domain::ports::{
    AccountStore, Authenticator, ChangelogStore, CommissionStore, Database, DidMinter, FileStore,
    ProfileCache, ProfileSource, UnitOfWorkFn, UserStore,
};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use tower_http::set_header::SetResponseHeaderLayer;

mod problem;
mod routes;
mod sweep;

pub use sweep::{run_deadline_sweeper, sweep_deadlines};

/// Session key under which the recognized visitor's `UserId` is stored. The
/// session carries our own key, not the DID: subsequent requests resolve
/// session → User through the repo, never re-asking the PDS (ZMVP-9 Criterion 3).
pub(crate) const SESSION_USER_KEY: &str = "user_id";

/// The deployment profile, selected by `ZURFUR_ENV` (`dev` → [`DEV`]). The only
/// behavioral fork it drives today is cookie security: [`STG`] and [`PROD`] set
/// the session cookie `Secure` (HTTPS-only) in `main`, while [`DEV`] leaves it
/// off so loopback HTTP doesn't drop the cookie.
///
/// Caveats: deserialized from config/env, so the spelling must match a variant
/// exactly (`DEV`/`STG`/`PROD`). New environments are an enum change, not config.
///
/// [`DEV`]: Environment::DEV
/// [`STG`]: Environment::STG
/// [`PROD`]: Environment::PROD
#[derive(Clone, Debug, Deserialize)]
pub enum Environment {
    /// Local development: plain HTTP on loopback, non-`Secure` cookies.
    DEV,
    /// Staging: HTTPS, `Secure` cookies — a production-shaped environment.
    STG,
    /// Production: HTTPS, `Secure` cookies.
    PROD,
}

/// The fully-resolved runtime configuration, produced by [`Config::load`] and
/// then moved into [`AppState`]. Every field is required at boot except
/// [`http_addr`], which defaults to `127.0.0.1:3621`, and [`handle_domain`], which
/// defaults to `zurfur.app`.
///
/// Caveats: figment layers config/{profile}.toml first, then `DATABASE_URL`,
/// then `ZURFUR_*` env (env wins); a missing required key fails the load.
/// [`database_url`] is read from the unprefixed `DATABASE_URL` on purpose — sqlx
/// tooling reads that exact name. [`public_url`] is the externally-visible
/// origin and must be a parseable URI: `main` builds the OAuth redirect URI from
/// it and aborts boot if it can't.
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
    /// (25 MiB); override via `ZURFUR_MAX_UPLOAD_BYTES`. The
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
    /// Default for [`Config::max_upload_bytes`]: 25 MiB — generous for a
    /// work-in-progress art file, bounded so no upload is uncapped (ZMVP-88).
    /// The one home for the number; the serde default and every test fixture
    /// reference it.
    pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;

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
        let profile = std::env::var("ZURFUR_ENV").unwrap_or_else(|_| "dev".into());

        // Anchor the config directory to this crate rather than the current working
        // directory: cargo, cargo-watch, and `just` all run from different CWDs, so a
        // relative `config/...` path resolves inconsistently. `CARGO_MANIFEST_DIR` is
        // `backend/crates/api`; the config lives at `backend/config`. A deployed binary
        // can point elsewhere via `ZURFUR_CONFIG_DIR`.
        let config_dir = std::env::var("ZURFUR_CONFIG_DIR")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../config").into());

        Figment::new()
            .merge(Toml::file(format!("{config_dir}/{profile}.toml")))
            .merge(Env::raw().only(&["DATABASE_URL"]))
            .merge(Env::prefixed("ZURFUR_"))
            .extract()
            .map_err(Box::new)
    }
}

/// The shared application state every handler receives via axum's [`State`]
/// extractor — the composition root's bag of dependencies. It is `Clone` (the
/// pool and every port are cheaply cloneable, behind [`PgPool`]/[`Arc`]), so axum
/// can hand each request its own copy.
///
/// Each port is an `Arc<dyn Trait>` precisely so the wiring picks the live
/// adapter once, in `main`, and the handlers stay ignorant of it: pg/atproto in
/// production, the in-process fakes (mem + a fake PDS) in the e2e tests. Adding a
/// capability is adding a field here plus a line in `main` — never a handler
/// rewrite.
///
/// References: DESIGN "Domains and Applications"; [`app`].
///
/// [`State`]: axum::extract::State
#[derive(Clone)]
pub struct AppState {
    /// The resolved runtime [`Config`]. Kept whole so handlers and `main` read
    /// the same values (e.g. cookie security keys off [`Config::env`]).
    pub config: Config,
    /// The Postgres connection pool. Shared directly (not behind a port) because
    /// it backs both the adapters built over it and the `health` probe.
    pub pool: PgPool,
    /// The [`Authenticator`] port: drives the OAuth handshake with a visitor's
    /// PDS — `start` yields the authorization URL, `complete` exchanges the
    /// callback for a DID. A trait object so the composition root chooses the
    /// live adapter (atproto's `AtprotoAuthenticator` in `main`, a fake PDS in
    /// e2e tests). Used by the `signin` and `signin_callback` handlers.
    pub auth: Arc<dyn Authenticator>,
    /// The [`UserStore`] read port: resolves a recognized visitor by id
    /// (`find`, the session-resolution path) or DID (`find_by_did`), off the pool.
    /// *Recognition* (`provision`) is a write and lives on the
    /// [`UnitOfWork`](domain::ports::UnitOfWork) vended by [`database`](AppState::database).
    /// pg in `main`, mem in tests.
    pub users: Arc<dyn UserStore>,
    /// The [`ProfileSource`] port: reads public profiles from the PDS. atproto
    /// in `main`, a fake in tests. A failure here degrades the `me` page to the
    /// DID rather than erroring.
    pub profile_source: Arc<dyn ProfileSource>,
    /// The [`ProfileCache`] port: private read-through cache fronting
    /// [`profile_source`](AppState::profile_source). Both `get` and the best-effort
    /// `put` are pool-backed — the cache fill is a documented exception to the Unit
    /// of Work (a read-path write with no transactional invariant; DD `24150017`).
    /// pg in `main` (entries expire after an hour, set in `main`), mem in tests.
    /// See `resolve_profile`.
    pub profile_cache: Arc<dyn ProfileCache>,
    /// The [`AccountStore`] read port: account/membership/invitation reads
    /// (`find`, `role_of`, `find_pending_invitation`, `find_invitation`) off the
    /// pool. Every account *write* lives on the [`UnitOfWork`](domain::ports::UnitOfWork)
    /// vended by [`database`](AppState::database). pg in `main`, mem in tests.
    pub accounts: Arc<dyn AccountStore>,
    /// The [`CommissionStore`] read port (ZMVP-87): the canonical commission
    /// reads — `find`, and the `is_participant` predicate every "a Participant
    /// does X" endpoint authorizes through (owner-arm-only until ZMVP-79 adds
    /// the seated arm). Commission *writes* live on the
    /// [`UnitOfWork`](domain::ports::UnitOfWork) vended by
    /// [`database`](AppState::database). pg in `main`, mem in tests.
    pub commissions: Arc<dyn CommissionStore>,
    /// The [`ChangelogStore`] read port (ZMVP-87): the ordered, participant-only
    /// changelog read. The *append* is a [`UnitOfWork`](domain::ports::UnitOfWork)
    /// view (`uow.changelog()`) — entries commit atomically with the domain
    /// writes they record (Changelog DD D4). pg in `main`, mem in tests.
    pub changelog: Arc<dyn ChangelogStore>,
    /// The [`FileStore`] port (ZMVP-88): the private blob store behind a commission
    /// file entry. Pool-backed and **outside** the Unit of Work — the blob write is
    /// a step that precedes the unit recording the file entry (bytes cannot ride a
    /// transaction; orphan-on-rollback accepted). v1 ships a mock/local
    /// implementation (a pg `bytea` table in `main`, the in-memory fake in tests);
    /// the real blob architecture is the future blob-architecture walkthrough.
    pub files: Arc<dyn FileStore>,
    /// The [`Database`] write factory: the **only** way to reach a private-store
    /// domain write. A handler calls `begin()`, issues its writes through the
    /// returned [`UnitOfWork`](domain::ports::UnitOfWork)'s view accessors
    /// (`uow.accounts().create(...)`, `uow.users().provision(...)`), then
    /// `commit()`s once (drop = rollback). Such writes cannot skip a transaction by
    /// construction (DD `24150017`). The profile cache is a documented exception —
    /// its best-effort fill is pool-backed (see [`profile_cache`](AppState::profile_cache)).
    /// pg in `main`, mem in tests.
    pub database: Arc<dyn Database>,
    /// The [`DidMinter`] port: mints a sovereign `did:plc` for a newly founded
    /// account. The live adapter is `RealDidMinter` (generates the account's
    /// rotation keys, signs an identity-only genesis operation, custodies the keys
    /// via `PgKeyStore`, and submits to a — no-op in v1 — directory); the mem/stub
    /// minter is used in tests. Used by the `create_account` handler.
    pub did_minter: Arc<dyn DidMinter>,
}

/// Run `f` inside one private-store transaction — the **one** place in this
/// crate that orchestrates `begin`/`commit`/`rollback` (DD "Transactions as a
/// capability" `24150017`). Opens a [`UnitOfWork`](domain::ports::UnitOfWork) via
/// [`Database::begin`](domain::ports::Database::begin), hands it to `f` as
/// `&mut dyn UnitOfWork`, then **commits on `Ok`, rolls back on `Err`**: the
/// closure body *is* the transaction boundary, so a commit can never be
/// forgotten. Strictly intra-Postgres; never a cross-store dual write.
///
/// `pub(crate)`, taking a bare `&dyn Database`, so both
/// [`AppState::transaction`] (every route) and [`sweep::sweep_deadlines`] (the
/// deadline sweeper, which holds a `Database` handle but no [`AppState`]) share
/// this one orchestrator rather than each re-implementing commit/rollback
/// (ZMVP-111; Engineer ruling on PR #100). `f`'s bound is
/// [`UnitOfWorkFn`](domain::ports::UnitOfWorkFn) plus explicit `F: Send`/`T: Send`
/// (the returned future holds both across `.await`s, so `Fut: Send` alone would
/// not keep a handler future `Send`), not std's `AsyncFnOnce` — see
/// that trait's doc comment for why (a compiler limitation with higher-ranked
/// `AsyncFnOnce` bounds, rust-lang/rust#110338).
pub(crate) async fn transaction<T, F>(db: &dyn Database, f: F) -> anyhow::Result<T>
where
    F: for<'a> UnitOfWorkFn<'a, T> + Send,
    T: Send,
{
    let mut uow = db.begin().await?;
    match f(&mut *uow).await {
        Ok(value) => {
            uow.commit().await?;
            Ok(value)
        }
        Err(err) => {
            // The closure's error is the meaningful one (e.g. `HandleTaken` →
            // 409); a rollback failure must never replace it. The unit is
            // abandoned either way (an uncommitted transaction also rolls back
            // on drop), so a rollback error here is secondary and deliberately
            // not surfaced over `err`.
            let _ = uow.rollback().await;
            Err(err)
        }
    }
}

impl AppState {
    /// Run `f` inside one private-store transaction — the **only** way a route
    /// reaches a private-store write. Delegates to the crate-level
    /// [`transaction`] orchestrator over [`self.database`](AppState::database).
    ///
    /// The call site reads `state.transaction(async |uow: &mut dyn UnitOfWork| {
    /// … }).await?` — an `async` closure, no `Box::pin`, no `&*state.database`.
    pub async fn transaction<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        F: for<'a> UnitOfWorkFn<'a, T> + Send,
        T: Send,
    {
        transaction(&*self.database, f).await
    }
}

/// Builds the axum [`Router`] over an [`AppState`], composing the per-domain route
/// groups from [`mod@routes`]. This is the canonical route table; the e2e tests and
/// `main` both mount it. `main` additionally layers the session middleware (the
/// [`Session`](tower_sessions::Session) extractor handlers rely on comes from that
/// layer, applied outside this fn).
///
/// Composition follows DESIGN "Domains and Applications": each area exposes a
/// `*_router()` builder and this fn merges them. A namespace boundary is also a
/// **policy boundary**, so the CSRF [`require_first_party_origin`](routes::require_first_party_origin)
/// guard is layered over the **cookie surface only** — `session` + `accounts` — and
/// not over `/health`, nor (in future) over the bearer `/plugin/v1` namespace, which
/// authenticates by `app_key` and is exempt by construction (ZMVP-23, DD "Auth
/// Surfaces, the Plugin Trust Boundary & CSRF"). That same cookie surface also carries
/// a `Cache-Control: no-store` response layer so authenticated identity/PII JSON is
/// never cached by a browser or shared intermediary (CWE-525, ZMVP-151); the public
/// `/health` and `/.well-known` GETs stay cacheable.
///
/// Routes: `GET /health`; `GET /.well-known/atproto-did` (handle resolution, also
/// top-level and CSRF-exempt); the sign-in flow (`POST /signin`,
/// `GET /signin-callback`, `GET /me`, `POST /logout`); the accounts tree
/// (`POST /accounts`, `POST`/`DELETE /accounts/{id}/members`,
/// `DELETE /accounts/{id}/members/me`, `POST`/`DELETE /accounts/{id}/invitations`,
/// `POST /accounts/{id}/invitations/decline`, `POST /accounts/{id}/invitations/accept`);
/// and the commissions tree (`POST /commissions` — user-scoped, no Account
/// required; `GET /commissions/{id}/changelog`, `POST /commissions/{id}/notes`,
/// `PUT`/`DELETE /commissions/{id}/channel` — participant-gated behind the
/// closed-door uniform 404, ZMVP-87).
///
/// Cross-persona unlinkability (ZMVP-17): this table is the public surface, and
/// no route on it may correlate one person's separate handles — join one
/// handle's User/Account graph to another's *as the same human*. The separation
/// holds by construction (separate handles → separate Users → separate DIDs/
/// logins); the only sanctioned correlation, opt-in User-Linking ("alts"), is
/// post-MVP. Before adding a read route that enumerates Users or returns the set
/// of handles/accounts tied to a person, weigh it against that invariant — a
/// single-account member roster (DESIGN/1DD decision 5) is fine, a person-level
/// "their other personas" surface is not. Guarded by
/// `tests/cross_persona_unlinkability.rs`.
///
/// References: [`AppState`]; the per-group docs under [`mod@routes`].
///
/// ```ignore
/// let router = api::app(state).layer(session_layer);
/// ```
pub fn app(state: AppState) -> Router {
    // Stamp `Cache-Control: no-store` on every cookie-surface response so a browser
    // or shared intermediary never caches authenticated identity/PII JSON — e.g.
    // `GET /me` (CWE-525, ZMVP-151 security review). `if_not_present` yields to any
    // handler that sets its own cache policy (none do today). Scoped to the cookie
    // surface deliberately: the public `/health` and `/.well-known` GETs are left
    // cacheable — over-scoping to them was called out in review.
    let no_store_cookie_surface = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );

    // The cookie surface: the browser/session flow and the account API, both reached
    // with the ambient session cookie. The first-party-`Origin` (CSRF) guard wraps
    // this surface once — a state-changing request from a foreign `Origin` is refused —
    // and the no-store layer sits outermost so even a CSRF rejection is uncacheable.
    let cookie_surface = routes::session_router()
        .merge(routes::accounts_router())
        .merge(routes::commissions_router(
            // Checked, not `as`: on a 32-bit target an oversized configured cap
            // saturates to usize::MAX instead of silently truncating the limit.
            usize::try_from(state.config.max_upload_bytes).unwrap_or(usize::MAX),
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::require_first_party_origin,
        ))
        .layer(no_store_cookie_surface);

    // `/health` and the atproto `/.well-known/atproto-did` resolver are mounted
    // top-level, deliberately outside the CSRF layer (they bear no cookie and change
    // no state — the resolver is a public unauthenticated GET). The future bearer
    // `/plugin/v1` namespace nests here too, exempt by construction rather than by a
    // remembered carve-out.
    Router::new()
        .merge(routes::health_router())
        .merge(routes::wellknown_router())
        .merge(cookie_surface)
        .with_state(state)
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
}

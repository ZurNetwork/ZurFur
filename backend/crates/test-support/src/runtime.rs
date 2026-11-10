//! The in-memory [`Runtime`] every driver's fast suite boots: one shared
//! fixture instead of a copy per test file. The fakes come from
//! `adapter-mem`; the pool is lazy and never connects; the config is a dev
//! profile; the Den's MAC handle is the test custodian's.

use std::sync::Arc;

use adapter_mem::{
    MemAuthenticator, MemBackend, MemDidMinter, MemIdentityResolver, MemProfileSource,
};
use composition::{BareOrigin, Config, Environment, Runtime};
use domain::elements::{bug_report::BuildId, did::Did, profile::Profile};

use crate::custody::test_custody;

/// The `Config::database_url` the fixture carries — what an identity file's
/// fingerprint must be taken against to match this runtime.
pub const DATABASE_URL: &str = "postgres://unused/unused";

/// A [`Runtime`] over the in-memory fakes plus the [`MemBackend`] behind it,
/// so a test can reach into the stores it wired.
pub struct MemRuntime {
    pub runtime: Runtime,
    pub backend: MemBackend,
}

/// Builder for [`MemRuntime`]; see [`mem`].
pub struct MemRuntimeBuilder {
    did: Did,
    profile: Profile,
    public_url: BareOrigin,
    max_upload_bytes: u64,
    build_id: BuildId,
}

/// Start a runtime that authenticates as `did`, with a bare profile under the
/// fixed handle `fixture.bsky.social`; override pieces before
/// [`build`](MemRuntimeBuilder::build).
pub fn mem(did: &Did) -> MemRuntimeBuilder {
    MemRuntimeBuilder {
        did: did.clone(),
        profile: Profile::new(did.clone(), "fixture.bsky.social"),
        public_url: "http://127.0.0.1:0".parse().expect("a bare origin"),
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        build_id: BuildId::try_from("unknown").expect("a build id"),
    }
}

impl MemRuntimeBuilder {
    /// The profile the fake PDS answers with for the acting DID.
    pub fn profile(mut self, profile: Profile) -> Self {
        self.profile = profile;
        self
    }

    /// The externally-visible origin (`Config::public_url`); panics when
    /// `public_url` is not a bare origin.
    pub fn public_url(mut self, public_url: impl AsRef<str>) -> Self {
        self.public_url = public_url.as_ref().parse().expect("a bare origin");
        self
    }

    /// `Config::max_upload_bytes`.
    pub fn max_upload_bytes(mut self, max_upload_bytes: u64) -> Self {
        self.max_upload_bytes = max_upload_bytes;
        self
    }

    /// `Config::build_id`, the API build a bug report records.
    pub fn build_id(mut self, build_id: BuildId) -> Self {
        self.build_id = build_id;
        self
    }

    pub fn build(self) -> MemRuntime {
        let backend = MemBackend::new();
        let config = Config {
            env: Environment::DEV,
            http_addr: "127.0.0.1:0".parse().expect("loopback socket address"),
            public_url: self.public_url,
            database_url: DATABASE_URL.to_string(),
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".parse().expect("a valid handle domain"),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: self.max_upload_bytes,
            build_id: self.build_id,
        };
        let runtime = Runtime {
            config,
            pool: adapter_pg::lazy_pool(DATABASE_URL).expect("lazy pool"),
            auth: Arc::new(MemAuthenticator::new(self.did)),
            users: backend.user_store(),
            profile_source: Arc::new(MemProfileSource::new(self.profile)),
            profile_cache: backend.profile_cache(),
            profile_lookups: Default::default(),
            identity_resolver: Arc::new(MemIdentityResolver::new()),
            accounts: backend.account_store(),
            commissions: backend.commission_store(),
            commission_rows: backend.commission_rows(),
            changelog: backend.changelog_store(),
            workflows: backend.workflow_store(),
            columns: backend.column_store(),
            characters: backend.character_store(),
            bug_reports: backend.bug_report_store(),
            files: backend.file_store(),
            database: backend.database(),
            did_minter: Arc::new(MemDidMinter::new()),
            den_mac: test_custody().den_mac,
        };
        MemRuntime { runtime, backend }
    }
}

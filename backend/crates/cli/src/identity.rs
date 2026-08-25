//! The identity file (ZMVP-203): *which* User the CLI acts as, recorded
//! locally after `login` and read by every command through
//! [`Principal`](crate::principal::Principal).
//!
//! One identity in v1 (Engineer ruling: overwritten on each login). The
//! record is versioned JSON, written `0600` and atomically (tempfile + rename
//! in the same directory), and bound to the database it was created against
//! by a [`fingerprint`] so a stale identity never acts on a different stack.
//! Corrupt or missing files are domain problems, never panics.
//!
//! Location: `$ZURFUR_CLI_HOME/identity.json` when that variable is set (the
//! harnesses use it), else the platform config dir via `directories`
//! (`$XDG_CONFIG_HOME/zurfur/identity.json` on Linux).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use domain::elements::did::Did;

use crate::CliError;

/// The current record version. Bump when the shape changes; a reader that
/// meets a newer version refuses rather than guessing.
pub const IDENTITY_VERSION: u32 = 1;

/// The file's name inside the CLI's config dir.
pub const IDENTITY_FILE_NAME: &str = "identity.json";

/// The environment variable that overrides the config dir.
pub const HOME_ENV: &str = "ZURFUR_CLI_HOME";

/// The persisted identity record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// Record version — [`IDENTITY_VERSION`] at write time.
    pub version: u32,
    /// The acting User's DID.
    pub did: String,
    /// [`fingerprint`] of the database this identity was recorded against.
    pub database_fingerprint: String,
    /// When `login` recorded it.
    pub created_at: DateTime<Utc>,
}

impl Identity {
    /// A fresh record for `did` against `database_url`, stamped now.
    pub fn new(did: impl Into<String>, database_url: &str) -> Self {
        Self {
            version: IDENTITY_VERSION,
            did: did.into(),
            database_fingerprint: fingerprint(database_url),
            created_at: Utc::now(),
        }
    }
}

/// A short, credential-free fingerprint of a database URL: SHA-256 of the
/// locator (`host:port/db`) — the part after the `user:password@`
/// credentials and before any `?query` (which may carry `password=`) — first
/// 16 hex chars. Two stacks on different hosts/ports/databases never collide.
pub fn fingerprint(database_url: &str) -> String {
    let without_query = database_url
        .split_once('?')
        .map_or(database_url, |(before, _)| before);
    let locator = without_query
        .rsplit_once('@')
        .map(|(_, after)| after)
        .or_else(|| without_query.split_once("://").map(|(_, after)| after))
        .unwrap_or(without_query);
    let digest = Sha256::digest(locator.as_bytes());
    let hex = format!("{digest:x}");
    hex[..16].to_string()
}

/// Where the identity file lives for this process: `$ZURFUR_CLI_HOME`, else
/// the platform config dir. Infrastructure problem `no_config_dir` when the
/// platform offers none.
pub fn default_path() -> Result<PathBuf, CliError> {
    if let Some(home) = std::env::var_os(HOME_ENV) {
        return Ok(PathBuf::from(home).join(IDENTITY_FILE_NAME));
    }
    let dirs = directories::ProjectDirs::from("app", "zurfur", "zurfur").ok_or_else(|| {
        CliError::infra(
            "no_config_dir",
            format!("no platform config directory; set {HOME_ENV}"),
        )
    })?;
    Ok(dirs.config_dir().join(IDENTITY_FILE_NAME))
}

/// Read the record at `path`. `Ok(None)` when absent; domain problem
/// `identity_corrupt` when present but not a valid record (bad JSON, another
/// version, not DID-shaped); infrastructure problem `identity_unreadable`
/// when the file exists but cannot be read (permissions, I/O).
pub fn load(path: &Path) -> Result<Option<Identity>, CliError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(CliError::infra(
                "identity_unreadable",
                format!("cannot read {}: {e}", path.display()),
            ));
        }
    };
    let identity: Identity = serde_json::from_slice(&bytes).map_err(|e| {
        CliError::domain(
            "identity_corrupt",
            format!("{} is not a valid identity record ({e}); run `zurfur session logout` to discard it", path.display()),
        )
    })?;
    if identity.version != IDENTITY_VERSION {
        return Err(CliError::domain(
            "identity_corrupt",
            format!(
                "{} is identity record version {}; this build reads exactly version {}",
                path.display(),
                identity.version,
                IDENTITY_VERSION
            ),
        ));
    }
    // The file is untrusted text: it enters the domain through `Did`'s
    // parsing door, never `Did::new`.
    identity.did.parse::<Did>().map_err(|e| {
        CliError::domain(
            "identity_corrupt",
            format!(
                "{} does not hold a DID ({e}); run `zurfur session logout` to discard it",
                path.display()
            ),
        )
    })?;
    Ok(Some(identity))
}

/// Write the record at `path`: parent dir created `0700`, file `0600`, atomic replace
/// (a tempfile in the same directory persisted over the target).
pub fn save(path: &Path, identity: &Identity) -> Result<(), CliError> {
    let dir = path.parent().ok_or_else(|| {
        CliError::infra(
            "identity_unwritable",
            format!("{} has no parent directory", path.display()),
        )
    })?;
    let unwritable = |e: std::io::Error| {
        CliError::infra(
            "identity_unwritable",
            format!("cannot write {}: {e}", path.display()),
        )
    };
    create_private_dir(dir).map_err(unwritable)?;
    let mut rendered = serde_json::to_vec_pretty(identity).expect("Identity serializes");
    rendered.push(b'\n');

    let mut temp = tempfile::Builder::new()
        .prefix(".identity-")
        .tempfile_in(dir)
        .map_err(unwritable)?;
    restrict_to_owner(temp.as_file()).map_err(unwritable)?;
    std::io::Write::write_all(&mut temp, &rendered).map_err(unwritable)?;
    temp.as_file().sync_all().map_err(unwritable)?;
    temp.persist(path).map_err(|e| unwritable(e.error))?;
    Ok(())
}

/// Remove the record at `path`. Idempotent: an absent file is `Ok(false)`.
pub fn delete(path: &Path) -> Result<bool, CliError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(CliError::infra(
            "identity_unwritable",
            format!("cannot remove {}: {e}", path.display()),
        )),
    }
}

/// Create the identity directory owner-only — and re-assert `0700` when it
/// already exists, since `DirBuilder` leaves a pre-existing directory's mode
/// alone (an earlier tool, or the user, may have made it `0755`).
#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn restrict_to_owner(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_to_owner(_file: &std::fs::File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fingerprint_ignores_credentials_and_the_query() {
        let a = fingerprint("postgres://alice:secret@db.local:5432/zurfur");
        let b = fingerprint("postgres://bob:other@db.local:5432/zurfur");
        let c = fingerprint("postgres://alice:secret@db.local:5433/zurfur");
        let d = fingerprint("postgres://db.local:5432/zurfur?password=hunter2");
        let e = fingerprint("postgres://db.local:5432/zurfur");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(d, e, "the query string never enters the hash");
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn a_non_did_in_the_file_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(IDENTITY_FILE_NAME);
        let mut identity = Identity::new("did:plc:abc", "postgres://h/d");
        identity.did = "'; DROP TABLE users; --".into();
        std::fs::write(&path, serde_json::to_vec(&identity).unwrap()).unwrap();
        assert_eq!(load(&path).unwrap_err().code(), "identity_corrupt");
    }

    #[test]
    fn unknown_fields_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(IDENTITY_FILE_NAME);
        let record = serde_json::json!({
            "version": IDENTITY_VERSION,
            "did": "did:plc:abc",
            "database_fingerprint": "0000000000000000",
            "created_at": "2026-08-24T00:00:00Z",
            "scopes": ["everything"]
        });
        std::fs::write(&path, record.to_string()).unwrap();
        assert_eq!(load(&path).unwrap_err().code(), "identity_corrupt");
    }

    #[test]
    fn save_load_delete_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join(IDENTITY_FILE_NAME);
        assert_eq!(load(&path).unwrap(), None);

        let identity = Identity::new("did:plc:abc", "postgres://x@h/d");
        save(&path, &identity).unwrap();
        assert_eq!(load(&path).unwrap(), Some(identity.clone()));

        // Overwrite is atomic and keeps the newest record.
        let newer = Identity::new("did:plc:def", "postgres://x@h/d");
        save(&path, &newer).unwrap();
        assert_eq!(load(&path).unwrap().unwrap().did, "did:plc:def");

        assert!(delete(&path).unwrap());
        assert!(!delete(&path).unwrap());
        assert_eq!(load(&path).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only_and_so_is_its_directory() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zurfur").join(IDENTITY_FILE_NAME);
        save(&path, &Identity::new("did:plc:abc", "postgres://h/d")).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }

    // A directory that already exists too openly is tightened, not trusted.
    #[cfg(unix)]
    #[test]
    fn a_pre_existing_open_directory_is_tightened() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("zurfur");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o755)).unwrap();
        save(
            &home.join(IDENTITY_FILE_NAME),
            &Identity::new("did:plc:abc", "postgres://h/d"),
        )
        .unwrap();
        let dir_mode = std::fs::metadata(&home).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
    }

    #[test]
    fn garbage_is_corrupt_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(IDENTITY_FILE_NAME);
        std::fs::write(&path, b"{not json").unwrap();
        let error = load(&path).unwrap_err();
        assert_eq!(error.code(), "identity_corrupt");
        assert_eq!(error.class(), crate::ExitClass::Domain);
    }

    #[test]
    fn any_other_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(IDENTITY_FILE_NAME);
        let future = serde_json::json!({
            "version": IDENTITY_VERSION + 1,
            "did": "did:plc:abc",
            "database_fingerprint": "0000000000000000",
            "created_at": "2026-08-24T00:00:00Z"
        });
        std::fs::write(&path, future.to_string()).unwrap();
        assert_eq!(load(&path).unwrap_err().code(), "identity_corrupt");
    }
}

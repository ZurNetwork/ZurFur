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

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

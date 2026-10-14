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
                public_url = "http://127.0.0.1:1111"
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
        jail.set_env("ZURFUR_PUBLIC_URL", "http://127.0.0.1:2222");

        let config = Config::load().expect("the config loads");
        assert!(matches!(config.env, Environment::DEV));
        assert_eq!(config.database_url, "postgres://from-env");
        let public_url: &url::Url = config.public_url.as_ref();
        assert_eq!(public_url.as_str(), "http://127.0.0.1:2222/");
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
        jail.set_env("ZURFUR_PUBLIC_URL", "http://127.0.0.1:8080");
        jail.set_env("ZURFUR_LOG_LEVEL", "info");
        jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");
        jail.set_env("ZURFUR_HANDLE_DOMAIN", " .Zurfur.App. ");

        let config = Config::load().expect("the config loads");
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
        jail.set_env("ZURFUR_PUBLIC_URL", "http://127.0.0.1:8080");
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

// A wildcard namespace is no hostname, so it fails the load too.
#[test]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
fn a_wildcard_handle_domain_fails_the_load() {
    figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
        jail.set_env("ZURFUR_ENV", "dev");
        jail.set_env("DATABASE_URL", "postgres://x");
        jail.set_env("ZURFUR_PUBLIC_URL", "http://127.0.0.1:8080");
        jail.set_env("ZURFUR_LOG_LEVEL", "info");
        jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");
        jail.set_env("ZURFUR_HANDLE_DOMAIN", "*.zurfur.app");

        let Err(error) = Config::load() else {
            panic!("a wildcard handle domain must not load");
        };
        assert!(error.to_string().contains("bare hostname"), "{error}");
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

// ---- The environment guard ---------------------------------------------

/// A loaded-looking configuration for `env`, over the given origin, listen
/// address and handle namespace.
fn config(env: Environment, public_url: &str, http_addr: &str, handle_domain: &str) -> Config {
    Config {
        env,
        http_addr: http_addr.parse().expect("a socket address"),
        public_url: public_url.parse().expect("a bare origin"),
        database_url: "postgres://x".to_owned(),
        log_level: "info".to_owned(),
        handle_domain: handle_domain.parse().expect("a valid handle domain"),
        did_key_root_key: "ZmlsZQ==".to_owned(),
        plc_directory_endpoint: default_plc_directory_endpoint(),
        plc_directory_submit: false,
        deadline_sweep_interval_secs: default_deadline_sweep_interval_secs(),
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
    }
}

fn dev(public_url: &str) -> Config {
    config(Environment::DEV, public_url, "127.0.0.1:8081", "zurfur.app")
}

fn staging(public_url: &str) -> Config {
    config(Environment::STG, public_url, "0.0.0.0:8081", "zurfur.foo")
}

fn production(public_url: &str) -> Config {
    config(Environment::PROD, public_url, "0.0.0.0:8081", "zurfur.app")
}

// DEV boots on the two loopback origins a sign-in can return to, any port.
#[test]
fn dev_boots_on_a_loopback_origin() {
    let loopback_origins = [
        "http://127.0.0.1",
        "http://127.0.0.1:8080",
        "http://127.0.0.1:8080/",
        "http://[::1]",
        "http://[::1]:8080",
    ];
    for origin in loopback_origins {
        let outcome = dev(origin).ensure_fits_environment();
        assert!(outcome.is_ok(), "{origin}: {outcome:?}");
    }
}

// Off this machine, or on a name the authorization server refuses, DEV won't
// boot. `localhost` is the case that would boot and then fail every sign-in.
#[test]
fn dev_refuses_a_public_url_off_loopback() {
    let refused_origins = [
        "http://localhost:8080",
        "http://zurfur.foo",
        "https://127.0.0.1:8080",
        "https://[::1]:8080",
        "http://127.0.0.2:8080",
        "http://0.0.0.0:8080",
        "http://[::]:8080",
        "http://[::2]:8080",
        "http://[::ffff:127.0.0.1]:8080",
    ];
    for origin in refused_origins {
        let outcome = dev(origin).ensure_fits_environment();
        assert!(
            matches!(outcome, Err(ConfigRefusal::DevPublicUrlNotLoopback)),
            "{origin}: {outcome:?}"
        );
    }
}

// A `public_url` carrying anything past the port is no origin: it fails the
// LOAD, in every environment, naming the rule and never echoing the value.
#[test]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
fn a_public_url_that_is_not_a_bare_origin_fails_the_load() {
    let not_bare = [
        ("DEV", "http://127.0.0.1:8080/app", "a path"),
        ("DEV", "http://127.0.0.1:8080//", "a path"),
        ("DEV", "http://127.0.0.1:8080?", "a query"),
        ("DEV", "http://127.0.0.1:8080/?next=x", "a query"),
        ("DEV", "http://127.0.0.1:8080#", "a fragment"),
        ("DEV", "http://user@127.0.0.1:8080", "credentials"),
        ("DEV", "http://user:secret@[::1]:8080", "credentials"),
        ("DEV", "http://:secret@127.0.0.1:8080", "credentials"),
        ("DEV", "file:///srv/zurfur", "a path"),
        ("STG", "https://zurfur.foo/app", "a path"),
        ("STG", "https://zurfur.foo/app/", "a path"),
        ("STG", "https://zurfur.foo/?next=x", "a query"),
        ("PROD", "https://zurfur.app/#top", "a fragment"),
        ("PROD", "https://user:secret@zurfur.app", "credentials"),
        ("PROD", "https://:secret@zurfur.app", "credentials"),
        ("PROD", "data:text/plain,zurfur", "a path"),
    ];
    for (env, public_url, rule) in not_bare {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", env);
            jail.set_env("DATABASE_URL", "postgres://x");
            jail.set_env("ZURFUR_PUBLIC_URL", public_url);
            jail.set_env("ZURFUR_HANDLE_DOMAIN", "zurfur.foo");
            jail.set_env("ZURFUR_LOG_LEVEL", "info");
            jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");

            let Err(LoadError::Figment(error)) = Config::load() else {
                panic!("{env} {public_url} must fail to load");
            };
            assert_eq!(error.path, ["public_url"], "{env} {public_url}");
            let message = error.to_string();
            assert!(message.contains("must be a bare origin"), "{message}");
            assert!(message.contains(rule), "{public_url}: {message}");
            assert!(!message.contains("secret"), "{message}");
            assert!(!message.contains("next=x"), "{message}");
            Ok(())
        });
    }
}

// DEV's API listens on loopback only; any loopback address will do.
#[test]
fn dev_refuses_to_listen_off_loopback() {
    let public_url = "http://127.0.0.1:8080";
    for http_addr in ["0.0.0.0:8081", "[::]:8081", "192.168.1.20:8081"] {
        let outcome =
            config(Environment::DEV, public_url, http_addr, "zurfur.app").ensure_fits_environment();
        assert!(
            matches!(outcome, Err(ConfigRefusal::DevHttpAddrNotLoopback)),
            "{http_addr}: {outcome:?}"
        );
    }
    for http_addr in [
        "127.0.0.1:8081",
        "127.0.0.2:8081",
        "[::1]:8081",
        "[::ffff:127.0.0.1]:8081",
    ] {
        let outcome =
            config(Environment::DEV, public_url, http_addr, "zurfur.app").ensure_fits_environment();
        assert!(outcome.is_ok(), "{http_addr}: {outcome:?}");
    }
}

// STG and PROD boot on an https bare origin, a port allowed.
#[test]
fn staging_and_production_boot_on_an_https_bare_origin() {
    for origin in ["https://zurfur.foo", "https://zurfur.foo:8443/"] {
        let staging_outcome = staging(origin).ensure_fits_environment();
        assert!(staging_outcome.is_ok(), "{origin}: {staging_outcome:?}");
    }
    for origin in ["https://zurfur.app", "https://zurfur.app/"] {
        let production_outcome = production(origin).ensure_fits_environment();
        assert!(
            production_outcome.is_ok(),
            "{origin}: {production_outcome:?}"
        );
    }
}

// Anything but https on a public domain name, on a port other than 0, is
// refused. A path, a query, a fragment or credentials never get this far: they
// fail the load.
#[test]
fn staging_and_production_refuse_anything_but_an_https_bare_origin() {
    let refused = [
        "http://zurfur.foo",
        "http://127.0.0.1:8080",
        "https://203.0.113.7",
        "https://[2001:db8::1]",
        "https://zurfur.foo.",
        "https://a..b",
        "https://zurfur.foo:0",
        "https://localhost",
        "https://localhost:8443",
        "https://zurfur",
        "https://zurfur.localhost",
    ];
    for url in refused {
        let staging_outcome = staging(url).ensure_fits_environment();
        assert!(
            matches!(
                staging_outcome,
                Err(ConfigRefusal::PublicUrlNotHttpsOrigin {
                    env: Environment::STG
                })
            ),
            "STG {url}: {staging_outcome:?}"
        );
        let production_outcome = production(url).ensure_fits_environment();
        assert!(
            matches!(
                production_outcome,
                Err(ConfigRefusal::PublicUrlNotHttpsOrigin {
                    env: Environment::PROD
                })
            ),
            "PROD {url}: {production_outcome:?}"
        );
    }
}

// Staging may not issue handles on or under production's namespace, set or
// defaulted.
#[test]
fn staging_refuses_production_handle_namespace_and_its_subdomains() {
    let production_namespaces = [
        DEFAULT_HANDLE_DOMAIN,
        "stg.zurfur.app",
        "staging.eu.zurfur.app",
    ];
    for handle_domain in production_namespaces {
        let on_production_namespace = config(
            Environment::STG,
            "https://zurfur.foo",
            "0.0.0.0:8081",
            handle_domain,
        );
        let outcome = on_production_namespace.ensure_fits_environment();
        assert!(
            matches!(outcome, Err(ConfigRefusal::StagingOnProductionHandleDomain)),
            "{handle_domain}: {outcome:?}"
        );
    }
    // A look-alike or a domain that merely contains it is staging's own.
    for handle_domain in ["zurfur.foo", "notzurfur.app", "zurfur.app.foo"] {
        let own_namespace = config(
            Environment::STG,
            "https://zurfur.foo",
            "0.0.0.0:8081",
            handle_domain,
        );
        let outcome = own_namespace.ensure_fits_environment();
        assert!(outcome.is_ok(), "{handle_domain}: {outcome:?}");
    }
    // Production and dev keep the default.
    let production_outcome = production("https://zurfur.app").ensure_fits_environment();
    assert!(production_outcome.is_ok(), "{production_outcome:?}");
    let dev_outcome = dev("http://127.0.0.1:8080").ensure_fits_environment();
    assert!(dev_outcome.is_ok(), "{dev_outcome:?}");
}

// Each refusal names its setting in its message.
#[test]
fn each_refusal_names_its_setting() {
    let staging_refusal = ConfigRefusal::PublicUrlNotHttpsOrigin {
        env: Environment::STG,
    };
    let production_refusal = ConfigRefusal::PublicUrlNotHttpsOrigin {
        env: Environment::PROD,
    };
    let refusals = [
        (ConfigRefusal::DevPublicUrlNotLoopback, "public_url"),
        (ConfigRefusal::DevHttpAddrNotLoopback, "http_addr"),
        (staging_refusal, "public_url"),
        (production_refusal, "public_url"),
        (
            ConfigRefusal::StagingOnProductionHandleDomain,
            "handle_domain",
        ),
    ];
    for (refusal, setting) in refusals {
        assert_eq!(refusal.setting(), setting, "{refusal:?}");
        assert!(refusal.to_string().contains(setting), "{refusal}");
    }
}

/// One load the guard refuses: the environment and settings it boots with, and
/// the refusal it must come back as.
struct RefusedLoad {
    env: &'static str,
    public_url: &'static str,
    http_addr: &'static str,
    handle_domain: &'static str,
    is_expected: fn(&ConfigRefusal) -> bool,
}

// Each refusal the guard raises comes back from the LOAD as
// `LoadError::Refused`, carrying the right variant, PROD's included.
#[test]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
fn each_refusal_comes_back_from_the_load_as_refused() {
    let refused_loads = [
        RefusedLoad {
            env: "DEV",
            public_url: "http://localhost:8080",
            http_addr: "127.0.0.1:8081",
            handle_domain: "zurfur.app",
            is_expected: |refusal| matches!(refusal, ConfigRefusal::DevPublicUrlNotLoopback),
        },
        RefusedLoad {
            env: "DEV",
            public_url: "http://127.0.0.1:8080",
            http_addr: "0.0.0.0:8081",
            handle_domain: "zurfur.app",
            is_expected: |refusal| matches!(refusal, ConfigRefusal::DevHttpAddrNotLoopback),
        },
        RefusedLoad {
            env: "STG",
            public_url: "http://zurfur.foo",
            http_addr: "0.0.0.0:8081",
            handle_domain: "zurfur.foo",
            is_expected: |refusal| {
                matches!(
                    refusal,
                    ConfigRefusal::PublicUrlNotHttpsOrigin {
                        env: Environment::STG
                    }
                )
            },
        },
        RefusedLoad {
            env: "PROD",
            public_url: "https://localhost",
            http_addr: "0.0.0.0:8081",
            handle_domain: "zurfur.app",
            is_expected: |refusal| {
                matches!(
                    refusal,
                    ConfigRefusal::PublicUrlNotHttpsOrigin {
                        env: Environment::PROD
                    }
                )
            },
        },
        RefusedLoad {
            env: "STG",
            public_url: "https://zurfur.foo",
            http_addr: "0.0.0.0:8081",
            handle_domain: "stg.zurfur.app",
            is_expected: |refusal| {
                matches!(refusal, ConfigRefusal::StagingOnProductionHandleDomain)
            },
        },
    ];
    for refused_load in refused_loads {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
            jail.set_env("ZURFUR_ENV", refused_load.env);
            jail.set_env("DATABASE_URL", "postgres://x");
            jail.set_env("ZURFUR_PUBLIC_URL", refused_load.public_url);
            jail.set_env("ZURFUR_HTTP_ADDR", refused_load.http_addr);
            jail.set_env("ZURFUR_HANDLE_DOMAIN", refused_load.handle_domain);
            jail.set_env("ZURFUR_LOG_LEVEL", "info");
            jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");

            let outcome = Config::load();
            let refused_as_expected = matches!(
                &outcome,
                Err(LoadError::Refused(refusal)) if (refused_load.is_expected)(refusal)
            );
            assert!(
                refused_as_expected,
                "{} {}: {:?}",
                refused_load.env,
                refused_load.public_url,
                outcome.err()
            );
            Ok(())
        });
    }
}

// The guard runs inside the LOAD, so a DEV config on `localhost` never loads,
// and the error names the key without echoing its value.
#[test]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
fn a_dev_config_off_loopback_fails_the_load() {
    figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
        jail.set_env("ZURFUR_ENV", "dev");
        jail.set_env("DATABASE_URL", "postgres://x");
        jail.set_env("ZURFUR_PUBLIC_URL", "http://localhost:8080");
        jail.set_env("ZURFUR_LOG_LEVEL", "info");
        jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");

        let Err(error) = Config::load() else {
            panic!("a DEV config on localhost must not load");
        };
        assert!(
            matches!(
                error,
                LoadError::Refused(ConfigRefusal::DevPublicUrlNotLoopback)
            ),
            "{error:?}"
        );
        let message = error.to_string();
        assert!(message.contains("`public_url`"), "{message}");
        assert!(!message.contains("localhost:8080"), "{message}");
        Ok(())
    });
}

// ...and so does a staging config left on production's handle namespace.
#[test]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
fn a_staging_config_on_the_default_handle_domain_fails_the_load() {
    figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
        jail.set_env("ZURFUR_ENV", "STG");
        jail.set_env("DATABASE_URL", "postgres://x");
        jail.set_env("ZURFUR_PUBLIC_URL", "https://zurfur.foo");
        jail.set_env("ZURFUR_LOG_LEVEL", "info");
        jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");

        let Err(error) = Config::load() else {
            panic!("a STG config on zurfur.app must not load");
        };
        assert!(
            matches!(
                error,
                LoadError::Refused(ConfigRefusal::StagingOnProductionHandleDomain)
            ),
            "{error:?}"
        );

        jail.set_env("ZURFUR_HANDLE_DOMAIN", "zurfur.foo");
        let config = Config::load().expect("the config loads");
        assert_eq!(config.handle_domain.as_str(), "zurfur.foo");
        Ok(())
    });
}

// A malformed `public_url` fails the load without echoing the value, which can
// carry credentials.
#[test]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
fn a_malformed_public_url_is_never_echoed() {
    figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("ZURFUR_CONFIG_DIR", jail.directory().display().to_string());
        jail.set_env("ZURFUR_ENV", "dev");
        jail.set_env("DATABASE_URL", "postgres://x");
        jail.set_env("ZURFUR_PUBLIC_URL", "http://user:hunter2@127.0.0.1:99999");
        jail.set_env("ZURFUR_LOG_LEVEL", "info");
        jail.set_env("ZURFUR_DID_KEY_ROOT_KEY", "ZmlsZQ==");

        let Err(LoadError::Figment(error)) = Config::load() else {
            panic!("a malformed public_url must fail to extract");
        };
        assert_eq!(error.path, ["public_url"]);
        let message = error.to_string();
        assert!(message.contains("must be a bare origin"), "{message}");
        assert!(message.contains("not a URL"), "{message}");
        assert!(!message.contains("hunter2"), "{message}");
        Ok(())
    });
}

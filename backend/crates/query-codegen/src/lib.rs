//! Zurfur's generation config for `sqlx-rust-codegen` — exported as a library
//! so the runner (`main.rs`) and the `codegen_current` staleness gate test the
//! SAME config and can never drift apart.

use sqlx_rust_codegen::{Config, Timestamptz};

/// The Zurfur workspace's generation config: `just gen-queries` as the regen
/// hint, and the `session` namespace on `time::OffsetDateTime` (the type the
/// tower-sessions-core API consumes); every other namespace stays on chrono.
pub fn config() -> Config {
    let mut config = Config {
        regen_hint: "run `just gen-queries`".to_string(),
        ..Config::default()
    };
    config
        .timestamptz
        .insert("session".to_string(), Timestamptz::Time);
    config
}

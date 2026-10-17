//! The domain crates' guard binary.
//!
//! The guards over every domain crate (kernel, identity, workflow, gallery,
//! moderation and this façade) live in one binary here, so no consumer crate
//! carries them.
//!
//! Filter as usual: `cargo test -p domain --test it elements_do_not_import_ports::`.

mod elements_do_not_import_ports;

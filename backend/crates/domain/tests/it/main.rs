//! The domain crates' guard binary.
//!
//! The guards over every domain crate (kernel, identity, workflow, gallery,
//! moderation and this façade) live in one binary here, so no consumer crate
//! carries them.
//!
//! Filter as usual: `cargo test -p domain --test it facade_paths::`.

mod domain_crates_point_one_way;
mod elements_do_not_import_ports;
mod facade_paths;
mod no_actor_port_takes_a_node_id;

# Cleanup checklist — the `macros` crate and the `#[use_case]` rollout

Opened 2026-09-19, after `xvlqvtsy` (`feat(application): #[use_case] and #[derive(WithPorts)]`).
Mechanical items are Claude's on the Engineer's go; ⚖️ items wait for a ruling.

## Positioning — where the macros are reached from

- [ ] Re-export the derive beside its trait: `pub use macros::WithPorts;` in `application/src/ports.rs` (macro namespace + type namespace share the name, the serde idiom). One `use crate::ports::WithPorts;` then brings both.
- [ ] Collapse the doubled imports (`use macros::WithPorts;` + `use crate::ports::WithPorts;`) in the ~20 namespace files.
- [ ] Re-export `use_case` from the `application` root (`pub(crate) use macros::use_case;`) so use-case files never name the `macros` crate.
- [ ] ⚖️ Rename the crate `macros` → `application-macros`? Both macros emit `crate::ports::WithPorts` / `crate::Ports`, so they only expand inside `application`.
- [ ] ⚖️ Only if a second crate ever needs them: `extern crate self as application;` + emit `::application::…` paths.

## `use_case.rs`

- [ ] The `#[unit]` wrapper calls `self.ports()` by method syntax (line 88); use `crate::ports::WithPorts::ports(self)` like the `#[ports]` injection does, then drop the `WithPorts` import from the use-case files that carry it only for the wrapper.
- [ ] Two `#[unit]` params → a spanned macro error instead of a borrow error in generated code.
- [ ] Attributes are copied onto both the inner and the outer fn (docs twice; a future `#[tracing::instrument]` would span twice).
- [ ] Names: `expand_transaction` → `expand_use_case`, `TokenStreamv2` → `TokenStream2`; `injection_of` returns an `&Attribute` nobody reads.
- [ ] ⚖️ `<name>_inner_injected` keeps the outer fn's visibility (`pub` today) — private, or deliberately callable with a caller's own unit?
- [ ] ⚖️ The macro rejects a fn that injects nothing; three use cases stay plain because of it (`account/facts/exist`, `character/delete`, `commission/status/direction/clear`).

## `with_ports.rs`

- [ ] Accessor name is `to_ascii_lowercase` — `ViewGrants` would become `viewgrants`; snake_case it.
- [ ] `field_names` is written and never read.

## Crate housekeeping

- [ ] `trybuild` is a dev-dependency with no `tests/ui`; add compile-fail cases for each `syn::Error` the macros raise.
- [ ] `proc-macro2` / `quote` / `syn` / `trybuild` versions move to `[workspace.dependencies]`.
- [ ] `///` on `domain::ports::Unit` and on the two `#[proc_macro_*]` entry points.
- [ ] `application/clippy.toml` reasons name only the `#[use_case]` wrapper; `transaction()` is the other sanctioned door.
- [ ] ⚖️ DD 64290818 (approved crate set) names `syn` only as already-carried; does the proc-macro stack need an amendment?

## Behaviour the rollout changed — rulings owed

- [ ] ⚖️ The unit opens before the body, so it now spans external I/O: DID mint (`account/create`, `character/create`), DID-document update (`account/change_handle`), blob write (`commission/files/upload`). Each such request also holds a second pool connection for its `ports` reads. Lazy `begin`, a split write-tail, or accepted?
- [ ] ⚖️ A `#[unit]` use case calling another `#[unit]` use case opens two independent units; nothing catches it now that the source guard is gone.
- [ ] ⚖️ `character/delete.rs` is a `todo!()` and the corpus defines no Character deletion (5668866, 23003138, 30343170 are silent).

## Design in sync

- [ ] `/design-sync` DD 55836674 and DD 24150017: both still describe plain `async fn` use cases calling `transaction()`, and 24150017's guard test no longer exists.

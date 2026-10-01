# Cleanup checklist — the `macros` crate and the `#[use_case]` rollout

Opened 2026-09-19, after `xvlqvtsy` (`feat(application): #[use_case] and #[derive(WithPorts)]`).
Mechanical items are Claude's on the Engineer's go; ⚖️ items wait for a ruling.

## Positioning — where the macros are reached from

- [x] Re-export the derive beside its trait: `pub use macros::WithPorts;` in `application/src/ports.rs` (macro namespace + type namespace share the name, the serde idiom). One `use crate::ports::WithPorts;` then brings both.
- [x] Collapse the doubled imports (`use macros::WithPorts;` + `use crate::ports::WithPorts;`) in the ~20 namespace files.
- [x] Re-export `use_case` from the `application` root (`pub(crate) use macros::use_case;`) so use-case files never name the `macros` crate.
- [x] Rename the crate `macros` → `application-macros` (ruled 2026-10-01: rename).
- [x] `extern crate self as application;` + `::application::…` paths — closed, not needed: it was conditional on a second consumer and there is none.

## `use_case.rs`

- [x] The `#[unit]` wrapper calls `self.ports()` by method syntax (line 88); use `crate::ports::WithPorts::ports(self)` like the `#[ports]` injection does, then drop the `WithPorts` import from the use-case files that carry it only for the wrapper.
- [x] Two `#[unit]` params → a spanned macro error instead of a borrow error in generated code.
- [x] Attributes are copied onto both the inner and the outer fn (docs twice; a future `#[tracing::instrument]` would span twice).
- [x] Names: `expand_transaction` → `expand_use_case`, `TokenStreamv2` → `TokenStream2`; `injection_of` returns an `&Attribute` nobody reads.
- [x] `<name>_inner_injected` is private (ruled 2026-10-01: private, no visibility emitted).
- [x] A `#[use_case]` on a fn that injects nothing passes through unchanged (ruled 2026-10-01: allowed); the three plain use cases now carry `#[use_case]`.

## `with_ports.rs`

- [x] Accessor name is `to_ascii_lowercase` — `ViewGrants` would become `viewgrants`; snake_case it.
- [x] `field_names` is written and never read.

## Crate housekeeping

- [x] `trybuild` is a dev-dependency with no `tests/ui`; add compile-fail cases for each `syn::Error` the macros raise.
- [x] `proc-macro2` / `quote` / `syn` / `trybuild` versions move to `[workspace.dependencies]`.
- [x] `///` on `domain::ports::Unit` and on the two `#[proc_macro_*]` entry points.
- [x] `application/clippy.toml` reasons name only the `#[use_case]` wrapper; `transaction()` is the other sanctioned door.
- [x] DD 64290818 (approved crate set) amendment for the proc-macro stack — ruled 2026-10-01 (B6); lands in the corpus PR.

## Behaviour the rollout changed — rulings owed

- [x] Ruled 2026-10-01: lazy `begin` — `#[lazy_unit]` injects an unopened `LazyUnit`; the body calls `uow.open()` right before its first write; the wrapper commits/rolls back only if opened. `#[unit]` + `#[ports]` is a compile error. 38 use cases converted; order tests in `application/tests/unit_order.rs`, begin/rollback tests in `lazy_unit/tests.rs`.
- [x] Ruled 2026-10-01: a task-local nested-unit guard debug-asserts when a unit opens inside another use case's unit (eager and lazy), with `#[should_panic]` tests.
- [x] `character/delete.rs` is a `todo!()` — closed for this PR; the Engineer's WIP implements it.

## Design in sync

- [~] `/design-sync` DD 55836674 and DD 24150017: both still describe plain `async fn` use cases calling `transaction()`, and 24150017's guard test no longer exists. (in progress as the corpus PR)
- [x] Ruled 2026-10-01: a Query never writes, enforced — the macro rejects `Query`/`*Query` beside `#[unit]`/`#[lazy_unit]` (trybuild cases), and `application::transaction` is a disallowed method in `application` (allowed only in `sweep_deadlines` and test seeding).

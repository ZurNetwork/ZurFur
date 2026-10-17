# Zurfur Domain Façade

The `domain` crate is the one path every other crate uses into Zurfur's domain. It holds no domain types of its own: it re-exports the domain crates below it, so `domain::elements::…`, `domain::ports::…`, `domain::datetime` and `domain::string_builder` resolve exactly as they did before the domain was split. The only code it owns is the unit of work.

## The domain crates

| Crate | Holds | Depends on (workspace) |
|---|---|---|
| `kernel` | the shared vocabulary: `did`, `text`, `maturity`, `markdown`, `blob`, plus `datetime` and `string_builder` | nothing (`shared` allowed) |
| `identity` | who acts: users, accounts, actor identities, handles, custody keys, profiles, public records, and their ports | `kernel` |
| `workflow` | the work: the commission aggregate and an account's boards, and their ports | `kernel`, `identity` |
| `gallery`, `moderation` | stubs, with no types and no dependencies yet | none |
| `domain` | this façade, plus the unit of work | `kernel`, `identity`, `workflow` |

Each domain crate keeps the same layout: `src/elements/…` for the nouns, `src/ports/…` for the role-named traits the adapters implement. Elements never import ports.

## The rules

- **Consumers name only `domain`.** No crate outside the domain crates depends on `kernel`, `identity`, `workflow`, `gallery` or `moderation` directly.
- **Dependencies point down.** A domain crate depends only on the crates below it in the table (and on `shared`), never on an adapter, a driver, `application` or this façade — dev-dependencies included.
- **The unit of work lives here.** `Database`, `UnitOfWork`, `Unit` and `UnitOfWorkFn` vend the write ports of both identity and workflow, so they sit above both. Every private-store write goes through `Database::begin()` and an open `UnitOfWork`.
- **Elements mirror the glossary.** Every element has a glossary page in the design corpus; the pointer lives in the owning crate's `NODE.json` under `refs`, never in a doc comment.

The guards live in this crate's one test binary, `tests/it/`:

- `elements_do_not_import_ports` — no domain crate's elements reference its ports; its crate lists equal the one-way table minus the façade;
- `domain_crates_point_one_way` — each domain crate declares only its allowed workspace dependencies, and consumers declare only `domain`; its table is the one list of domain crates, and `scripts/gate-light.sh`'s list is checked against it;
- `facade_paths` — every path the façade re-exports still resolves.

## Architecture

Zurfur follows ports and adapters. The domain crates are the I/O-free centre: entities, value objects and the ports that say how the system reaches the outside world. The adapters (`adapter-pg`, `adapter-atproto`, `adapter-mem`) implement those ports; `application` holds the use cases; `composition` wires the live adapters, and the drivers (`api`, `cli`) run on top of it.

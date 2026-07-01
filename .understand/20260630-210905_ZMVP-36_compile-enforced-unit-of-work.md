# 🔎 Understanding ZMVP-36 — Adopt a compile-enforced Unit of Work for private-store writes

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-36 · **Generated:** 2026-06-30 21:09:05 UTC · **Snapshot:** `.understand/20260630-210905_ZMVP-36_compile-enforced-unit-of-work.md`
> **Parent epic:** none on the ticket — cross-cutting private-store infrastructure (the multi-aggregate driver is **Commission**, ZMVP-18) · **Priority:** Medium · **Owner of the decision:** **SETTLED** (DD 24150017, all 7 decisions DISPOSED; one migration-*scope* sub-question remains for the Engineer — §8)

## 🧭 1. Context (cold-start)

`PgAccountRepo::accept_invitation` once issued **two `.execute(&self.pool)` writes that should have been one transaction** — a half-state (invitation `Accepted`, no membership) was observable. That bug was fixed *minimally* in ZMVP-20 (the current `account.rs:385` `self.pool.begin()` + `rows_affected()` guard at `:405`). Nothing in the type system stopped the original: **sqlx cannot type-distinguish a read from a write** (`query!` only validates SQL + infers columns), and **any `Executor`** — `&PgPool` or `&mut Transaction` — can issue any statement. So a compile-time guarantee can't come from tagging queries; it can only come from **ensuring no bare `Executor` (the pool) is reachable at a write site.**

This ticket generalizes the one-off fix into a structural guarantee, per **DD 24150017** "Transactions as a capability — a compile-enforced Unit of Work in the private store" (`adapter-pg/CLAUDE.md:7` records it). The shape is fully DISPOSED across the DD's 7 decisions; the only thing the DD itself left "cosmetic/open" — accessor methods (`uow.accounts().x()`) vs. free functions — is now **decided by the Engineer: accessor methods.** What remains for the Engineer is **not a design fork** but a **migration-scope call** (§8): convert account+invitation writes first, or every pg write adapter in one pass.

> **Load-bearing fact (verified):** the seam is **greenfield** — `grep` for `UnitOfWork` / `Database::begin` / `AccountWrites` / `AccountStore` across `backend/` is **clean** (no matches). Today every private-store write is **bare-pool or ad-hoc inline `self.pool.begin()`**. Write sites confirmed: `account.rs` inline-tx at `:106` (`create`), `:228` (`revoke_role`), `:240` (`leave`), `:385` (`accept_invitation`); bare-pool `.execute(&self.pool)` at `:215` (`grant_role`), `:268` (`create_invitation`), `:363` (`revoke_invitation`). Plus `session_store.rs:75/101/129/144`, `profile.rs:86`, `user.rs:42` (the upsert in `provision`).

## 🗺️ 2. Domain

This is **infrastructure / the private data boundary**, not a glossary entity — so the "domain" here is the architecture invariant, not a new noun.

- **The private store** (`adapter-pg`, DESIGN/"Domains and Applications" `11763713`) — app-owned rows, UUIDv7 keys, transactions. The *only* boundary this ticket touches.
- **No cross-store transactions** (DD/Data Boundaries `10354698`; `adapter-pg/CLAUDE.md:6`) — **unchanged.** Anything touching both boundaries (lock private facts **and** publish a PDS record) stays a separate retryable step (outbox-style). The PDS write is **never** folded into this transaction. The UoW is *strictly intra-Postgres*.
- **Unit of work = a transaction, owned by the handler** (DD 24150017 Decision 1) — a handler opens **one** transaction, threads N writes across aggregates, and `commit()`s once (drop = rollback). This generalizes the single-method atomicity `create`/`accept_invitation` already have to **multi-write, multi-aggregate** units — the real driver is **Commission** (ZMVP-18): completion writes commission state + EXP + rating; transfer appends a log row + updates a pointer.
- **Capability narrowing** (DD Decision 2) — the factory holds the pool + serves reads; the vended `UnitOfWork` holds **only** the `sqlx::Transaction`. **Write methods exist only on the tx-bound handle.** No pool is in scope at any write site → a bare-pool write is *unrepresentable*.
- **sqlx-free `domain` port** (DD Decision 3, "Option 1") — `Database`/`UnitOfWork`/`…Writes` traits are named by role and live in `domain` (which today imports only `async_trait` — `ports.rs:6`); `adapter-pg` owns the sqlx `Transaction`; the **mem fake mirrors the seam** as no-ops. Adapter-only UoW was *rejected* (couples handlers to `adapter-pg`, mem can't mirror it).
- **Aggregate-neutral, standalone factory** (DD Decision 7) — `begin()` lives on its own `Database` port, **never** a method on an aggregate repo (per-aggregate `begin()` rejected: two transactions can't be atomic together). Writes are reached as **views over the shared tx via accessor methods**: `uow.accounts().create(...)`. **Today's `AccountRepo` bisects along the read/write line** — reads → a standalone pool-backed `AccountStore` (no tx tax); writes → `AccountWrites`, reachable only via the uow.
- **Typestate (`Db<InTransaction>`) DEFERRED** (DD Decision 4) — do **not** build it; it lives entirely inside `adapter-pg` (domain has no pool), adds no decoupling, and its only extra catch (an adapter author adding a write to the read store) is covered by a cheap CI guard. (Aligns with memory `feedback_traits_dependency_inversion`: no abstraction without a consumer.)

Memory consulted: `project_transaction_unit_of_work` (the DD in one place + the `Transaction<'static>`/`#[async_trait]` mechanics), `feedback_make_unsoundness_unreachable` (one enforced shared path beats per-site checks — the whole point), `feedback_traits_dependency_inversion` (why typestate is deferred), `project_branching_main_only` (feature → main, squash). DD fetched live: 24150017.

## 🎯 3. Goal & scope

**Goal:** make "a private-store write goes through a transaction" a **compile-time guarantee by construction** — the bare-pool `accept_invitation`-class bug becomes *unrepresentable* — without restructuring the composition root (the `Arc<dyn>` model survives; the singleton repo becomes a factory).

**In scope:**
- A sqlx-free `Database` factory port + `UnitOfWork` handle + per-aggregate `…Writes` view trait in `domain` (`#[async_trait]`).
- `adapter-pg`: `PgDatabase` (holds pool, `begin() -> Box<dyn UnitOfWork>`), `PgUnitOfWork` (holds the `Transaction<'static>`, `commit()`/drop-rollback), `PgAccountWrites` view over `&mut PgConnection`; **bisect `PgAccountRepo`** into a pool-backed `PgAccountStore` (reads) + the tx-bound write view.
- Move the **7 account/invitation write methods** onto the write view; keep the **4 reads** (`find`, `role_of`, `find_pending_invitation`, `find_invitation`) on the read store, still pool-backed.
- Mirror the seam in **adapter-mem** as in-memory no-ops (`MemDatabase`/`MemUnitOfWork`), sharing the underlying maps so writes are visible to reads.
- Rewire `AppState` + `main.rs` + the e2e test composition: `account_repo` splits into `accounts` (read store) + `database` (write factory).
- Convert the **handler call sites** in `routes/accounts.rs` (7 writes → `begin()`/view/`commit()`; reads → the read store).
- A **CI guard** (clippy `disallowed-methods` or a one-line grep test) banning `.execute(&self.pool)` (bare-pool writes).

**Out of scope (explicit):**
- **Typestate `Db<InTransaction>`** — DEFERRED by DD Decision 4. Do not build.
- **Cross-store transactions / PDS writes** — unchanged; the no-cross-store rule still holds (DD Decision 5).
- **Read coverage by the compile guarantee** — reads on a pool are safe; they stay non-transactional (DD Decision 5).
- **A convenience single-write helper** — the DD notes it as a possible softening of the `begin()/commit()` ceremony, but it's optional, not required.
- **Commission writes** (ZMVP-18) — this ticket *builds the home*; it does not implement the Commission aggregate writes.

## 📦 4. Deliverables

- [ ] **`domain`**: `Database` port (`async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>>`), `UnitOfWork` (accessor `accounts(&mut self) -> Box<dyn AccountWrites + '_>`, consuming `commit`), `AccountWrites` (the 7 writes, `&mut self`). All `#[async_trait]`, sqlx-free.
- [ ] **`domain`**: `AccountStore` read port (the 4 reads), split out of today's `AccountRepo`. Retire/replace `AccountRepo`.
- [ ] **`adapter-pg`**: `PgDatabase { pool }`, `PgUnitOfWork { tx: Transaction<'static> }`, `PgAccountWrites<'a> { conn: &'a mut PgConnection }`, `PgAccountStore { pool }`. The 7 writes normalize on `&mut PgConnection`; the inline `pool.begin()` at `account.rs:106/228/240/385` disappears (the uow owns the tx); `settle_member_departure` (already `&mut PgConnection`, `:44`) is reused verbatim by the view.
- [ ] **`adapter-mem`**: `MemDatabase`/`MemUnitOfWork` whose `begin()`/`commit()` are no-ops; the write view and the read store **share** the `Arc<Mutex<…>>` maps so a write is visible to a later read (today `MemAccountRepo` owns the maps directly — `lib.rs:252-266` — they must lift into shared `Arc`s).
- [ ] **`api`**: `AppState.account_repo` (`lib.rs:184`) → `accounts: Arc<dyn AccountStore>` + `database: Arc<dyn Database>`; wire both in `main.rs:86` from `pool.clone()`; update the e2e composition root.
- [ ] **`api`**: rewrite the 7 write call sites in `routes/accounts.rs` (`:165` create, `:210` accept_invitation, `:243` leave, `:328` grant_role, `:403` revoke_role, `:491` create_invitation, `:594`/`:629` revoke_invitation) to `begin()`/view/`commit()`; repoint the reads (`:86/:97/:203/:237/:318/:392/:465/:571/:617`) at the read store.
- [ ] **CI guard** banning `.execute(&self.pool)` writes — with a **scoped exception for `session_store.rs`** (see §7) — wired into the suite `/prepare-pr` mirrors.
- [ ] **Doc comments** (`///`) on every new port/struct/method per repo convention (`feedback_rust_doc_comments`); the rich existing docs on `AccountRepo`'s methods migrate to the split traits.
- [ ] **Design sync**: flip DD 24150017 from **PROPOSED → ACCEPTED/IMPLEMENTED** and tick its "Migration scope" + "Mem fake" + "CI guard placement" open-items (offer `/design-sync`).

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done (evidence) |
|---|---|---|---|---|
| **Migration-scope call** — account+invitation only, vs. all pg write adapters in one pass | 2 — *coordination, not technical* | P0 | 🧑 Engineer | ⬜ — the guard can only flip globally once **all** writes convert; recommendation = all-in-one-pass (§8) |
| **`domain` ports** — `Database`/`UnitOfWork`/`AccountWrites` + `AccountStore` read split | 4 — shape is fixed by the DD; risk is dyn-safety/lifetime of the accessor view | P1 | 🤖 Claude | ⬜ — greenfield (grep clean); `ports.rs` is sqlx-free today (`:6`) |
| **`adapter-pg` impls** — `PgDatabase`/`PgUnitOfWork`/`PgAccountWrites`/`PgAccountStore`; drop inline `pool.begin()` | 5 — mechanical move of existing SQL onto `&mut PgConnection`; `settle_member_departure` already fits | P1 | 🤖 Claude | ⬜ — write sites enumerated `account.rs:106/215/228/240/268/363/385` |
| **`adapter-mem` seam** — `MemDatabase`/`MemUnitOfWork` no-ops; lift maps into shared `Arc<Mutex>` | 4 — must share state between read store + write view; existing repo tests (`lib.rs:657-831`) must stay green | P1 | 🤖 Claude | ⬜ — today maps are owned, not shared (`lib.rs:252-266`) |
| **`api` wiring** — split `AppState`, `main.rs`, e2e composition | 3 — `pool` already in `AppState` (`lib.rs:160`); two `Arc`s from one pool | P1 | 🤖 Claude | ⬜ — `main.rs:86`, `lib.rs:184` |
| **Handler rewrites** — 7 writes → `begin/commit`; reads → read store | 4 — repetitive; the multi-write win shows where two ops should share one `begin()` | P1 | 🤖 Claude | ⬜ — call sites listed above in `routes/accounts.rs` |
| **CI guard** — clippy `disallowed-methods` / grep test, + `session_store` exception | 3 — placement decided at build (DD open-item); verify it actually *fails* on a bare-pool write | P2 | 🤖 Claude | ⬜ — must be enabled **only after** full conversion (§8); cite memory `feedback_verify_command_output_not_exit_status` |
| **Doc + design-sync** | 2 | P2 | 🤖 Claude | ⬜ — `/document`; offer `/design-sync` to flip the DD status |

**Domain weight: ~1/8.** Every shape fork — handler-owned tx, capability narrowing, sqlx-free port, aggregate-neutral factory, accessor methods, typestate-deferred, reads-pool-backed, CI-guard-not-typestate — is **already disposed** in DD 24150017. The lone Engineer item is the **migration-scope** call (P0), which is sequencing/coordination, not domain judgment.

**Owner split:** the **bulk is 🤖 Claude** — this is faithful mechanical execution of a settled design (difficulty 3–5, no judgment). One 🧑 Engineer item: a scope/ordering decision, not design.

## ✅ 6. Test checklist (TDD)

The guarantee is **structural** — its primary "test" is **that the code compiles** (a bare-pool write *won't*). So the checklist is behavior-preservation + the seam, not new domain behavior.

- **Compile-time (the actual guarantee)** — *asserts that* a write method is **only** reachable on the tx-bound handle: there is no pool-typed value in scope at any `…Writes` method → a bare-pool write fails to compile. Demonstrated by construction (no pool field on `PgAccountWrites`); optionally a `compile_fail` doctest. → DD Decisions 1–2.
- **CI guard (the residual hole)** — *asserts that* the clippy `disallowed-methods`/grep guard **fails** when `.execute(&self.pool)` is (re)introduced into a write adapter. **Verify it emits the failure, not just a green exit** (memory `feedback_verify_command_output_not_exit_status`). → DD Decision 4.
- **Behavior preservation — `accept_invitation` atomicity** (the original bug): a lost race (offer already accepted/revoked) seats **no** membership and rolls back — the existing guard at `account.rs:405` survives the move onto the view. Existing e2e coverage must stay green.
- **Behavior preservation — `create`**: account row + founder Owner membership commit together or not at all (the unit `create` already is). Mem test `create_then_find_returns_the_account` (`lib.rs:658`) and the `accounts.rs` e2e stay green.
- **Multi-write atomicity (new capability)** — *asserts that* two writes across one `begin()`/`commit()` are atomic and that **dropping** the uow before `commit()` rolls back both (pg integration test; mem is a no-op so this is a pg-only assertion).
- **`leave`/`revoke_role`** — the shared `settle_member_departure` still re-homes children + revokes pending issued invitations atomically on the view (ZMVP-21/40). Existing tests stay green.
- **Mem seam** — *asserts that* `MemDatabase::begin()` → `uow.accounts().create(...)` → `commit()` makes the account visible to `MemAccountStore::find` (shared-state wiring works); all current `MemAccountRepo` tests (`lib.rs:657-831`) port over unchanged.
- **Read-path unchanged** — `find`/`role_of`/`find_pending_invitation`/`find_invitation` still resolve off the pool with no transaction.

> The bar is **"the bad state couldn't get that far," not "we found it"** (memory `feedback_make_unsoundness_unreachable`): the win is the *unrepresentable* bare-pool write, with the CI guard closing the one adapter-internal hole the type system can't.

## 🧠 7. Logic & shape

**The split.** `AccountRepo` (one trait, reads + writes — `ports.rs:91-187`) bisects:

```rust
// domain — the factory + handle (sqlx-free, #[async_trait])
#[async_trait]
pub trait Database: Send + Sync {
    /// Begin ONE private-store transaction; the handle owns it. Drop = rollback.
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>>;
}

#[async_trait]
pub trait UnitOfWork: Send {
    /// A view of the Account write surface over THIS transaction (accessor form,
    /// Engineer's choice over free-functions). The borrow keeps the view tied to
    /// the shared tx; no pool is reachable here.
    fn accounts(&mut self) -> Box<dyn AccountWrites + '_>;
    // future: fn commissions(&mut self) -> Box<dyn CommissionWrites + '_>; (ZMVP-18)
    /// Commit once; consuming the handle. Forgetting it drops → rollback.
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;
}

#[async_trait]
pub trait AccountWrites: Send {                       // the 7 writes, &mut self
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()>;
    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()>;
    async fn revoke_role(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()>;
    async fn leave(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()>;
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<()>;
    async fn revoke_invitation(&mut self, id: InvitationId) -> anyhow::Result<()>;
    async fn accept_invitation(&mut self, invitation: Invitation, listed_on_profile: bool)
        -> anyhow::Result<UserAccount>;
}

#[async_trait]
pub trait AccountStore: Send + Sync {                 // the 4 reads, pool-backed, &self
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>>;
    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>>;
    async fn find_pending_invitation(&self, account: AccountId, invited: UserId)
        -> anyhow::Result<Option<Invitation>>;
    async fn find_invitation(&self, id: InvitationId) -> anyhow::Result<Option<Invitation>>;
}
```

```rust
// adapter-pg — sqlx lives ONLY here
struct PgDatabase { pool: PgPool }            // serves begin()
struct PgUnitOfWork { tx: Transaction<'static> }   // holds ONLY the tx — no pool in scope
struct PgAccountWrites<'a> { conn: &'a mut PgConnection } // writes normalize on &mut PgConnection
struct PgAccountStore { pool: PgPool }        // reads only

// pool.begin() yields Transaction<'static>, so Box<dyn UnitOfWork> carries no borrowed lifetime
// (memory project_transaction_unit_of_work; DD Decision 6).
```

**Handler before/after** (`routes/accounts.rs`):

```rust
// before                                   // after
state.account_repo.create(&account,&owner)  let mut uow = state.database.begin().await?;
    .await?;                                 uow.accounts().create(&account, &owner).await?;
                                             uow.commit().await?;
state.account_repo.role_of(u, a).await?      state.accounts.role_of(u, a).await?   // read store
```

**Enforcement topology** — the point is **one gate, not per-site discipline** (memory `feedback_make_unsoundness_unreachable`):

```
  every write site ──► uow.accounts()  ──► &mut PgConnection (the tx)  ──► commit()
                         (no PgPool in scope anywhere on this path → bare-pool write won't compile)
  CI guard: ban .execute(&self.pool)  ←─ closes the adapter-internal hole the type system can't
```

**Implementation risks to verify at build time (flag, don't assert):**
1. **Accessor dyn-safety / lifetime.** `accounts(&mut self) -> Box<dyn AccountWrites + '_>` is the dyn-safe encoding of the accessor form; the `'_` ties the view to the tx borrow. Confirm `async_trait` + this borrowed-box return compose cleanly; if not, the fallback is a concrete `PgUnitOfWork::accounts()` inherent method (still accessor-shaped) with the domain handle exposing it differently. **Couldn't fully prove the exact dyn shape from here — verify first thing.**
2. **`commit(self: Box<Self>)`** — confirm `async_trait` accepts the `Box<Self>` consuming receiver; else model commit as `async fn commit(&mut self)` + a `committed` flag, with drop rolling back if unset.
3. **`session_store.rs` is bound by the *external* `tower_sessions_core::SessionStore` trait** (`:56-133`) — fixed `&self` signatures, each op independently atomic, no place to thread our uow. Its `.execute(&self.pool)` writes (`:75/101/129/144`) **cannot** move onto the UoW and **must carry a guard exception** (`#[allow]` / grep exclusion) no matter how the migration scopes. `profile.rs:86` (cache upsert) and `user.rs:42` (`provision` upsert) are single-statement and could either move onto the seam or be exempted — Engineer's scope call (§8).
4. **Mem shared state** — `MemAccountRepo`'s three maps (`lib.rs:255/260/265`) must lift into `Arc<Mutex<…>>` shared by `MemDatabase`, the read store, and the write view, so a no-op-"committed" write is visible to a later read.

## 🚀 8. Next steps

1. ⚠️ **Engineer decision (P0, scope — not design):** convert **account+invitation writes only**, or **all pg write adapters in one pass**?
   - **Recommendation: all-in-one-pass.** The CI guard banning `.execute(&self.pool)` can only be switched **on globally** once every write adapter is converted; a partial conversion leaves the guard either un-shippable or riddled with per-file exceptions that rot. One pass lets the guard land green and *stay* the enforcement (memory `feedback_make_unsoundness_unreachable` — one enforced path, not drifting per-site allows).
   - **Caveat that survives either choice:** `session_store.rs` is bound by the external `tower_sessions::SessionStore` trait and **will need a guard exception regardless** (§7 risk 3). `profile.rs`/`user.rs` are single-statement upserts — fold them onto the seam (cleanest for a global guard) or exempt them; that detail rides on this same call.
   - **Counter-consideration to put honestly:** all-in-one-pass widens the diff/PR and touches session/profile/user adapters with no atomicity bug today. If the Engineer prefers a tight, reviewable first PR, do account+invitation first and **gate the guard behind a follow-up** that finishes the conversion — accepting that the guarantee is only *partially* enforced until then.
2. Once scoped: write the failing **mem-seam** + **multi-write atomicity** tests (§6, red), then build `domain` ports → `adapter-pg` impls → `adapter-mem` seam → `api` wiring → handler rewrites (green). Resolve §7 risks 1–2 **before** the big mechanical move.
3. Land the **CI guard** last, enabling it **only after** the in-scope conversion is complete; prove it *fails* on a planted bare-pool write (memory `feedback_verify_command_output_not_exit_status`).
4. `/document` the new signatures; offer `/design-sync` to flip DD 24150017 PROPOSED → IMPLEMENTED and tick its three open-items.
5. **Collision watch** (`/close-gaps`): any ticket touching `routes/accounts.rs` or `AppState` collides — ZMVP-47 (capability-scoped write-gate, same handlers), ZMVP-44 (handle issuance, touches `account.rs`/`AppState`), ZMVP-39 (router split — already merged, `5743f0b`). This refactor should ideally land **before** Commission (ZMVP-18) builds its multi-aggregate writes on the seam, and **before/with** the account-write tickets to avoid double-converting.

---

## ✅ ENGINEER DISPOSITIONS — FINAL (2026-06-30, uow planning)

- **UoW shape:** single aggregate-neutral `Database`/`UnitOfWork` factory; aggregate writes via **accessor-method views** (`uow.accounts().create()`), per DD 24150017 Decision 7. SETTLED.
- **Migration scope:** **ALL pg write adapters in ONE pass** (account + invitation + user + profile) so the bare-pool-write CI guard switches on **globally**. `session_store.rs` is a **documented guard exception** (externally bound by `tower_sessions::SessionStore` — `&self`, independently atomic — cannot move onto the UoW). SETTLED.
- **DD 24150017 status:** currently **PROPOSED** — offer `/design-sync` to flip to DECIDED at close-out.

**Open questions / unknowns:**
- ~~Migration scope~~ — DECIDED above (all-one-pass).
- The exact dyn-safe encoding of the accessor view + consuming `commit` (§7 risks 1–2) — an implementation detail to settle in the first hour, not a domain fork.
- Guard mechanism: clippy `disallowed-methods` (needs `clippy.toml`, runs in the lint gate) vs. a one-line grep test (runs in the test gate). DD left this open; pick at build — grep is simpler and adapter-scoped, clippy is more precise about the receiver.

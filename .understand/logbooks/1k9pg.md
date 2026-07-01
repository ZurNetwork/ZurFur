# uow 1k9pg — ZMVP-49: real did:plc minter + per-user rotation-key storage

Branch `feature/zmvp-49-didplc-minter` (worktree `~/code/zurfur-zmvp-49-minter`, pg :23603).
Replaces `StubDidMinter` with a real, identity-only `did:plc` minter. LOCKED decisions —
DD 26804226 (custody), DD 26935298 (identity-only v1). Security-critical.

## What shipped
- **Byte-exact derivation** (`adapter-atproto/src/plc.rs`): typed genesis-op structs +
  `serde_ipld_dagcbor` (canonical key sort proven) → sha256 → base32/24. **Safety-net
  vector test passes**: `did:plc:ewvi7nxzyoun6zhxrhs64oiz`.
- **Real minter** (`did_minter.rs`, `RealDidMinter`): 3× secp256k1 (atrium-crypto),
  rotationKeys `[cold(0), operational(1)]`, `#atproto` signing key, identity-only
  (services `{}`, no PDS), sign no-`sig` DAG-CBOR with operational key (low-S/64B/base64url).
  `StubDidMinter` kept for tests/dev.
- **Key custody** (`domain/elements/account_keys.rs` + `adapter-pg/key_vault.rs`,
  `key_store.rs`): `KeyStore` port; XChaCha20-Poly1305 envelope encryption under a
  config root key; `account_keys` migration. Secrets zeroize + redacted Debug; not `Serialize`.
- **Directory** (`plc_directory.rs`): `PlcDirectory` port; no-op default (C2), gated HTTP impl.
- **Wiring**: `Config` gains dev-only root key + directory fields; `main.rs` wires the real
  minter; `POST /accounts` gains a validated `handle` (bound into `alsoKnownAs`).

## Seams flagged for the Engineer / close-gaps
1. **Port signature change** `DidMinter::mint(&self, handle: &Handle)` — needed for the
   genesis `alsoKnownAs`. Updated stub + mem + the one call site.
2. **`CreateAccountBody.handle`** — validated via the `Handle` gate but **NOT persisted**
   (`accounts.handle` column, resolution, and alsoKnownAs *updates* are ZMVP-44). Shared
   surface with ZMVP-30/44/45 — watch for merge collision.
3. **Bare-pool write exemption** — `key_store.rs` added to `no_bare_pool_writes` EXEMPT
   (custody write happens *during minting*, before the account row exists; no txn home).
   Ratify.

## Deferred (guardrails held)
No canonical plc.directory registration (C2 no-op), no KMS (ZMVP-53), no PDS (identity-only),
no alsoKnownAs update outbox (ZMVP-50).

## Gates
fmt ✓ · clippy `-D warnings` ✓ · `cargo test --workspace` ✓ (vector + custody + crypto) ·
sqlx offline cache regenerated + committed.

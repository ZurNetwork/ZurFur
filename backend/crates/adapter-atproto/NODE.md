---
path: backend/crates/adapter-atproto
charted: 2026-08-21
fs:
  - name: CLAUDE.md
    role: settled invariants: did:plc everywhere, a Character carries no DID, no cross-store transactions
    node: false
  - name: Cargo.toml
    role: jacquard/jacquard-oauth, atrium-crypto, DAG-CBOR/sha2 for PLC, reqwest+rustls, sqlx for the auth store
    node: false
  - name: queries/auth_store/
    role: 6 statements for OAuth persistence: auth request info + session save/get/delete
    node: false
  - name: src/
    role: lib.rs (AtprotoAuthenticator), did_minter (Real/Stub), plc + plc_directory (genesis op, hashing, submission), profile, public_records, auth_store, secret_vault, @generated queries.rs
    node: false
  - name: tests/
    role: auth_store round-trip, public_records against the throwaway PDS + conformance suite, wipe_replay
    node: false
---
**Is:** The public data boundary: the only crate that knows the AT Protocol — OAuth sign-in, PDS record/blob writes, public profile reads, and real did:plc minting with key custody — quarantining jacquard behind domain port traits.

**Conventions:** nothing protocol-shaped leaks past this crate's surface (the jacquard client type is a private alias). Uses `PgPool` as a plain DB client injected by `api`; its queries run against adapter-pg's migrations. OAuth at-rest secrets (DPoP key, tokens, PKCE verifier) are envelope-encrypted under the same custody root key as adapter-pg's `key_vault`. `auth_store` is one of the five documented Unit-of-Work exemptions. Genesis signatures are atproto-canonical ECDSA-SHA256 low-S via atrium-crypto — never hand-rolled.

**Entry points:** `src/lib.rs`.

**Refs:** `CLAUDE.md` here · DD "did:plc Identity Custody" (26804226) · DD "DID:PLC vs DID:Web" (4358151) · DESIGN "Platform Authority" (9207856).

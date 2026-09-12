1. CONFIRMED. The code block for `application/src/account/delete.rs` shows the tombstone call occurring before `uow.commit().await?`, and errors are logged via `tracing::warn!` without being returned or retried.
2. CONFIRMED. DD 23003138 D3 explicitly states tombstoning is "a separate retryable step, never inside the private transaction".
3. UNVERIFIABLE. The context block does not contain the results of a workspace-wide `grep` for "outbox" or a full listing of `backend/crates/`.
4. CONFIRMED. `require_live_account` will fail if the row has been hard-deleted (and the current implementation still deletes rows), preventing the caller from re-running the tombstone step.
5. CONFIRMED. The did:plc specification explicitly defines a 72-hour recovery window using higher-priority rotation keys.
6. CONFIRMED. The `AccountWrites::hard_delete` documentation explicitly states that `account_keys` are left in place to preserve the 72-hour recovery window.
7. CONFIRMED. DD 57081857 D6 explicitly dictates this change, and the framing notes acknowledge it is decided but not yet built.
8. CONFIRMED. The `sweep_deadlines` code block calls `application::transaction`, and its doc comment confirms the wall-clock and advisory-lock elements live in the `api` driver.
9. CONFIRMED. The `DidOperations` trait doc explicitly notes "no adapter implements it yet", and `RealDidMinter::tombstone` is the current implementation in `adapter-atproto`.
10. CONFIRMED. The PLC directory rejects subsequent operations on a tombstoned DID as invalid. Because `RealDidMinter::tombstone` advances the local operation log on success, a replay would attempt to chain a new tombstone onto the previous tombstone, which is an invalid operation.
11. UNVERIFIABLE. The code block for `application/src/account/delete.rs` shows the usage of `facts::exist`, but the implementation of this function is not provided in the context block to verify it is stubbed to `false`.
12. CONFIRMED. The PLC directory is a central HTTP server where operations are submitted; as an external HTTP API, calls are inherently subject to network failures, timeouts, and replays.
13. CONFIRMED. The `adapter-atproto/src/auth_store.rs` doc string confirms the adapter writes to Postgres for OAuth state and that the DDL lives in `adapter-pg`.
14. CONFIRMED. The `NoopPlcDirectory` code confirms it is a gated no-op in v1, and DD 23003138's open items specifically track adding an outbox/retry mechanism prior to real directory launch.
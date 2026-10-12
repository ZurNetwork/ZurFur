# Copilot triage: PRs #238 and #239

2026-10-08 · Designer (identity review triage) · For the tech lead. Read at the PR heads: #238 = change `rukvlkst` (commit `247730b9`), #239 = change `vvltkpum` (commit `8b90db5d`), the same commits Copilot reviewed. Slice 3 (Stoat, `qswzyknv`) was read where a fix ripples into it. Nothing was edited.

## Summary

1. All six comments are real. None is a live hole today, and none is above Low.
2. **#238 (Lynx): three fixes.** Two are gaps in the shared conformance suite: both adapters already answer correctly, but the suite doesn't pin it. One is a stale doc on `HandleError`. The suite change breaks slice 3's runner until Stoat stages the new cases on restack.
3. **#239 (Otter): two small fixes.** Tighten the redirect scan, which `client.rs`'s own tests already back up, and strip a caller's `User-Agent`.
4. **#239, `public_records.rs`:** it has no production caller. Under the alpha rulings, its first caller writes to Zurfur's own PDS, not to a PDS a user picks. Moving it onto `GuardedHttp` now would refuse the dev PDS, the test PDS and any PDS on a private network. That makes it a trade-off for the Engineer, not a slice-2 fix. The rule that actually closes the risk is that its endpoint comes from operator config, never from a DID document.
5. **Recommendation:** add a guard tripwire now (Otter), and take the transport question to the Engineer with the pending "Hosted Repos" DD. The leaning answer is a hardened client of its own whose endpoint can only come from config.

## Triage

| PR | Comment (path:line) | Verdict | Why | Exact fix | Builder |
|---|---|---|---|---|---|
| #238 | `test-support/src/identity_resolver_contract.rs:24` (one fixture covers "claims no handle" and "claims another") | **Fix** | **Low.** The field's doc allows either case (`:23-24`), and both runners stage only "claims another": mem points `mallory` at alice's DID (`adapter-mem/tests/identity_resolver.rs:44-45`); the real runner points `unconfirmed` at a document claiming `someone-else` (slice 3 `resolver/client/tests.rs:676`, `:708-711`). Today both adapters answer `NotConfirmed` for a document that claims nothing (fake `identity_resolver.rs:162-165`; real `resolver/client.rs:56-58`, where `Claim::Absent` is not `Claim::Handle`), but nothing pins that. An adapter that mapped "claims nothing" to `NotFound` would pass. The suite is slice 3's acceptance gate. | Narrow the doc at `:23` to "…whose document claims another handle". Add `handle_whose_did_claims_nothing: AtHandle`, assert `Err(NotConfirmed)` for it in `an_unconfirmed_handle_is_not_confirmed` (`:92-101`), and add it to `failing_handles` (`:219-225`). Mem runner: `seed_handle_claim(&staged.handle_whose_did_claims_nothing, &staged.handleless_did)` after `:49`. This works because `seed_handle_claim` keeps an already-staged document (`identity_resolver.rs:74-78`, `or_insert`). **Stoat, on restack:** add the field and a TXT record pointing at `handleless_did` (whose document is already served with no handle, `:698-703`). | Lynx (+ Stoat restack) |
| #238 | `test-support/src/identity_resolver_contract.rs:174` (classes "at either step" only half-pinned) | **Fix** | **Low.** The doc promises the class survives "at either step of a handle lookup" (`:149-150`). At the document step, only `Unavailable` is staged (`handle_naming_unavailable_document`, `:167-174`). The real adapter propagates the document's own class (`resolver/client.rs:56`, `document(&did).await?`), and so does the fake (`identity_resolver.rs:162`), but no case pins `Refused` or `NotFound` there. Practical impact today is nil: anonymous routes collapse all four classes, the CLI maps `NotFound`/`NotConfirmed`/`Refused` alike (design §3.5), and retries key only on `Unavailable`. It is still a gap in the suite. | Add `handle_naming_refused_document` and `handle_naming_missing_document`. Assert `Err(Refused(_))` and `Err(NotFound)` in `lookup_failures_keep_their_class`, and add both to `failing_handles`. Mem runner: `seed_handle_claim(&…refused_document, &staged.refused_did)` after `refuse_did` (`:54`). The order matters: `or_insert` keeps the refusal. For the missing document, `seed_handle_claim` would *create* a claim-less document, so add a handle-only seeding helper to `MemIdentityResolver` (the handle names a DID, and no document is staged) and point it at `staged.unknown_did`. **Stoat, on restack:** TXT records pointing at `refused_did` and `unknown_did`; both are already served, as a private host and a 404 (`:717`, `:684`). | Lynx (+ Stoat restack) |
| #238 | `domain/src/elements/handle/at_handle.rs:34` (`HandleError` docs contradict `AtHandle`) | **Fix (doc only)** | **Info.** `AtHandle::from_str` returns `HandleError` (`at_handle.rs:34`), but the enum says it is "Why a string was rejected as a [`Handle`]" (`errors.rs:4`). `PunycodeLabel` says "Rejected in both namespaces" (`:43`). `AtHandle` can never return `PunycodeLabel` or `ReservedLabel`: it runs only `split_labels` and `check_top_level` (`at_handle.rs:48-50`; `syntax.rs:7-62`). The shared error type is what design §3.1 specifies, and the Engineer approved it, so the fix is the docs, not a new type. | `errors.rs:4`: "Why a string was rejected as a [`Handle`] or an [`AtHandle`](super::AtHandle)…". `:43`: "A label begins with `xn--`. Only a [`Handle`] refuses it, in both namespaces; an `AtHandle` accepts it." `:46`: append "Only a [`Handle`] refuses it." | Lynx |
| #239 | `adapter-atproto/tests/network_door.rs:36` (`public_records.rs` allowlisted but takes any endpoint) | **Question for the Engineer**, plus a small fix now | **Latent (Low today).** Every caller is a test: `tests/public_records.rs:29`, `:98` and `tests/wipe_replay.rs:206`. The endpoint is the test rig's `http://127.0.0.1:<port>` (`test-support/src/pds.rs:90`), standing in for Zurfur's own PDS. No runtime crate constructs it, on any branch or workspace (`git log -S`, `rg`). The config key `ZURFUR_PDS_ENDPOINT` exists, but no config field reads it (`backend/config/dev.toml:16-22`). Copilot is right that nothing *enforces* "the operator configures it": `new(endpoint: &str, …)` takes any URI (`public_records.rs:46`) over a bare reqwest client (`:50`). But the alpha rulings point its first caller at Zurfur's own PDS (analysis below), and the prescribed fix has costs the Engineer has to weigh. | **Now (Otter):** in `forbidden_on_line` (`:156-172`), flag `AtprotoPublicRecords` on any line outside `adapter-atproto/src/public_records.rs` and `adapter-atproto/src/lib.rs`. Today it appears only there (`lib.rs:41`, the re-export). Add a detector case such as `let records = adapter_atproto::AtprotoPublicRecords::new(&endpoint, jwt)?;` in `composition/src/runtime.rs`. Integration tests stay unscanned (`tests/`). Reword the reason at `:33-36` to "holds a Bearer token for Zurfur's own PDS; no production caller, and the guard blocks one until its transport is ruled". **Later:** the transport ruling below. | Otter; ruling: Engineer |
| #239 | `adapter-atproto/tests/network_door.rs:66` (redirect scan misses `Policy::default()`, `.redirect(policy)`) | **Fix** | **Low.** The scan gap is real, but nothing can reach it today. In `client.rs`, `.redirect(Policy::default())` already fails two tests: `no_redirect_is_ever_followed`, and the client's Debug test (`client/tests.rs:561-582`). reqwest prints `redirect_policy` only when the policy is not the default 10-hop one (`$R/reqwest-0.12.28/src/async_impl/client.rs:1002-1006`, `:2947-2949`; `redirect.rs:155-157`). The other two allowlisted files use `reqwest::Client::new()`, which follows redirects anyway, and every other file is barred from naming reqwest. What is wrong is the doc comment, which claims "every way to reopen … a redirect" (`:40-44`). | `client/tests.rs:578-581`: assert `guarded_shape.contains("redirect_policy: \"Policy(None)\"")` (Debug form at `redirect.rs:214-227`), which catches any spelling. `network_door.rs` `forbidden_on_line`: flag a line containing `.redirect(` unless it also contains `Policy::none()`. Add `"Policy::default"` to `FORBIDDEN_EVERYWHERE`, and detector cases for `.redirect(Policy::default())`, `.redirect(policy)` and `.redirect(Default::default())`. The only `.redirect(` in the scanned sources is `client.rs:115`, which passes. Reword `:40-44` to "the named ways". | Otter |
| #239 | `adapter-atproto/src/guarded_http/client.rs:59` (caller's `User-Agent` survives) | **Fix** | **Info.** Correct: `user_agent()` only sets a default header (`$R/reqwest-0.12.28/src/async_impl/client.rs:1125-1132`), defaults fill only headers the request lacks (`:2590-2594`), and `dispatch` copies the caller's headers (`client.rs:86`). Nothing sets one today: jacquard 0.12.0's identity, OAuth and XRPC paths add no `User-Agent`. Only `richtext::fetch_opengraph_metadata` (`$R/jacquard-0.12.0/src/richtext.rs:1098`) and an explicit `XrpcCall::header` do. So nothing lies today, but the advertised identity is not enforced. | `client.rs:58-59`: also `parts.headers.remove(http::header::USER_AGENT);`, with the comment reading "a caller's Host or User-Agent never does". `client/tests.rs:338-341`: add `.header(http::header::USER_AGENT, "someone-else/1.0")` to the request, so the existing assertion proves the override. | Otter |

**Restack note for the tech lead.** `StagedIdentities` is built with a struct literal, so the three new fields in #238 stop slice 3 from compiling until Stoat stages them (`resolver/client/tests.rs:722-739`). That is wanted, because it forces the real adapter onto the new cases. Every DID they need is already served there.

## The `public_records.rs` question, for the Engineer

**Who picks the endpoint, under the alpha rulings.**
- **Accounts and public Characters** get repos on **Zurfur's own PDS**. Core writes to them with a per-repo password, opening sessions on demand and keeping the JWTs only in memory (`project/589826`, line 89; DD 54427650, "Hosted repos" and "Write credential", lines 67-68; DD 29687820, decision 6). That is exactly `AtprotoPublicRecords`' shape: a Bearer token for one repo (`public_records.rs:56-62`). Its endpoint is Zurfur's PDS, which the operator chooses.
- **Users** bring their own PDS. Zurfur writes to it only through the user's **OAuth session** (DD 29687820 decision 6: "A User's repo is never Zurfur-hosted … the OAuth rail above is exactly how the `owner` half reaches it"). atproto OAuth "mandates use of DPoP for all client types" and "Tokens are always bound to a unique session DPoP key" (atproto.com/specs/oauth, fetched 2026-10-08). So the Bearer-only `AtprotoPublicRecords` can't carry a User's token. That path runs over the jacquard bridge into `GuardedHttp` (design §3.2, §3.5).

**The trap that matters.** Even a Zurfur-hosted repo's PDS address in its DID document is user-controllable. The owner's key is senior in `[owner, pds, zurfur]` (54427650, "Hard requirements", line 64), and that is what makes repos migratable. If a caller ever took the endpoint from the DID document instead of config:
- `GuardedHttp` would stop internal targets;
- but it would still send the hosted-repo session token to whatever public host the document names.

So `GuardedHttp` is not needed when the endpoint comes from config, and not enough when it doesn't. **The rule to enforce is: endpoint from operator config, never from a DID document.**

**Options**

| | What | Costs |
|---|---|---|
| **(a) Own client, endpoint only from config** | `AtprotoPublicRecords` keeps its own client, made safe by construction: its endpoint type can only be built from `ZURFUR_PDS_ENDPOINT` in composition. Add no redirects, no proxy, timeouts and a body cap; today the bare client follows 10 redirects, and reqwest drops `Authorization` only when the host or port changes (`$R/reqwest-0.12.28/src/redirect.rs:239-250`). | A second outbound client with its own rules. The PDS may sit on a private network; dev and tests keep working unchanged. |
| **(b) Everything through `GuardedHttp`** | One client for all atproto egress (Copilot's ask). | Zurfur's PDS must sit at a public https name on port 443 with a public address, reached from Zurfur itself. The dev PDS (`http://localhost:2583`, `.env.example:104`) and the test rig are refused. Dev then needs its own exception, the same fork the design parked for the local PLC directory (I23). `tests/public_records.rs` and `tests/wipe_replay.rs` must move into unit tests, because the relaxed route exists only under `#[cfg(test)]` (I12). It **still** needs the config-only endpoint rule. |
| **(c) Decide later, with the first caller** | The tripwire above stops any production caller until (a) or (b) is ruled, ideally in the pending DD "Hosted Repos: Attach, Custody and the Write Credential". | Nothing now. The decision lands where the deployment of Zurfur's PDS is decided. I found no ruling yet on that PDS's production address. |

**Recommendation: (c) now, leaning (a) for the DD.** (a) matches the ruled shape and closes the real risk (an endpoint taken from a DID document) by type. It doesn't force a deployment layout. The one extra thing (b) would catch is a mistyped operator endpoint. The Engineer may weigh that differently, since the corpus puts security and viability over speed.

**Side note for the tech lead.** `.env.example:94` still says this PDS is "dev-only, never deployed", citing the superseded identity-only DD. That has been stale since the 2026-09-12 hosted-repos ruling.

## Draft replies (to post later, as the tech lead)

**#238, `identity_resolver_contract.rs:24`**
> Claude: Agreed. Both runners stage only "claims another handle", so a mistake on a document that claims no handle would pass. We'll add a second staged handle whose DID's document claims nothing, and assert `NotConfirmed` for both. Both adapters answer correctly today; this pins it.

**#238, `identity_resolver_contract.rs:174`**
> Claude: Agreed. At the document step only `Unavailable` is pinned. We'll stage a handle whose DID document is refused (`Refused`) and one whose document is missing (`NotFound`). Both adapters already pass the document's class through; the suite now enforces it.

**#238, `at_handle.rs:34`**
> Claude: Agreed, docs only. `HandleError` is shared by `Handle` and `AtHandle`. We'll say so on the enum, and mark `PunycodeLabel` and `ReservedLabel` as `Handle`-only. We're keeping the one shared error type, as the approved design specifies.

**#239, `network_door.rs:36`**
> Claude: Partly agreed. `AtprotoPublicRecords` has no production caller today; only two integration tests use it, against the test PDS. Under the current rulings, its first caller writes to Zurfur's own PDS, not to a PDS a user picks. A User's repo is written through their OAuth session, which already goes through `GuardedHttp`. Moving it onto the guard would also refuse the dev PDS and any PDS on a private network, so that is a decision for the Engineer, not this slice. Note too that the guard alone wouldn't stop a token going to a public host named in a DID document; the rule that matters is "endpoint from config only". Meanwhile the guard will fail if anything outside the adapter names `AtprotoPublicRecords`, so no caller can land before that decision.

**#239, `network_door.rs:66`**
> Claude: Agreed on the scan. In `client.rs`, `Policy::default()` already fails two tests: the no-redirect test, and the Debug check, because reqwest only prints a non-default policy. The other two allowlisted files use reqwest's default client, which follows redirects anyway. We'll flag any `.redirect(` that isn't `Policy::none()`, pin `Policy(None)` in the Debug test, and fix the doc comment that says "every way".

**#239, `client.rs:59`**
> Claude: Agreed. reqwest fills in its defaults only where the request has none, so a caller's `User-Agent` wins. Nothing sets one today, but we'll strip it beside `Host` and extend the test to prove the override.

<details>
<summary><b>Sources</b></summary>

- **Code:**
  - PR heads `247730b9` (#238) and `8b90db5d` (#239), read with `jj file show`;
  - slice 3 at `54bbf8f7` (`~/code/zurfur-workspaces/stoat`), read only;
  - caller search: `rg` over `backend/` in every workspace, plus `git log --all -S "AtprotoPublicRecords::new"`.
- **Vendored crates** (`$R = ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f`):
  - reqwest 0.12.28: `async_impl/client.rs` lines 1002-1006, 1125-1132, 2590-2594 and 2947-2949; `redirect.rs` lines 155-163, 214-227 and 239-250;
  - jacquard 0.12.0: `richtext.rs` line 1098;
  - jacquard-common 0.12.0: `xrpc.rs` lines 481 and 508-511;
  - a search of jacquard, jacquard-common, jacquard-oauth and jacquard-identity 0.12.0 for `User-Agent`.
- **Spec:** https://atproto.com/specs/oauth, fetched 2026-10-08 (DPoP mandatory; tokens bound to a session DPoP key).
- **Corpus** (`~/code/zurfur-design`):
  - `project/589826-project-mvp.md`, lines 33, 89 and 102;
  - `decisions/54427650-*.md`, lines 64 and 66-68;
  - `decisions/29687820-*.md`, decision 6 (line 40).
- **Design and reviews:**
  - `.understand/20261008-hardened-resolver-design.md`: §3.5 "Not callers", I12, I23, QA10 and its resolution;
  - `.understand/20261008-resolver-security-review-1-2.md`, finding 9;
  - `.understand/20261008-resolver-qa-1-2.md`.

</details>

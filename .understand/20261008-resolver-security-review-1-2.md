# Security review: hardened resolver, slices 1 and 2

2026-10-08 · Heron (identity security) · Threat-first review against `.understand/20261008-hardened-resolver-design.md` (§3.1–§3.3, §3.6, §3.7, §6, QA).

- **Slice 1, the domain words** (Lynx): change `rukvlkst`, commit `44de1da793ab`, bookmark `feature/identity-resolver-1-domain-words`.
- **Slice 2, the guarded web client** (Otter): change `vvltkpum`, commit `106b15ee938e`, bookmark `feature/identity-resolver-2-guarded-client`.

Both sit directly on `main` (`9a01808a`). Probes ran in a throwaway jj workspace, which has since been forgotten and deleted.

---

## Summary

**Slice 2, the guarded web client: ship after one fix.**
The client holds against everything I threw at it:
- **IP literals.** All 38 spellings I tried are refused before any DNS lookup. That covers percent-encoded, full-width and ideographic dots, hex, octal, short and trailing-dot forms, mapped and NAT64 IPv6, zone ids and backslash tricks.
- **DNS that points inward.** Real public names that resolve to the metadata service, loopback, a private address or AWS's IPv6 metadata address are refused end to end over the system DNS. I used `169.254.169.254.nip.io`, `127.0.0.1.nip.io`, `10.0.0.1.sslip.io`, `localtest.me` and `fd00-ec2--254.sslip.io`.
- **Proxy variables.** I did the manual check from the design: a guarded fetch with `HTTPS_PROXY` set, and again with `ALL_PROXY`. The client ignores both, while a plain reqwest client went through the proxy.
- **Redirects, body cap and timeouts.** No redirect is followed. The body cap holds whether or not the length is declared, and when it lies. A host that trickles its headers or its body is cut off at the total timeout.
- **The address table.** It matches IANA's registries as they stand today. No new block has been added since they were last updated (2025-10-09 and 2025-10-23).

**The one fix (must-fix).** The test that keeps the "known doors" shut misses three doors of exactly the kind it exists to catch. Each is a jacquard shortcut that quietly builds a plain, unprotected web client:
- `MemoryCredentialSession`;
- `UnauthenticatedSession::new_public`;
- `JacquardResolver::default()`, which also switches on Google DNS.

I put these lines in a file and the guard passed them. Jacquard's own docs suggest them for "public API access", so a builder writing the User Page could reach for one. Fix: add the names to the guard's list.

**Strongly recommended (one Cargo.toml line).** Turn off jacquard's default features. That removes the old, advisory-carrying DNS library (hickory 0.24) from the build entirely, so the `deny.toml` exception can go. It also makes jacquard's own DNS switches cease to exist at compile time. I checked: the crate builds, its 101 unit tests pass, and the old copy leaves `Cargo.lock`. The Engineer's DNS-version ruling accepted the old copy as a cost; with this change there is no old copy.

**Slice 1, the domain words: ship.**
- `AtHandle` matches the atproto handle spec rule for rule.
- `ResolveError`'s message never repeats the input.
- The in-memory fake applies the both-ways rules as §3.3 describes.

Two low items, both easy:
- The error's debug form prints its cause, and a cause can carry the handle.
- The shared test suite misses three error cases, so a real resolver could get them wrong and still pass.

**One question for you, Engineer** (details in "Questions for the Engineer" below): the older claim type, `Handle`, accepts the Kelvin sign "K" as a letter k. It does no harm, but it doesn't follow the spec. Should it follow the spec?

---

## Findings

Severity: **Medium** = fix before the next slice builds on it. **Low** = fix soon; no attack today. **Info** = noted; no change needed now.

| # | Sev | Slice | Where | What | How it would be exploited | Fix |
|---|---|---|---|---|---|---|
| 1 | **Medium (must-fix)** | 2 | `adapter-atproto/tests/network_door.rs:37-50` | The guard's door list is incomplete. Three jacquard constructors hard-wire a plain `reqwest::Client::new()` and pass the scan: `MemoryCredentialSession::unauthenticated()` and its `Default` (`$R/jacquard-0.12.0/src/client.rs:422-427`, `:467-470`), `UnauthenticatedSession::new_public()` and its `Default` (`:170-184`), and `JacquardResolver::default()` (`$R/jacquard-identity-0.12.0/src/lib.rs:1215-1232`, which also turns on system/Google DNS and the cache). A `let x: MemoryCredentialSession = Default::default();` passes too | A later slice (User Page, `me`, vulpes ports) uses one of these for a "quick public read". That puts a client on an anonymous path with **no** address filter, the system proxy, 10 redirects (https→http included), no timeout, no body cap and the Bluesky fallback. The guard stays green. Shown: an uncompiled file holding these lines passed `the_known_doors_stay_shut` | Add `MemoryCredentialSession`, `UnauthenticatedSession` and `JacquardResolver::default` to `FORBIDDEN_EVERYWHERE` now (slice 6 adds the bare `JacquardResolver` later), and add the new tokens to `detector_flags_each_door` |
| 2 | Low | 2 | `network_door.rs:71-76`, `:94-96` | The guard reads only `src/**/*.rs`, and skips any file named `tests.rs` or `proptests.rs` by name alone | (a) A crate's `Cargo.toml` renames the dependency (`web = { package = "reqwest", … }`); then `web::Client::new()` has no `reqwest` token. (b) A `mod tests;` without `#[cfg(test)]` compiles a `tests.rs` into the binary, and the scan never reads it. Shown: a non-test `heron_dir/tests.rs` naming reqwest passed | Scan each runtime crate's `Cargo.toml`: only `adapter-atproto` may list `reqwest`, and no `package = "reqwest"` rename is allowed anywhere. Assert that every `tests.rs` and `proptests.rs` is declared under `#[cfg(test)]` |
| 3 | Low | 2 | `network_door.rs:37-50`; `guarded_http/client.rs:111-124` | Weakening the transport is guarded only for `danger_*` and custom roots. Not covered: `use_preconfigured_tls` (any rustls verifier), `.resolve(` and `.resolve_to_addrs(` (pin a name to an address past the DNS filter), `.proxy(`, and `redirect::Policy::limited`/`custom`. No automated test pins `.no_proxy()` | An edit to `client.rs`, which is allowed to name reqwest, re-enables a proxy or redirects, or pins a name, and nothing fails. The proxy check is manual by design | Forbid those tokens everywhere. Keep the manual proxy check on the per-release list (done for this slice; result below) |
| 4 | Low (strongly recommended) | 2 | `adapter-atproto/Cargo.toml:11`; `deny.toml:30-39` | jacquard's **default** features turn on `dns` and `cache`. They are the only reason hickory-resolver/proto 0.24.4 (RUSTSEC-2026-0119; GHSA-5j98-2g5x-46v6) and mini-moka are in the build, and they make `new_dns`/`with_system_dns` exist (`#[cfg(feature = "dns")]`, `jacquard-identity lib.rs:358`, `:373`) | No attack today: nothing calls them. It is dead advisory-carrying code plus a door that has to be policed by a text scan | `jacquard = { version = "0.12.0", default-features = false, features = ["api_bluesky", "loopback", "derive"] }`. **Verified:** `cargo check -p adapter-atproto --tests` is green, the 101 unit tests pass, hickory 0.24/proto 0.24/mini-moka leave `Cargo.lock`, and the `deny.toml` ignore can then be deleted. Not run: the full `just gate` (machine load) |
| 5 | Low | 2 | hickory-resolver 0.26.3 `name_server_pool.rs:209-215` | hickory itself logs the queried name at `error!` when a DNS response is malformed (record class ≠ query class). In slice 2 that name is a handle's domain (jacquard's well-known fetch) or a PDS host. Rule I18 says the handle is never logged at `info` | A visitor's handle lands in production logs at `error` if a resolver returns a malformed answer. This is a privacy leak, not SSRF | Have the production log filter silence `hickory_resolver`, `hickory_net` and `hickory_proto` below `warn`, or turn them off. The configured default exists only for `dev` (`backend/config/dev.toml:14`), so production is whatever the deployment sets **[depends on deployment]** |
| 6 | Info | 2 | `guarded_http/client.rs:171-210` | Classification nits. rustls refusing a hostname (`InvalidDnsNameError`, e.g. a label starting `--`) lands in `Unavailable` (retryable), though it is permanent. Any `io::ErrorKind::InvalidData` in the chain counts as a TLS refusal | Retry semantics only (R5, CLI). Nothing leaks, and there is no SSRF. Shown: `--1.sslip.io` → `Unavailable` | Optionally map a rustls `InvalidDnsName` to `Refused` |
| 7 | Info | 2 | `Cargo.lock` (`chacha20 0.10.0`) | `chacha20 0.10.0` is **yanked**. It comes in through `rand 0.10.1`, which hickory 0.26 now also uses for DNS query ids. It was already on `main` through `sqlx-postgres`. Its changelog gives the reason: "Use of SSE4.1 intrinsic in SSE2 backend of RNG" | A CPU-feature bug. Any weakness in its output is **[unverified]**; none is stated | `cargo update -p chacha20@0.10.0 --precise 0.10.2` |
| 8 | Info | 2 | design §6 residuals | The residual "Zurfur's own public addresses pass the filter" is wider than stated. Any public https:443 host is reachable, including the PAR **POST** to an attacker-chosen path. So any service that trusts Zurfur's **egress IP** (a partner allowlist, or infrastructure behind a shared NAT) is reachable too. And if the system resolver serves split-horizon names with public addresses, those are reachable as well | A blind POST, and the attacker never sees the response. One exception: a JSON body whose `request_uri` field comes back in the redirect | Deployment note for the tech lead's egress follow-up. Give identity fetches a dedicated egress IP that nobody allowlists, and use a public resolver with no internal zones |
| 9 | Info | 2 | `plc_directory.rs:36-60`; `public_records.rs:35-50` | Allowlisted files checked. `plc_directory.rs` posts only to the operator's `plc_directory_endpoint` (`composition/src/runtime.rs:126-128`), at Zurfur-minted DIDs. `AtprotoPublicRecords::new` has **no production caller** (none in `composition`, `api`, `application` or `cli`). Both are bare clients: they honour proxy variables, follow redirects and have no timeout (already out of scope in §6) | Only through a compromised or misconfigured operator endpoint | None now. Once R4 settles the dev PLC story, `https://plc.directory` passes the policy, so the production submitter could move onto `GuardedHttp` |
| 10 | Low | 1 | `domain/src/ports/errors.rs:55-72`; `ports/errors/tests.rs:8-9`; `test-support/src/identity_resolver_contract.rs:180-217` | `ResolveError` derives `Debug`, and `Debug` prints the `#[source]` cause. The slice's own test models that cause carrying the handle (`"alice.example.com resolved to 10.0.0.1"`). The suite's echo check covers `Display` only | One `tracing::info!(error = ?e, …)` or `{e:#}` in a later caller writes the handle or host at `info`, against rule I18 | Either hand-write `Debug` to print the variant only (the cause stays on `source()` for a deliberate `debug!`), or extend the suite to assert that `{:?}` and `{:#}` never contain the input and require slice 3's causes to be host-free. I recommend the hand-written `Debug`: one enforced path |
| 11 | Low | 1 | `identity_resolver_contract.rs:18-43` | The suite misses three error-class cases: (a) a DID whose **own document** is refused must be `Err(Refused)`, not `Ok(None)` (§3.3: "A DID whose own document fails to resolve is an error"); the fake has `refuse_did`, but the suite never stages it; (b) `resolve_handle` whose **back-check** fetch is `Unavailable` must be `Unavailable`, not `NotConfirmed`; (c) `resolve_did` whose claimed handle names **nothing** must be `Ok(None)` | A real resolver that gets these wrong still passes. Every case fails closed, so this is not an SSRF. The cost is that R5 and the CLI would read an outage as permanent | Add the three staged identities and assertions |
| 12 | Info → question | 1 | `domain/src/elements/handle/value.rs:51` (and `HandleDomain`, `domain.rs:35`) | `Handle` (the claim type) lowercases with Unicode rules. `"\u{212A}itty.zurfur.app"` (Kelvin sign) is accepted and stored as `kitty.zurfur.app`. The spec says non-ASCII input is invalid and normalization is ASCII `A-Z`→`a-z`. U+212A is the **only** code point whose lowercase is ASCII (checked over every code point with the local rustc). `AtHandle` refuses it | None found. Uniqueness and the reserved-label check run on the normalized ASCII value, so there is no duplicate and no bypass; the effect is leniency on input | The Engineer's call (it changes which claim inputs are accepted). See "Questions for the Engineer" |
| 13 | Info | 1 | `domain/src/elements/handle/errors.rs` | `HandleError` messages echo one Debug-escaped character, or a label that already passed the `[a-z0-9-]` check | Benign; sign-in collapses every failure to one 422 in slice 4 | None |
| 14 | Note for slice 3 | 1→3 | design §3.1, §3.3 | `AtHandle` accepts `.test`, as approved. The URL policy refuses `.test` hosts, but the TXT step would still ask the system resolver for `_atproto.<x>.test.`. The spec: `.test` "should fail in real-world environments" | Only an operator's own resolver could answer, so there is no external attack | Slice 3: answer `NotFound` for `.test` handles before any DNS query. That aligns resolution with the policy's existing `.test` refusal. Tech lead: route it to the Engineer if you read it as a domain call |

---

## Questions for the Engineer

**Should `Handle` (the claim type) lowercase the way the spec says, ASCII only?**
- **What it means.** Today a claim typed with the Kelvin sign (U+212A, which looks like "K") is accepted and stored as a normal `k`. The new `AtHandle` refuses it. The atproto spec says handles are ASCII and normalized `A-Z` to `a-z`. The Kelvin sign is the only character this affects.
- **(a)** Change `Handle` to ASCII-only lowercasing. A Kelvin-sign claim is then refused ("invalid character"), and both handle types follow one rule.
- **(b)** Leave it. Nothing unsafe happens: the stored handle is plain ASCII either way.
- **Recommendation: (a), at low priority.** One rule for both types, exactly the spec. The cost is one changed test expectation for `Handle`. It can wait for a sweep; it does not block either slice.

---

<details>
<summary><b>The design's §6 checklist: where each check stands after slices 1 and 2</b></summary>

| §6 # | Check | Status | Evidence |
|---|---|---|---|
| 1 | Every identity request through `GuardedHttp` | **Holds for slice 2's scope**, with guard gaps (findings 1–3) | `lib.rs` builds `OAuthClient::new_from_resolver(store, JacquardResolver::new(GuardedHttp, …), …)`; `OAuthClient` keeps one `Arc<T>` for itself and its registry (`$R/jacquard-oauth-0.12.0/src/client.rs`, `new_from_resolver`). `profile.rs` builds `Agent<CredentialSession<…, JacquardResolver<GuardedHttp>>>`. jacquard's streaming path needs `HttpClientExt`, which `GuardedHttp` doesn't implement, so the compiler shuts that path. A search of the runtime crates for hyper, sockets, `TcpStream`, `ureq` and websockets found only the API's own listener. DID documents are still fetched by jacquard (slices 3–4) |
| 2 | Address table = IANA registries as of the review date, plus the metadata addresses | **Holds** | IPv4 and IPv6 special-purpose registries re-fetched today; still "updated 2025-10-09". IPv6 address-space registry: "updated 2025-10-23". Every block that is not globally reachable is refused, either listed (IPv4) or outside `2000::/3` (IPv6), the 2025 Dummy Prefix `100:0:0:1::/64` included. AWS (`169.254.169.254`, `fd00:ec2::254`), GCP (`fd20:ce::254`), Alibaba (`100.100.100.200`) and Azure (`168.63.129.16`) are all on their vendor pages today, and all refused |
| 3 | IP literals refused in every spelling | **Holds** | Probe below: 38 spellings, each straight through the policy and also through `http::Uri` → `to_string()` → the policy (the real `admit` path) |
| 4 | The checked URL is the dialled URL | **Holds** | `dispatch` builds `reqwest::Request::new(method, url.into())` from the checked `url::Url` (`client.rs:85`). reqwest reparses that URL's text for hyper. A WHATWG domain host can't contain `@ : [ ] \ %`, so the host keeps its boundaries. hyper-util skips DNS only for IP hosts, and the policy has already refused those |
| 5 | No redirect followed, 307/308 included | **Holds** | `Policy::none()` (`client.rs:115`) plus the 3xx branch (`:91-93`); test `no_redirect_is_ever_followed` covers 301/302/307/308 × 3 targets with 0 hits |
| 6 | Proxy environment variables ignored | **Holds (manual check done)** | See "Manual proxy check" below |
| 7 | Certificates on; no `danger_*` | **Holds** | No `danger_`, `add_root_certificate`, `tls_built_in`, `use_preconfigured_tls` or `.resolve(` anywhere in the runtime crates. reqwest is built with `rustls-tls-webpki-roots` (`cargo tree -e features`). Guard gap: finding 3 |
| 8 | Body cap with and without `Content-Length`; decompression off; `send_capped` callers | **Holds** | Tests cover a declared length (refused unread) and chunked. The probe adds close-delimited 2 MiB (refused at the cap in 2 ms) and a short `Content-Length` (truncated to 10 bytes). `.no_gzip/.no_brotli/.no_deflate/.no_zstd` are set, and the test shows no `Accept-Encoding` is sent. `send_capped`'s only caller is `send_http` |
| 9 | Timeouts: per fetch, per DNS lookup, per port call | **Fetch and DNS hold**; the port-call deadline is slice 3 | reqwest's total timeout is carried into the response body (`$R/reqwest-0.12.28/src/async_impl/client.rs:3087-3091` → `response.rs:36-45`). Probe: trickled headers → `Unavailable` at 701 ms; trickled body → `Unavailable` at 700 ms (700 ms test timeout). DNS: outer `tokio::time::timeout` (`dns.rs:102-104`), test `Hang` → `Unavailable` |
| 10 | System DNS, fully qualified names; TXT rules; no fall-through | **A/AAAA hold**; TXT is slice 3 | `TokioResolver::builder_tokio()` → `read_system_conf()` (`$R/hickory-resolver-0.26.3/src/resolver.rs:76-117`). `fully_qualified` adds the dot, and hickory then skips the search list (`resolver.rs:257-270`). Test: the scripted lookup receives `pds.example.com.` |
| 11–14 | Both-ways rules, F3, F5, no third-party fallback | Slices 3–4 | In the interim, jacquard's default `ResolverOptions` still has the Bluesky fallback on (`$R/jacquard-identity-0.12.0/src/resolver.rs:268-299`). It is stated in `NODE.json` and runs over `GuardedHttp`. The feature branch must not reach `main` before slices 4–6 |
| 15 | Anonymous callers get one answer; timing | **Holds for slice 2** | `POST /signin` still discards the error into one 422 (`api/src/routes/session.rs:60-64`). Timing: a policy refusal takes µs, a DNS refusal or not-found takes ms, and a fetch takes 100s of ms. All of it is computable by the attacker from public DNS, except split-horizon names (finding 8) |
| 16 | Relaxed policy only under `#[cfg(test)]` | **Holds** | Every `cfg(` in `guarded_http*`, `profile.rs` and `lib.rs` is `cfg(test)`; the crate has no `[features]`. `PublicHttpsUrl::unchecked`, `TestRoute`, `relaxed`, `scripted` and `upstream` are all `cfg(test)` |
| 17 | jacquard `tracing` off; no handles at `info` | **Holds**, plus finding 5 | `cargo tree -e features` shows no `tracing` feature on any jacquard crate. Slice 2 adds no log line |
| 18 | Account/Character DID fails closed at provision | Slice 4 | n/a |
| 19 | jacquard citations at the locked version | **Holds** | `Cargo.lock` still pins jacquard 0.12.0, reqwest 0.12.28, hyper-util 0.1.20 and url 2.5.8 |

</details>

<details>
<summary><b>Manual proxy check (§6 check 6): method and result</b></summary>

**Method.** I ran a test in the throwaway workspace with the proxy variables set on the test process from outside. The test does three things:
- binds a listener on `127.0.0.1:38917`;
- sends one request through a **control** client: a default `reqwest::Client`, which honours system proxies;
- sends one request through the production `GuardedHttp::new()`, which uses the system DNS. Both requests go to `https://example.com/`.

**Run 1:** `HTTPS_PROXY`, `https_proxy` and `HTTP_PROXY` set to `http://127.0.0.1:38917`.
- The listener saw `CONNECT example.com:443 HTTP/1.1`, from the control.
- The control's result was a tunnel EOF.
- The **guarded result was `Ok(200)`**: it went direct.
- The listener counted **1 connection in total**, the control's.

**Run 2:** `ALL_PROXY` and `all_proxy` set. Same result: the control tunnelled, and the guarded client went direct and got 200.

**Source.** `.no_proxy()` clears the proxy list and sets `auto_sys_proxy = false` (`$R/reqwest-0.12.28/src/async_impl/client.rs:1427-1431`; read at `:419`).

</details>

<details>
<summary><b>Probe results: URL spellings, addresses, real DNS, slow and lying servers</b></summary>

**URL spellings.** "direct" means straight through `PublicHttpsUrl::try_from`; "via Uri" means `http::Uri` → `to_string()` → policy, the real path.

| Input | direct | via Uri |
|---|---|---|
| `https://127%2E0%2E0%2E1/`, `https://%31%32%37.0.0.1/` | IpLiteral | Uri rejects |
| full-width `１２７.０.０.１`, ideographic-dot `127。0。0。1` | IpLiteral | Uri rejects |
| `0X7F.1`, `0x7f000001`, `127.0.1`, `0`, `00000000000000000177.0.0.1`, `0x7f.0x0.0x0.0x1` | IpLiteral | IpLiteral |
| `[::ffff:127.0.0.1]`, `[0:0:0:0:0:ffff:7f00:1]`, `[64:ff9b::7f00:1]`, `[::127.0.0.1]` | IpLiteral | IpLiteral |
| `[fe80::1%25eth0]` (zone id) | NotUrl | NotUrl |
| `https:\\127.0.0.1\`, `12<TAB>7.0.0.1`, `127.0.0.1\@bsky.social` | IpLiteral | Uri rejects |
| `bsky.social@127.0.0.1` | Userinfo | Userinfo |
| `127.0.0.1#@bsky.social` | IpLiteral | IpLiteral |
| `localhost%2E`, `x。local`, `LOCALHOST`, `x.LOCAL.`, `ⓛocalhost` | LocalName | LocalName or Uri rejects |
| `1.2.3.4.5`, `foo.123`, `foo.0x12`, `foo.09` | NotUrl | NotUrl |
| `169.254.169.254.nip.io` | admitted (a name: the DNS filter's job, below) | admitted |
| `bsky.social:443`, `:0443`, `:` | admitted as `https://bsky.social/` | same |

**Addresses.**
- Refused:
  - `::ffff:0:7f00:1` (SIIT), `::ffff:169.254.169.254`, `64:ff9b::a9fe:a9fe`, `64:ff9b:1::808:808`;
  - Teredo `2001:0:…`, AS112-v6 `2001:4:112::1`, ORCHIDv2 `2001:20::1`, 6to4 `2002:7f00:1::1`, Dummy `100:0:0:1::1`;
  - `fd00:ec2::254/253/123`, `fec0::1`, `ff02::1`, `::`, `::1`, `::7f00:1`, `3fff:fff::1`, `4000::1`;
  - `0.0.0.0`, `100.100.100.200`, `169.254.0.23` (Tencent metadata), `192.0.0.170`.
- Admitted: `::ffff:8.8.8.8`, `64:ff9b::808:808`, `2620:4f:8000::1`, `2606:4700:4700::1111`, `1.1.1.1`, `192.31.196.1`, `192.52.193.1`, `192.175.48.1`, `198.20.0.0`, `223.255.255.255`.
- The admitted ones are all public by design or IANA "globally reachable".

**Real DNS, production client** (`GuardedHttp::new()`):

| URL | Result | Error chain (Display) |
|---|---|---|
| `https://169.254.169.254.nip.io/latest/meta-data/` | Refused(Address) | "fetch refused" → "forbidden address" |
| `https://127.0.0.1.nip.io/`, `https://10.0.0.1.sslip.io/`, `https://localtest.me/`, `https://fd00-ec2--254.sslip.io/` | Refused(Address) | same, no host in the chain |
| `https://--1.sslip.io/` | Unavailable (rustls: invalid DNS name) | no host in the chain (finding 6) |
| `https://heron-nonexistent-q8z7x.com/` | NotFound | "host not found" |
| `https://example.com/` | 200 | n/a |

**Slow and lying servers** (relaxed client, 700 ms total):

| Server | Result |
|---|---|
| Headers trickled 1 byte per 100 ms | `Unavailable(TimedOut)` at 701 ms |
| Chunked body trickled 1 byte per 100 ms | `Unavailable(TimedOut)` at 700 ms |
| Close-delimited 2 MiB, no length | `Refused(BodyTooLarge)` at 2 ms |
| `Content-Length: 10`, then 600 KiB | `Ok`, 10 bytes (hyper stops at the length) |

**Header-size bounds** (no setting needed): HTTP/1 allows 100 headers (`hyper-1.8.1/src/proto/h1/role.rs:31`); HTTP/2 allows a 16 KiB header list (`proto/h2/client.rs:52`).

</details>

<details>
<summary><b>Guard bypass probe (findings 1–2): exactly what passed</b></summary>

I dropped an uncompiled `src/heron_bypass.rs` into `adapter-atproto`. The guard scans every `*.rs` under `src/`, whether or not it is a module. I also added a non-test `src/heron_dir/tests.rs`. `the_known_doors_stay_shut` flagged only one line, and only because a trailing comment on it contained "reqwest". These passed:

```rust
let r = jacquard::identity::JacquardResolver::default();
let s = jacquard::client::MemoryCredentialSession::unauthenticated();
let u = jacquard::client::UnauthenticatedSession::new_public();
let x: jacquard::client::MemoryCredentialSession = Default::default();
let c = web::Client::new();          // after `web = { package = "reqwest" }`
let o = <OAuthClient<_, _>>::new(store, data, Default::default());
crate::plc_directory::bare_client!() // a macro or alias exported from an allowlisted file
```

And `heron_dir/tests.rs` containing `reqwest::Client::new()` was never read. Slice 6's planned `JacquardResolver` token catches the first line only.

**Can't be closed at type level.** jacquard, jacquard-oauth and jacquard-identity all enable `jacquard-common/reqwest-client` unconditionally (their `Cargo.toml`s), so `reqwest::Client: HttpClient` always exists. The name list is the control; finding 4 removes the DNS part of it at compile time.

</details>

<details>
<summary><b>Dependencies: hickory and advisories</b></summary>

- **OSV, queried 2026-10-08:**
  - hickory-resolver, hickory-proto and hickory-net 0.26.3: **no advisories**.
  - hickory-resolver 0.24.4: GHSA-5j98-2g5x-46v6. hickory-proto 0.24.4: RUSTSEC-2026-0119 / GHSA-q2qq-hmj6-3wpp.
  - None for rustls 0.23.45, rustls-webpki 0.103.15, webpki-roots 1.0.7, h2 0.4.18, hyper 1.8.1, hyper-util 0.1.20, reqwest 0.12.28, url 2.5.8, idna 1.1.0 or tokio 1.50.0.
- **`cargo deny check advisories`** on slice 2: "advisories ok". The yanked-crate warning for `chacha20 0.10.0` is finding 7.
- **GHSA-5j98 has no RUSTSEC alias** (OSV record). So cargo-deny, which reads RustSec, cannot see it. That matters only while the 0.24 copy is in the build. Finding 4 removes it.
- **The `deny.toml` note for RUSTSEC-2026-0119 is accurate today.** Nothing calls `new_dns`/`with_system_dns`, and `JacquardResolver::new` leaves DNS unset. The old copy is compiled and never run. After finding 4 it is not compiled either.
- **The 0.26 DNS client reads the system configuration**, not Google's (`resolver.rs:112-117`). DNSSEC validation is off (the default), so the class of failure GHSA-5j98 describes does not apply to Zurfur's lookups either way.

</details>

<details>
<summary><b>Slice 1: what was checked and holds</b></summary>

- **`AtHandle` against atproto.com/specs/handle** (fetched today):
  - ASCII only, at most 253 characters;
  - at least 2 segments, each 1–63 of `a-z0-9-`, with no hyphen at either end;
  - the last segment must not start with a digit;
  - "Trailing dot syntax … is not allowed";
  - normalization is ASCII `A-Z` → `a-z` only (`to_ascii_lowercase`);
  - the eight disallowed TLDs;
  - IDN accepted in encoded form only ("In their encoded form, IDNs are already valid hostnames, and thus valid handles").

  Everything matches. URLs, DIDs, `@handle` and IP forms fail on the character check. The only constructor is `FromStr`: no serde, no `From<String>`.
- **`ResolveError` `Display`** is four fixed strings, and a test pins them. The `Debug` side is finding 10.
- **The fake's rules match §3.3.**
  - `resolve_handle`: forward lookup, then the document must claim exactly this handle, else `NotConfirmed`; an unknown DID is `NotFound`.
  - `resolve_did`: no claim gives `None`; the claimed handle is resolved **forward only**. The same DID gives `Some`; `Unavailable` gives `Err`; anything else gives `None`.
  - The suite pins the security-critical halves: a handle whose DID disowns it is never confirmed, and a DID whose claimed handle names another DID gets `None`. The gaps in the error classes are finding 11.
- **The proptest `every_claimed_handle_is_an_at_handle`** keeps the two types from drifting on what they store.
- **The Kelvin sign.** `"\u{212A}itty.zurfur.app".to_lowercase()` is `"kitty.zurfur.app"` with rustc locally; `to_ascii_lowercase()` leaves it alone, so `AtHandle` refuses it. An exhaustive scan of every code point finds U+212A is the only one that lowercases to ASCII (finding 12).

</details>

<details>
<summary><b>Sources</b></summary>

- atproto handle spec: https://atproto.com/specs/handle (fetched 2026-10-08)
- IANA IPv4 special-purpose registry (CSV and XML, updated 2025-10-09): https://www.iana.org/assignments/iana-ipv4-special-registry/
- IANA IPv6 special-purpose registry (updated 2025-10-09): https://www.iana.org/assignments/iana-ipv6-special-registry/
- IANA IPv6 address space (updated 2025-10-23): https://www.iana.org/assignments/ipv6-address-space/
- Vendor metadata pages (re-fetched 2026-10-08):
  - AWS: `instancedata-data-retrieval.html`
  - GCP: `querying-metadata`
  - Alibaba: `view-instance-metadata`
  - Azure: `what-is-ip-address-168-63-129-16`
- OSV API `/v1/query` and `/v1/vulns/GHSA-5j98-2g5x-46v6` (2026-10-08)
- chacha20 changelog: https://raw.githubusercontent.com/RustCrypto/stream-ciphers/master/chacha20/CHANGELOG.md
- crates.io versions API for chacha20 (0.10.0 and 0.10.1 yanked; 0.10.2 current)
- Vendored sources under `$R = ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`:
  - jacquard 0.12.0, jacquard-identity 0.12.0, jacquard-oauth 0.12.0, jacquard-common 0.12.0
  - reqwest 0.12.28, hyper 1.8.1, hyper-util 0.1.20
  - hickory-resolver 0.26.3, hickory-net 0.26.3

</details>

---

## Verification of the fix round (2026-10-08)

Checked: Otter's fix round, commit `8b90db5d276b` (change `vvltkpum`), diffed against `106b15ee938e`. Probes ran in a throwaway jj workspace, since forgotten and deleted.

**Verdict: slice 2's must-fixes are closed (mine and Magpie's), and nothing new was opened. Ship.** What's left is low or informational. All 27 guarded-client tests pass, and so do the guard's own checks on the real tree.

| Item | Status | Evidence |
|---|---|---|
| My must-fix: the guard missed jacquard shortcuts | **Closed** | `MemoryCredentialSession`, `UnauthenticatedSession` and `JacquardResolver::default` are now forbidden, and the detector test covers them. Re-running my bypass lines, all four jacquard spellings are flagged |
| Cargo rename; a `tests.rs` compiled outside `#[cfg(test)]` | **Closed** | The new manifest check flagged a live `web = { package = "reqwest" }` that I added to `cli/Cargo.toml`. The new module check flagged an ungated `mod tests;` |
| Transport-weakening calls | **Closed** | `use_preconfigured_tls`, `.resolve(`, `.resolve_to_addrs(`, `.proxy(`, `Policy::limited` and `Policy::custom` are now forbidden; `Policy::limited (3)` with a space is flagged too |
| Feature trim (hickory 0.24) | **Closed** | `cargo tree --workspace --all-features -e all -i hickory-proto@0.24.4` finds no such package. `Cargo.lock` holds only hickory 0.26.3. With the RUSTSEC-2026-0119 exception removed, `cargo deny check advisories` still reports "advisories ok", so removing it was safe. The one remaining warning is the yanked `spin` crate, unchanged from `main` |
| chacha20 yanked | **Closed** | Bumped to 0.10.2 |
| Magpie's must-fix: the TLS classifier | **Closed** | It now classifies only connect errors and follows each `io::Error` into its payload with `get_ref()`. The three new end-to-end tests are real; I broke the code each way and they caught it: dropping the connect-only check fails the garbled-body test, and walking `source()` again fails both TLS tests. Two cases the tests don't cover, using my own test CA trusted for the probe: a certificate for the **wrong hostname** → `Refused(Tls)`; an **expired** certificate → `Refused(Tls)`; a valid control certificate → 200 |
| Magpie: allowlist not pinned; DNS test could hang; no cap-boundary test | **Closed** | The allowlist is pinned to exactly three files. The DNS test now has an outer 2 s bound. The cap is tested at exactly the limit and one byte over, with and without a declared length |
| New tests real or tautological? | **Real** | Removing `.no_proxy()` fails the `Debug` test. reqwest pushes the system-proxy matcher whenever automatic proxies are on, whatever the environment (`$R/reqwest-0.12.28/src/async_impl/client.rs:418-421`), and `Debug` prints `proxies` whenever the list is non-empty (`:2761-2763`). The test's control client asserts that premise, so the test can't pass vacuously. Removing `.https_only(true)` fails the plain-http test |
| The committed key pair | **Acceptable as labelled test data** | It is a throwaway P-256 key for a self-signed certificate (`CN=untrusted test certificate`, 127.0.0.1 and localhost, valid to 2126) that nothing trusts. It is pulled in only by `include_bytes!` in `client/tests.rs`, so it never reaches a release build, and the crate is `publish = false`. Generating it at run time would need a new dev-dependency (`rcgen`) and buys no security. **GitHub today:** the repo is public, with secret scanning and push protection on and **non-provider patterns off** (`gh api repos/ZurNetwork/zurfur`). GitHub classes a `-----BEGIN PRIVATE KEY-----` key as the non-provider pattern `generic_private_key` ([supported patterns](https://docs.github.com/en/code-security/secret-scanning/introduction/supported-secret-scanning-patterns)), so it isn't scanned. The push went through (the remote branch is at `8b90db5d`), and the open secret-scanning alerts list is empty. If the Engineer ever enables non-provider patterns, expect one alert to dismiss as "used in tests". Whether push protection covers that pattern is **[unverified]**: the docs mark push protection as configurable for only "some" generic patterns. No CI secret scanner (gitleaks, trufflehog) is configured |

**Still open (all low, none blocking).**
- **A `JacquardResolver` door until slice 6.** `let r: JacquardResolver<_> = Default::default();`, `<JacquardResolver<_>>::default()` and `JacquardResolver::<_>::default()` pass the guard. jacquard's only `Default` impl is for the bare reqwest client, and Rust infers the type from that single impl: I compiled the same pattern on a standalone model. Slice 6's bare `JacquardResolver` token closes it. A cheap interim fix is to forbid `JacquardResolver<_>` and `JacquardResolver::<` now; `lib.rs` and `profile.rs` use neither spelling.
- **Two module spellings the module check misses.** `#[cfg_attr(not(test), path = "…/tests.rs")]` and `pub(in path) mod tests;` still pass. A one-line `#[allow(..)] mod tests;` can't survive the format gate: rustfmt puts the attribute on its own line, and then it is flagged. Contrived, so informational.
- **Earlier items unchanged.** A macro or type alias exported from an allowlisted file is still invisible to the scan (inherent to a per-file allowlist). Earlier finding 5 (hickory logging names at `error`) and finding 6 (an invalid DNS name counts as retryable) are unchanged. Both were low or informational.
- **Hostnames in the cause chain.** The probe confirms Magpie's heads-up: rustls's message carries both hostnames ("certificate not valid for name "127.0.0.1"; certificate is only valid for DnsName("other.example.com")"). That strengthens slice 1's finding 10 (a variant-only `Debug` on `ResolveError`) and the rule for slices 3 and 4: log the failure class only at `info`.
- **Optional tightening.** The `Debug` test checks only that `redirect_policy` appears. Asserting `Policy(None)` would also catch a change to a limited policy, which the guard already forbids by name.

---

## Slice 3 review (2026-10-08)

Reviewed: Stoat's change `qswzyknv`, commit `6e0372106ee5`, which merges slice 1 (`247730b9`) and slice 2 (`8b90db5d`). Held to design §3.1–§3.3, §5, §6, the carried items and the Engineer's handle ruling. Probes ran in a throwaway jj workspace, since forgotten and deleted. The 56 resolver tests pass, and so do the network-door guard's 9.

**Verdict: ship. No must-fixes.** The resolver confirms a handle only when both directions agree, refuses every odd DID shape before any request, and logs the failure class only at `info`.

**For slice 4 (low).** The two crate-private entry points sign-in will call directly, `confirm_handle` and `document`, run outside the 20 s port-call deadline. Sign-in's 30 s `start` deadline must wrap them, and the callback's issuer check needs one too. Today each step is still bounded: 10 s per fetch, 5 s per DNS lookup.

<details>
<summary><b>The attacks, one by one</b></summary>

| Attack | Result | Evidence |
|---|---|---|
| **Strict document parse: mini-docs** | **Refused.** jacquard's `DidDocument` requires `id`, so the Slingshot shape (`did`, `handle`, `pds`) fails to parse → `NotFound`. A mini-doc with an `id` added parses, but its `did`/`handle`/`pds` fields fall into ignored extra data: no claim, no PDS. That gains an attacker nothing | `$R/jacquard-common-0.12.0/src/types/did_doc.rs` (`id: Did<S>` has no default). Probe: `{"id":…,"did":…,"handle":…,"pds":…}` → `claim=Absent`, `pds=NotFound` |
| **`id` match** | **Holds.** Exact string match against the requested DID. A trailing space, an uppercase `DID:` or another DID → `NotFound` | `document.rs:112-118`; probes |
| **Duplicate JSON keys** | **Refused (fail closed).** Two `id`s in either order, two `alsoKnownAs` or two `service` → serde's duplicate-field error → `NotFound`. So no "first key vs last key" disagreement with JavaScript parsers can arise | Probes |
| **Trailing garbage, BOM, wrong types** | **Refused** → `NotFound`: `alsoKnownAs` as a string, an array holding a number, a BOM, text after the JSON | Probes |
| **`alsoKnownAs`** | **Holds the spec.** The first entry that is `at://` plus a valid handle is the claim. Entries that are padded (`at:// a…`), carry a path (`at://a/…`) or have an uppercase scheme (`AT://`) are skipped, and the next valid entry counts. A mixed-case handle is lowercased and compared normalized. A first valid entry under a disallowed TLD is `Unusable` and never looked up. A `.test` claim parses, then fails before DNS | `document.rs:84-99`; probe: `["at:// a…","AT://b…","at://C.Example.COM"]` → claim `c.example.com` |
| **`#atproto_pds` service** | **Holds.** Type `AtprotoPersonalDataServer` and an id ending `#atproto_pds`. An object endpoint gives `NotFound`; an `http://` endpoint gives `Refused`. Lenient but harmless: an id of `<another DID>#atproto_pds` is accepted. The document's author controls its own endpoint anyway | `document.rs:137-152`; probes |
| **did:web host and port** | **Holds.** The host must be `[A-Za-z0-9.-]` with no trailing dot. So `%3A` ports, `:` paths (`did:web:host:443`), `_`, `%2F` and empty hosts are refused before any URL exists. IP spellings (`127.0.0.1`, `2130706433`, `0x7f.1`, `127.1`), `1.2.3.4.5`, `localhost` and `.local` are then refused by the URL policy. `did:web:169.254.169.254.nip.io` builds a URL, but the slice-2 DNS filter refuses it at connect (verified end to end in slice 2). The host is restricted to those characters, so no `@ / ? #` can reach the URL | `document.rs:60-70`, `:44`; Stoat's test plus my probe of 12 forms |
| **did:plc** | **Holds.** Exactly 24 characters of lowercase base32. An uppercase id or a suffix is refused, and the URL is always `https://plc.directory/<did>` | `document.rs:50-56` |
| **TXT: multiple records** | **Holds the spec.** Each record's strings are concatenated. Values that don't start `did=`, or don't parse as a DID, are ignored. The same DID twice is one answer; different DIDs are `Conflicting` → `NotFound` | `handle.rs:25-46`; `dns.rs` (`txt_data.concat()`) |
| **TXT: DNS failures** | **No fall-through.** In hickory 0.26.3 only NXDOMAIN and NOERROR-without-answer become `NoRecordsFound`. SERVFAIL, REFUSED and the rest become response-code errors → `Unavailable` | `$R/hickory-net-0.26.3/src/error.rs:401-475` |
| **TXT: the 244-character cutoff** | **Correct.** `_atproto.` (9) + 244 = 253 characters, which is DNS's limit before the trailing dot (255 octets on the wire). A longer handle goes straight to HTTPS, as the spec says. It can't bypass anything: the HTTPS step has the same policy and filter | `limits.rs:19`, `handle.rs:57-60` |
| **TXT answers from a private resolver** | **No exposure.** A TXT answer yields only a DID, never an address to dial. That DID then goes through the same document rules and the guarded fetch. A split-horizon resolver could answer `_atproto` for internal-only names, but only the operator can put records there. Residual: the attacker picks the TXT name queried, which is inherent | `handle.rs:79-95` |
| **`.test` short-circuit** | **Before DNS.** `is_public_domain` (the 9 reserved TLDs, now one list derived from the spec's 8 plus `test`) → `NotFound`, and Stoat's test asserts nothing is asked | `handle.rs:66-69`; `reserved.rs` (`with_test`) |
| **Both ways** | **No path confirms a handle that doesn't point back.** `resolve_handle` returns a DID only when the document's claim equals the handle; otherwise `NotConfirmed`, or the document's own error class. `resolve_did` returns `Some` only when the forward lookup names the same DID. A forward `Unavailable` → `Err(Unavailable)`; `NotFound`, `NotConfirmed` or `Refused` → `None`. The conformance suite now stages all 15 cases, including the three I flagged and Magpie's two, and checks `{:?}` and `{:#}` for echoes | `client.rs:51-76`; `test-support/src/identity_resolver_contract.rs` |
| **Status classes** | **Holds; can't be turned against someone else.** Any 2xx is a body (the design said 200; harmless). 408/429/5xx → `Unavailable`; other statuses → `NotFound`; 3xx is refused by the client. Only the hosts in an identity's own chain decide its class, so an attacker can force retries only on lookups of their own identity. Nobody else can turn their outage into `NotFound` | `fetch.rs:38-51` |
| **Logging** | **Class only at `info`.** `failure = ?error` prints the variant only: `ResolveError` now has a hand-written `Debug`, which closes my slice-1 finding 10. The cause chain goes to `debug`. Probe with a log capture across `Unavailable`/`Refused`/`NotFound`: the `info` lines read `identity lookup failed failure=Unavailable`, and **no `info` line names a handle**. No `#[instrument]` and no other log line | `client.rs:106-111`; `domain/src/ports/errors.rs` (hand-written `Debug`); probe |
| **One shared hickory client** | **Opens nothing.** Every A/AAAA answer, cached or fresh, still passes the address filter on every use. A TXT answer is never dialled. hickory caches per query (name and type), and an answer can only inflate the attacker's own name. It does mean one resolver now carries every lookup (load note below) | `dns.rs`; `client.rs:31-41`; `$R/hickory-resolver-0.26.3/src/caching_client.rs:316-470` |
| **Carried items and the ruling** | **Done.** The TLD lists are one source (`RESERVED_TLDS = with_test(DISALLOWED_TLDS)`, imported by the URL policy). `.test` is `NotFound` before DNS. Logging is class-only. `AtHandle` refuses whitespace, and `Handle` lowercases ASCII only | diff |

</details>

**Low and informational (none blocking).**
- **plc.directory quota.** Every anonymous sign-in, and later every User Page view, costs one `plc.directory` fetch. A flood could hit plc.directory's per-IP rate limit, and then every lookup is `Unavailable` for everyone. This predates the work (jacquard made the same fetch) and is the Engineer's Q2: rate limits come with the User Page. Worth remembering that its cache answers this too.
- **Shared resolver under load.** One hickory resolver carries all TXT and A lookups. A burst of slow attacker domains, each held up to 5 s, contends with legitimate lookups. Same Q2 note.
- **`ResolvedDocument` derives `Debug`** over the whole document, handles included. Nothing logs it today; keep it out of `info`.
- **No test pins the log level.** My probe is the only evidence that `info` carries the class alone. A small log-capture test would pin it.

---

## Slices 4 and 5 review (2026-10-08)

Reviewed:
- **Slice 4, sign-in:** Lynx's change `xsroluls`, commit `de52f193`.
- **Slice 5, `me`:** Otter's change `nmmntuwt`, commit `efec6bbc`.

Both sit on slice 3 (`915c274b`). One probe ran against slice 4 in a throwaway jj workspace with a real Postgres test container; the workspace has since been forgotten and deleted.

### Must-fixes first

**1. Slice 4: the `complete` deadline can strand the wrong account's tokens. Must-fix; demonstrated.**
- **What happens.**
  - `complete` runs the whole `finish` under `tokio::time::timeout` (`authenticator.rs:221`).
  - jacquard saves the new session inside `callback`, before Zurfur's account check runs (`$R/jacquard-oauth-0.12.0/src/client.rs`, `create_session`, then `registry.set`).
  - On a mismatch, `logout()` fetches the authorization server's metadata, revokes, and only then deletes (`client.rs:819-835`).
  - If the deadline fires while `logout()` is still waiting, the future is dropped. Neither the delete nor the fallback delete ever runs.
- **Probe.** Using the slice's own test world, I signed in as mallory where the typed handle named alice, with the logout's metadata fetch delayed 1.5 s and a 1 s deadline. Result:
  - the error was `sign-in deadline passed`, not `AccountMismatch`, so the route says `exchange_failed`;
  - `store.get_session(mallory, state)` returned the row: **`stranded_session=true`**.
- **Who can trigger it.** An authorization server an attacker runs can answer the callback's fetches promptly and then stall the logout's metadata fetch. The same window strands an orphan session for the *right* account whenever a slow callback finishes just past 30 s.
- **Impact.** Today, low: nothing hands the stranded row to anyone, and the tokens belong to the account that consented at its authorization server. But it breaks the design's promise ("never leave its tokens stored"). Later code that loads a session by DID (the Character owner half, design page 54427650) could pick it up.
- **Fix.**
  1. On a mismatch, **delete locally first**, then revoke best-effort with the token still in memory.
  2. Keep that cleanup out of the cancellable deadline: run it after the timeout, under its own short limit.
  3. On *any* `complete` failure after the callback has started, delete whatever row has `session_id = state`. jacquard uses the `state` as the session id (`client.rs`, `session_id: auth_req_info.state`), and Zurfur owns the store, so a delete-by-state query is easy to add.

**2. Slice 5: no deadline over the whole profile read. Fix now; it's one line.**
- `fetch` makes three bounded calls: `document()` (the guarded client's 10 s, no port deadline), then `resolve_did` (20 s), then `getRecord` (10 s). Worst case is about 40 s per uncached read.
- Today only a signed-in user can do this, and only to their own `GET /me`, which is why it is low. But the User Page will put the same source behind anonymous requests for any DID, where it becomes a cheap way to hold server tasks.
- **Fix:** one `tokio::time::timeout` (20 s, or the design's port-call value) around `read` in `fetch`. Map it to an error so nothing is cached.

### Question for the Engineer: tie sign-in to the browser that started it (an older gap, not new in these slices)

- **What it means.** Today any browser that opens a valid callback link finishes that sign-in. The `state`, PKCE verifier and DPoP key are stored server-side under `state`, and nothing ties them to the browser that typed the handle: `POST /signin` writes nothing to the visitor's session (`api/src/routes/session.rs`).
  - So an attacker can start a sign-in as *their own* handle and approve it at their own PDS without following the final redirect.
  - The attacker then gets a victim to open `https://…/signin-callback?code=…&state=…&iss=…`.
  - The victim is now signed in to the **attacker's** account. This is classic login CSRF: anything the victim then uploads, writes or commissions lands in an account the attacker reads.
  - The DID binding doesn't help, because the bound DID *is* the attacker's.
  - The attacker can run their own authorization server with long-lived codes, so the link can be staged in advance.
- **Why the recorded defense doesn't cover it.** The Authenticators DD (19431425) accepts "PKCE *or* validated `state`" as the CSRF defense. RFC 9700 §2.1.1 adds the condition that makes PKCE work: the PKCE challenge must be "securely bound to the client and the user agent in which the transaction was started". §4.7.1 says the same of `state` ("links the request to the redirection URI to the user agent session"). Here the verifier is looked up by the `state` the callback itself carries, so neither defense is tied to the user agent. ([RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html), fetched today.)
- **(a)** Fix it in slice 4. At `POST /signin`, give the browser a short-lived, HttpOnly, SameSite=Lax cookie carrying the `state` or a MAC of it. The web front end calls the API server-side, so the front end has to relay that cookie to the browser. `signin-callback` then refuses unless the cookie matches, and clears it.
- **(b)** Same fix as its own slice, before the feature branch merges to `main`.
- **(c)** Accept and record the residual.
- **Recommendation: (b).** It touches the front end, the API and the callback contract, so a focused slice and review is cleaner than widening slice 4. It should land before this work ships, because sign-in is exactly what this branch hardens.
- **The cost of any fix:** a sign-in begun in one browser and finished in another (some mobile in-app browsers) fails and must be retried.

### Verdicts

- **Slice 4: ship after must-fix 1.** The login-CSRF question is separate and older.
- **Slice 5: ship after must-fix 2.**

<details>
<summary><b>Slice 4: the attacks, one by one</b></summary>

| Attack | Result | Evidence |
|---|---|---|
| **Complete as a DID other than the bound one** | **Refused.** The bound DID is read from Zurfur's store before jacquard's callback deletes the request. After the exchange, `signed_in` must equal it exactly, or the visitor gets `AccountMismatch` with logout plus the fallback delete. Two independent checks stand in the way: jacquard's `verify_issuer` already refuses a `sub` whose PDS has a different authorization server, so a malicious server can't hand back a victim's DID | `authenticator.rs:138-175`; `$R/jacquard-oauth-0.12.0/src/resolver.rs:372-387` |
| **Missing binding** | **Fails closed.** A request saved before this deploy has no `account_did`: it is deleted and refused | `authenticator.rs:179-194`; test `an_auth_request_without_a_bound_did_fails_closed` |
| **Replayed or forged `state`** | **Refused.** jacquard deletes the request before the code exchange, so a replay finds nothing (`UnknownState`). Two racing callbacks read the same binding, and the authorization server accepts a code once. A forged `state` names no request. **But** a *genuine* `state` plus code from the attacker's own flow completes in any browser: see the question above | `client.rs` (`callback_core` deletes first) |
| **The save-before-check residual** | **Not exploitable to gain access.** The session id is the `state` (unique). There is no in-memory cache (`registry.get` reads the store), and the stored session belongs to the account that consented. **The deadline turns the residual into a stranded row**: must-fix 1 | `session.rs:547-576` (jacquard) |
| **Mismatch cleanup** | `logout()` returns early when its metadata fetch fails, before revoke or delete. The fallback delete covers that (tested). The token lived in jacquard's per-call `OAuthSession` only, which is dropped. **No tokens in logs**: `info` gets the class `Display`; `debug` gets jacquard error causes (hosts and issuers), never the token set. If the store itself is down, the fallback fails and the route answers `exchange_failed` (low) | `authenticator.rs:161-172`, `:280-283` |
| **F5: authorization endpoint before the PAR** | **Holds.** `PublicHttpsUrl::try_from(authorization_endpoint)` runs before `par(...)`, and the test asserts the PAR mock receives 0 requests. The browser URL is the checked endpoint plus `client_id` and `request_uri`, added with `query_pairs_mut` (encoded), so a hostile `request_uri` can't leave the query. A fragment or existing query on the endpoint only affects the attacker's own server. `javascript:`, `http:`, ports, userinfo and IP literals are all refused | `authenticator.rs:106-108`, `:294-300` |
| **Server-side destinations** | **All guarded.** The protected-resource metadata, the authorization-server metadata, PAR, token, revocation and the issuer check go through `JacquardBridge` → `GuardedHttp`. The PAR endpoint is taken from the attacker's metadata, so it is the known blind-POST residual (public https:443 only; earlier finding 8) | `jacquard_bridge.rs:68-78` |
| **PDS `resource` comparison** | **Sound.** `resource_text` strips the one trailing slash the URL parser adds. jacquard requires `resource == <that text>` and, when present, `protected_resources` to contain it. Stripping a slash can't change the origin | `authenticator.rs:287-290`; `$R/jacquard-oauth-0.12.0/src/resolver.rs` (`resolve_protected_resource_info`, `get_resource_server_metadata_impl`) |
| **The bridge** | **Closed.** `HttpClient` delegates to `GuardedHttp`, and `resolve_did_doc` is Zurfur's checked fetch under a 20 s deadline; jacquard parses a buffer that already passed the strict parse. `resolve_handle` is a logging tripwire. It is unreachable on this path: `verify_issuer` builds an `AtIdentifier` from `sub`, a DID, so jacquard's `resolve_ident_owned` takes the DID branch, and `start` no longer calls `start_auth`/`resolve_oauth`. Options: empty `handle_order`/`did_order`, no PDS fallback, no Bluesky fallback. The only default methods that read them are the two overridden ones. jacquard's type-only PDS pick in the issuer check is the stated residual; it still goes through the URL policy | `jacquard_bridge.rs`; `$R/jacquard-identity-0.12.0/src/resolver.rs:387-403` |
| **Route: one 422** | **Holds.** Both refusals come from `signin_refused()`. The test pins status, content type and bytes across not-a-handle and five failure classes. Timing tells "not a handle" (µs) from "a handle that failed" (ms+), which reveals only what the attacker already typed. `trim()` happens at the route only (the ruling); `AtHandle` lowercases ASCII and refuses `@`, URLs and DIDs | `api/src/routes/session.rs`; `api/tests/session.rs` |
| **`account_mismatch`** | **Reveals nothing new.** The visitor chose the other account at their PDS | `callback-errors.ts` |
| **Deadlines** | `start` 30 s and `complete` 30 s, wrapping everything, which closes my slice-3 note; the bridge's document fetch has 20 s. Cancelling `complete` is the problem in must-fix 1. Cancelling `start` after the save leaves an unused request (harmless) | `authenticator.rs:44-61`, `:202-231`; `jacquard_bridge.rs:27` |
| **Logs** | Class only at `info` (`failure = %failure`, fixed strings); causes at `debug`. The tripwire logs no handle. jacquard's `tracing` feature stays off, so its `state`-instrumented `callback` span isn't compiled | `authenticator.rs:236-283` |

</details>

<details>
<summary><b>Slice 5: what holds, and two lows</b></summary>

- **The PDS comes from the validated document.** `document(did)?.pds()` is a `PublicHttpsUrl`; `getRecord` goes over `resolver.http()` (the guarded client), which re-admits the final URL. An `http`, ported or IP PDS fails before any fetch (`ProfileReadError::Pds`).
- **The handle comes from `resolve_did`, both ways.** `None` shows the DID instead.
- **An outage is an error, and nothing is cached.** `resolve_profile` caches only `Ok` (`application/src/user.rs`). A missing record is a clean `Ok(None)`.
- **Logs:** a static `failure.class()` at `info`. Otter added a test-only log recorder (`resolver/log_recorder.rs`, `#[cfg(test)]`), which closes my slice-3 "no test pins the log level".
- **The document is fetched twice per uncached read** (low; efficiency, not security). `document()` supplies the PDS and `resolve_did` fetches it again for the claim. A did:web host could serve two different documents, but both are the same controller's, so mixing them gains nothing. It does double the plc.directory load (the quota note in the slice-3 section). Suggest one resolver call that returns the document and the confirmed handle together.
- **The avatar CID isn't encoded or validated** (informational; older than this slice). `blob_url` pastes `avatar.blob().cid()` raw into the query string. jacquard defers CID validation (`$R/jacquard-common-0.12.0/src/types/cid.rs`, `Cid::Str`). A hostile profile record can add query parameters, but not change the host: everything after `?` stays on the validated PDS. Suggest building the URL with `url::Url::query_pairs_mut`, or checking `is_valid()`.

</details>

**The test constructors.** Lynx's `scripted(http, txt)` and Otter's `for_tests(http, txt)` sit in the same `#[cfg(test)] impl` with identical bodies, so unifying them on restack has no security relevance.

---

## Verification of slices 4 and 5 fix rounds (2026-10-08)

Checked:
- **Slice 4:** `de52f193` → `b5bdd257`.
- **Slice 5:** `55531406`, now on slice 4.

Probes ran in a throwaway jj workspace with a real Postgres test container, since forgotten and deleted.

**Slice 4: the stranding is fixed, but the fix opened a new must-fix.**
- **Fixed (re-run probe).** With logout's metadata fetch stalled past a 1 s deadline, the result is now `AccountMismatch` with `stranded_session=false`. The callback alone runs under the deadline. Cleanup runs after it: delete by `session_id = state` first, then a best-effort revoke within 10 s. It runs inside a spawned task, so a dropped request can't cut it short.
- **New must-fix: a replayed callback deletes a signed-in user's stored OAuth session (demonstrated).** `discard(state)` runs on *every* failure, including `UnknownState`. So a second `GET /signin-callback?code=…&state=<S>` after a successful sign-in deletes that user's live session row. Probe:
  - `first_ok=true`, `session_after_first=true`;
  - the replay answers `unknown sign-in state`, and `session_after_replay=false`.
  - **Who can trigger it:** the user, by reloading the callback page or pressing back; or anyone who sees the callback URL or its `state` (browser history, a screenshot, a request log that records query strings).
  - **What it costs:** today, little, since nothing reads stored OAuth sessions yet. Once PDS writes use them (the Character owner half), it becomes an unauthenticated way to break a user's PDS session.
  - **Fix:** delete by `state` only when *this* attempt found the auth request (`bound_account` succeeded). Only then can the callback have saved a session under that `state`: jacquard deletes the request before the exchange, and a `state` is single-use. `UnknownState` and `Unbound` should skip the delete.
  - Slice 7's browser cookie, if it is checked before `complete`, would also stop outsiders and reloads. Even so, the adapter shouldn't delete sessions its own attempt didn't create.
- **The spawned task.**
  - **No leak:** the `JoinHandle` is awaited; a dropped request only detaches the task, which still finishes.
  - **No double cleanup:** one `discard` per path. On a mismatch, logout's own delete finds the row already gone, which is harmless.
  - **Bounded:** the callback's 30 s, plus one DB `DELETE`, plus at most 10 s of revoke. The one unbounded piece is a hung Postgres statement, since no `statement_timeout` is set (low, app-wide). A panic inside the task would skip cleanup and surface as `Task` (low; no panicking path seen).
  - **The 10 s revoke limit is sane:** the row is deleted before revoking, so a slow or hostile authorization server can only delay a best-effort call. It covers one metadata fetch plus the revoke, each capped at 10 s by the guarded client.
- **Residual (low).** If the deadline fires while jacquard's session `upsert` is already on the wire, the dropped query can still commit *after* `discard`'s `DELETE`, on another pooled connection. The window is one database round-trip and hard to aim at. A periodic sweep of sessions no web session points to, or a second delayed `discard`, would close it.
- **Logs** stay class-only. `Resolve` and `Pds` now print `ResolveError`'s variant-only `Debug`.

**Slice 5: both items fixed. Ship.**
- **Whole-read deadline.** `read_within_deadline` wraps the entire read in the resolver's port-call deadline (20 s), and a timeout is an error, so nothing is cached. The resolver call inside also runs under its own 20 s, so the total is ≤ 20 s.
- **One document fetch.** `resolve_did_with_document` returns the document and the handle from a single fetch. It keeps `resolve_did`'s semantics exactly: the DID's own document failing is an error; no usable claim → `None`; a forward match → `Some`; a forward `Unavailable` → `Err(Unavailable)`; a forward `NotFound`/`NotConfirmed`/`Refused` → `None`. `resolve_did` now delegates to it, so the port's conformance suite still covers it.
- **Test-only items.** `with_port_call_deadline` and the unified `scripted` constructor are `#[cfg(test)]`.
- **Unchanged and informational:** the avatar CID is still pasted into the query unencoded. It is older than these slices and can't change the host.

**Verdicts.**
- **Slice 4: ship after the new must-fix** (delete by `state` only after this attempt found its auth request).
- **Slice 5: ship.**

---

## Slice 7 review and slice 4 replay verification (2026-10-08)

Reviewed:
- **Slice 4's replay fix:** `b5bdd257` → `795645fe`.
- **Slice 7, sign-in bound to the browser:** Lynx's change `ovpxvmvz`, commit `9ff32ba8`, on `795645fe`.

The replay probe ran in a throwaway jj workspace with a real Postgres test container, since forgotten and deleted. I didn't run `scripts/web-smoke.sh`: it needs the whole dev stack up (axum, vite and Caddy), so I read the routing from the `Caddyfile` instead.

### Must-fixes

**None.** No must-fixes in either slice.

### Verdicts

- **Slice 4: the replay is fixed. Ship.**
- **Slice 7: login CSRF is closed. Ship.**

### Slice 4: the replay fix

- **Re-run probe.** `first_ok=true`, `session_after_first=true`; the replay answers `unknown sign-in state`, and now **`session_after_replay=true`**: the live session survives.
- **The rule.** `found_request` is set only after `bound_account` succeeds. Unknown, unbound and replayed states, and (in slice 7) a refused browser, delete nothing. The cleanup `DELETE` has its own 5 s limit, so a stuck statement can no longer hold the task.
- **Residual (low).** Two callbacks for the same `state` racing in flight would both pass `bound_account`. The loser fails inside jacquard (`MissingState`) and then deletes the winner's freshly saved row. That takes a duplicated navigation by the same browser, or an observer of the URL in real time. Slice 7's cookie doesn't help, since both requests carry it. Suggest skipping the delete when jacquard's callback fails with `MissingState`, because the other attempt owns the session.

<details>
<summary><b>Slice 7: the attacks, one by one</b></summary>

| Attack | Result | Evidence |
|---|---|---|
| **The original login CSRF** (attacker's callback link opened by a victim) | **Closed.** The victim's browser either has no binding cookie (`NoBrowserBinding`) or one bound to another sign-in (`BrowserMismatch`). Both are refused **before any code exchange**, and the attacker's pending request is deleted. Tested both ways | `authenticator.rs` (`check_browser` runs first in `finish`); tests `a_callback_from_a_browser_without_the_cookie_is_refused`, `a_callback_carrying_another_sign_ins_cookie_is_refused` |
| **Forcing the victim's browser to *start* the attacker's sign-in** (so the browser holds a matching cookie) | **Blocked at both entry points.** The SvelteKit login action is covered by SvelteKit's default `csrf.checkOrigin` (cross-origin form `POST`s are refused in production; [SvelteKit configuration docs](https://svelte.dev/docs/kit/configuration); no override in `vite.config.ts`). Axum's `POST /signin` (reachable as `/api/v1/signin` through Caddy) sits under `require_first_party_origin`, which refuses a `POST` whose `Origin` isn't the public URL. Browsers send `Origin` on every non-GET request, `null` included, which also fails | `api/src/lib.rs:47-59`; `api/src/routes/mod.rs:33-49` |
| **Cookie tossing from a sibling subdomain** (`*.zurfur.app`) | **Closed by the `__Host-` prefix.** A `__Host-` cookie must be `Secure` and `Path=/` with no `Domain`, so no sibling can set or overwrite it. In staging and production **only** `__Host-zurfur.signin` is read, so a sibling-planted `zurfur.signin` is ignored (unit test with both present). Today no `*.zurfur.app` host serves user content anyway: handle hosts are answered by Zurfur's own well-known route | `api/src/routes/session.rs` (`browser_binding_cookie_name`, `presented_browser_binding`); `session/tests.rs` |
| **The nameless-cookie prefix bypass** (a sibling sets `=__Host-zurfur.signin=T`, which the browser sends back as `__Host-zurfur.signin=T`) | **Residual, browser-dependent.** A server can't tell such a cookie apart. Chromium rejects nameless cookies impersonating a prefixed one ([Chromium change](https://chromium.googlesource.com/chromium/src/+/23286dcf612b213d38a5c362f2a5106d5fc9c78c)); Safari rejects nameless cookies; Firefox fixed the related shadowing in 141 (CVE-2025-8037) and has since dropped nameless cookies (IETF HTTP WG thread, 2026). Exact versions are **[unverified]**. It also needs an attacker-controlled sibling host, which doesn't exist today. Keep it that way: never serve user content from a `zurfur.app` subdomain, which the plugin security model (41222145) already requires | search 2026-10-08 |
| **The dev name in production** | **Ignored.** The name follows `Environment::secure_cookies()` (STG/PROD → prefixed, Secure). Residual: an environment misconfigured as `DEV` in production loses `Secure` on both this cookie and `zurfur.sid` (older, config-level) | `composition/src/config.rs` |
| **A fixed or planted token** | **Can't be chosen.** The server mints 256 bits from `OsRng` per sign-in; the attacker can't pick the value bound to a state, and can't plant their own without the tossing above | `new_browser_token` |
| **`SameSite=Lax` on PDS → callback** | **Sent.** jacquard's PAR leaves `response_mode` unset (`$R/jacquard-oauth-0.12.0/src/request.rs:583`), and for `response_type=code` the default is `query`. So the PDS returns via a top-level cross-site **GET**, which carries Lax cookies in Chrome, Firefox and Safari. A `form_post` return would drop the cookie, but the route is GET-only and only an attacker's own server would choose it, failing their own sign-in | jacquard source; route is `get(signin_callback)` |
| **Token storage** | **Only the SHA-256 is stored** (`auth_request.browser_binding`, nullable; old rows → `StaleSignin`, fail closed; tested). The value is unsalted, but it hashes a 256-bit random token, so the stored hash reveals nothing. The raw token lives only in the `Set-Cookie`/`Cookie` headers. `BrowserBinding`'s `Debug` is redacted, and `SigninStarted` derives `Debug` over that redacted field. No `TraceLayer` or header logging in axum. The dev `Caddyfile` has no access log; a production Caddy log should keep its default redaction of cookie headers **[unverified for the production config]** | migration `20261008221546_…`; `domain/src/ports/authenticator.rs` |
| **Comparison** | **No useful leak.** The presented token is hashed, then the two 32-byte hashes are compared with an OR-fold behind `black_box`, and a stored value of another length never matches. Even a variable-time compare of hashes would leak only hash bytes, which can't be inverted. The lookup is by `state`, never by token. The 10-minute lifetime is enforced server-side (`created_at > now() - 600 s`) and pinned by a test | `hashes_match`; `get_browser_binding.sql` |
| **Clearing** | **Every callback answer clears it:** `signin_callback` wraps `complete_signin` and appends the cleared `Set-Cookie` with the same name, path and flags on success, `denied`, `invalid_callback`, `exchange_failed`, `account_mismatch` and the 500s (tested). A stale cookie can't break the next sign-in: each `/signin` overwrites it. The one cost is two sign-ins started in parallel tabs, where the older fails and is retried | `session.rs`; test `every_callback_failure_clears_the_cookie` |
| **Canceling someone's pending sign-in** (low) | A refused browser check deletes the pending request, so whoever knows a victim's *pending* `state` can cancel that sign-in. Only the victim's browser and its authorization server know it. The victim retries | `check_browser` |
| **The SvelteKit relay** | **Accepts only what fails closed.** `parseBrowserBindingCookie` takes only the two known names, a non-empty value and a positive integer `Max-Age`; it never takes `Domain` or `Path` from the backend and fixes `Path=/`, `HttpOnly`, `SameSite=Lax`. A wrong name or `Secure` from a buggy backend fails closed: the browser refuses a `__Host-` cookie without `Secure`, and production reads only the prefixed name. An oversized `Max-Age` keeps the cookie longer but can't extend the server's 10-minute window. A base64url token needs no encoding, so it round-trips through `cookies.set`. A missing cookie on the 303 is a `ContractViolation` (500), never a sign-in without a binding. `rewriteApiRequest` still forwards `zurfur.sid` only (pinned by a test) | `frontend/web/src/lib/server/api/zurfur-api.ts`; `routes/login/+page.server.ts`; `api-proxy.ts` |
| **Caddy** | `/signin-callback` is proxied to axum verbatim, and `reverse_proxy` forwards the browser's `Cookie` header, so the binding reaches the callback. The browser also sends the cookie (`Path=/`) on direct `/api/v1/*` calls, which Caddy proxies to axum, but no other route reads it, so that is harmless. SSR calls go through `rewriteApiRequest` and never carry it | `caddy/Caddyfile:43-70` |

</details>

**Corpus follow-up (already noted in the design doc).** This tightens the Authenticators DD (19431425), which accepted PKCE or `state` alone; a design-sync is owed.

---

## Slice 6 review and final verdict (2026-10-08)

Reviewed:
- **Slice 7's fix round:** diffed `9ff32ba8` → `d5155679`, restacked as `b675601b` onto slice 5.
- **Slice 6, the close-out:** Stoat's change `xlypqkqz`, commit `6e9069d0`.

A guard probe ran in a throwaway jj workspace, since forgotten and deleted; I also compiled one standalone rustc model.

### Final verdict on slices 1–7 as one unit

**Nothing must close before `feature/identity-resolver` merges to `main`. Ship.** Every must-fix raised on this branch, mine and QA's, is closed and verified:
- the guard's missing doors;
- the TLS classifier;
- the hickory 0.24 copy;
- the stranded sessions;
- the replay that deleted a live session;
- the whole-read deadline on `me`;
- login CSRF.

### Slice 7 fix round: verified

- **The race fix is right.** On `CallbackError::MissingState` from jacquard's callback, `found_request` is cleared, so the losing attempt deletes nothing. jacquard returns `MissingState` only from the start of `callback_core`, before any session is saved (`$R/jacquard-oauth-0.12.0/src/client.rs:297`, `:306`), so the loser never owns a row. A deadline mid-callback still deletes, which is correct. A unit test pins the classification.
- **The no-bound-DID test is restored** and re-saves the request bound to the same browser, so the missing DID is what refuses it.
- **The restack is clean.** `d5155679` → `b675601b` adds exactly slice 5's four files and the same diff; no slice-7 code changed.

### Slice 6: verified

**The guard.**
- `JacquardResolver` is now forbidden everywhere. That closes the single-impl `Default` spellings from my slice-2 verification: `let r: JacquardResolver<_> = Default::default();`, `<JacquardResolver<_>>::default()` and `JacquardResolver::<_>::default()`.
- `ResolverOptions` may be built only as the bridge's struct literal, never with a `::` constructor, a default, or a `..Default::default()` update.
- **Probe on the real tree.** I dropped a file of bypass lines into `src/` and ran the guard. These are now all flagged: both `JacquardResolver` spellings, `ResolverOptions::default()`, a typed `ResolverOptions = Default::default()`, `MemoryCredentialSession`, `PublicResolver` and a limited redirect.
- **The one line that slips:** `type Opts = …::ResolverOptions;` then `let o: Opts = Default::default();`. It is harmless: an options value does nothing without `JacquardResolver` (forbidden) or a new jacquard `IdentityResolver` impl, which a review would see. Informational.
- **Inference can't open a door either.** I compiled a standalone model: with no type named, rustc refuses to infer the bare resolver from trait bounds (`E0283: type annotations needed`). So every remaining door needs a forbidden token, a renamed reqwest (the manifest check), or a macro or alias exported from one of the three allowlisted files (unchanged and inherent).

**The hickory filter.**
- `composition::logging::without_dns_client()` is a global `FilterFn` layer on top of the operator's `EnvFilter`, used by both `api` and `cli`. Its `enabled()` vetoes every `hickory_resolver`, `hickory_net` and `hickory_proto` target (prefix plus `::` boundary, so `hickory_resolverish` still logs), whatever `RUST_LOG` says.
- Tests in `composition`, `api` and `cli` capture real subscriber output with the filter wide open (`trace,hickory_resolver=trace,…`): the domains never appear, and other events still flow. hickory logs through `tracing`, and a `log`-bridged event would carry the same target.

**Empty userinfo.** `https://@host/` and `https://:@host/` serialize as the plain URL, so there is no userinfo smuggling.

### Trying to get a handle into a log line

| Path | Result |
|---|---|
| **hickory** | Dropped at every level by the filter above (closes the earlier hickory-logging finding) |
| **jacquard** | Its `tracing` feature is off, so the `callback` span (`state`) and the `resolve_handle` span (handle) aren't compiled. The bridge's tripwire logs no handle |
| **The resolver** | `info` carries the class only (`ResolveError`'s variant-only `Debug`); causes, hosts included, at `debug` only (probe in the slice-3 section) |
| **Sign-in** | `info` carries `SigninFailure`'s fixed `Display`, with the resolver's class inside; never the handle, a host, `state` or the browser token (`BrowserBinding` `Debug` is redacted). A test pins it |
| **`me`** | `info` carries a static `failure.class()`; a test pins it with a log recorder |
| **The HTTP/TLS stack** | reqwest and hyper name hosts only at `debug`/`trace`. rustls's client-side `warn`/`error` lines name no host; its SNI warnings are server-side only. `api` has no request or header logging |
| **Whole runtime code** | A multi-line search of every non-test runtime file finds no `info`/`warn`/`error` call naming a handle, host, token or `state`. The PLC directory lines log a Zurfur-minted DID, not a handle (older) |

**Result: I found no path that puts a visitor's handle into a log line at `info` or above.**

### Still open, none blocking the merge

| Item | Owner and when |
|---|---|
| Anonymous sign-in has no rate limit: plc.directory per-IP quota exhaustion, and one shared resolver under a flood | The Engineer's Q2 ruling: with the User Page and its cache |
| Identity fetches need an egress IP no internal or partner service trusts; IMDSv2-only; a production Caddy access log that keeps cookie headers redacted **[unverified for the production config]**; no user content on any `zurfur.app` subdomain (that keeps the nameless-cookie prefix bypass moot) | Deployment checklist, tech lead |
| A dropped session `upsert` can commit after the deadline's `DELETE` (one DB round-trip window); no Postgres `statement_timeout` app-wide | Low; a periodic sweep of orphan OAuth sessions would close the first |
| Whoever knows a victim's *pending* `state` can cancel that sign-in | Low; accepted |
| `AtprotoPublicRecords` transport | Parked for the Engineer (no production caller) |
| Avatar CID pasted into the URL unencoded (can't change the host) | Informational; older |
| Design-sync of the Authenticators DD (19431425: browser binding now required), and the DD recording this design | Process follow-ups (design doc §6 and the binding ruling) |

# Unified identity: an investigation

2026-10-07 · Fox (identity lead), with Reynard (vulpes lead) · QA: Magpie · **Investigation only. Nothing here is decided.**

Every claim cites a corpus page id, a code path, a vulpes file and section, or a URL. "Unverified" means nobody checked it.

---

## 1. Summary

**The identity topics, in four groups** (full list in §2):

- **Sign-in:** who is using Zurfur right now. That covers web atproto sign-in, the CLI, plugin and Golem keys, and other apps reading the Den.
- **Issuing:** the DIDs, handles and repos Zurfur creates.
- **Resolving:** looking up handles, DIDs and URLs that anyone can type.
- **Linking:** alts, Character ownership, and Discord or Telegram accounts for Wolven.

**A unified provider?** One identity *module*, yes. A separate *provider* that merges many logins into one account, not now:
- Zurfur doesn't own Users' identities. Their DID comes from their own PDS (786439).
- Any design where several logins reach one User keeps a list of them, and B does too. What differs is two things:
  - who holds the list: Zurfur, or Vulpes or a third party;
  - whether the list spans products.
- Personas stay apart because each is a separate User (21594113). The list must never let one outside account reach two Users.

**Your question: Vulpes implements the ACP, so should it also be the provider?** Yes, Vulpes implements the ACP: "only meant to enforce the ACP as we designed it" (F49). **Caveat:** F49 is your 2026-10-01 ruling, but it is written up only on a local vulpes branch: commit `ebb0256`, not pushed, no PR. Vulpes `main` still lists "the first attestor" as a vulpes item. Q6 asks you to confirm it. "Provider" is two jobs:

| Job | In Vulpes? |
|---|---|
| 1a. **Proving** "this Discord account is this DID" as a public claim anyone can check | **Yes.** That is the ACP (acp.md "Claim kinds"). The `externalAccount` payload still needs defining (#24). |
| 1b. **The private list** of which sources are one person | **No.** acp.md: "never an ACP record". F49 gives storage to the app. |
| 1c. **Checking** the Discord account before attesting it | **No.** That's the attestor's work, which F49 leaves to Zurfur. |
| 2. **Signing in** from many sources | **No.** It fails F49's test ("does the ACP need it?"). F49 ruled OAuth out, and no second consumer signs anyone in. |

**I agree with the tech lead's lean** (its message to Fox, 2026-10-07)**, with two corrections:**
1. Only 1a goes in Vulpes; 1b and 1c stay in Zurfur.
2. Of the three reasons given against sign-in in Vulpes, "gate" and "centralizes logins" hold only for a *hosted* Vulpes. F49's scope test is what decides it.

Details in §3.

**What to unify:** one safe resolver; one path from any credential to a User DID; one rule for attaching Discord, Telegram or email.

**The options:**
- **A.** Vulpes does both jobs. This reverses F49.
- **B.** The split. Vulpes proves (1a). Zurfur signs in and keeps the private list, in one identity module that extends Authenticators (19431425). If you answer Q2 (a) and Q3 (b), other sources only attach to an existing User.
- **C.** A separate identity service, built or bought (Rauthy), shared with Wolven.
- **D.** atproto-only sign-in; other sources one at a time, as claims or Golems.

**Recommendation: B, starting with the safe resolver.**
- This follows from Q1 (c), Q2 (a) and Q3 (b). If you answer those differently, the recommendation changes.
- B fits the corpus rulings and F49 as written up, and fixes a hole that exists today.
- It gives new sources a slot that can't be used to take over an account.
- The resolver is needed under every option, so starting there loses nothing.
- A undoes the 2026-10-01 ruling. C adds a service and a central login list that no current need calls for. D is right if you want no other sources at all.

**Build or buy: buy the parts, not a front door** (§3b).
- Crates already cover each sign-in method: jacquard-oauth for atproto, `openidconnect` and `oauth2` for Discord and Telegram, webauthn-rs for passkeys.
- Brokers handle many sources, but only Rauthy speaks atproto. Its atproto login asks for full access plus the user's email, against 29687820 D1.
- No tool gives a Discord-only person a DID, or proves links without a central list. No Rust atproto resolver refuses private addresses by default.
- No new system is needed.

**Decisions for you** (§5):
- Q1 what "many sources" means
- Q2 whether someone without atproto can be a User
- Q3 whether other sources may sign in
- Q4 one safe resolver now
- Q5 where "this Discord account is this DID" lives
- Q6 whether F49 is in force, then how the two jobs split between Vulpes and Zurfur
- Q7 whether the CLI's ruled login (ZMVP-204, option C) should reopen
- Q8 the sign-in scope

**Found along the way** (for `/security-review`, not a ruling; details in §2.5):
- **F1.** Anyone can make the server send GETs, and a POST, to hosts of their choosing, through the sign-in form.
- **F2.** Sign-in asks for `transition:generic`, more than identity, unlike 29687820 D1.
- **F3.** Sign-in doesn't check the returned DID against the handle typed.
- **F4.** Handle lookups fall back to Bluesky's `public.api.bsky.app` when HTTPS fails, and DNS lookup is off.
- **F5.** The authorization URL Zurfur redirects to isn't checked to be https (589826's "https-only PDS redirect").
- **F6.** Web sign-in is a development-only localhost client. The confidential client that 24543244 D1 requires isn't built, so production sign-in is still identity work under every option.

---

## 2. Inventory: every identity source and flow

### 2.1 Sign-in: turning a credential into a User

| # | Source or flow | Status | What it yields | Governed by |
|---|---|---|---|---|
| S1 | atproto OAuth on the web: type a handle, approve at your PDS | Built **for local development only**: a public localhost client (`lib.rs:106`; `keyset: None` at `:110`), which the spec calls "a special exception for the localhost development workflow". The confidential client 24543244 D1 requires is unbuilt: a hosted client-metadata document, a signing key with JWKS, a public redirect (F6) | A verified User DID from the user's own PDS; a cookie session (`zurfur.sid`) | 19431425; 29687820 D1; 24543244 D1, D6; `api/src/routes/session.rs`; `adapter-atproto/src/lib.rs` |
| S1a | Same flow, started from a DID or a PDS URL | Reachable, not designed: jacquard accepts an `https://` URL as sign-in input | Same as S1 | `jacquard-oauth-0.12.0/src/resolver.rs:408-428` (`resolve_oauth_impl`) |
| S2 | CLI `zurfur session login` | Stub that answers `not_implemented`; its text, "blocked on the client-model ruling", is stale. Jira ZMVP-204 ruled **option C** on 2026-08-24: `login --handle \| --did` records which existing User the CLI acts as, with no OAuth, because the CLI already holds `DATABASE_URL` and the root key. Plan approved 2026-09-10, parked | A chosen existing User, no proof of possession | 24543244 "The three principal-establishment surfaces"; `cli/src/commands/session.rs:78-79`; ZMVP-204; plan `~/.claude/plans/plan-the-implementation-lovely-donut.md` |
| S3 | Plugin key: `app_key` per (plugin, account), exchanged for short-lived scoped tokens | Designed, Release | A delegated principal: (plugin, User) | 24543244 D3; 30572603; 589826 (Release) |
| S4 | Golem key: one credential per Golem, held by its external host | Designed, Release | The Golem's own DID | 28049410; Golem 12451841 ("Credential" row) |
| S5 | Golem admin acting as the Golem, for admin tasks only, through the admin's own session | Designed | Admin operations on the Golem | 28049410 D4, D5 |
| S6 | A login for third-party views of the Den (a "second authenticated surface") | Not designed | Lets other views read private nodes | 2026093001 D2 and its accepted tradeoff |
| S7 | Dev sign-in as a fixed user ("alice") | Dev only; must be unreachable in a deployed build | A fixed DID | Den question sheet §5 item 18; `adapter-mem/src/lib.rs:829` |
| S8 | Discord, Telegram, email, passkeys | Not decided anywhere | — | The Telegram bot is a Release Golem, not a sign-in (589826); a Discord bridge is post-Release (589826) |

### 2.2 Issuing: identities Zurfur creates

| # | Source or flow | Status | Governed by |
|---|---|---|---|
| I1 | did:plc for each Account, with per-DID keys, submitted to `plc.directory` | Built | 26804226; 57081857 D1 |
| I2 | did:plc for each Character, minted with no handle; owner keys per Character | Decided | 57081857 D2, D3; 54427650; 2026100701 D20 |
| I3 | did:plc for each Golem | Decided | 28049410 D2 |
| I4 | `*.zurfur.app` handles, served over HTTPS well-known | Built | 26607618; `api/src/routes/wellknown.rs` |
| I5 | Own-domain (BYO) handles for Accounts, checked both ways before commit | Decided; "BYO-domain verification" is on the security-first list | 24870914 D2, D3; 27852802 D6; 26050561; 589826 |
| I6 | PDS repos hosted by Zurfur for Accounts and public Characters, written with a per-repo password | Decided, alpha; own DD pending | 54427650; 29687820 D6 |
| I7 | Character handles | Open | 54427650 open rows; 57081857 D3 |
| I8 | A user's own recovery key, senior to Zurfur's (credible exit) | Decided, opt-in; enrolment not built | 26804226 D2, D3; 27852802 "credible exit" |

Not issued: **a DID for a human User.** "The platform does not issue new user DIDs; users authenticate with the identity they already have" (786439). "A User's repo is never Zurfur-hosted" (29687820 D6 note).

### 2.3 Resolving: looking up identities others own

| # | Path | Who chooses the host | Status | Governed by |
|---|---|---|---|---|
| R1 | Sign-in start: handle, DID or URL, then the PDS, then the authorization server | **Anyone, before signing in** | Built | 19431425; `session.rs:60`; see F1 |
| R2 | `me`: the DID document, then the handle checked back to the DID | A signed-in user, for their own DID | Built | `adapter-atproto/src/profile.rs:67-88` |
| R3 | User Page: `/profile/{DID or domain}` | Anyone | Planned | 21594113 D2 ("resolves for any resolvable handle"); `~/.claude/plans/user-page-jacquard-ssrf-audit.md` |
| R4 | Checking an own-domain handle at Account create or rename | The Account Owner | Decided | 27852802 D6 |
| R5 | Recording a bare DID the first time it's seen (credits, firehose) | Whoever wrote the record | Decided | 34013187 D6 |
| R6 | Showing a handle only when it checks out both ways | — | Decided | 2026100701 D6 |
| R7 | Vulpes ports `DidResolver`, `RepoReader` and `StatusSource`, which Zurfur implements | `RepoReader`: the `serviceEndpoint` in the DID document, which its owner writes. `DidResolver`: `plc.directory` for did:plc, the DID's own domain for did:web, which ACP accepts. `StatusSource`: the attestor | Ports defined; only `StatusSource` carries SSRF rules (F42: no redirects, refuse non-global addresses). Zurfur's implementations not built | Reynard: `src/acp/ports.rs`; F42; F49; acp.md "Use with the AT Protocol" |

### 2.4 Linking: saying two identities belong together

| # | Link | Public or private | Status | Governed by |
|---|---|---|---|---|
| L1 | User-Linking: proving two of your handles are you | Public, opt-in | Post-MVP | 21594113 (the "two separate craft reputations" paragraph) |
| L2 | Character ownership: paired halves with one edge id, `owner` in the User's repo and `owned` in the Character's | Public | Decided, alpha | 54427650; vulpes `docs/ccs.md`; FORKS F48 |
| L3 | Assertions about someone (achievements, ratings), signed by a Provider | Public | Waits on the Provider envelope | 29622283 D4 |
| L4 | `identity.externalAccount`: "this Discord or Telegram account is this DID" | Public ACP claim, attested by whoever checked it | Seed kind only; no payload schema | Reynard: vulpes `docs/acp.md` "Claim kinds"; issue #24 |
| L5 | `identity.email` | Public ACP claim | Seed kind only | Same |
| L6 | The "two-signature connected identity" proof | Private | Pattern only, never built. Listed in vulpes ROADMAP "Later", but its private store conflicts with F49 | Reynard: `docs/identity-model.md` "The central tension"; ROADMAP "Later" |

### 2.5 Findings from reading the code

These come from reading code, not from running an attack. Each goes to `/security-review` before anyone relies on it.

- **F1. The sign-in form lets anyone make the server send GETs, and a POST, to hosts they choose.** (Widened after QA4.)
  - `POST /signin` passes the typed string straight to jacquard (`session.rs:60`, `lib.rs:70`).
  - **The input names hosts.**
    - jacquard-oauth 0.12.0 (the version in `Cargo.lock`) treats an input that starts with `https://` as a server to fetch (`resolver.rs:415-424`).
    - A plain handle is enough too: jacquard fetches `https://{handle}/.well-known/atproto-did` (`jacquard-identity lib.rs:718`). Handle syntax rejects `.local`, `.internal` and the like, but not a public name whose DNS points at a private address.
    - From there it fetches the PDS the DID document names, then the authorization server the PDS names, then sends a PAR **POST** to that server (`resolver.rs:530-557`; `request.rs:590-601`).
  - **The client has no guards.**
    - It is a bare `reqwest::Client::new()` (`lib.rs:113`).
    - Neither jacquard-oauth nor jacquard-identity filters private addresses.
    - reqwest follows up to 10 redirects by default, including https to http (`reqwest-0.12.28 redirect.rs`).
    - Bodies are read whole, with no size cap (`jacquard-common http_client.rs:87`).
    - The identity fetches have no timeout (`jacquard-identity lib.rs:462-488`).
  - **Taking only handles doesn't fix it.** That removes one branch. The fix is the guarded client.
  - The Origin guard on the route lets a request with no `Origin` header through (`api/src/routes/mod.rs:31-48`). It is a CSRF defence (24543244 D1), not an SSRF control. The web form's own check doesn't protect the API.
  - The memory `project_jacquard_resolver_no_ssrf_controls` says sign-in only resolves "an identity the user already authenticated as". That is not true for the *start* of sign-in.
  - Vulpes had closed only the input half: F29 made `start` take a validated `Handle`, so the raw-URL branch was unreachable. The fetches of the well-known file and of the DID document's `serviceEndpoint` stayed the deployment's to guard, by supplying a guarded client. F49 retired that code with OAuth (Reynard: FORKS F29, F49; vulpes README "SSRF: install a guarded connector").
  - The atproto spec asks for "hardened" HTTP clients against URLs that resolve to private addresses (atproto.com/specs/oauth, Security Considerations).
- **F2. Sign-in asks for more than identity.** The code requests `atproto transition:generic` (`lib.rs:97`). 29687820 D1 says sign-in requests the minimal scope. The spec says "a client may include only the atproto scope if they only need account authentication", and that `transition:generic` is "equivalent to the previous App Password authorization level" (atproto.com/specs/oauth).
- **F3. The DID returned isn't checked against the handle typed.**
  - jacquard checks that the returned DID's own PDS uses this authorization server (`request.rs:734`, `verify_issuer`).
  - It never stores the DID found at start: `account_did: None` (`request.rs:611`). So it never compares the two.
  - The spec calls that comparison "critical" when sign-in starts from a handle or DID (atproto.com/specs/oauth). 19431425's own contract requires it too ("`sub` matches the DID bound at `start()`").
  - Impact is unverified. It is probably low, because the DID returned is still one that this authorization server legitimately controls.
- **F4. Handle lookups quietly fall back to Bluesky.** (QA5.)
  - jacquard-identity's defaults set `public_fallback_for_handle: true` (`resolver.rs:296`).
  - `JacquardResolver::new` sets `dns: None` (`lib.rs:352`), so the DNS TXT method returns nothing.
  - When HTTPS well-known fails, the resolver asks `public.api.bsky.app` (`lib.rs:736-760`). Both the OAuth client and the profile client use these defaults.
  - For handles that only DNS TXT verifies, which is common for own-domain handles, two things follow:
    - Bluesky learns which handles are typed into Zurfur;
    - "checks out both ways" (2026100701 D6; R2) becomes "Bluesky's AppView says so". The same would hold for BYO-domain verification (R4) built on these defaults.
- **F5. The authorization URL isn't checked to be https.** (QA6.)
  - The server's `authorization_endpoint` comes from metadata that whoever runs the server controls (`jacquard-oauth types/metadata.rs:19`).
  - It is joined into the URL (`client.rs:253`) and returned as the 303 `Location` (`session.rs:65`).
  - Severity is low: OAuth sends the user to their own server anyway. But "the https-only PDS redirect" is on the 589826 security-first list.
- **F6. Web sign-in only works for local development.** (QA3.) See S1. Production needs the confidential client of 24543244 D1. The scope string (F2) is part of today's localhost `client_id`, so F2 and F6 are fixed together.

---

## 3. Options compared

### Your Vulpes question, in detail

The tech lead's lean (its message to Fox, 2026-10-07, relaying your question): job 1 belongs in Vulpes and job 2 doesn't, because sign-in in Vulpes would (i) make it a gate, (ii) centralize every app's login against credible exit, and (iii) pull sessions, OAuth secrets and URL fetching in beside the keys. Checked against the vulpes files:

- **(i) and (ii) apply to a hosted Vulpes, not to the library F49 rules it to be.** A hosted "reference instance" is still described in the vulpes docs and was never formally retired (identity-model.md "The pivot" 3; ccs.md "Position in the three-layer role"). So the reasons hold against putting sign-in in *that*. "Facilitator, never a gate" is the phrase of vulpes #19, filed for Wolven from your account on 2026-08-24, before F49. #19 describes a hosted vulpes that "attests on other applications' behalf". The ruled vulpes equivalents are acp.md Conformance (no attestor "privileged by protocol") and ccs.md "The kill test".
- **(iii) holds only partly.** As a library, vulpes code runs in Zurfur's process, beside Zurfur's own keys, either way. What it would add is network-fetching code in the crate everyone trusts for key rules.
- **The decisive reasons:**
  - F49's own test, "does the ACP, or one of its conformance clauses, need it?";
  - F49 ruled OAuth out on 2026-10-01, and sessions, CSRF and provisioning were never carried over (F17/F18);
  - Wolven, the other consumer, wants claims, verification and minting, not sign-in (Reynard, from vulpes #19).
- **Job 1 is not all ACP.**
  - The public claim and attestation shapes and their checks (1a) are the ACP.
  - The private list of a person's sources (1b) is "an implementation-private concern, never an ACP record" (acp.md "Privacy and security concerns").
  - The diligence behind an attestation (1c) is the attestor's work, and "the first attestor" is Zurfur's to build (vulpes ROADMAP "Later"; F49).
- **F49 itself hasn't landed.** The ruling is written up only on a local branch: commit `ebb0256` on `docs/f49-pure-library`, not pushed, no PR. `origin/main` has no F49, and its ROADMAP still lists "the first attestor" (an email attestor) as a vulpes item. The code cut hasn't happened either: OAuth, axum, vc and postgres still ship. Everything here that says "since F49" depends on your confirming it (Q6).
- **Out of scope, found on the way.** Whether Zurfur ever mints through Vulpes is a separate question: 54427650 says "*our* minter", and Zurfur has its own (`adapter-atproto/src/did_minter.rs`). If it did, Vulpes would lack four things, according to Reynard plus QA11. It needs an input for a client-generated owner key. It needs an "attach a PDS" update op, which F8's strict guard would refuse today. It needs a handle-less mint for Characters: vulpes `mint` requires a handle (`minter.rs:310-318`), while 57081857 D3 mints Characters without one. And it may need a PDS key role, which is unclear because `MintPolicy` can already declare a PDS service.

**What each option is:**

- **A. Vulpes does both jobs (the provider).** Vulpes takes back OAuth and storage, and a hosted service if every app is to share it. It holds each person's logins, and Zurfur asks it who someone is.
- **B. The split: Vulpes proves, Zurfur signs in.** Vulpes keeps only job 1a: claim and attestation shapes and their checks. Zurfur gets one identity module. This is the planned `identity` crate (CLAUDE.md, Architecture: the `domain` crate "splits into per-domain crates (`identity`, …)"). It has four parts:
  - **one sign-in seam:** each credential kind ends in a User DID (atproto OAuth now, then the CLI, then keys);
  - **one hardened resolver** for every handle, DID and URL lookup;
  - **issuing,** as it is today;
  - **linking:** a private table of extra sign-in sources, plus public ACP claims made through vulpes, with Zurfur as the attestor.

  If you answer Q2 (a) and Q3 (b), Discord, Telegram or email can only attach to a User who signed in with atproto, and never create one.
- **C. A separate identity service.** A service run by Zurfur that accepts Discord, Telegram, email and atproto, and issues tokens to Zurfur, Wolven and others. It can be built, or bought and run, for example Rauthy (§3b).
- **D. atproto only.** Sign-in stays atproto. Other sources come in case by case, as claims or as Golems (the Telegram bot), never as sign-in.

| Criterion | A. Vulpes does both jobs | B. The split | C. New service | D. atproto only |
|---|---|---|---|---|
| **Job 1a (public proof) lives in** | Vulpes | Vulpes | The service, or Vulpes | Vulpes, if used at all |
| **Jobs 1b and 1c (private list, the check behind an attestation)** | Vulpes | Zurfur | The service | — |
| **Job 2 (sign-in) lives in** | Vulpes | Zurfur (extends 19431425) | The service | Zurfur, atproto only |
| **Takeover through the weakest linked source** | Depends on the attach rules, as for B (§4 B), but a second party holds the list | Contained, if the attach rules hold: attach only while signed in with atproto plus a fresh atproto login; never match on email; key by issuer and subject (§4) | Depends on the attach rules, and the service becomes a single target for every product | None: one source |
| **SSRF from resolving typed URLs** | Needs the hardened resolver | Hardened resolver is a named part of the module | Hardened resolver lives in the service, which then becomes a high-value target | Needs the hardened resolver (F1 exists today) |
| **DID and handle correlation** | Vulpes would see every sign-in. By analogy, that is the correlator its rulings guard against for disclosures (identity-model.md T1; F-A: "never persists a (RP, Holder, question, time) record") | The private link table sits in Zurfur's store, which already holds everything about the User. It must never be queryable from outside (Reynard Q3) | A new party sees every sign-in, for every product | Nothing new |
| **Privacy and unlinkability** | Conflicts with the vulpes pivot: vulpes "is not in the disclosure loop" (identity-model.md "The pivot") | Holds, if one outside account can attach to at most one User and links are never shown publicly unless the user publishes them | Same risk as A | Holds by construction (21594113) |
| **Credible exit** | A User's DID is still their PDS's; sign-in now depends on vulpes | Unchanged: humans keep PDS DIDs; extra sources are conveniences, never the only way in (Reynard Q3a) | If people sign in only through the service, they're tied to it | Unchanged |
| **Fit with decisions; what it would supersede** | Reverses vulpes F49 (2026-10-01) and the NQ1 pivot; touches 54427650 ("vulpes holds no keys") | Fits. Amends 19431425 (sign-in gains more methods) and 786439 only if Q2 changes. Uses the ACP as decided | Supersedes 19431425 and the principal surfaces of 24543244; likely 786439 | Supersedes nothing |
| **Effort** | Very high: rebuild what F49 just removed, across two repos | Medium, in steps: resolver (small), sign-in seam (medium), each new source (medium), attestor (medium, later) | High if built. Medium-high if a broker is run (Rauthy, §3b). Either way: its own ops, keys and trust root, plus the resolver | Low: the common work only (below) |
| **Where it lives** | Vulpes | Zurfur | A new repo and service | Zurfur |
| **Serves Wolven #24?** | Claim half only | Claim half only. Zurfur could attest after its own Discord check (Reynard Q5), but then Wolven depends on Zurfur (Q5 (d)) | Claim half only | No |

**Common to every option:**
- the hardened resolver, with DNS on and the Bluesky fallback off (F1, F4);
- the production confidential client (F6) and the sign-in scope (F2), fixed together;
- the start-bound DID check (F3);
- the https check on the authorization URL (F5);
- the CLI's ruled `login --handle | --did` (ZMVP-204, option C).

**No option serves #24's lookup half:** "given `(service, id)`, a consumer can obtain the candidate DIDs". Who may run that index is open (Q5 (e)).

**C after the build-or-buy survey (§3b).** C doesn't have to be built: Zurfur could run an existing broker. Rauthy is the only one with atproto login, which would bring C's effort down from high to medium-high. Three things don't change:
- the broker issues its own subject id, so getting the DID out needs custom mapping;
- it still can't give a DID to a Discord-only person;
- it becomes the list of every login, which is the correlation risk above;
- Rauthy's atproto login requests `transition:email`, `atproto` and `transition:generic` and fails without an email (QA16; `src/data/src/entity/atproto.rs:70-73`). Every sign-in would ask for full access plus the email, against 29687820 D1 and F2, and Zurfur would collect PDS emails it doesn't hold today.

---

## 3b. Build or buy: existing tools

Surveyed 2026-10-07 by a research agent from primary sources: repos, `LICENSE` files, docs and NVD. Fox re-checked the Rauthy and AIP rows. "GHSA" counts a repo's published security advisories; that reflects how mature its disclosure practice is as much as how risky it is.

| Tool | What it is | Many sources? | atproto / DID | Linking default | Licence | Fit for Zurfur |
|---|---|---|---|---|---|---|
| Keycloak | Broker and identity server (Java) | Yes; Discord and Telegram via community extensions | None found | Safe: detects the existing account, then asks the user to confirm by email or re-login | Apache-2.0 | Heavy to run; no atproto |
| Ory Kratos + Hydra | Headless identity API + OAuth2 server (Go) | Yes, including Discord, passkeys and email codes | Kratos: none. Hydra lets Zurfur's own login app set `subject` to the DID | Safe: re-prove an existing credential; auto-link only for Google and Apple | Apache-2.0 | Hydra could front Zurfur's own atproto login; a second service to run |
| Authentik | Broker (Python) | Yes: Discord, Telegram, OIDC, SAML, passkeys | PR #24875 open, not merged | Safe: matches on the source's own id | MIT core | No atproto yet |
| Zitadel | Broker (Go) | Yes | None found | Safe: auto-link prompt off by default | AGPL-3.0 | No atproto; AGPL |
| Logto | Identity server (TypeScript) | Yes | Only a third party's connector (`@openverifiable/connector-bluesky`, Apache-2.0). It skips the spec's DID→PDS→issuer check, and generates PKCE and DPoP keys at callback time when no state store is configured | Prompts before linking | MPL-2.0 | Unsafe atproto path |
| Casdoor | Broker (Go) | Yes | None | **Unsafe:** links silently on email or phone; CVE-2026-9092 (CVSS 9.1 from the CNA; NVD status "Deferred"), a takeover through an unverified email claim | Apache-2.0 | No: the takeover case in practice |
| Dex | OIDC federation shim (Go) | Yes, 16 connectors | None | No linking across connectors | Apache-2.0 | No atproto |
| **Rauthy** | Broker and identity server (**Rust**) | Partly: generic OIDC/OAuth upstreams; passkeys first-class | **Native atproto login, opt-in since v0.31.0**, built on atrium-oauth | Safe: refuses a colliding email unless linked from a signed-in session or `auto_link` is on | Apache-2.0 | Closest broker, with caveats. Its atproto login asks for `transition:email atproto transition:generic` and requires an email, against 29687820 D1. The independent audit at v0.32 doesn't mention atproto. `atrium-oauth` has `todo!()`s an attacker's server can reach. One upstream per user (survey's inference) |
| Kanidm, Authelia, Pocket ID | Identity servers | No upstream federation, or passkeys only | None | — | MPL-2.0 / Apache-2.0 / BSD-2 | Don't do the job |
| **AIP** (graze-social) | atproto → OAuth 2.1 / OIDC server (Rust, axum) | atproto only; would feed a broker | `sub` = DID; PAR, DPoP | — | MIT | The only credible bridge. No audit found; last release 2026-01-08 |
| ATLogin, at-oidc, DecentAuth | atproto → OIDC bridges and libraries | atproto only | `sub` = DID | — | ATLogin: BSD-3 on GitHub, MIT in its README / unverified / none | ATLogin says it has "never been security reviewed" and invents an email from the handle; DecentAuth is "not reviewed for security" |
| **jacquard-oauth / -identity** | Rust atproto OAuth client and resolver | atproto | Checks issuer ↔ DID; PAR, DPoP | — | MPL-2.0 | In use (0.12.0 locked; 0.12.2 out). No private-address filter; the handle→DID alias check is only a warning |
| atrium-oauth | Rust atproto OAuth client | atproto | Checks the issuer | — | MIT | `todo!()` panics on some server responses (`oauth_client.rs` L253, L300) |
| `openidconnect`, `oauth2` | Rust OIDC / OAuth2 clients | Any plain OIDC/OAuth source | No DPoP ("not planned"), so not usable for atproto | — | MIT / MIT OR Apache-2.0 | Right for Discord (OAuth2) and Telegram (OIDC with PKCE) |
| webauthn-rs | Rust passkeys | Passkeys | — | — | MPL-2.0 | Audited by SUSE |
| reqwest-ssrf-guard, http-acl-reqwest | SSRF guards for reqwest | — | Not atproto-aware | — | MIT / Apache-2.0 | Right design: they filter at DNS resolution, so the address checked is the one dialed, and they re-check redirects. **But both need reqwest 0.13,** and jacquard 0.12 locks reqwest 0.12.28. Until jacquard moves, Zurfur writes its own small `reqwest::dns::Resolve` filter the same way |
| `ssi` (SpruceID) | Rust DID and credential library | — | **No did:plc** | — | Apache-2.0 | Doesn't fit DID resolution here |
| DIF Universal Resolver | DID resolver, one container per method (Java) | — | did:plc driver at 0.0.1 "dev" | — | Apache-2.0 | Heavy, and immature for did:plc |
| Auth.js, better-auth | JS auth libraries | Yes | No atproto provider (better-auth: PR open) | Auth.js safe; **better-auth links by default** on a verified email | ISC / MIT | JS only; our backend is Rust |
| IndieLogin.com | Domain login service (PHP) | IndieAuth, GitHub, email, Bluesky | Returns a domain URL, not a DID | rel=me | MIT | Different identity model |

**What the survey says:**

1. **Sign-in from many sources is a solved problem,** as libraries and as brokers. A new system isn't needed for it.
2. **No tool turns a Discord, Telegram or email login into a DID.** Every broker issues its own subject id and keeps the link list itself. Q2 and Q3 still need your ruling whatever is bought.
3. **The takeover risk lives in linking defaults.** Casdoor's CVE and better-auth's link-by-default show the failure. Keycloak's re-login and Rauthy's link-from-a-signed-in-session show the safe pattern (§4 B 1).
4. **For option B, every piece exists as a crate:**
   - jacquard-oauth for atproto;
   - `openidconnect` or `oauth2` for Discord and Telegram;
   - webauthn-rs, if you ever want passkeys;
   - an SSRF guard for the resolver. The two guard crates need reqwest 0.13, so until jacquard moves this is a small resolver of our own (Q4).
5. **For option C, Rauthy is the only broker with atproto,** and its atproto login breaks identity-only sign-in (full access plus email).
6. **Proving links without a central list is not covered by any tool.** That is the ACP's job (job 1a). Prior art worth comparing: `atproto-attestation` (ngerakines' atproto-crates, MIT).

Full survey with sources: the research agent's report to Fox, 2026-10-07. Key sources:
- https://github.com/sebadob/rauthy (CHANGELOG "Bluesky / AT Protocol")
- https://github.com/graze-social/aip
- https://github.com/goauthentik/authentik/pull/24875
- https://www.ory.com/docs/kratos/social-signin/link-multiple-provider-account
- NVD CVE-2026-9092
- https://core.telegram.org/bots/telegram-login
- https://www.better-auth.com/docs/concepts/users-accounts

---

## 4. Risks, and what must be true for each option to be safe

### Risks that apply whatever you choose

- **SSRF.** A server that fetches URLs people type must follow the practice the sources agree on (Appendix §A5). That means:
  - checking every resolved address, then dialing that exact address;
  - blocking private, link-local and metadata ranges;
  - no redirects, or each redirect re-checked;
  - timeouts and a size cap.
  Bluesky's `safeFetchWrap` defaults are a 10 s timeout, a 512 kB cap, SSRF protection on and no implicit redirects.
- **A handle pointing at someone else's DID.** Show a handle only when it checks out both ways. The spec: "otherwise anybody could create handle aliases for third-party accounts" (atproto.com/specs/handle). 2026100701 D6 already rules this for display. The User Page must do it for lookups (R3).
- **Using Zurfur as an amplifier.** Every anonymous lookup fans out to other hosts.
  - Rate limits on lookups are Fox's proposal. The corpus precedent is 24543244 F1: per-`app_key` rate limits on every by-DID lookup.
  - The spec's advice to keep auth-flow caches under 10 minutes caps freshness. It doesn't argue for rate limits.

### A. Vulpes does both jobs

Safe only if:
- F49 is reversed;
- vulpes keeps no record of who signed in where. That extends its no-log ruling for disclosures (identity-model.md F-A) to sign-in, and conflicts with the reason a provider exists;
- vulpes is never the only proof of identity, so it still passes the kill test (acp.md "The kill test").

### B. The split (Vulpes proves, Zurfur signs in)

Safe only if:
1. **A new source never creates a User.** It attaches to a User who is signed in with atproto, after a fresh atproto login. Keycloak, Ory Kratos and Auth.js all refuse silent linking *by default*. Each has an opt-in auto-link that it warns about (Appendix §A1).
2. **Never link on a matching email.** This is Fox's proposal. It is stricter than the paper, which allows a merge once "the service must ensure that the user currently controls both accounts" is met (Sudhodanan & Paverd, USENIX Security 2022). Linking on email alone is the pre-hijacking attack.
3. **Key each link by issuer plus a stable subject.** For example Telegram `iss` + `sub`, or Discord's user snowflake. Never by email or display name. Keycloak's CVE-2026-9087 shows why: as Red Hat describes it, its linking proof was "not bound to the upstream identity that was actually verified". (Reported in QA15; Fox hasn't checked the source.)
4. **Removing a link, and account recovery, need atproto.** For recovery, the paper suggests asking which links to keep, with unlinking as the default. That is one of its options, and adopting it would be your call.
5. **One outside account attaches to at most one User,** so the link table can't tie two personas together inside Zurfur (21594113: "not correlated in-product").
6. **The table is private and never answered to outsiders.** A lookup over private links is a correlation oracle. This is by analogy: acp.md "Possible future changes" says it of cross-boundary active checks (Reynard Q3b).
7. **A public `externalAccount` claim is a separate, explicit act by the user.** Publishing a claim is publishing a link (acp.md "Privacy and security concerns").
8. **Each sign-in method has the same verification bar as atproto OAuth today.** Telegram now offers OIDC with PKCE. Its older login widget is an HMAC keyed with SHA-256 of the bot token, and checking freshness (`auth_date`) is optional (Appendix §A6).

### C. A separate identity service

Everything in B, plus:
- the service is not a new place that records every sign-in across products;
- if it ever goes away, nobody loses their identity: no DID is held only by it;
- it has its own security review, key custody and operations, like the minter (26804226).

### D. atproto only

- F1 to F3 are fixed.
- You accept that people without an atproto account can't use Zurfur. Bluesky signup is the on-ramp.

---

## 5. Questions for the Engineer

**Q1. What did "many sources" mean?**
- (a) Ways to sign in as the same User.
- (b) Proof that an outside account (Discord, Telegram, email) belongs to a DID, as Wolven needs.
- (c) Both.
- **Recommendation: (c), handled as two separate things.** Sign-in is Q2 and Q3; proof is Q5. Mixing them is what makes a single "provider" look necessary.
- **Unblocks:** which of Q2 to Q5 matter now.
**Engineer (2026-10-07, in chat):** "Initially, we are only accepting BSKY as a source. However, we may need Google and the like as well. We also quite need to understand that our webpage will have subdomains, so a single identity provider should understand all of them." *Read as:* (a) for now, atproto (Bluesky) sign-in only; more sign-in providers such as Google later. **New requirement: one sign-in must work across all of Zurfur's subdomains.** Q5 (proofs for Wolven) is still open.

**Q2. Can someone without an atproto account become a User?**
- (a) No. atproto sign-in stays required (today's rule, 786439).
- (b) Yes: Zurfur mints a did:plc for them.
  - This reverses 786439.
  - If Zurfur follows the ACP's custodian rules, they bind it. The owner must hold a key of "equal-or-senior priority to the custodian's key" (ccs.md "The senior-key custody rule"), generated client-side (F46). A keyless start is ruled: "A user who has not yet made a cold key starts on the floor, `[vulpes, zurfur]`, and the client nags them up" (ccs.md "Rotation-key layout"). Zurfur dropped `vulpes_recovery` (54427650), so Zurfur's keyless floor would hold operator keys only.
  - A User's DID is their only identifier (57081857 D1). A Zurfur-minted DID would become their permanent key. If they later bring a Bluesky DID, that is a second User, and no merge path is designed: 57081857 lists DID migration as a non-objective.
- (c) Yes, through a PDS signup: Bluesky's, or one Zurfur runs for Users. They then sign in like everyone else. This reverses 29687820's "a User's repo is never Zurfur-hosted" only if Zurfur runs it.
- **Recommendation: (a) for the alpha.** If signup friction shows up later, choose (c) over (b): (c) keeps "every User's DID comes from a PDS", and the person can move.
- **Unblocks:** whether any non-atproto source can ever be primary.
**Engineer (2026-10-07, in chat):** "Without an ATProto? No... We do not mint DIDs either for that as of the MVP. Maybe in the future, but we use the DID for everything, so @:// is needed." *Read as:* (a). An atproto account is required, and Zurfur mints no DID for non-atproto people in the MVP.

**Q3. May Discord, Telegram or email sign someone in as an existing User?**
- (a) No. They are never sign-in methods (this is D).
- (b) Yes, as extra methods under the rules in §4 B 1-8. Not in the alpha.
- (c) Yes, with lighter rules.
- **Recommendation: (b), after the alpha.**
- **Unblocks:** the shape of the sign-in seam (option B), and a second 19431425-style DD for "sign-in methods".
**Engineer (2026-10-07, in chat):** "You attach them to an existing account and then, when signing in with them, you can login because we know the DID." *Read as:* (b). Other sources are attached to an existing atproto-backed User and then work as sign-in methods for that DID. Per Q1, they come after the initial Bluesky-only phase.

**Q4. Should one hardened resolver serve every lookup, and be built now?**
- (a) Yes, before the User Page ships and before the alpha. It covers R1 to R5 and Zurfur's own implementations of the vulpes ports (R7).
- (b) Harden each path on its own.
- **Sub-question 1:** should sign-in also refuse raw `https://` server URLs, which jacquard accepts, and take only handles, as vulpes F29 did? This narrows the input. It doesn't fix F1; the guarded client does.
- **Sub-question 2:** DNS lookup on and the Bluesky fallback off (F4)? Or is a third-party fallback ever acceptable?
- **Recommendation: (a), handles only, DNS on, fallback off.** One enforced path beats one per site (memory `feedback_make_unsoundness_unreachable`).
  - The client: no redirects, checked addresses, a body cap, a timeout on every fetch.
  - Effort note: the off-the-shelf guard crates need reqwest 0.13 (§3b). Until jacquard moves, this is a small DNS filter of our own.
- **Unblocks:** F1, F4, the User Page, BYO-domain verification, and the vulpes `RepoReader`, `DidResolver` and `StatusSource` implementations.

**Engineer (2026-10-08, in chat):** "`a` is the one that makes the most sense, right? It's DRY and it follows good architecture by being more of a plugin than not." *Read as:* **(a)**, one hardened resolver for every lookup, built now. **Sub-question 1: yes**, sign-in accepts handles only. **Sub-question 2: yes**, DNS lookups on and the Bluesky fallback off (both confirmed 2026-10-08). F3 (DID↔handle check) and F5 (https-only redirect) are built in the same work, because they enforce existing rules (the atproto spec plus 19431425; 589826).

**Q5. Where does "this Discord account is this DID" live?**
- (a) A public ACP `identity.externalAccount` claim in the user's repo. Zurfur attests it after its own check, and only when the user asks. This serves the claim half of Wolven #24.
- (b) A private Zurfur link only.
- (c) Both, as separate acts: private for sign-in, public only when the user asks.
- **Recommendation: (c).** It needs these decided, all unruled today:
  - the `externalAccount` payload: the ACP authority's to define, so the Vulpes side (job 1a; acp.md "Claim kinds", Resolution; #24);
  - which DID Zurfur attests as, and how that key is held: Zurfur's, as the first attestor (F49; ROADMAP "Later");
  - the status `list` identifier: "the `list` identifier the reference tooling mints is decided with 'The first attestor'" (FORKS F47).
  (Reynard Q9.)
- **(d) Who attests for Wolven?** Zurfur, Wolven itself, or a hosted vulpes. If Zurfur does, Wolven's identity root is another product, which is the centralization the tech lead's reason (ii) objects to (QA8). No recommendation: this is a cross-product call.
- **(e) Who may run a `(service, id)` → DID index** (the lookup half of #24)? A public index of published claims is fine by Reynard's reading (Q3b). It is still a correlation surface, to weigh against §4 B6 and B7. No recommendation yet.
- **Unblocks:** the claim half of Wolven #24, and Zurfur as "the first attestor" (vulpes ROADMAP "Later"). The lookup half needs (e).

**Engineer (2026-10-08, in chat):** "I vote `c`." *Read as:* **(c)**. A private link exists for sign-in by default; a public, Zurfur-attested claim exists only when the user chooses to publish it. **(d) and (e) are deferred** until attaching sources is built or Wolven needs them; neither was answered.

**Q6. How are the two jobs split between Vulpes and Zurfur?** This is your question about Vulpes (§1).
- **First:** is F49 in force? It is written up only on a local vulpes branch (commit `ebb0256`, no PR), and vulpes `main` still lists "the first attestor" as a vulpes item. If yes, the vulpes lead pushes it and opens a PR.
- (a) Vulpes does job 1a only: public claim and attestation shapes and their checks, which keeps F49 as it is. Zurfur does 1b, 1c and sign-in (option B).
- (b) Vulpes does all of job 1, including the private list of a person's sources. This revisits F49's storage rule and acp.md's "never an ACP record", and the Holder layer's home is already unreconciled (Reynard: ROADMAP "Later" vs F49).
- (c) Vulpes does both jobs (option A). This reverses F49.
- **Recommendation: confirm F49, then (a).** (Whether Zurfur mints through Vulpes is a separate question; see §3, "Out of scope".)
- **Unblocks:** removes option A; settles where the `externalAccount` payload (#24) and the private list live.

**Engineer (2026-10-08, in chat):** "Yes. Vulpes is a LIBRARY ONLY. Given that Vulpes is public only and Zurfur has an index and storage, that means that `a` IS the way to go. Otherwise, Vulpes would require to do more." *Read as:* **the 'pure library' decision of 2026-10-01 is confirmed**, and **(a)**: Vulpes does the public proof format only; Zurfur does the private list, the checking and sign-in.

**Q7. Should the CLI's ruled login reopen?** (Rewritten after QA2. An earlier draft re-asked a ruled fork.)
- **What's ruled.** Jira ZMVP-204 ruled **option C** on 2026-08-24: `login --handle | --did` records which existing User the CLI acts as. There is "no browser, no OAuth, no DPoP in the CLI until the first ticket that writes to a PDS (post-v1)", which re-opens option A. Options A (loopback OAuth) and B (a link code) were retired. The plan was approved 2026-09-10 and is parked.
- **Why ask.** C rested on "v1 is identity-only" (26935298), and 26935298 was superseded in full on 2026-09-12. Accounts and public Characters now get hosted repos, and the Character `owner` half is written through the user's OAuth session (54427650). Does either meet C's re-open trigger, "the CLI writes to a PDS"?
- (a) No. C stands; build the parked plan.
- (b) Yes. Reopen A when the first CLI command writes to a PDS.
- **Recommendation: (a) for now.** No CLI command writes to a PDS yet: nothing in `cli/` or `application/` uses `PublicRecords`. The trigger will likely fire once hosted-repo writes are built, though:
  - they run inside whatever process runs the use case, the CLI included;
  - a CLI command that publicizes a Character couldn't write the User's `owner` half without the user's OAuth session.

  Revisit then. The stale "client-model ruling" text in `session.rs` should be corrected either way.
- **Unblocks:** S2; the security review owed on the CLI trust root (24543244).

**Engineer (2026-10-08, in chat):** "To make it easier, in the Alpha, let's use a browser, since everybody tends to have that anyways. If we can, let's follow it up immediately with a console-only version that uses OAuth using the console rather than a browser. The point is to make it possible for me to use it in a CLI-only environment. Technically, we can have a terminal emulator that we make in the web as part of this tool, but it is mostly just for me for now."
*Read as:* **the August command-line login ruling is reopened.**
- **Alpha:** browser-based OAuth login for the command-line tool (the loopback option).
- **Right after:** a console-only OAuth login with no browser on that machine. Whether Bluesky's servers support the device-code flow is unverified; the fallback is a link code shown on any device and pasted back.
- **Later, optional:** a web terminal emulator.
- **Answered 2026-10-08:** "CLI-to-HTTP is just HTTP-through-CLI. I am fine with that."
  - **User commands go over HTTP:** the terminal tool becomes an API client with its own OAuth session and holds no database password.
  - **Operator commands (`migrate`, maintenance) stay in-process,** next to the database.
  - **Ruled 2026-10-08:** "Terminal tool calls the API's operations." It uses a typed Rust client generated from the Protobuf contract, behind a client-side port, the same shape as the web app's `ZurfurApi`. Business rules stay on the server, and the tool never runs use cases locally for user commands.

**Q8. Should sign-in ask only for `atproto`?**
- (a) Yes. Change the code to match 29687820 D1, and ask for write scopes at first use (D2).
- (b) Amend 29687820 to allow `transition:generic` at sign-in.
- **Recommendation: (a).** The DD and the spec agree (F2).
  - Dropping `transition:generic` doesn't break `me`, which reads profiles without authentication (QA19).
  - Two things have to exist first: the production client metadata (F6), and the first-use scope request (29687820 D2). The second has to be in place before the Character `owner` half ships.
- **Unblocks:** F2, together with F6.

---

## Appendix: prior art

Gathered 2026-10-07 by a research agent that fetched each page and quoted it verbatim. Fox re-fetched the atproto OAuth quotes.

**A1. Identity brokers refuse silent linking by default.** Each has an opt-in auto-link it warns about: Keycloak's AutoLink, "dangerous in a generic environment"; Kratos automatic mode for Google and Apple; Auth.js `allowDangerousEmailAccountLinking`.
- **Keycloak:** "Automatically linking the existing local account to the external identity provider is a potential security hole. You cannot always trust the information you get from the external identity provider." The default first-login flow detects the existing account, then asks the user to confirm by email or by logging in. — https://www.keycloak.org/docs/latest/server_admin/index.html ("First login flow")
- **Ory Kratos:** "Ory doesn't support fully automatic account linking for arbitrary social login providers. This restriction is in place to mitigate several classes of account takeover attacks." By default, "Users must verify their identity with an existing credential before linking." — https://www.ory.com/docs/kratos/social-signin/link-multiple-provider-account
- **Auth.js:** "Automatic account linking on sign in is not secure between arbitrary providers and is disabled by default" (`allowDangerousEmailAccountLinking`). — https://authjs.dev/reference/core/providers

**A2. Takeover through the weakest source.**
- Sudhodanan & Paverd found 35 of 75 services vulnerable to account pre-hijacking. "The root cause of all of the attacks … is failure to verify ownership of the claimed identifier."
- Their fix for merging accounts: "the service must ensure that the user currently controls both accounts."
- Sources: https://arxiv.org/abs/2205.10174 (USENIX Security 2022).
- RFC 9700: a client talking to many authorization servers must defend against mix-up attacks. — https://www.rfc-editor.org/rfc/rfc9700.html

**A3. URL as identity.**
- **IndieAuth** is a living standard, not a W3C Note. Profile URLs "MUST NOT be ipv4 or ipv6 addresses". The returned `me` must be confirmed to declare the same authorization server. — https://indieauth.spec.indieweb.org/
- **RelMeAuth:** "Consolidated identities do not carry inherent trust". The identity is only as strong as the weakest linked site. — https://indieweb.org/RelMeAuth
- **WebFinger** (RFC 7033) is HTTPS-only, and redirects must stay on HTTPS. It warns that queries reveal whether a URI is valid. It says nothing about SSRF. — https://www.rfc-editor.org/rfc/rfc7033.html

**A4. DID resolution.**
- W3C DID Resolution is a Candidate Recommendation Draft (01 October 2026). It prefers local resolvers. — https://www.w3.org/TR/did-resolution/
- The DIF Universal Resolver runs one driver container per DID method. Its public instance is "for testing purposes". — https://github.com/decentralized-identity/universal-resolver
- atproto accepts only hostname-level did:web. — https://atproto.com/specs/did

**A5. atproto OAuth and handles; SSRF.**
- A client must confirm the authorization server is authoritative for the account. If sign-in started from a handle or DID, "it is critical for the client to verify that this DID matches the expected DID bound to the session earlier." — https://atproto.com/specs/oauth
- Sign-in only needs `atproto`. — https://atproto.com/specs/oauth
- "Care must be taken to prevent harmful fetches due to maliciously crafted URLs, including hostnames which resolve to private or internal IP addresses." — https://atproto.com/specs/oauth
- Handles "must be bi-directionally linked to a DID". — https://atproto.com/specs/handle
- **OWASP SSRF cheat sheet:**
  - check every A/AAAA address;
  - bind the connection to the checked address, keeping Host, SNI and certificate checks on the hostname;
  - turn off redirects;
  - block the private, link-local and metadata ranges.

  https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html
- **Bluesky `safeFetchWrap`:** a 512 kB cap, a 10 s timeout, SSRF protection and no implicit redirects. `unicastLookup` refuses any non-unicast address. — https://github.com/bluesky-social/atproto/blob/main/packages/internal/fetch-node/src/safe.ts and …/unicast.ts

**A6. OpenID Federation, Telegram and Discord.**
- **OpenID Federation 1.0** became a Final Specification on 2026-02-17. It builds trust through a "Trust Anchor". It fits governed federations, not consumer sites like Discord, and not atproto, whose trust root is DID resolution. — https://openid.net/openid-federation-1-0-final-specification-approved/
- **Telegram** supports standard OIDC with PKCE (`https://oauth.telegram.org`). The older widget is an HMAC keyed with SHA-256 of the bot token, and checking `auth_date` is optional. Whether its `sub` is global or per-bot is **unverified**. — https://core.telegram.org/widgets/login
- **Discord** is OAuth2: the `identify` scope gives the user's snowflake id. An OIDC discovery document is live at `discord.com/.well-known/openid-configuration` (fetched 2026-10-07). QA found the `openid` scope documented in Discord's Social SDK docs; Fox hasn't checked that. — https://github.com/discord/discord-api-docs

---

## QA challenges

2026-10-07 · Magpie (identity QA). Question asked of every claim: how did you reach this, and shouldn't the Engineer or the tech lead decide it?

**What I checked against:**
- the working copy of `~/code/zurfur`;
- the vendored jacquard 0.12.0 sources: `jacquard-oauth`, `jacquard-identity`, `jacquard-common` and `jacquard`, plus `reqwest` 0.12.28, the version adapter-atproto locks. `jacquard-oauth` 0.12.1 and `jacquard-identity` 0.12.1 have byte-identical `src/`, so the 0.12.0 findings carry over to 0.12.1;
- the corpus on `main` at 177a370;
- vulpes, both the local branches and `origin/main` (aa50433), plus GitHub issues #19 and #24;
- Jira ZMVP-204;
- every Appendix and §3b URL, re-fetched 2026-10-07 by QA sub-agents.

**Verdicts:**
- **holds**: the claim is correct.
- **wrong**: the claim is incorrect.
- **overstated**: the claim goes further than its source.
- **missing**: something is left out that should be in.
- **ask**: the Engineer or the tech lead should decide it, not this doc.

**BLOCKING** marks a problem to fix before the Engineer reads §5.

**Blocking, in short:**
- QA1: F49 isn't merged anywhere.
- QA2: Q7 asks again about a fork that is already ruled.
- QA3: web sign-in is a development-only localhost client, so "Built" overstates it.

---

**QA1. F49 is an unmerged write-up. — BLOCKING · ask**
- **Claim (§1 table, §3, Recommendation, Q5, Q6):** "On 2026-10-01 it was ruled a library…" and "B fits every current ruling". "A undoes a week-old ruling". 1c is "the attestor's work, which F49 leaves to Zurfur".
- **Challenge:** Where does F49 live?
- **Evidence:**
  - Commit `ebb0256` ("[docs] F49 — vulpes is a pure ACP library…", 2026-10-01) is only on the local branch `docs/f49-pure-library`.
  - `git merge-base --is-ancestor ebb0256 origin/main` fails, the branch isn't on the remote, and there is no PR.
  - `origin/main:FORKS.md` has no F49.
  - `origin/main:docs/ROADMAP.md:54` still lists "**The first attestor** — email challenge → sign → deliver" as a vulpes ACP v0.1 item. It is an *email* attestor, not Discord.
  - vulpes #19 (2026-08-24, posted from the Engineer's account) describes "vulpes-the-application … mints, attests on other applications' behalf …, maintains status lists, and may also answer reads … over an API".
  - The quote itself is verbatim on the branch (FORKS.md:43-48).
- **Verdict:** The Engineer's ruling may well be real. But every "since F49" claim rests on text that exists only on one machine, and #19 describes the opposite model.
- **Fix:** Before §5 goes out, the Engineer confirms F49 is in force and Reynard pushes and PRs it. Until then, cite it as "F49 (unmerged write-up of the 2026-10-01 ruling)" and drop "fits every current ruling".

**QA2. Q7 asks again about a ruled fork, and recommends the option that was retired. — BLOCKING · wrong**
- **Claim (Q7, §3 "Common to every option", S2):** "blocked on a 'client-model ruling'", "the parked plan only covers resolving a handle", and the recommendation "(a) the CLI runs atproto OAuth itself".
- **Challenge:** Was this already decided?
- **Evidence:**
  - Jira ZMVP-204 is titled "trusted-driver pick (option C; no OAuth until the CLI writes to a PDS)". Its description says "RULED 2026-08-24 — option C … No browser, no OAuth, no DPoP in the CLI until the first ticket that writes to a PDS (post-v1) — that ticket re-opens option A (public localhost client)". It also says: "Retired by this ruling: the loopback listener, … PKCE/DPoP-in-CLI, the piggyback link code (B)".
  - The plan file's "Rulings (fixed)" section records the same ruling, and the plan was approved 2026-09-10 (memory `project_zmvp204_login_plan_parked`).
  - The CLI is DB-direct: `cli/src/lib.rs:168` calls `Runtime::connect(config)`, and `cli/src/principal.rs:29` fingerprints `database_url`. OAuth would prove a DID to a process that already holds `DATABASE_URL` and the root key. That is the ruling's own reasoning.
  - The `session.rs:2-3, 79` text is stale.
- **Verdict:** Q7 re-opens a decision without saying it exists. Options (a) and (b) are the retired A and B.
- **Fix:** Rewrite Q7 as: "ZMVP-204 ruled C on the premise 'v1 is identity-only (26935298)'. 26935298 was superseded on 2026-09-12, and Accounts and public Characters now get hosted repos. Does that, or the Character `owner` half written through the user's OAuth session (54427650), meet C's re-open trigger?" Drop the CLI from "Common to every option", or say "option C's `login --handle`".

**QA3. S1 "Built" only works for local development. Production sign-in is unbuilt identity work, and it is missing from every option. — BLOCKING · missing**
- **Claim (S1; Q7 (a) "jacquard already builds one (`lib.rs:99-106`)"):** S1 is "Built".
- **Challenge:** Built for which deployment?
- **Evidence:**
  - `adapter-atproto/src/lib.rs:106` calls `AtprotoClientMetadata::new_localhost(...)`, and `:110` sets `keyset: None`.
  - In jacquard-oauth `atproto.rs:306`, `client_id = "http://localhost/?redirect_uri=…&scope=…"`. With no keyset, the token-endpoint auth method is `none`, which is a public client (`atproto.rs:336-389`).
  - Only `backend/config/dev.toml` exists.
  - The spec, re-fetched: the localhost client is "a special exception for the localhost development workflow". "Authorization Servers are encouraged to support this exception - including in production environments - but it is optional". Its redirects are `http://127.0.0.1` and `http://[::1]` only (atproto.com/specs/oauth).
  - 24543244 D1 says the first-party app is "a *confidential* atproto client". 19431425 says "the `client_id` is itself a URL fetched at auth time".
  - ZMVP-204 cites the same spec facts for rejecting the loopback design.
- **Verdict:**
  - The code that Q7 (a) cites is the web server's own development client, not a CLI building block.
  - A production web sign-in needs three things: a hosted client-metadata document, a confidential-client signing key with JWKS (a new secret to keep, as 26804226 does for the minter), and a redirect on the public origin.
  - That is identity work under every option. It belongs in the inventory, in "Common to every option", and in B's effort. It is also tangled with F2, because the scope string is written into today's localhost `client_id`.

**QA4. F1 understates the hole, and "handles only" (Q4) doesn't close it. — overstated · for `/security-review`**
- **Claim (§2.5 F1, Q4):** "Vulpes had closed the input half" with handles only, and the recommendation "(a), with handles only for now … Unblocks: F1".
- **Challenge:** Does accepting only handles stop the server fetching hosts an attacker chooses?
- **Evidence, all in vendored 0.12.0:**
  - **Handles name a host.** `jacquard-identity lib.rs:718` fetches `https://{handle}/.well-known/atproto-did`. Handle syntax rejects `.local`, `.internal`, `.localhost`, `.arpa`, `.onion`, `.alt`, `.example` and `.invalid` (`jacquard-common types.rs:52-64`). Any other domain whose DNS points at a private address passes.
  - **More hosts follow.** After the DID document's PDS comes the authorization server that the PDS metadata names (`resolver.rs:530-557`). Then comes a PAR **POST** to that server's endpoint (`request.rs:590-601`).
  - **Redirects.** reqwest's default redirect policy is `Policy::limited(10)` (`reqwest-0.12.28 redirect.rs:161-163`). `https_only` is false by default, and https→http is followed with only the Referer dropped (`redirect.rs:278, 294`). So the `https://` prefix check is no barrier.
  - **No size cap.** Bodies are read whole: `resp.bytes()` (`jacquard-common http_client.rs:87`).
  - **Mixed timeouts.** The OAuth metadata fetches go through `JacquardResolver`'s `HttpClient` impl, which has a 20 s default timeout (`lib.rs:938-957`; `resolver.rs:297`). The identity fetches (`get_text` and `get_json_bytes`) call the bare client and have none (`lib.rs:462-488`).
  - The jacquard-oauth grep in F1 missed `jacquard-identity`. It has no private-address filter either.
- **Verdict:**
  - The input restriction removes one branch. The guarded client is the fix.
  - The fix needs no redirects, a body cap, and a timeout on every fetch. Those come from the HTTP client, not the input check.
  - Reword F1 to "anyone can make the server issue GETs, and a POST, to hosts they choose".
  - Make Q4's sub-question "should sign-in also reject URL input?", separate from "Unblocks: F1".

**QA5. A silent third-party fallback in handle resolution. — missing · for `/security-review`**
- **Claim (R1, R2, R6, Q4):** "the handle checked back to the DID".
- **Challenge:** Who answers that check?
- **Evidence:**
  - `ResolverOptions::default()` sets `public_fallback_for_handle(true)` (`jacquard-identity resolver.rs:296`).
  - `JacquardResolver::new` sets `dns: None` (`lib.rs:352`), so the DNS TXT step returns nothing (`lib.rs:492-494`), even though the `dns` feature is compiled in (`cargo tree -e features`).
  - When HTTPS well-known fails, the resolver asks `https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle` (`lib.rs:736-760`).
  - Both `OAuthClient::new` (`jacquard-oauth client.rs:87`) and `BasicClient::unauthenticated` (`jacquard client.rs:146-149`, used by `profile.rs:36`) take these defaults.
- **Verdict:** For handles that only DNS TXT verifies, which is common for own-domain handles, two things follow:
  1. Bluesky learns which handles are typed into Zurfur sign-in and profile lookups.
  2. "Checks out both ways" (2026100701 D6; R2 at `profile.rs:81`) becomes "Bluesky's AppView says so".

  The same would hold for BYO-domain verification (R4; 24870914 D2) built on the defaults. Add it to the resolver's requirements under Q4: DNS on, the public fallback off. Whether a third-party fallback is ever acceptable is a question for the Engineer.

**QA6. The "https-only PDS redirect" security item is missing. — missing**
- **Claim (§2, "Common to every option"):** the inventory of sign-in risks.
- **Evidence:**
  - The server's `authorization_endpoint` comes from metadata the attacker controls and is never checked (`jacquard-oauth types/metadata.rs:19`). It is concatenated into the URL (`client.rs:253`) and returned as the 303 `Location` (`session.rs:65`).
  - 589826's security-first list includes "the **https-only PDS redirect**".
- **Verdict:** The severity is low, because OAuth sends the user to their own server anyway. But it is a listed security-first item on this exact path. Add it to "Common to every option".

**QA7. "A provider's list of every login is exactly the link between personas" doesn't hold, and B keeps the same list. — overstated**
- **Claim (§1):** "A provider's list of every login a person has is exactly the link between personas that 21594113 and Vulpes keep private."
- **Evidence:**
  - 21594113:49 defines the privacy model as "cross-persona unlinkability: a person's separate personas are separate Users". Its scope is "*not correlated in-product, not surfaced publicly by default*" (:69).
  - A list of one User's sign-in methods links no two personas unless one outside account attaches to two Users, and §4 B5 forbids that.
  - Option B's "private table of extra sign-in sources" is that same list, held by Zurfur.
- **Verdict:** The argument doesn't tell B apart from a provider. The real differences are who holds the list (Zurfur, or a third party or Vulpes) and whether it spans products. Reword the summary bullet to say that.

**QA8. If Zurfur attests Discord accounts for Wolven, Wolven depends on Zurfur. — ask**
- **Claim (§1 1c; §3 table, "Serves Wolven #24? Partly: Zurfur can be the attestor").**
- **Evidence:**
  - Under F49 (QA1), "the first attestor" moves to Zurfur. Wolven's subjects arrive as `("discord", id)` (#24).
  - #19 expects a facilitator that "attests on other applications' behalf".
- **Verdict:** Using Zurfur as Wolven's attestor makes one product the identity root for another. That is the centralization the tech lead's reason (ii) objects to. Who attests for Wolven (Zurfur, Wolven itself, or a hosted vulpes) is a question for the Engineer, not a "Partly".

**QA9. Wolven #24 asks for a reverse lookup, and no option provides one. — overstated · ask**
- **Claim (§1 "It needs proof…", §3 "Serves Wolven #24?", Q5 (a) "This serves Wolven #24").**
- **Evidence:** #24 asks, "given `(service, id)`, a consumer can obtain the candidate DIDs and *prove* each one". It also wants "a ruling on the lookup pattern: who may operate one, what it serves".
- **Verdict:** A payload schema alone doesn't serve #24.
  - A public `(service, id) → DID` index is also a correlation surface, to weigh against §4 B6 and B7.
  - Add a Q5 sub-question on who may run the index, or say plainly that no option serves #24.
  - "Wolven doesn't need sign-in" holds: #19 and #24 never mention it.

**QA10. The tech lead's lean is paraphrased without a source. — ask**
- **Claim (§1 "I agree with the tech lead's lean"; §3 "(i) gate, (ii) centralize…, (iii) pull sessions…").**
- **Challenge:** Where did the lean come from: which message or file?
- **Evidence:** None is cited. The doc's own rule is "Every claim cites…".
- **Verdict:** Cite it, or have the tech lead confirm the paraphrase.
  - The rebuttal that (i) and (ii) "apply to a hosted Vulpes" is weaker than it reads. The hosted model is still in ccs.md:285, acp.md:656 and #19, and only the unmerged F49 retires it (QA1).
  - The "'Facilitator, never a gate' is Wolven's phrase" attribution: #19 is a vulpes issue posted from the Engineer's account.

**QA11. Q6 assumes Zurfur will mint through Vulpes, and no ruling says so. — ask**
- **Claim (Q6 Recommendation):** "For Zurfur to mint through Vulpes, Vulpes would need three things…"
- **Evidence:**
  - 54427650 amendment (7): "Hosted repos attach after *our* minter mints".
  - Zurfur has its own minter (`adapter-atproto/src/did_minter.rs`; 26804226, built), and no Zurfur `Cargo.toml` depends on vulpes.
  - Per the vulpes fork:
    - "a PDS key role" is unclear, because `MintPolicy` can already declare an `atproto_pds` service (`policy.rs:93-97`);
    - a fourth gap exists, because vulpes `mint(&self, handle: &Handle)` requires a handle (`minter.rs:310-318`) while 57081857 D3 mints Characters handle-less.
- **Verdict:** Whether Zurfur moves minting to Vulpes is its own question for the Engineer. It is off-topic for "provider" and shouldn't sit inside Q6's recommendation. Ask Reynard what "PDS key role" meant.

**QA12. The recommendation answers Q1-Q3 before the Engineer does. — ask**
- **Claim (§1 Recommendation and option B):** "Other sources only attach to an existing User".
- **Verdict:** That rule is the answer to Q2 (a) and Q3 (b). B as written is conditional on Q1 (c), Q2 (a) and Q3 (b). Say so, and present the recommendation as what follows from those answers, not ahead of them.

**QA13. Q2 (b) cites a vulpes Holder rule as binding on Zurfur Users. — overstated**
- **Claim (Q2 (b)):** "The first DID becomes their permanent anchor, with no swap later (identity-model.md Q4)."
- **Evidence:** identity-model.md:267 rules the vulpes Holder model. The file is headed "DRAFT, in design", predates F49, and binds no Zurfur User. The ccs.md floor `[vulpes, zurfur]` assumes a hosted vulpes recovery key, which 54427650 dropped.
- **Verdict:** Cite a Zurfur source (57081857) or drop the bullet.

**QA14. Other comparisons that go too far.**
- **§3 table, "Takeover … A: High exposure: one place merges every source".** Takeover exposure depends on the attach rules (§4 B), which could bind A just as well. B's link table is also one place that merges every source. **overstated.**
- **D's effort, "Low: resolver only".** This contradicts "Common to every option", which includes F2, F3 and the CLI. **overstated.**
- **L6, "no home since F49".** The ROADMAP "Later" section still lists the Holder/aggregation layer. Use Q6 (b)'s own wording: "listed, but its private store conflicts with F49". **overstated.**
- **Q5, "in the reference tooling's format (F47)".** The source says "the `list` identifier the reference tooling mints is decided with 'The first attestor'" (FORKS.md:381-382). **garbled.**

**QA15. Prior art is used beyond what the sources say. — overstated**
- **§4 "amplifier" bullet.** The "<10 minutes" quote ("any caching of identity resolution should choose cache lifetimes carefully. Cache lifetimes of less than 10 minutes are recommended for auth flows specifically") caps freshness. It argues for *more* lookups, not for rate limits. Rate limits need their own basis. **wrong use.**
- **§4 B1, "Keycloak, Ory Kratos and Auth.js all refuse silent linking".** That is true of their defaults only. Each has an opt-in auto-link: Keycloak's AutoLink, documented as "dangerous in a generic environment"; Kratos `account_linking_mode: automatic` for Google and Apple; Auth.js `allowDangerousEmailAccountLinking`.
- **§4 B2, "never link on a matching email".** This is stricter than the paper, which allows merging once control of both accounts is proven (§6.2.2). It is a sound choice, but label it Fox's proposal, not the paper's rule.
- **§4 B4, "Recovery unlinks by default (same paper)".** The paper is about password reset, and it offers retain-by-default as an alternative.
- **§4 B3 is stronger than cited.** Red Hat on CVE-2026-9087: Keycloak's linking proof "is keyed only by (local userId, idpAlias) and is not bound to the upstream identity that was actually verified". Telegram `sub` being stable across bots stays **unverified**: the docs' example shows `sub` ≠ `id`.
- **A5, "must be bi-directionally linked to a DID".** This isn't verbatim. The spec reads: "The link between handle and DID must be confirmed bidirectionally, otherwise anybody could create handle aliases for third-party accounts".
- **A6, Discord OIDC "isn't documented".** Overstated. `discord.com/.well-known/openid-configuration` is live (fetched 2026-10-07), and the `openid` scope is documented in the Social SDK docs.
- **A6, the Telegram widget key.** The HMAC key is SHA-256(bot token), not the token itself. The point stands.

**QA16. §3b: Rauthy's atproto login breaks 29687820 D1 and requires the user's email. — missing**
- **Claim (§3b Rauthy row: "Closest broker"; §3 "C after the build-or-buy survey"; §1 "only Rauthy speaks atproto").**
- **Challenge:** Does Rauthy's atproto login fit Zurfur's identity-only sign-in?
- **Evidence:**
  - Rauthy `src/data/src/entity/atproto.rs:70-73` requests `transition:email`, `atproto` and `transition:generic`.
  - `auth_providers.rs:1110` fails with "No `email` in ID token claims. This is a mandatory claim".
  - Its `auto_link` matches on email taken from the PDS's `getSession`. A PDS that lies about `email_confirmed` could therefore take over an account when `auto_link` is on.
  - The independent audit report never mentions atproto, so atproto was very likely out of scope.
  - It is built on atrium-oauth 0.1.7, whose `todo!()`s at L253 and L300 are reachable from an attacker-run authorization server.
- **Verdict:** Under C, every sign-in through Rauthy would ask for full access plus the email, against D1 and F2. It would also collect PDS emails that Zurfur doesn't hold today. Add this to the Rauthy row, to "C after the survey", and to the summary. It weakens C further.

**QA17. §3b: the suggested SSRF guards don't fit the current stack. — overstated**
- **Claim (§3b "Candidates for the hardened resolver's client"; §3b point 4 "an SSRF guard for the resolver").**
- **Evidence:**
  - reqwest-ssrf-guard 0.2.0 needs `reqwest = "0.13"` (its `Cargo.toml`). http-acl-reqwest 0.13.0 needs reqwest 0.13.4.
  - Zurfur's jacquard stack locks reqwest 0.12.28 (`Cargo.lock`: adapter-atproto and jacquard-common both resolve to `reqwest 0.12.28`), and jacquard-common 0.12.2 still requires ^0.12.
  - The design of both crates is right: they filter at DNS resolution, so the address checked is the one dialed, and they re-check redirects. IP-literal URLs skip the DNS hook and need the URL check as well.
- **Verdict:** "Every piece exists as a crate" holds only after a reqwest-0.13 jacquard, or with Zurfur's own small `reqwest::dns::Resolve` built the same way. State that as an effort item under Q4.

**QA18. §3b: "no tool refuses private addresses" contradicts the doc's own appendix. — overstated**
- **Claim (§1 "No tool … ships a resolver that refuses private addresses").**
- **Evidence:** Appendix A5 cites Bluesky's `safeFetchWrap` with `ssrfProtection = true` by default, and §3b lists two guard crates.
- **Verdict:** The accurate statement is "no Rust atproto resolver refuses private addresses by default".
- **Other §3b points:**
  - Logto's "community connector" isn't Logto's (`@openverifiable/connector-bluesky`, Apache-2.0). It is worse than stated: it generates PKCE and DPoP keys at callback time when no state store is configured.
  - Casdoor's 9.1 score comes from the CNA; NVD lists the CVE as "Deferred".
  - ATLogin's licence is inconsistent: BSD-3 on GitHub, MIT in its README.
  - The other rows hold: licences, AIP, `ssi`, DIF 0.0.1, better-auth, webauthn-rs/SUSE, the openidconnect/oauth2 DPoP status, jacquard 0.12.2 on crates.io, and jacquard's alias check being a warning (`jacquard-identity lib.rs:1043-1048`).

**QA19. Claims that hold, as checked.**
- **F2.** `lib.rs:97`; 29687820 D1 "requests the minimal atproto scope"; both spec quotes verbatim. Dropping `transition:generic` won't break `me`, which reads profiles unauthenticated (`profile.rs:19-38`). Q8 (a) holds, but it needs QA3's client metadata, and D2's first-use request has to exist before the Character `owner` half ships (29687820 amendment, 2026-09-12).
- **F3.** `start_auth` never records the DID (`client.rs:221-246`; `request.rs:611`), and `callback_core` never compares (`client.rs:295-370`). `verify_issuer` ties `sub` to the issuer (`request.rs:733-735`), so "probably low" is reasonable. Whatever the impact, it breaks 19431425's stated contract ("Mandatory ('critical')", :62, :132). It applies only to the handle and DID path. For URL input, the spec's check is the issuer check jacquard does.
- **Inventory rows.**
  - S2: `session.rs:77-80` and 24543244:83.
  - S3 and S4: 589826:157 and 12451841:75.
  - S5: 28049410 D4, D5.
  - S6: 2026093001:405.
  - S7: `adapter-mem lib.rs:829` and the question sheet §5 item 18.
  - I2: 57081857 D2, D3 and 2026100701 D20.
  - I3: 28049410 D2.
  - I5: 589826:103.
  - I6: 29687820 D6, superseded note.
  - I7: 54427650:85.
  - 786439:75 is verbatim. 2026100701 is `status: decided` on corpus `main`, but `docs/design-index.md` doesn't list it yet; run `just design-index`.
- **The memory correction in F1** holds: the start of sign-in is an anonymous choice of host. The memory `project_jacquard_resolver_no_ssrf_controls` should get its "Why" line fixed. That file is outside this doc.
- **The vulpes citations not listed above:** F29, F42/`ports.rs:177-190`, F45, F46, F48, F8, acp.md "Claim kinds", "Privacy and security concerns", "Possible future changes", "The kill test" and "Use with the AT Protocol", ccs.md senior-key and floor quotes, and identity-model.md "The pivot". All verbatim or faithful.

---

## QA resolutions

Fox, 2026-10-07. Every challenge is fixed in place above or moved to a question. Fox re-verified the three blocking items before acting:
- QA1: `git branch --contains ebb0256` lists only the local `docs/f49-pure-library` branch, and `origin/main:FORKS.md` has no F49.
- QA2: the parked plan's "Rulings (fixed)" records ZMVP-204 option C.
- QA3: `lib.rs:106` calls `new_localhost`, and `:110` sets `keyset: None`.

| QA | Verdict accepted? | Resolution |
|---|---|---|
| QA1 (blocking) | Yes | The F49 caveat is in §1 and §3. "Fits every current ruling" became "fits the corpus rulings and F49 as written up". Q6 now opens with "is F49 in force?". The orchestrator is asked to have the vulpes lead push F49 and open a PR. |
| QA2 (blocking) | Yes | Q7 was rewritten as "should ZMVP-204's option C reopen?", with the re-open trigger tested against the 2026-09-12 hosted-repos ruling. S2 and "Common to every option" were corrected. The old (a) loopback recommendation is withdrawn. |
| QA3 (blocking) | Yes | S1 now says "built for local development only". F6 was added. The production confidential client is in "Common to every option" and tied to F2 and Q8. |
| QA4 | Yes | F1 was widened: hosts named by plain handles, the PAR POST, redirects including https to http, no body cap, mixed timeouts. Q4's input restriction is now separate from the F1 fix. |
| QA5 | Yes | Added as F4. It is in the Q4 recommendation (DNS on, fallback off), and whether a third-party fallback is ever acceptable became Q4 sub-question 2. |
| QA6 | Yes | Added as F5 and to "Common to every option". |
| QA7 | Yes | The §1 bullet was reworded: B keeps a list too. The differences are who holds it and whether it spans products. |
| QA8 | Yes | Moved to a question: Q5 (d), who attests for Wolven. |
| QA9 | Yes | "Serves #24" now says "claim half only". "No option serves the lookup half" was added. Who runs the index became Q5 (e). |
| QA10 | Yes | The lean's source is cited (the tech lead's message to Fox, 2026-10-07). The "Wolven's phrase" attribution is corrected to vulpes #19, filed from the Engineer's account. The hosted-vulpes rebuttal now states that the hosted model is still documented. |
| QA11 | Yes | The mint-through-Vulpes gaps moved out of Q6 into §3 "Out of scope", with the fourth gap (a handle-less mint) added. Whether Zurfur mints through Vulpes is noted as a separate question. |
| QA12 | Yes | The recommendation and option B now say they follow from Q1 (c), Q2 (a) and Q3 (b). |
| QA13 | Yes | Q2 (b) now cites 57081857 D1 and its non-objective on DID migration instead of the vulpes Holder draft. The ccs.md rules are framed as "if Zurfur follows the ACP's custodian rules". |
| QA14 | Yes | A's and C's takeover cells now read "depends on the attach rules". D's effort is "the common work only". L6 is reworded. The F47 wording is now verbatim. |
| QA15 | Partly | "<10 minutes" no longer supports rate limits; 24543244 F1 is cited instead. B1 says "by default". B2 and B4 are labelled as proposals. The Keycloak CVE-2026-9087 point is added, marked as not checked by Fox. Discord OIDC and the Telegram HMAC key are corrected. **Not accepted: A5 "not verbatim".** Fox re-fetched atproto.com/specs/handle: "Handles must be bi-directionally linked to a DID: the DID document must claim the handle, and the handle must resolve to the DID identifier" is verbatim. The second quote is too. |
| QA16 | Yes | The Rauthy scopes and email requirement are added to §1, to "C after the survey", and to the §3b row and point 5. Fox confirmed `transition:email` in Rauthy's `atproto.rs`. |
| QA17 | Yes | The reqwest 0.13 requirement is added to the §3b row, point 4 and the Q4 effort note. |
| QA18 | Yes | §1 now says "no Rust atproto resolver refuses private addresses by default". The Logto connector, the Casdoor score source and the ATLogin licence are corrected. |
| QA19 | n/a | These claims hold. The Q8 preconditions from it are added. |

**Outside this file, routed to the orchestrator in Fox's report:**
1. The memory `project_jacquard_resolver_no_ssrf_controls` has a wrong "Why" line: the start of sign-in is an anonymous choice of host.
2. `docs/design-index.md` lacks 2026100701 (`just design-index`).
3. The CLI stub's "client-model ruling" text in `cli/src/commands/session.rs` is stale.
4. F49 needs pushing and a PR in vulpes, once the Engineer confirms it.
5. F1, F3, F4 and F5 go to `/security-review`.

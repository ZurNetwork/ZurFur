# The fork

**How should Zurfur monetize — which revenue model(s), and in what sequence, given its hard gates?**

Known candidate options (argue for a model+sequence; you may add options):

- **A. Donations / sponsorship** — Ko-fi-style donations, Open Source Collective (10% host fee), GitHub Sponsors (0%, 140+ countries). Some require an OSS license; the repo is public with NO license file.
- **B. Grants** — NLnet call open 2026-09-03 → deadline 2026-11-03 (requires FOSS license + "European dimension"); AT Protocol Community Fund (Raft Foundation fiscal host) active.
- **C. Premium tool subscription** — Toyhouse-premium-style paid tier sold via a merchant of record (Paddle / Lemon Squeezy / Polar / Dodo onboard individuals w/o an entity) — but every MoR prohibits adult content, and Zurfur's community (furry art commissions) has substantial NSFW.
- **D. Commission transaction fee** — Stripe Connect **direct charges + application_fee**: charge lands on the artist's connected account; Zurfur receives only a fee, never custody ("tier 2.5", not yet in any DD). Requires a platform Stripe account → entity in a supported country, and NSFW pre-clearance.
- **E. Memberships take (v2.0 roadmap)** — Patreon-like tier-gated subscriptions (User→Account, flat per-user price, patron-priority commissions differentiator); platform could take a cut.
- **F. Plugin marketplace revenue share** — public marketplace is post-v1; plugin acquisition/pricing explicitly deferred by DD (2026-07-04); Golem registration is free.
- **G. Ads** — the furry-art-site norm (FA, Inkbunny, Weasyl, Itaku run on donations/ads).

# Project context (design record, condensed — treat as ground truth about Zurfur)

- **What Zurfur is:** an AT Protocol-native art-commission platform (Rust backend, SvelteKit frontend). The Commission is the first-class object: lifecycle, phases, client-visible progress, invoices, characters, gallery posts as atproto records in the user's own repo. Pre-alpha; the MVP/alpha has not shipped. Target market: independent (largely furry) artists with ADHD/executive-function challenges scaling commissions into income, plus their commissioners.
- **Payments DD (6422530, decided):** pre-alpha = manual mark-as-paid. Tier 1 manual → tier 2 connected invoicing (artist's own processor account, Zurfur only makes invoices, needs entity) → tier 3 platform-in-the-middle (marketplace/escrow, fees possible, heaviest compliance). Hard recorded constraint: **no registered business in any processor-supported country exists**. Each tier up re-triggers the entity question.
- **Invoices DD (30048258, decided):** MVP payments are Invoices only — a commission fact, marked paid manually by the issuer (receiver attests). Zurfur moves nothing, custodies nothing, verifies nothing. Rails/fees/NSFW-processor policy/KYC = future DDs behind a payment port.
- **Project MVP (589826):** v2.0 roadmap = subscription access (Patreon-like) with patron-priority commissions as the differentiator. Public third-party plugin marketplace is post-v1 (API surface ships v1 first-party-only). Analytics are aggregate, never surveillance. NSFW is "blurred, never barred" — adult content is a first-class, labeled part of the platform.
- **Project Philosophy (786450):** born from hatred of centralized deplatforming; "the internet should be free reign"; burden onto the user only what is theirs; keep information decentralized; **"we should win because we are the best, not because we lock our users in."** Credible exit is engineered in (users own their data/DIDs; identity custody has an opt-in user recovery key).
- **Non-Toxic Path (30572545, ruled):** user sanity outranks optimization; any mechanic failing "does this hurt anyone's sanity?" is disqualified, not discounted. Precedents: no punitive decay, no public shaming, no leaderboards, decline is silent, no auto-transitions.
- **Market facts from the Engineer's 2026-08-22 research snapshot** (re-verify anything you rely on): take rates — VGen 5% creator-side; Skeb 9.8% w/ frequent 0% campaigns; Artconomy Shield 6% + $3 escrow; commissions.gg 0% creator / 5% client; Patreon 10%; Ko-fi 5% (Gold $12/mo → 0%). Trend: 0%-creator, client-pays. Price anchors: Toyhouse premium ~$5/mo, ArtTrackr Pro $9/mo, Ko-fi Gold $12/mo. Furry-site norm is donations/ads, not fees. Every MoR bans adult content; Stripe prohibits porn and requires preapproval for "content creation platforms."
- **Operator reality:** solo developer ("the Engineer"), no entity, country/entity question open, project pre-revenue and pre-launch.

# Doctrine floor (binding — an option violating these is disqualified, not discounted)

1. **Anti-domination:** exit and voice preserved by construction; no lock-in as a moat.
2. **The non-toxic path outranks optimization** — any mechanic failing "does this hurt anyone's sanity?" is disqualified.
3. **Zurfur never holds funds absent an explicit ruling** (non-custody is the standing default).
4. **Consensus is not evidence.**

You may challenge the doctrine explicitly (argue it should be amended) — but you may not silently ignore it.

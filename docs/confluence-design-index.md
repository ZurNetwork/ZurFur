# Confluence DESIGN — local page index

Canonical design lives in the **zurnetwork** Confluence, space **DESIGN** — the single source of truth for Zurfur design decisions. This file is a local *pointer index*, NOT a copy of the content: when a topic below sounds familiar, **fetch the real page before asking or asserting from memory.**

## Fetch coordinates

- cloudId: `cafe5eef-9c51-4800-85df-ef42187f9414`
- DESIGN space id: `98310`
- Fetch a page: `getConfluencePage` with the `pageId` below. Web URL: `https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/{id}`
- New DD pages should be created via `/design-decision` (writes the Confluence page + Jira tickets). Index snapshot built 2026-06-30 — re-list with `getPagesInConfluenceSpace` if something's missing and update this file.

## Settled-decision quick facts

Know these without fetching; fetch the linked DD page for detail.

- `did:plc` everywhere · Character has its own DID day one · ratings anchored to commission membership (2-dim Creator/Commissioner, 1–10★ positive-only) · fact-anchored deletion · payments = pre-alpha manual mark-as-paid.

## Page index

### Meta / structure
- `98422` — Design (space homepage)
- `2490388` — Design decision (DD page template/convention)
- `10125333` — The Index
- `786450` — Project Philosophy
- `9895947` — Product
- `589826` — Project MVP (current MVP doc)
- `3670017` — MVP & Roadmap — SUPERSEDED, merged → Project MVP (`589826`); known to be in flux/contradictory, don't treat as ground truth
- `9994307` — Blocking Gaps for v1

### Core entities / glossary
- `786439` — User
- `1966081` — Account
- `2162692` — Roles (Owner/Admin/Manager/Member hierarchy)
- `3276807` — Commission
- `5668866` — Character
- `5931025` — Slots
- `8912899` — Collections
- `9961473` — Batch
- `8978433` — Portfolio
- `8978492` — Post
- `10190849` — Gallery
- `9895957` — Workflow
- `2949165` — Tags
- `3047451` — Plugin
- `3244063` — First-party plugins
- `1933322` — Achievement
- `10453000` — Lens

### Design decisions (DDs)
- `4358151` — DID:PLC vs DID:Web
- `4882433` — Character Ownership Model
- `2490393` — User Reviews & Comments
- `3014657` — Deletion of Commissions
- `6422530` — Payments & Billing Model
- `6848513` — External Chat Tracking (core vs plugin)
- `21594113` — User-Profiles, the Handle Swap & Content Maturity
- `23003138` — Account Deletion, Tombstoning & Handle Reuse
- `23101442` — Notification Service, Fan-out-on-Read & the Seen Cursor
- `23592962` — API Response Shape & Error Model (RFC 9457)
- `24150017` — Transactions as a capability — compile-enforced Unit of Work in the private store
- `24182787` — Collection as a Generic Referenceable Membership Primitive
- `24182820` — Invitation Validity & Issuer Departure
- `24543244` — Auth Surfaces, the Plugin Trust Boundary & CSRF
- `24870914` — The Default Account's Handle
- `19431425` — Authenticators
- `8978453` — Where should Portfolios, Batches, Commissions and Collections live? (in progress)
- `8978501` — Portfolio Live vs Static
- `9863207` — Arrangement — SUPERSEDED, folded into Workflow (`9895957`)

### Data layer / infrastructure
- `9994298` — Where does Data live?
- `10354698` — Data Boundaries
- `9207877` — The Index & Data Boundaries — SUPERSEDED, moved → Data Layer
- `9207856` — Platform Authority
- `10125341` — Blobs, PDS & Private Storage
- `9994275` — Blob
- `10354710` — Lexicon
- `11763713` — Domains and Applications
- `12451841` — Golem

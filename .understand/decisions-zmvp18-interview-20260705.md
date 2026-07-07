# ZMVP-18 pre-build interview — Engineer rulings (2026-07-05)

Nine ticket-deferred decisions ruled by the Engineer in the pre-build interview for the stacked build of epic ZMVP-18 "The Reason (Commissions)". Source of truth for the unit ledger's `decisions{}`. **Posted to Jira 2026-07-05 with the Engineer's explicit approval** (ruling comments 10031–10039, one per ticket; AC corrections applied and verified on ZMVP-31/93/94). Note: the 31 edit normalized its criteria checkboxes to the `* \[ \]` bullet form — the same round-trip artifact already present on previously MCP-edited tickets (57/67/88); content unchanged.

| # | Ticket | Ruling |
|---|--------|--------|
| 1 | ZMVP-31 | Maturity vocabulary = **atproto self-labels** per Maturity Vocabulary DD 29982722 — Safe / Suggestive / Nudity / Adult + orthogonal Graphic; ratings ARE the label values, no mapping layer. The AC's Safe/Questionable/Explicit is superseded pre-DD text. Commissions-only in this epic (Product half → Product epic). **[AC text correction owed]** |
| 2 | ZMVP-93 | **No "Commission Admin" role in v1** — Phase declare/edit/check-off authority is the Owner only. Adding an Admin role later is additive. **[AC text correction owed]** |
| 3 | ZMVP-83 | Role grant/revoke authority = **Owner only** in v1 (no Creator-grants, no Admin tier). Widening later is additive; narrowing would be breaking. |
| 4 | ZMVP-86 | **Delayed = manual** Participant "slipping" flag (explicit act). The system sets **only Late** (deadline passed); a standing Delayed upgrades to Late. No derived Delayed threshold. |
| 5 | ZMVP-68 | **Un-archive exists**: Owner un-archives as an explicit act; archive and un-archive are both changelog entries; facts untouched in both directions. |
| 6 | ZMVP-82 | A declined User **may re-apply** to the same Seat — decline closes the application, not the door. The 10/day quota is the abuse control; no per-seat exclusion machinery. |
| 7 | ZMVP-95 | Billing authority = **Creator-role** Participants (issue, void+reissue, mark paid); mark-payment-sent is the Client-role act. No seated Creator → no invoices (mirrors 94's no-Client rule). |
| 8 | ZMVP-89 | **No UI this epic.** 89 lands as the API contract — upload and Status-set are two explicit, never-coupled calls (upload never mutates Status; negative test) — exercised verbatim in the ZMVP-91 walkthrough. Real form ships with the future commission UI. |
| 9 | ZMVP-94 | v1 = **changelog entry only** (entry shape is the future notification-feed source; delivery ships with ZMVP-100). AC wording "notifies Participants per the standard pipeline" to be corrected. **[AC text correction owed]** |

Interview conducted batched (3 rounds, AskUserQuestion); Engineer took the recommended option on all nine. Scout workflow `wf_0eae4cb5-d62` was in flight during the interview — any additional forks it surfaces go to the Engineer as a second batch at Gate A; these nine are settled and are not to be re-litigated there.

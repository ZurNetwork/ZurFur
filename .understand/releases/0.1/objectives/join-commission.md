# Joining the halves in 0.1: commission

Stage: pre-alpha-v0.1.0 · Lead: Wolf · Under the Engineer's ruling of 2026-10-09 (joined), on Claude's answer: option (b), no character storage.

**In short:** small. Our five PostgreSQL reads and the arms already serve any adapter. Joining adds four packages: three S and one M, 5 units. All of it runs alongside the arms, on den's new check and den's PostgreSQL test setup, and stays off the critical path, with about 2 units of slack. It adds one security sitting.

## 1. New or changed work

| Package | What it is | Size | Depends on | Security review |
|---|---|---|---|---|
| **New: commission reads that bypass the Den move behind it** | Today three GETs skip the Den: the owner's commission list, the changelog read and the file download (the last two check "is a Participant" themselves). The changelog read and the file download stay, since the Den serves no content in 0.1. Before reading, they call den's new check, "may this viewer open the node at this path?" (den's own package, M, after the den module). The check runs on the node each read actually serves: the commission's `changelog` node for the changelog read, and the file's own node for a download. A later view grant or wider level then can't open them through a second copy of the rule (Badger's proposal, agreed). A card doesn't count as opening, and every other case is the Den's one not-found. In 0.1 this matches today's "is a Participant". It merges into the My Den branch before the end-of-slice security review starts, which leaves about 2 units of slack. | M | den's check; commission arms | Yes (authorization, private data) |
| **New: retire the owner's commission list** | Remove the list route and its contract entry. The Den's `commissions` folder replaces it, and nothing calls it: neither the web nor the CLI does (checked; Fennec agrees). The create response keeps its shape. | S | your answer to question 2 | No (it removes a surface); it goes on the end-of-slice review's list |
| **New: our PostgreSQL seeder and end-to-end test** | Den is building a test setup that runs the app on PostgreSQL. In it we seed one commission and one archived commission through today's use cases. bob is added as a Participant with a test-only insert, because no route admits a second Participant yet: invitations can be issued and revoked, never accepted, so on PostgreSQL today the creator is the only Participant. Then we read them back through our arms, as a participant and as a stranger. This proves that rows written by real code come out right; it is our part of the "security pass over PostgreSQL". Sized S because it reuses den's setup and existing use cases. | S | den's PostgreSQL setup; commission arms | Yes (folds into the arms' sitting) |
| **New: a script to fill your Den** | Calls today's commission routes as you, after a real sign-in: two commissions with files and slots, one of them archived. No server code. | S | sign-in on `main` | Light: the script must not log or store your session cookie |
| Changed: commission read ports and fixtures | No new work. Its five reads are now served, not only tested, so Heron reviews the PostgreSQL queries as served. | (L, unchanged) | — | Already yes |

## 2. What existing packages already cover

- **The five PostgreSQL reads and the archived rows:** the participant listing returns archived rows so the hiding check sees them. The by-user index on the participant table also ships, so a listing never scans the table (commission read ports, resized to L).
- **The same arms serve both stores.** They contain nothing tied to one store, and the everyday stack's ports already include PostgreSQL. Wiring the den routes there is den's work.
- **Participants:** creating a commission writes the owner's participant row in the same unit of work (checked in `adapter-pg/src/commission.rs`), so your commissions list for your real DID.
- **Private only:** every commission is born Private, and no route widens it: create takes only a title, a deadline and a maturity. So the two access paths' disagreement on Listed commissions still can't show.
- **The one not-found:** a stranger never reaches our arm, because mounts are matched against the viewer's own set. "Missing" and "not yours" are therefore the same query. The timing re-test stays den's.
- The name rule, the logging rule and the archived fix before the branch is cut: unchanged.

## 3. Risks and questions (each pre-answered)

1. **Where do your real commissions come from?** No web or CLI screen creates one; only today's commission routes do. *Proposed:* the script above. A seed run inside the server would be more code, and it would write around the API's rules.
2. **Retire the owner's commission list instead of rerouting it?** *Proposed: yes.* Nothing calls it, and rerouting would keep a second listing to keep in step with the Den. It is a contract break, which pre-alpha allows at `v1`. If you say no, we rewire it as a list of your own commissions, each passed through den's check (S → M).
3. **Names stored before the stricter rule.** Joining reads your real rows. A stored name that fails the new rule fails the read, and that error sinks the whole listing. No route renames a commission. *Proposed:* run `just db-reset` once before the first joined run, then fill the Den with the script. If you'd rather keep your data, a read-only check that lists failing rows by id is an S package.
4. **The name rule refuses emoji made with a zero-width joiner (👩‍🎨, 🏳️‍🌈) and the zero-width non-joiner that Persian spelling uses.** Both are format characters (category Cf, checked with Python's Unicode data), and your rule of 2026-10-07 refuses every format character. *Proposed:* keep refusing control characters, the bidirectional controls and other invisible format characters, but allow those two joiners. Names are display text, not addresses, so the joiners can't redirect a path. Joining makes this real for your titles.
5. **Workflows:** no route creates one, so your real Accounts' `workflows` folders say "This folder is empty." *Proposed:* accept. The Workflow checks stay on the dev profile; a create route is a write, which is outside 0.1.
6. **Security reviews pile up at the arms** (already a plan risk). *Proposed:* Heron reviews the move behind the Den in the same sitting as the arms, since both use the same access rule.

## 4. Revised objectives

- **Objective 4 grows:** "…and the commission reads outside the Den follow its rules. The owner's list is gone (question 2). The changelog read and the file download ask the Den first." Done when: their route tests match the Den's answers, and Heron's review shows no open must-fix.
- **Objective 5 becomes:** **After a real Bluesky sign-in on `just dev`, My Den shows the commissions you take part in, from PostgreSQL.** Done when: sign in and run the script. `commissions` lists the live commission, and it opens to its four entries with its real files and slots. The archived one appears only with the toggle on, marked "Archived". `cargo test -p adapter-pg` runs the five read tests, and the PostgreSQL end-to-end test passes (its command is named in its PR).
- Objectives 1 to 3 are unchanged on the dev profile. The archived seed, "Old sketch", is still wanted for the golden responses and bob's half.
- "Not this release" loses the bypassing-reads bullet.

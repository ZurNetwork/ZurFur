# Joining the halves in 0.1: den

Lead: Badger · 2026-10-10 · Under the Engineer's ruling of 2026-10-09 (joined), on Claude's answer: option (b), without Character storage. The seams below were agreed on the board with Fox, Wolf and Fennec.

**In short:** den adds four packages: one M and three S, 5 units in all. It also changes two. Everything runs beside the arms with about 2 units of slack, so den adds nothing to the critical path. The end-of-slice security review grows, and Heron sizes that. One question for you: whether equal not-found timing on PostgreSQL becomes a gate.

## 1. New or changed work

| Package | What it is | Size | Depends on | Lands in | Security review |
| --- | --- | --- | --- | --- | --- |
| **New: den's access check** | One check that the reads bypassing the Den will call: "may this viewer open the node at this real path?" It runs Atta's own rules and answers "opens" or the one not-found. A card doesn't count as opening, and real paths stay on the server. It checks the node each read serves, for example the `changelog` node for the changelog read. Identity and commission move their reads onto it and size that work themselves. Wren, units 5 to 7. | M | Atta's read side; the den module | its own slice, before the identity and commission rewiring | Yes |
| **New: the PostgreSQL test setup** | No API test reads data from PostgreSQL today: the shared test server (`test_support::http::serve`) boots only the in-memory runtime. This setup serves the app on the PostgreSQL stores, signed in through the in-process sign-in. Identity and commission seed their own entities in it. Mole, from unit 3. | S | nothing new | its own slice | No (test code only) |
| **New: the den route tests on PostgreSQL** | The den route suite runs on both setups: your root, a membership mount, a typed archived path, the one `404` for every cause, bob's DID as a path piece, `%2F` and `%252F`. | S | the den API routes; both teams' arms and seeders | its own slice | Yes |
| **Moved in from 0.2: the not-found timing re-test** | For each pair of not-found causes at the same depth (absent, and present but not mine), a test checks that both run the same database statements. Times are measured and handed to the review. | S, or M if a pair differs | the PostgreSQL route tests; identity's bounded root-name lookup | its own slice, before the review | Yes |
| **Changed: the den API routes; den.proto into the contract** | The routes are mounted on every stack. The dev-only mount and its test go, and the route pin lists the two den routes like any other. | M and S, unchanged | as planned | as planned | Yes, as planned |

**What the dev-profile split no longer needs:** the den routes' dev-only mount, its test and the pin's dev-only marking. Also gone: den's request that `just gate` and CI build the dev-profile feature for den code. The dev profile's own guard test still needs that build (the tech lead's). The web's switch that hides My Den goes too (Fennec's). Den doesn't depend on the dev profile at all: golden responses are built in test code. Keeping the dev profile for the fixture walk-through (bob, characters, an archived commission) is the tech lead's call.

## 2. Already covered by "both adapters, no stubs"
- Every new read already has a PostgreSQL version and tests, from identity's and commission's ports-and-fixtures packages. Commission also adds the index on its participant table.
- Atta, the den module and both teams' arms reach storage only through ports, so they serve PostgreSQL unchanged.
- The empty `characters` folder on PostgreSQL is the keeper read's named exemption: a doc comment, a pinning test, and a criterion on the Character slice.
- Stored names that fail the Den's name rules get objective 2's test and the fallback word "Unnamed" (question 5 of `den.md`). Joining makes that path live on real data. It adds no work.
- A Den read never holds a connection for its whole run: a Query use case can't open a unit of work (`application-macros/src/use_case.rs`). That fits the 5-connection pool and the web's limit of 4 reads in flight.

## 3. Risks and questions
1. **Question: should equal timing be a gate on PostgreSQL?** The design document says it isn't a gate yet. *Proposed:* yes. The gate is that both causes in each pair run the same statements, which is deterministic. Wall-clock times are measured for Heron, not gated, because they are too noisy for CI.
2. **When the reads that bypass the Den are due.** The tech lead set "before the Den first reads PostgreSQL". *Proposed reading, for the tech lead to confirm:* the check and both teams' rewiring merge into `feature/my-den` before the end-of-slice security review starts. The routes can then be mounted everywhere from their first slice, and Heron reviews a single access path. Under the literal reading, the routes stay dev-only until the rewiring lands, and the web's switch lives on until then.
3. **The check is one access path that three teams' reads share,** so a mistake in it leaks through every caller. *Mitigation:* it is Atta's own resolve, not a second copy of the rule. Each caller gets a test that a node the Den refuses is that read's usual `404`. Heron reviews it with the arms.
4. **Your root's name comes from your PDS on every answer.** A cold cache or an outage could stall reads. Identity bounds the lookup, runs one per DID at a time, and falls back to the DID, so no error reaches Atta. The timing test requires the lookup to be the same for every not-found cause.
5. **The test setup is built before `feature/my-den` takes the hardened sign-in.** That restack may change the sign-in seam the setup drives. *Proposed:* drive only the sign-in port, and accept a small fix at the restack.
6. **Whole-folder reads.** Atta reads a folder whole before it filters, and a page token is an offset, so a write between two pages can repeat or skip an entry. *Proposed:* accept this for 0.1, which holds one person's data, and bound folder size with the first public listing.

## 4. Revised objectives
- **3, "Every miss looks the same":** the not-found tests run on both setups, and the timing test passes on PostgreSQL.
- **4, den.proto:** the route pin lists the two den routes like any other, with no dev-profile marking.
- **New, "One rule decides who opens what":** the Accounts list, `/me`, the changelog read and the file download each call den's check, and a test for each read shows that a node the Den refuses gets that read's usual `404`. It lands on `feature/my-den` before the end-of-slice security review. It serves den's check and the identity and commission follow-up.
- **Not this release:** drop "the Den on PostgreSQL and the everyday stack (0.2)". The timing re-test moves into 0.1.

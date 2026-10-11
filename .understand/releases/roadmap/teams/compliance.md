# Roadmap: compliance

Lead: Tortoise · Reviewers Pangolin, Armadillo · QA Avocet (team charter pending in PR #252) · Stages covered: pre-alpha, alpha, beta, release

> **This is a map for the Engineer. It is not legal advice.** Each stage below lists what the law and other binding rules require before that stage's audience may use Zurfur. **[lawyer]** marks a point a real lawyer must answer before anyone relies on it. Legal claims cite a source from the list at the end, with a marker:
> - **[read]**: the primary text (statute, regulation, court ruling or regulator page) was read on 2026-10-10, by me or by a research agent I sent;
> - **[excerpt]**: seen only in a search result or a secondary source;
> - **[unverified]**: no source was found.
>
> Many duties depend on where Zurfur's operator is based, and that is still open: the Payments & Billing decision (6422530) records "no entity", and no country is chosen. Where the answer changes with the country, I say so.

## In one screen

- **Pre-alpha:** the law asks almost nothing while only you can reach a build, but most alpha duties take weeks to arrange or fix the shape of stored data. So pre-alpha settles ten questions (Q1–Q10: the operator, a lawyer, age checks, minimum age, content limits, the license), drafts the legal pages and privacy records, and starts the registrations.
- **Alpha is the big line.** Before A1, or before any earlier build that someone other than you can reach: the Terms, Privacy, Content and Copyright policies live and accepted; an age check that meets the law wherever adult art shows; the CSAM scan gate with its route to NCMEC; one report path that runs every legal clock (copyright, intimate images in 48 hours, illegal content); a registered DMCA agent; privacy requests and breaches handled (by hand is fine); no cookie that needs consent; license notices for what browsers download.
- **Finding 1, the age gate.** The ruled default, Blurred for everyone with an "I'm 18+" click, is self-declaration. UK guidance, Brazilian and French law and the US-state laws say it isn't enough, and Australia's and Germany's adults-only rules need more than a click [S3, S6, S14–S16, S23, S24, S26, S27].
- **Finding 2, no company and no country.** CSAM vendors, NCMEC, processors and counsel wait on it, and UK law treats you personally as "the provider" [S8].
- **Finding 3, AI tools.** Real users' data must not reach an AI tool on consumer terms, and CSAM evidence must reach no agent at all [S55].
- **Beta:** re-measure the size and reach tests as the group grows; plugins and public posts add a developer agreement, third-party data flows and public-record notices; self-serve export and deletion replace the manual process.
- **Release:** subscriptions bring a company, a processor that accepts adult content, VAT (in the EU Zurfur may count as the seller), US sales tax, consumer-contract rules and platform tax reporting. Open sign-up meets age-check law at full scale.

## How to read the slots

Compliance builds no product slot of its own. Each duty lands on a team's card and must be met by a slot:
- **"By A1"** means before the first alpha build opens to its closed group.
- **"With <card>"** means in the slot that ships that card, wherever the consolidator puts it.

**One rule moves every "by A1" duty earlier.** Your ruling on moderation's deadline (D-26) is that the deadline is "the first build anyone else can reach", not a stage name. I apply the same rule to compliance: every "by A1" duty is due before any pre-alpha build that someone other than you can reach. Pre-alpha has no sign-in allowlist, so any Bluesky user could sign in to a deployed build (moderation's 0.1 QA).

## Pre-alpha

- **P2: the forks that everything else waits on are ruled, and a lawyer is engaged.**
  - Features: proposal "legal footing" (no card).
  - Depends on: none.
  - Done when:
    - the rulings file carries your answers to Q1–Q10;
    - a lawyer's name and scope are recorded;
    - the operator's legal name and postal address exist, ready to go on the legal pages and the registrations.
- **P3: drafts, records and registrations that have lead time.**
  - Features: proposals "legal pages", "privacy operations" and "age assurance"; security-asap (its CSAM scanning and report path items).
  - Depends on: moderation (report categories and the CSAM procedure); the tech lead (the hosting and processor list); identity (what the User record stores about terms and age).
  - Done when:
    1. Counsel holds drafts of the Terms, Privacy Policy, Content Policy and Copyright Policy.
    2. These exist where the tech lead files them, each dated:
       - the record of processing;
       - the data-protection impact assessment;
       - the retention schedule;
       - the breach runbook;
       - the runbook for access, erasure and export requests.
    3. NCMEC has answered the registration email for online service providers. Registration is by email to its ESP team [S30].
    4. A CSAM-scanning vendor and an age-assurance vendor are vetting or have accepted the application. PhotoDNA, for example, is "subject to approval by third-party vetting service" (moderation study).
    5. The DMCA agent is designated, once the operator's name is fixed [S33].
- **P3, alongside: compliance reviews the stored data that hardens at alpha.**
  - Features: none of its own; it reviews other teams' designs.
  - Depends on: identity, moderation, commission and posts naming the fields below.
  - Done when each owning team's design either names the field or says why it isn't needed, and compliance has signed it off:
    1. The User record holds the Terms version accepted, its date, and the age-check result. It holds the result only, never a document or an image.
    2. The report record holds the category, the legal clock, the reporter's details and a tracking number.
    3. The moderation action record holds the Terms clause, the Terms version and the statement of reasons sent.
    4. How long ratings are kept, and what erasure does to them.
    5. What goes into a PLC operation, the permanent public log of Zurfur-minted identities.
    6. Who reads the bug reports.
- **P4 (the last pre-alpha slot): the alpha gate audit.**
  - Features: compliance's end-of-release review (charter, PR #252).
  - Depends on: every row of the A1 gate below.
  - Done when: `.understand/releases/<version>/compliance.md` lists every A1 row as met with its evidence, or as accepted by you in writing.

**Exit:** every A1 row is met or accepted in writing. No pre-alpha build that anyone but you can reach is missing an A1 row.

## Alpha

- **A1: Zurfur is lawful to open to a closed group of real people.**
  - Features:
    - proposals "legal pages", "privacy operations", "age assurance" and "agents and real data";
    - the cards security-asap, maturity-ui, deployment, frontend and login.
  - Depends on: see "Cross-team dependencies".
  - Done when the gate table below is all met or accepted in writing, and you can do each of these on the real domain:
    1. Signed out, open the Terms, Privacy, Content, Copyright and Contact pages from any page's footer. Each shows a version and a date.
    2. Signed out, find every Adult-labeled work shown the way your Q3 ruling says (under Claude's answer, a label with no image).
    3. File one report of each category and get a reference number.
    4. As a new user, be asked to accept the Terms and to pass the age check you ruled for.

### The A1 gate

| # | Duty | Required by | Lands on (team · card) | Lawyer |
|---|---|---|---|---|
| 1 | **The operator is named** on every legal page: legal name, postal address and contact points. | The DSA's points of contact and legal representative (Arts 11–13) [S36]; the DMCA agent filing needs the provider's legal name and street address [S33]; a sole individual with no company counts as "the provider" under UK law [S8]. | the Engineer | yes |
| 2 | **Terms of Service**, covering:<br>• rules with stable clause numbers;<br>• how moderation works, including automated tools and human review;<br>• termination of repeat copyright infringers;<br>• the minimum age;<br>• the license users grant Zurfur to store and show their work and to publish it to the atproto network;<br>• that Zurfur is not a party to the artist–client contract. | DSA Art 14 [S36]; 17 U.S.C. § 512(i)(1)(A) [S33]; the UK Online Safety Act's terms duties (s.10) [S2]; P2B Arts 3–5, if it applies [S39]; Bluesky's developer guidelines [S71] | web (pages), moderation (clause numbers), the Engineer with counsel (text) · proposal legal pages (Inventory Closures sliver 4, which has no card) | yes |
| 3 | **Privacy Policy**, covering:<br>• everything GDPR Arts 13 and 14 require;<br>• every processor: hosting, error tracking, traffic analytics, the CSAM vendor, the age-check vendor, AI tools;<br>• the public network: PDS repos, relays, and the PLC log, which is never erased;<br>• retention;<br>• rights, and how to use them;<br>• the EU and UK representatives;<br>• a UK complaints route. | GDPR Arts 13, 14, 27 [S41]; California's privacy-policy law (CalOPPA) if Zurfur counts as "commercial" [S51]; the UK complaints duty in force since 19 June 2026 [S47]; the moderation study's risk 3 (private files reach the vendor) | same as row 2 | yes |
| 4 | **Content Policy**, covering:<br>• the maturity rules;<br>• banned content: CSAM, including drawn sexual depictions of minors (Q6); non-consensual intimate images; sexual content of real people (Q5). | UK Coroners and Justice Act ss.62, 65 (covers "an imaginary child") [S12]; 18 U.S.C. § 1466A (drawings) [S21]; Canada's Criminal Code s.163.1 [S31]; the TAKE IT DOWN Act [S32] | moderation, the Engineer · proposal legal pages | yes |
| 5 | **Each user accepts the Terms, and the version is recorded.** On a change, users accept again, and EU artists get 15 days' notice. | P2B Art 3(2) [S39]; moderation actions cite a clause and a Terms version (moderation study) | identity (User record), web (screen) · login | |
| 6 | **The age check follows your ruling on Q3 and Q4.** No self-declaration wherever adult art is shown, including Adult-rated commission files. | UK Online Safety Act s.12(4) and (6): age verification or estimation that is "highly effective" [S3]; Ofcom: self-declaration is "not capable of being highly effective" [S6, S7]; *Free Speech Coalition v. Paxton* and the state laws [S13–S17]; France [S23]; Germany [S24]; Australia [S26]; Brazil [S27] | identity (age status); web and posts (the gate's screens and public views); commission and den (Adult-rated commission files); deployment (only if geo-restriction is chosen) · maturity-ui, proposal age assurance | yes |
| 7 | **UK paperwork, if UK users are admitted:**<br>• an illegal-content risk assessment;<br>• a children's access assessment;<br>• a children's risk assessment, unless an age check makes the service adults-only;<br>• the written records;<br>• a named person responsible for compliance. | Online Safety Act ss.7, 9, 23, 35–37 [S2, S4]; Ofcom's Q&A [S8] | compliance (drafts), the Engineer (the named person) | yes |
| 8 | **CSAM**, covering:<br>• the scan runs before any bytes can be served;<br>• NCMEC registration;<br>• a written reporting procedure;<br>• one year of preservation in a locked store that no agent can read. | Inventory Closures items 7 and 13 [I2]; 18 U.S.C. § 2258A(a) and (h) [S29]; NCMEC [S30]; UK s.66 (reports already made to NCMEC count) [S10] | moderation · security-asap (CSAM scanning); the Engineer (registration) | yes |
| 9 | **One report path that runs every legal clock:**<br>• CSAM;<br>• non-consensual intimate images: removal within 48 hours, a tracking number, and an outcome notice;<br>• copyright: a § 512(c)(3) notice and a § 512(g) counter-notice;<br>• illegal content: the DSA Art 16 fields and a decision notice;<br>• threats to life (DSA Art 18);<br>• a monitored contact address, and records of every report. | TAKE IT DOWN Act, enforced by the FTC since 19 May 2026 [S32]; § 512 [S33]; DSA Arts 16–18 [S36]; Online Safety Act ss.20, 21 [S2]; Bluesky's developer guidelines [S71] | moderation (records and clocks), web (the form) · security-asap (report path) | yes (which ones apply) |
| 10 | **A statement of reasons** goes to the person whose content or account is restricted. On the CSAM path it waits until counsel answers (moderation's question 5). | DSA Art 17, which has no exemption for micro enterprises [S36]; P2B Art 4 [S39]; § 512(g) [S33] | moderation (Moderator Actions) | yes |
| 11 | **The DMCA agent** is registered with the US Copyright Office and shown on the site. The registration lapses after 3 years, so renewal is tracked. | 37 CFR 201.38 [S33] | the Engineer; web shows it | yes (non-US operator) |
| 12 | **Privacy operations:**<br>• a record of processing;<br>• the impact assessment, signed before real data flows;<br>• a lawful basis for each purpose, including a basis for special-category data, because adult commissions can reveal a person's sex life or orientation;<br>• a retention schedule;<br>• access, erasure and export within one month, as named operator actions, never hand-written SQL;<br>• a breach runbook: 72 hours to every EU authority whose residents are affected, plus the US state notices;<br>• processor contracts;<br>• EU and UK representatives. | GDPR Arts 3(2), 9, 12(3), 27, 28, 30, 33, 35 [S41]; the *OT* ruling (C-184/20) [S43]; EDPB guidance [S44]; your ruling that nobody edits the database by hand [I10] | compliance (drafts); the Engineer (signs, appoints); identity, commission and den (named export and erase actions); the tech lead (processor list) · proposal privacy operations | yes |
| 13 | **No cookie or tracker that needs consent:**<br>• the session cookie only;<br>• traffic analytics within the French regulator's (CNIL) exemption and the UK analytics exemption, with an opt-out link;<br>• the error tracker with no session replay and no browser storage. | ePrivacy Directive Art 5(3); EDPB Guidelines 2/2023; CNIL; UK PECR Sch A1 [S50] | web, deployment · frontend, deployment | yes (error tracking in the EU is unsettled) |
| 14 | **Real users' data stays off consumer-plan AI tools, and CSAM evidence reaches no agent.** | On consumer plans, Anthropic trains on the data when the user's setting allows it; its Commercial Terms include a data processing agreement and bar training [S55]; GDPR Art 28 [S41]; § 2258A(h) (secure storage, limited access) [S29] | internal (agent tooling), moderation (evidence store), the Engineer (the plan) · proposal agents and real data | |
| 15 | **License notices** for every file the browser downloads (scripts, fonts, icons), and an npm license gate like the Rust one. | MIT, BSD, Apache-2.0 and the SIL Open Font License (self-hosting a web font counts as distribution) [S68]; cargo-deny already gates the Rust side [S69] | web (a notices page), internal (CI) · frontend | |
| 16 | **Bluesky's developer guidelines:**<br>• a way to report;<br>• blocking;<br>• a way to delete content;<br>• a monitored contact email;<br>• records of reports. | [S71] | moderation, identity, web · blocks, security-asap | |
| 17 | **Accessibility:** keep the WCAG 2.2 AA gate, and publish an accessibility statement with a contact address. | The European Accessibility Act exempts microenterprises that provide services [S63]; the ADA has no regulation for private websites [S64]; the UK Equality Act's duty is anticipatory [S65] | web · frontend | |

### Alpha cards with a compliance check (each due with the slot that ships the card)

- **With user-pages.** Zurfur shows pages for people who never signed up, so they get a GDPR Art 14 notice and a way to opt out. The "disproportionate effort" exemption "should not be routinely relied upon" [S41, S44]. [lawyer]
- **With did-ops, account-pds, character-presence and handle-change.** Data written to the PLC log and published to relays can't be called back:
  - The Account Deletion decision (23003138) accepted an Account's handle history living in the PLC log forever [I4]. That holds for a studio name, but not if a handle is a person's name.
  - EDPB guidance on immutable ledgers "strongly discourage[s]" plain personal data on them, and says "technical impossibility cannot be invoked" to excuse non-compliance [S45].
  - Art 17(2) asks for reasonable steps to tell other services when a person erases data they made public [S41].
  - The impact assessment covers this before the first real PLC submission. [lawyer]
- **With reputation.**
  - **Privacy.** Ratings are scoring of people (a criterion for an impact assessment) [S44]. "Stored forever" (User Reviews, 2490393) needs a lawful basis, and a rater's erasure request should anonymize the rating [I5, S41].
  - **Review rules.** Once Zurfur is a trader, the UK and EU rules on publishing reviews apply: say how ratings are checked (each is anchored to a completed commission), and never hide negatives [S40].
  - **Threats.** The US rule against threats that suppress reviews binds anyone [S40].
  - **Defamation.** Ratings carry no free text, which keeps defamation risk low. [lawyer]
- **With invoices.**
  - **No financial licensing.** Zurfur moves no money, so it is not a money transmitter, not a payment service under PSD2, and not a settlement organization for Form 1099-K [S61, S62].
  - **Labelling.** An invoice's legal content is the seller's duty, and so are e-invoicing mandates [S58, S60]. So the document must not pose as Zurfur's or as a VAT invoice (Q7).
  - **Platform tax reporting.**
    - The EU reporting directive (DAC7) covers payments whose amount is "known or reasonably knowable" to the operator [S56]. HMRC's guidance counts a platform that shows the agreed amount [S57].
    - That reporting binds an operator that is an entity (a company or other legal arrangement). The UK excludes sole traders outright [S56, S57].
    - So it is dormant while there is no company, and wakes when one forms. [lawyer]
- **With ask-for-art.** It connects artists with buyers. If an EU artist acts in a professional capacity and Zurfur counts as an information-society service, the EU's platform-to-business rules (P2B) need:
  - plain-language terms;
  - 15 days' notice of changes;
  - reasons for a restriction;
  - 30 days' notice before ending a business user's account [S39].

  The European Commission has proposed repealing P2B [S39, excerpt]. [lawyer]
- **With blocks and friendship.** Both are public records in user repos, so the Privacy Policy says so. Blocks also meet Bluesky's guideline [S71].
- **With user-linking.** It is opt-in already. Record the consent, and erase the link when it ends.
- **With fact-deletion.** The retention schedule is implemented. A legal hold beats deletion (moderation). The exceptions to erasure are written down [S41].
- **With markup, changelog and claims.** Comments and notes are user text, so the report path covers them. A redaction made under a legal duty is recorded (moderation's question 6).
- **With gallery-view and portfolios.** Public views apply the age gate. In an age-check case, the EU Court of Justice held on 16 June 2026 that a provider which ranks content "by means of an algorithm" exercises control and "is not exempted from liability" [S23]. [lawyer]
- **With deployment.** Before A1, the processor list and the Privacy Policy record:
  - the hosting region, and any transfers of data out of the EU and UK;
  - how long logs and backups are kept;
  - log redaction (already on the deployment checklist);
  - any CDN or DDoS-protection provider.

**Exit:**
1. One request drill (access, then erasure) and one breach drill are done and recorded.
2. The UK "significant number" test and the US one-third test are measured on alpha data.
3. Every beta gate row has an owner.

## Beta

- **B1: the bigger group still passes the size and reach tests.**
  - Features: proposal "re-measure" (compliance's end-of-release review).
  - Depends on: moderation and posts for counts by rating; the tech lead for users by country.
  - Done when the compliance report gives the measured value for each test:
    - **UK.** A "significant number" of UK users. The term is undefined, and no service can apply for an exemption [S1, S8].
    - **US adult-content laws:**
      - each state's share of the site that is adult material (one-third in Texas, Louisiana, Arizona and Missouri) [S14, S16];
      - Tennessee also covers individuals, counts a "fictitious character's body", and makes a violation a felony [S15].
    - **US social-media laws.** Mississippi and Tennessee require age checks for every account holder and have no size threshold. Bluesky blocked Mississippi, then let in adults who pass age assurance [S18, S19].
    - **EU.** Whether Zurfur is a paid service ("normally provided for remuneration") and still a micro enterprise [S37, S38].
    - **US privacy.** Texas bars even small businesses from selling sensitive data, including data on "sexuality", without consent [S52].
- **With gallery-posts.**
  - The Privacy Policy covers public `feed.post` records. Their maturity labels travel with them (Maturity Vocabulary, decision 5).
  - The EU copyright-upload rule (DSM Art 17) applies only to a service run "for profit-making purposes" [S34].
  - Recheck the ranking ruling above. [lawyer]
- **With plugin-api.**
  - A plugin developer agreement.
  - The Privacy Policy discloses which data plugins receive.
  - Whether a plugin is a processor or an independent controller [S41]. [lawyer]
  - The report path covers plugins.
  - P2B covers EU plugin developers who act professionally [S39].
- **With hash-chain.** A tamper-evident chain versus erasure. "Scrub the content, keep the hash" (the Eventlog decision) matches EDPB advice to keep only a commitment, not the data, in the permanent record [S45]. The impact assessment is updated.
- **With seals and achievements.** Zurfur-minted labels are statements about people, so the GDPR's accuracy and rectification rights apply [S41].
- **With commission-comments and replies.** More user text, so the report path covers it.
- **With kafka.** The record of processing and the processor list are updated.
- **Proposal: self-serve export and deletion.** Manual handling won't scale to the beta group (GDPR one-month clock, Art 12(3) [S41]).
- **Proposal: age assurance at scale.** If the alpha used an interim posture (Q3 option c), the vendor check replaces it here.

**Exit:** every release gate row has an owner; Zurfur has a company (Q1), since subscriptions can't open without one.

## Release

- **R1: Zurfur opens to everyone.**
  - Features: open sign-up; age assurance everywhere adult art is shown; geo-blocks for the countries Zurfur chooses not to serve.
  - Depends on: identity and web (sign-up); deployment (geo).
  - Done when:
    - each country Zurfur serves has a row in the compliance report: the age-check law, the minimum-age law, and the reporting route;
    - each country it doesn't serve is blocked, and the block is tested.
  - Laws to cover:
    - **Australia.** If Zurfur counts as social media (its sole or significant purpose is social interaction), the under-16 ban applies with no size threshold. A service is out only if none of its material is reachable in Australia [S26]. [lawyer]
    - **Brazil.** Adult content needs an age check at every access, wherever the provider is based [S27]. [lawyer]
    - **UK.** Under-16 restrictions are coming (s.214A) [S11].
    - **US.** COPPA stays out of reach with an 18+ rule and no actual knowledge of a child, but an age check that reveals a child creates that knowledge [S20].
- **With subscriptions.** Zurfur takes or routes money:
  - **A company,** and a processor that accepts adult content. Internal research of 2026-08-22 found the merchant-of-record services refuse it; check again [excerpt].
  - **EU VAT.** Zurfur is presumed the seller of electronically supplied services through its portal when it authorises delivery or sets the terms (Art 9a), so it registers for the EU's One-Stop Shop (OSS) [S58]. Commissioned art itself is likely not an electronically supplied service [S59].
  - **UK VAT.** A non-UK business must register from its first UK sale [S59].
  - **US sales tax.** Marketplace-facilitator tests differ by state: California needs no payment handling, while New York and Texas do [S61].
  - **Platform tax reporting.** DAC7 applies once a company exists, and the UK rules only if that company is UK-based [S56, S57]. Form 1099-K applies if Zurfur settles payments [S61].
  - **Review rules** bind Zurfur fully once it is a trader [S40].
  - **Consumer law** (cancellation rights for digital content, auto-renewal and cancellation rules) [unverified: not researched]. [lawyer]
- **With marketplace.**
  - A developer agreement.
  - Trader traceability (DSA Art 30), once Zurfur is no longer a micro enterprise [S36].
  - P2B's ranking parameters (Art 5) [S39].
  - EU VAT on paid plugins (Art 9a) [S58].
- **With openings-feed and lens.**
  - Disclose the main ranking parameters (P2B Art 5) [S39].
  - The DSA's recommender-transparency duty (Art 27) exempts micro enterprises [S36].
  - Recheck the ranking ruling [S23].
- **With disputes.** When Zurfur decides between users, it needs terms, reasons and an appeal. Bring counsel in before design. [lawyer]
- **With notifications.** Email rules differ for service messages and marketing [unverified: not researched].
- **With telegram and trello-import.** Third-party API terms [unverified]. Imported data needs a GDPR Art 14 notice [S41].

**Exit:** the release gate review lists no open must-fix, or you accept each in writing.

## One-way doors, and the slot that closes each

| Door | Why it can't be undone | Closes |
|---|---|---|
| The operator's country and legal form | They decide which laws bind Zurfur, which representatives it needs, its regulator, its tax registrations and its personal liability [S8, S41, S56] | A1 |
| What goes into a PLC operation | The PLC log is public and permanent. "Technical impossibility" is no defence [S45, I4] | The first real PLC submission (did-ops, account-pds) |
| Records published to the atproto network | Relays and other apps copy them, and erasure reaches only as far as reasonable steps go [S41, S72] | The first publish to a real relay (account-pds, character-presence, claims, friendship, blocks) |
| What the User record stores about terms and age | Its stored shape becomes real at alpha. A bad choice, such as keeping ID images, creates data that can't be un-collected (Texas fines each retention of identifying information [S14]) | A1 |
| The report and moderation-action records | They need the legal clocks, the clause and Terms version, and whether reasons were sent. Without these, past actions can't be proven later | A1 |
| Promises made in the Terms and the Privacy Policy | Changing them needs notice (P2B: 15 days) and never undoes past processing [S39] | A1 |
| Keeping ratings forever | Every rating is personal data for both people involved [S41, I5] | With reputation |
| The code license | A version released under a license stays under it. Outside contributions made with no license muddy ownership: GitHub's contribution rule applies only to repos that have a license [S66] | The first release under a license, or the first outside contribution |
| Filing a CSAM report | It starts a duty to preserve for 1 year [S29] | The first report |

## Cross-team dependencies (what I need, from whom, by which slot)

| From | What | By |
|---|---|---|
| **The Engineer** | Rulings Q1–Q10 | P2 |
| | A lawyer engaged | P2 |
| | The company and the registrations (NCMEC, DMCA agent, representatives) | P3 |
| | Signatures on the legal pages and the impact assessment, and written acceptance of any risk left open | P4 |
| **moderation** | Report categories with their legal clocks and notifier fields; statements of reasons; Moderator Actions that cite a clause and a Terms version | A1 (record shape reviewed in P3) |
| | The CSAM procedure and an evidence store no agent can reach | A1 |
| | Enforcement against repeat infringers | A1 |
| **web** | Legal pages with footer links on every page, signed out included | A1 |
| | The Terms acceptance screen | A1 |
| | The age-gate screens | A1 |
| | The report form | A1 |
| | The license notices page | A1 |
| | The analytics opt-out link | A1 |
| | The accessibility statement | A1 |
| | Keep the "zurfur.sid is the only cookie" test | A1 |
| **identity** | The Terms version and age status on the User | A1 |
| | Account export and erasure as named operator actions | A1 |
| | An opt-out for people who never signed up | With user-pages |
| | A review of what goes into PLC operations | Before the first real PLC submission |
| | Deletion that reaches relays | With account-pds |
| **commission** | Adult-rated files gated by the age status (with den) | A1 |
| | Invoice labelling | With invoices |
| | Rating retention and anonymization | With reputation |
| | An export and erasure path for commission data | A1 |
| **den** | The file read checks age status alongside its existing access check, if your ruling gates Adult-rated files | A1 |
| | Export of a user's tree | B1, with self-serve export |
| **posts** | The age gate on every public view | A1 |
| | Notices for public records | With gallery-posts |
| **plugin** | The developer agreement, and a list of the data plugins receive | Before the first third-party plugin (beta) |
| **internal** | An npm license gate in CI | A1 |
| | The LICENSE file, once you rule on Q8 | Per Q8 |
| | No real user data in agents on consumer plans, the bug-report reader included | A1 |
| **The tech lead** | The processor and hosting list, the region, and how long logs and backups are kept | P3 |
| | Owner of the breach runbook | P3 |
| | A geo signal at the edge, if Q3 picks geo-restriction | A1 |

## Not on the board (proposals, each with why)

1. **Legal pages** (Terms, Privacy, Content, Copyright and Contact, plus Terms acceptance). The Inventory Closures decision already puts "ToS/legal pages" in MVP scope [I2], but no Board card holds it. Due A1.
2. **Privacy operations:**
   - the record of processing and the impact assessment;
   - the retention schedule;
   - access, erasure and export as named operator actions;
   - the breach runbook, processor contracts and representatives.

   The GDPR applies to a free service offered to people in the EU, "irrespective of whether a payment … is required" [S41]. Due A1.
3. **Age assurance.** The age check from Q3 and Q4. The maturity-ui card covers the labels, not the check. Due A1 under Q3 (b); fully in beta under Q3 (c).
4. **Agents and real data.** No real person's data goes to an AI tool without a processor contract, and evidence goes to no agent [S55]. Due A1.
5. **License notices and the npm license gate.** Required wherever browsers download third-party code or fonts [S68]. Due A1.
6. **The repository license.** Your call (Q8). Due before the first outside contribution or grant application.
7. **Self-serve export and deletion.** Manual handling won't scale past the alpha. Due B1.
8. **Re-measuring the thresholds** at every stage. Size and reach decide which laws apply. Each end-of-release review.

## Questions for the Engineer (each pre-answered)

Each answer is Claude's proposal. Every one is yours to rule.

**Q1. Where will Zurfur's operator be based, and will there be a company before the alpha?**

- **Claude's answer:** form a limited company before A1, in your country of residence unless counsel advises otherwise.
- **Why:** today you personally are "the provider" under UK law [S8], and you are personally liable for an adult-content host. CSAM vendors vet applicants (PhotoDNA is "subject to approval by third-party vetting service", moderation study), and whether an individual can register with NCMEC is [unverified] [S30]. Subscriptions will need a company anyway.
- **Cost:** a company switches on platform tax reporting, which skips an individual (DAC7 needs an "Entity"; the UK excludes sole traders) [S56, S57]. [lawyer]

**Q2. Should Zurfur hire a lawyer before the alpha?**

- **Claude's answer:** yes, in P3, for a fixed-scope review of:
  - the four policies;
  - the age-check posture;
  - the CSAM procedure;
  - the impact assessment;
  - the [lawyer] rows of the A1 gate, in that order.

**Q3. How does adult art meet age-check law in the alpha?** This amends the ruled default in User-Profiles (21594113, decision 10): Blurred for everyone, with "I'm 18+" before Adult work. Options:

- **(a) Keep self-declaration.** It fails the UK, Brazil, France and the US states with these laws, and very likely Australia's and Germany's adults-only rules [S3, S6, S13–S16, S23, S24, S26, S27].
- **(b) A vendor age check, everywhere.** Adult-labeled work opens only to signed-in users who passed it. Signed-out visitors see a label with no image.
- **(c) Geo-restrict Adult work** in the jurisdictions that require checks, and keep self-declaration elsewhere. Ofcom warns that weak geoblocking can leave a service in scope [S7], though it weighed a UK block when setting one fine [S9]. The list of jurisdictions grows every few months.
- **(d) Serve no Adult-labeled work in the alpha.**

**Claude's answer:** (b).
- One rule fits every jurisdiction.
- It keeps your reason for blurring: adults who passed the check are never barred, so labelling stays honest.
- A closed alpha makes the per-check cost small.
- The age data stored is the result only, never a document [S14].

**Q4. What is the minimum age?**

- **Claude's answer:** 18 for the alpha and beta, enforced by the same check at first sign-in.
- **Why it simplifies things:** it takes the UK children's duties off the table [S4], and with them the ICO children's code [S49], COPPA [S20], the GDPR's consent age for children (13–16) [S41], and the under-16 laws in Australia, Brazil and the UK [S11, S26, S27].
- **What it costs:** the furry art community has many teens. Revisit 13+ for the release only if age assurance runs at full scale.

**Q5. Is sexual content of real people allowed?**

- **Claude's answer:** no. Zurfur hosts drawn and rendered art. Photographs and videos of real people that are sexually explicit are banned, and so are sexual depictions of a real, identifiable person in any medium.
- **Why:**
  - the TAKE IT DOWN Act turns on an "identifiable individual" [S32];
  - the US record-keeping law (§ 2257) reaches only images of "an actual human being" [S21];
  - Ofcom treats a non-graphic fursuit as not pornography [S6].

  The ban removes the most dangerous categories at no cost to the product's purpose.

**Q6. What about sexual depictions of characters who are, or look like, minors?** This is moderation's open question 18.

- **Claude's answer:** prohibit them in every medium, whatever age the character is given, everywhere. Don't gate them per country.
- **Why:**
  - the UK covers "an imaginary child" and images whose "predominant impression" is a child [S12];
  - Canada covers any "visual representation" [S31];
  - 18 U.S.C. § 1466A(a)(2) has no obscenity requirement [S21].

  [lawyer]

**Q7. What does an "invoice" say?**

- **Claude's answer:** keep the name. Show the issuer plainly: "Issued by <artist>. Zurfur does not issue invoices or collect payment." Let the artist add their own tax details. Zurfur computes no VAT in the alpha.
- **Why:** the legal content of an invoice is the seller's duty [S58].
- [lawyer] on platform tax reporting once a company exists.

**Q8. Which license, and when?**

- **Claude's answer:** decide before A1. AGPL-3.0-or-later for the server and web app, and Apache-2.0 for the contract (protos and lexicons) so other apps can implement them.
- **Why:**
  - with no license, the code is all rights reserved [S66];
  - grants such as NLnet's need an open license [S70];
  - AGPL keeps hosted forks open (§ 13) [S68].
- **Caveats:**
  - the US Copyright Office says purely AI-generated material isn't copyrightable [S67];
  - NLnet won't pay for "outcomes purely generated by AI" [S70].

  How far a license can be enforced on mostly AI-written code is a [lawyer] question.

**Q9. Should Zurfur appoint EU and UK representatives for a closed alpha?**

- **Claude's answer:** yes, if the alpha admits EU or UK users.
- **Why:**
  - the GDPR exemption covers only "occasional" processing [S41, S44];
  - the DSA's representative duty has no exemption for small providers [S36].
- **The alternatives:** keep EU and UK users out of the alpha, or admit them without representatives and accept that risk in writing.
- Whether one firm can act for both the GDPR and the DSA is [unverified].

**Q10. Which AI tools may touch real users' data?**

- **Claude's answer:** before A1, any agent that may read alpha data (bug reports, logs, database rows, reports) runs under commercial terms with the data processing agreement [S55]. No agent ever reads the evidence store.
- **Until then:** you read alpha bug reports yourself, not the Orchestrator. The 0.1 product design document already notes that what the Orchestrator reads "reaches its model provider" [I9].

## Sources

All web sources were read on 2026-10-10 unless marked otherwise. "Study" means moderation's study `.understand/20261008-moderation-study.md`, where its QA re-read the text.

**Age and online safety**

- **[S1]** UK Online Safety Act 2023 s.4 (links with the UK). legislation.gov.uk/ukpga/2023/50/section/4 [read]
- **[S2]** UK Online Safety Act ss.7, 9, 10, 20, 21, 23 (duties on every user-to-user service). legislation.gov.uk/ukpga/2023/50/section/7 [read]
- **[S3]** UK Online Safety Act s.12(3), (4), (6). legislation.gov.uk/ukpga/2023/50/section/12 [read]. Subsection (5), the exception where the Terms ban the content for everyone, is not quoted here.
- **[S4]** UK Online Safety Act ss.35–37 (children's access assessment). A provider may conclude children can't access the service "only … if age verification or age estimation is used". [read]
- **[S5]** UK Online Safety Act s.236, and Ofcom's guidance on content harmful to children, para 2.12, which includes "Paintings, drawings or other non-photographic images". [read]
- **[S6]** Ofcom, "Adults only: what to do if your online service allows pornography" (updated 13 January 2026). It says "All services that allow pornography will need to implement highly effective age assurance", and that self-declaration is "not capable of being highly effective". [read, from an archived copy, because ofcom.org.uk blocks fetching]
- **[S7]** Ofcom, highly effective age assurance FAQ: "Self-declaration of age" is not acceptable, and weak geoblocking may leave a service in scope. [read]
- **[S8]** Ofcom, "Online Safety Act explained" Q&A (28 May 2025): "significant number" is undefined, no service can apply for an exemption, and an individual can be the provider. [read]
- **[S9]** Ofcom enforcement updates: 4chan £20k, Itai Tech £50k (its UK block was taken into account), AVS £1m, Kick £800k. [read]
- **[S10]** UK Online Safety Act s.66, in force 7 April 2026, and Ofcom's industry bulletin of March 2026: reports already made to NCMEC count. [read]
- **[S11]** UK Online Safety Act s.214A (inserted 29 April 2026), and the government's statement of 15 June 2026 on under-16s. [read]. The spring-2027 start date is [excerpt].
- **[S12]** UK Coroners and Justice Act 2009 ss.62 and 65 (imaginary children). legislation.gov.uk/ukpga/2009/25/section/65 [read]
- **[S13]** *Free Speech Coalition v. Paxton*, No. 23-1122 (27 June 2025): intermediate scrutiny; "adults have no First Amendment right to avoid age verification"; footnote 2 lists 21 other states. supremecourt.gov/opinions/24pdf/23-1122_3e04.pdf [read]
- **[S14]** Texas Civil Practice and Remedies Code ch. 129B (HB 1181). It covers sites where more than one-third of the material is sexual material harmful to minors. A "commercial entity" includes a sole proprietorship, and the law includes "animated" depictions. Fines apply for keeping identifying information. capitol.texas.gov HB01181F [read]
- **[S15]** Tennessee, Protect Tennessee Minors Act (Public Chapter 1021). It covers an "individual or commercial entity", a "fictitious character's body", and makes a violation a Class C felony [read: I read the introduced bill, SB1792]. The threshold is 10% in the introduced bill and reportedly one-third in the enacted text [read by a research agent; check it].
- **[S16]** Louisiana R.S. 51:2121 (private right of action); Arizona A.R.S. 18-701 ("including a social media platform"); Missouri § 407.3405 (from 28 August 2026); Florida § 501.1737. [read]
- **[S17]** The count of 27 US states with adult-content age-check laws: the Free Speech Coalition and AVPA trackers. [excerpt]
- **[S18]** Mississippi HB 1126 (no size threshold) [read]; *NetChoice v. Fitch*, the Supreme Court's denial of 14 August 2025 with Justice Kavanaugh's concurrence [read]; Bluesky's blog of 22 August 2025 and its update of 8 December 2025 [read].
- **[S19]** Tennessee Code 47-18-5701 and following (social media; no size threshold; in force) [read]. Florida, Texas SCOPE, Utah and Georgia: litigation status [excerpt].
- **[S20]** 15 U.S.C. § 6502; the FTC's COPPA FAQ; the COPPA amendments at 90 FR 16918 (compliance by 22 April 2026); the FTC's policy statement on age verification (February 2026). [read]
- **[S21]** 18 U.S.C. § 2257 and 28 CFR 75.1 ("actual human being"; hosting exemption (h)(2)(B)(v)); 18 U.S.C. § 1466A. [read]
- **[S22]** DSA Arts 19 and 28; the Commission's Art 28 guidelines, C/2025/5519, paras 6, 8, 37 and 52 (micro enterprises exempt; self-declaration is not appropriate). [read]
- **[S23]** French Penal Code art. 227-24, quoted in Arcom's age-verification standard: liability even when access "résulte d'une simple déclaration" (results from a mere self-declaration) [read]. CJEU C-188/24, 16 June 2026, press release cp260087 (age checks for providers in other member states; algorithmic ranking means "control") [read].
- **[S24]** Germany, the youth-media treaty (JMStV) §2(1) and §4(2). [read]
- **[S25]** Italy, AGCOM decision 96/25/CONS (listed sites; self-declaration is not effective). [read]
- **[S26]** Australia, Online Safety Act 2021 ss.63C and 134, and eSafety's FAQ on the age-restricted material codes ("must ensure a user is 18 or older", from 9 March 2026). [read]
- **[S27]** Brazil, Lei 15.211/2025 (ECA Digital) art. 9, in force 17 March 2026: applies wherever the provider is based; checks "a cada acesso" (at every access); self-declaration is barred. planalto.gov.br [read]
- **[S28]** EU age-verification blueprint (digital-strategy.ec.europa.eu, updated 22 September 2026). [read]

**CSAM, intimate images and copyright**

- **[S29]** 18 U.S.C. § 2258A (a), (e), (f), (h). [read, study]
- **[S30]** NCMEC: missingkids.org/csam (register through espteam@ncmec.org); missingkids.org/cybertiplinedata (23% of registered providers are outside the US and registered voluntarily). [read]. Whether an unincorporated individual can register is [unverified].
- **[S31]** Canada, S.C. 2011 c.4, and Criminal Code s.163.1. [read, study]
- **[S32]** TAKE IT DOWN Act, Pub. L. 119-12, s.3 and s.4(3) (covered platform; no size threshold; 48 hours); the FTC's compliance guidance and its May 2026 enforcement release. [read]
- **[S33]** 17 U.S.C. § 512 (c)(2), (c)(3), (g), (i); 37 CFR 201.38 (online filing, $6, lapses after 3 years, a street address); the copyright.gov DMCA FAQ. [read]. Whether a non-US provider can rely on § 512 is [unverified].
- **[S34]** EU copyright directive 2019/790, Art 2(6) and Art 17; Commission guidance COM(2021) 288. [read]
- **[S35]** UK Electronic Commerce Regulations 2002, reg. 19 (still in force). [read]

**Platform rules (EU)**

- **[S36]** DSA, Regulation 2022/2065: Arts 3, 11–18, 19, 24(5), 27, 29 and 30; the Transparency Database legal notice (micro enterprises are exempt from Art 24(5)). [read]
- **[S37]** Directive 2015/1535 Art 1(1)(b) ("normally provided for remuneration"); *McFadden* C-484/14, paras 40–41. [read]
- **[S38]** Commission Recommendation 2003/361 (micro enterprise: under 10 staff and up to €2m; includes the self-employed). [read]
- **[S39]** P2B Regulation 2019/1150 Arts 1(2), 3, 4, 5, 11(5). [read]. The Digital Omnibus repeal proposal is [excerpt].

**Reviews**

- **[S40]** 16 CFR Part 465 and the FTC's Q&A; UCPD Art 7(6); UK DMCC Act 2024 Sch 20 para 13 (in force 6 April 2025); UK Defamation Act 2013 s.5 and the 2013 Regulations. [read]

**Privacy**

- **[S41]** GDPR, Regulation 2016/679: Art 2(2)(c) and recital 18; Arts 3(2), 8, 9, 12(3), 13, 14, 17, 27, 28, 30, 33, 35. [read]
- **[S42]** *Lindqvist* C-101/01 para 47; *Ryneš* C-212/13 (the household exemption). [read]
- **[S43]** *OT* C-184/20 (indirect disclosure of sexual orientation is special-category data); *Meta v Bundeskartellamt* C-252/21. [read]
- **[S44]** EDPB guidance:
  - Guidelines 3/2018 (territorial scope; "occasional");
  - Guidelines 9/2022 para 73 (notify every authority whose residents are affected);
  - Guidelines 05/2021 Example 1 (transfers);
  - WP248 (impact assessments: two criteria);
  - WP260 (Art 14(5)(b) "not routinely").

  [read]
- **[S45]** EDPB Guidelines 02/2025 on blockchain, version 2.0, adopted 7 July 2026: paras 35, 55 and 102–103, and "technical impossibility cannot be invoked". [read, by me]. These apply to the PLC log by analogy only. [lawyer]
- **[S46]** EU-US Data Privacy Framework: *Latombe* T-553/23 dismissed [excerpt]; appeal C-703/25 P pending [read].
- **[S47]** UK GDPR Art 27; the Data (Use and Access) Act 2025 ss.70, 81, 103 and Sch 12; commencement under SI 2026/82 (complaints duty from 19 June 2026). [read]
- **[S48]** ICO data-protection fee page, and disclosure IC-389257 (whether the fee applies outside the UK is unresolved). [read]
- **[S49]** ICO Children's code, "services covered" (it doesn't apply if the service prevents children's access). [read]
- **[S50]** ePrivacy Directive Art 5(3); EDPB Guidelines 2/2023 paras 33, 55, 56; CNIL's exemption conditions for audience measurement; UK PECR Sch A1 paras 4 and 5 (as amended by the 2025 Act). [read]
- **[S51]** CalOPPA, Cal. Bus. & Prof. Code §§ 22575–22577 [read, from a mirror]; Delaware DOPPA [read]; CCPA § 1798.140 and the CPPA's adjustment ($26,625,000) [read].
- **[S52]** Texas Data Privacy and Security Act § 541.107 [read]; Connecticut PA 25-113 (any processing of sensitive data) [excerpt].
- **[S53]** Washington My Health My Data Act, RCW 19.373.010. [read]
- **[S54]** US state breach laws: NCSL [excerpt]; Cal. Civ. Code § 1798.82 [read, from a mirror].
- **[S55]** Anthropic, Claude Code "Data usage" (code.claude.com/docs/en/data-usage): on consumer plans, "We will train new models using data from Free, Pro, and Max accounts when this setting is on". Anthropic Commercial Terms §B: "Anthropic may not train models on Customer Content from Services"; §C incorporates the data processing agreement. [read, by me]

**Tax and payments**

- **[S56]** DAC7, Directive 2021/514, Annex V §I.A.1, A.2, A.8, A.10, B.4(d), C.1; Art 8ac(4). [read]
- **[S57]** HMRC manual IEIM901100, 901200, 901400, 901630, 901730, and gov.uk's reporting rules for digital platforms. [read]
- **[S58]** VAT Implementing Regulation 282/2011 Arts 7, 9a and 54b; VAT Directive Arts 218, 219a, 220, 220a, 226 and 242a. [read]
- **[S59]** HMRC's VAT rules for digital services (commissioned content is not covered); gov.uk "Register for VAT" (non-UK businesses register from the first UK sale). [read]
- **[S60]** Belgium's e-invoicing mandate (B2B from 1 January 2026); Poland's KSeF timetable. [read]
- **[S61]** 26 U.S.C. § 6050W; Cal. Rev. & Tax. Code § 6041; New York Tax Law § 1101; Texas HB 1525. [read]
- **[S62]** 31 CFR 1010.100(ff)(5); PSD2 Arts 3(j) and 4(5). [read]

**Accessibility**

- **[S63]** European Accessibility Act, Directive 2019/882, Arts 2(2), 3(4), 3(23), 3(30), 4(5) [read]. Ireland's National Disability Authority on EN 301 549 V4.1.1 (WCAG 2.2; its Official Journal citation is expected 16 December 2026) [read].
- **[S64]** US Department of Justice web guidance (2022) and the Title II rule page. [read]
- **[S65]** UK Equality Act 2010 s.29, Sch 2 para 2, and the explanatory notes on territorial reach. [read]

**Licenses and AT Protocol**

- **[S66]** GitHub's "Licensing a repository" page; GitHub Terms of Service D.5 and D.6. [read]
- **[S67]** US Copyright Office, "Copyright and Artificial Intelligence, Part 2" (January 2025). [read]. *Thaler* certiorari denied [excerpt].
- **[S68]** MIT; BSD-3; Apache-2.0 §4; SIL OFL and its FAQ §2.1; AGPL-3.0 §13. [read]
- **[S69]** cargo-deny license-check documentation [read]; this repo's `deny.toml` and the CI `deny` job [read, by me]; yarn v1 `licenses` [read].
- **[S70]** nlnet.nl/propose; NLnet's generative-AI policy (v1.1). [read]
- **[S71]** Bluesky developer guidelines (bsky.network/docs/developer-guidelines). [read]
- **[S72]** atproto account spec; plc.directory API; the PLC Organization (plcred.org). [read]. No published terms of use for plc.directory were found [unverified].

**Internal sources** (corpus pages under `~/code/zurfur-design/`, plus this repo)

- **[I1]** Payments & Billing Model (6422530): "no entity".
- **[I2]** Inventory Closures (30343170), items 7 and 13.
- **[I3]** User-Profiles, the Handle Swap & Content Maturity (21594113), decision 10; Maturity Vocabulary (29982722).
- **[I4]** Account Deletion, Tombstoning & Handle Reuse (23003138): the PLC log is permanent.
- **[I5]** User Reviews & Comments (2490393): ratings are stored forever.
- **[I6]** Publish Consent & OAuth Scopes (29687820).
- **[I7]** Project MVP (589826): the security-first list; analytics with Plausible and Sentry.
- **[I8]** `.understand/20261008-moderation-study.md`.
- **[I9]** The 0.1 product design document (`.understand/releases/0.1/pdd.typ`): D-25, D-26, D-34.
- **[I10]** Your moderation rulings of 2026-10-08: nobody edits the database by hand. The relay of these rulings is not yet in the corpus.

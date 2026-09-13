**None is alpha-ready unchanged; this ranks architectural shape, not implementation readiness.**

1. **F — Own DID + repo, ownership through an Account.** Best of a flawed set because it preserves portable Character data and a Character-authored claim while obeying CCS’s single-edge rule; however, Account governance, Keeper exit, and key control remain **UNVERIFIABLE** and must be specified before shipping.

2. **A — Own DID + full repo.** Strongest direct anti-domination design—stable identity, portable publication, no intermediary owner—but DD 54427650’s multi-owner claims knowingly produce a CCS conflict, and the production PDS, backup, blob migration, and account lifecycle do not yet exist.

3. **E — Own DID + claims-only repo.** Correctly separates Character speech from heavy content hosting and limits premature lexicon work, but it still incurs nearly the full per-account PDS security burden while leaving the actual creative identity trapped in Zurfur’s Index; omit the unjustified Bluesky profile stub.

4. **D — DID-only now, full repo later.** Operationally honest and reversible, but “exit later” violates the doctrine for alpha users, and the deferred A destination remains incoherent until ownership, custody, export, and migration semantics are repaired.

5. **B — Own DID, no repo.** Simplest implementable alpha shape, but the surviving DID is an empty husk if Zurfur disappears: neither Character-authored consent nor independently recoverable profile/content exists, so anti-domination fails outright.

6. **C — Records in Keeper’s repo, no Character DID.** Worst because it reverses built identity architecture and turns transfer continuity into an application convention; it also publicly correlates Characters through the Keeper’s repo and leaves competing old/new “canonical” records unresolved.

**Evidence that would flip my #1:** an authoritative vulpes specification plus interoperability test demonstrating that multiple simultaneous human→Character ownership edges are valid co-ownership—not a CCS conflict—would move **A above F**, because direct Keeper ownership is preferable to inserting an Account intermediary once both models are equally protocol-valid.
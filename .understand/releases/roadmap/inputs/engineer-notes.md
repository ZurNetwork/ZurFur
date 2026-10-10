# The Engineer's notes for later stages

Noted by the Orchestrator from the Engineer's words. These are directions, not decided designs. Place them in the roadmap. Don't design them.

1. **A storage-routing crate, if needed** (2026-10-10). Today the application layer decides whether a thing goes to private storage (PostgreSQL) or public storage (the user's PDS), and it holds ports for both. If that gets convoluted, add a crate that exposes one method call that routes a write to the right store. Then the application layer stops juggling both ports. Build it only when the pain shows.
   - Rules it must keep: a write that touches both stores is still a separate, retryable outbox step, never one atomic unit. The class test in "The AT Protocol Boundary Contract" stays the one rule that picks the destination.

2. **External-storage mounts: alpha, maybe beta** (2026-10-10). Because of the Den tree and the nature of the PDS, a mounted node may point at storage Zurfur doesn't control: Google Drive, IceDrive, someone's NAS, anything. That storage must comply with Zurfur's representation, which makes it a large, composable, plug-in storage. **Not for 0.1.**
   - Open tensions, for when its slot comes:
     - Today a mount is made by the system from a person's access, so external storage is a new kind of thing.
     - A Private level can't be enforced on storage we don't control.
     - Fetching from endpoints users supply invites SSRF, and we'd hold third-party OAuth tokens.
     - Content hashes are what let us verify the bytes.
     - Storage that goes offline needs an honest state in the tree.

3. **Teams that don't plan by version.** Internal works by project, not by release. Wolven is paused (ruling D-27): its tree walk is committed to a branch, and it has no roadmap file. Vulpes keeps building on its own roadmap in parallel; its file maps that roadmap onto Zurfur's stages.

4. **Reviews at the end of every release** (ruling 2026-10-10, pending merge): the compliance team checks what Zurfur is legally bound to follow, and the architects team reviews the whole project's architecture. A legal must-fix blocks a release unless the Engineer accepts the risk in writing.

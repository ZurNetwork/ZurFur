---
name: Readability over cleverness
description: User strongly prefers readable, maintainable code over clever or terse patterns
type: feedback
---

Always favor readability and maintainability over cleverness or brevity. Code that's easy to read is code that's easy to maintain.

**Why:** Clever code "looks cool" but costs time when someone (including future-you) needs to understand, debug, or modify it. Readable code pays for itself every time it's revisited.

**How to apply:**
- Break complex expressions into named intermediate variables
- Prefer explicit over implicit (e.g., named variables over long chains)
- Don't compress logic to save lines — clarity wins over conciseness
- If a pattern requires a second read to understand, simplify it
- This applies to production code, tests, and mock implementations equally
- Idiomatic Rust chaining (`.iter().filter().map().collect()`) is fine when each step is self-explanatory
- A chain becomes "magic" when it mixes concerns (locking + searching + transforming + wrapping) — break those apart

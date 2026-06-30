---
name: No generic module names
description: Never name files/modules "helpers", "base", "misc", "utils" — use domain-specific names instead
type: feedback
---

Never name files or modules as "Helpers", "Base", "Misc", or similar generic catch-alls. Use descriptive, domain-specific names instead.

**Why:** The user considers this bad practice in almost all cases. Generic names obscure what the module actually contains and make it harder to navigate the codebase.

**How to apply:** When creating test infrastructure, name files after what they mock (e.g., `mock_users.rs`, `mock_organizations.rs`, `test_state.rs`). When creating shared utilities, name them after the domain concept they serve, not "helpers" or "utils".

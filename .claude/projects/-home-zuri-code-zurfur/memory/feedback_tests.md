---
name: Always include tests
description: User requires extensive tests for both frontend and backend in every submodule PR
type: feedback
---

Add extensive tests to both frontend and backend as part of every submodule.

**Why:** User explicitly requested this — tests should not be an afterthought or separate PR.

**How to apply:** Every `feature/auth_*` (and future submodule) branch must include relevant tests before the PR is created. Cover unit tests for domain logic, integration tests for persistence/API, and frontend component tests where applicable.

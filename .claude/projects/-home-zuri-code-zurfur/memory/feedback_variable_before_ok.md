---
name: Assign before returning Ok
description: Always assign results to a named variable before wrapping in Ok() — never chain directly into Ok()
type: feedback
---

Always assign the result to a named variable before returning it wrapped in `Ok()`. Never chain computations directly into `Ok(...)`.

**Why:** Chained returns are harder to read, debug, and modify. A named variable makes the return value immediately clear and lets you inspect or transform it without unwinding the chain.

**How to apply:** Instead of `Ok(self.items.lock().await.iter().find(|x| ...).cloned())`, write:
```rust
let item = self.items.lock().await.iter().find(|x| ...).cloned();
Ok(item)
```

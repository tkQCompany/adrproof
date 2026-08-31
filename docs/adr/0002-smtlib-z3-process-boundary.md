---
id: ADRP-0002
status: accepted
---

# Use SMT-LIB2 and a pinned Z3 process

The textual boundary is replayable, inspectable, and keeps the typed frontend
independent of a particular in-process solver binding.

```adrlogic
bool replayable_solver_input;
rule C1 "solver input is retained" { replayable_solver_input; }
```

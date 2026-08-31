---
id: ADRP-0003
status: accepted
---

# Check one global effective specification

Lifecycle is resolved before all active clauses and extracted facts are conjoined.
Pairwise contradiction checks are insufficient.

```adrlogic
bool global_conjunction;
rule C1 "all active clauses share one context" { global_conjunction; }
```

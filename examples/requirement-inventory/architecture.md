---
id: ADR-1
status: accepted
---

Database access must be mediated by the repository boundary.
Critical operations must also have reviewed failure-recovery behavior.
The boundary is intended to reduce coupling to storage technology.

```adrlogic
bool repository_boundary;
rule C1 "require repository boundary in the declared model" { repository_boundary; }
```

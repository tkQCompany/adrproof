---
id: ADRP-0005
status: accepted
---

# Scope closed-world facts and preserve evidence history

A provider may emit negative facts only inside an explicit, precise completeness
claim. Missing or partial coverage cannot produce PASS. Verification evidence is
an immutable execution record; current validity is computed from current semantic
inputs, backend version, and configuration.

```adrlogic
bool absence_requires_coverage;
rule C1 "absence is false only under explicit complete coverage" {
    absence_requires_coverage;
}
```

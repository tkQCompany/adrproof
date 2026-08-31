---
id: ADRP-0001
status: accepted
---

# ADRProof is a project meta-verifier

ADRProof owns project-level relational consistency and delegates program semantics
to specialized verifiers. It will not implement a complete Rust semantics.

```adrlogic
bool meta_verifier;
rule C1 "ADRProof remains a meta-verifier" { meta_verifier; }
```

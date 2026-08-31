---
id: ADRP-0004
status: accepted
---

# Project Intent IR is the semantic core

ADRLogic is a small frontend for relational project constraints. It lowers into a
backend-neutral Project Intent Model and does not become the interchange language
for function contracts, temporal models, hardware descriptions, API schemas, or
other specialized formalisms.

The Project Intent Graph standardizes identities, typed links, provenance, proof
obligations, input fingerprints, and evidence validity. Specialized artifacts may
remain opaque and are checked by pinned external verifiers. Z3 is the first
constraint backend, not the semantic center of ADRProof.

```adrlogic
bool specialized_verifiers_remain_external;
rule C1 "specialized verifier semantics remain external" {
    specialized_verifiers_remain_external;
}
```

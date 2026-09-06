---
id: ADRP-0007
status: accepted
---

# Maintain reviewed links from requirements to current evidence

## Context

Architectural maintenance is a long-term activity. A consistent formal model
and a fresh verifier result do not establish that every applicable requirement
was represented, or that its formalization still expresses the approved intent.
An ADR may contain multiple requirements, rationale and historical alternatives.
FactCoverage measures completeness of extracted facts, not completeness of prose
formalization. These must never be conflated.

## Decision

Extend the existing Project Intent Model and typed graph, rather than introduce
a parallel registry, to track applicable requirements and their reviewed links
to formal artifacts, obligations and evidence. Reuse existing identities,
provenance, lifecycle and fingerprint mechanisms wherever possible.

The target workflow distinguishes:

1. The inventory of applicable decisions and explicitly identified requirements.
2. Human-approved correspondence between a requirement and its formalization.
3. The result and current validity of the checks on that formalization and code.

The requirement-to-formalization review binds content hashes of both sides and
the relevant mapping, with reviewer provenance. Filesystem modification times
are not authoritative. A change to either side requires reassessment; rerunning
a verifier or regenerating a formalization cannot renew that review. A reviewer
may confirm that a prose-only correction needs no formal change. Start with
conservative invalidation; finer granularity needs a tested selection contract.

One requirement may need several checks, and checks may support several
requirements. Formal artifacts may remain in verifier-native languages. A
test result, bounded model result and deductive proof retain distinct authority.
A text UML artifact must have an explicit role: specification with defined
semantics, or generated view, not an implicitly synchronized second truth.

The inventory must expose missing mappings, incomplete formalization, outdated
reviews and unavailable verification. Removing a required item or changing its
scope is a reviewed specification change, never a way to repair implementation
by obtaining an empty or weakened PASS. Historical and out-of-scope requirements
remain visible with an explicit reason. Human attestation is not a machine proof.

## Boundaries and consequences

No deterministic check of hashes proves equivalence between prose and a formal
contract. Approval records a reviewed interpretation, not mathematical proof of
human intent. LLM-authored translations cannot approve themselves. New schemas
must be independently versioned; published protocol/report v1 is unchanged.

This decision specifies target behavior, not a declaration of full implementation.
The first [declared-inventory slice](../REQUIREMENT_INVENTORY.md) implements selected
requirements and mapping-gap inspection. The subsequent
[formalization-review slice](../FORMALIZATION_REVIEWS.md) binds unsigned external
human attestations to inventory/ADR hashes and independently assesses current,
stale and missing reviews. It supports explicit no-semantic-change reapproval.
Authentication, scope-removal approval and a required-evidence gate remain outside
that slice. Review records are not solver evidence and are not renewed by a check.
Acceptance of this ADR must not be represented by a self-asserted Boolean proof
that the implementation complies.

Before claiming implementation, tests must cover: text changed while the old
contract still passes; formalization changed after review; valid no-semantic-change
review; missing/deleted requirement; changed applicability; missing mapping; and
fresh versus stale evidence. A recreated PASS must not bypass an outdated review.

See [ADRP-0004](0004-project-intent-ir.md),
[ADRP-0005](0005-fact-coverage-and-immutable-evidence.md), and
[the framework plan](../../AKTUALNY_PLAN_RAMOWY.md).

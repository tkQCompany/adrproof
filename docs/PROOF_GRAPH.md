# Proof graph semantics

A **Decision** records project intent and lifecycle. A **Constraint** is its
machine-checkable relational clause. An extracted **Fact** is a deterministic claim
about an artifact, qualified by provider coverage. A **ProofObligation** selects
constraints and facts for a backend. A verification execution creates immutable
**Evidence** containing its result, exact inputs, backend version, and configuration.
**Evidence validity** is not stored as a mutable result: CURRENT or STALE is computed
against today's inputs and tool context.

The current graph uses typed paths:

```text
Artifact --Defines--> Constraint --ParticipatesIn--> ProofObligation
Artifact --Produces--> Fact --RelevantTo-----------> Constraint
ProofObligation --EvidenceFor----------------------> Evidence
Artifact --Defines---------------------> NativeTestObligation
NativeTestObligation --EvidenceFor----> NativeTestEvidence
ChildObligation --RequiredBy----------> ParentGate
```

Relevance is structural: a fact matches a relation occurrence and its fixed
arguments; quantified variables act as wildcards. Impact follows outgoing typed
edges and reports deterministic shortest paths. This fine-grained dependency graph
coexists with one global Z3 execution, preserving detection of contradictions that
only appear in the conjunction of three or more clauses.

PASS evidence means only that one identified obligation passed under its recorded
formalization, inputs, assumptions, backend and configuration. It does not close
the semantic/specification gap or prove all human intent.

Impact queries merge relational, scenario, model, correspondence and native-test
subgraphs before following typed edges. This lets a changed implementation file
reach its directly fingerprinted obligations, their evidence, model-validation
links and any parent gate that requires them.

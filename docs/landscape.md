# Verification landscape and ADRProof's boundary

Research snapshot: 2026-08-15. The links below are primary project sources. The
central conclusion is that ADRProof should orchestrate several verification
classes, not flatten them into one language or reimplement them.

## Closest systems

### MUSUBIX

[MUSUBIX](https://github.com/nahisaho/MUSUBIX) is a broad neuro-symbolic,
specification-driven development environment. Useful ideas are Git-native project
knowledge, explicit requirement-to-code traceability, staged quality gates,
EARS-shaped requirements, Datalog/knowledge-graph analyses, SMT integration, and
separating agent workflows into tools. Its breadth is also the important warning:
confidence scores, semantic search, hallucination detection, LLM translation, and
human-readable explanation are useful authoring aids but are not proof. ADRProof
therefore adopts traceability and orchestration, but puts a hard process boundary
around a small deterministic core. It will not adopt MUSUBIX's entire ontology,
agent platform, custom inference stack, or make confidence a CI verdict.

What is missing for this use case is a lifecycle-aware conjunction of all active
ADR constraints, source-span provenance on every solver assertion, reproducible
solver policy, proof ledger, and a strict SAT/UNSAT/UNKNOWN gate. ADRProof supplies
that narrow layer.

### Anodized

[Anodized](https://github.com/anodized-rs/anodized) aims to be a common Rust
specification layer: ordinary Rust expressions in `#[spec]` attributes describe
preconditions, postconditions, loop invariants, and refinements, and multiple
enforcers can consume them. This is highly complementary. ADRProof should later
implement a `RustProofProvider` adapter that invokes a pinned Anodized-compatible
enforcer and imports its machine-readable result and provenance. It should not
invent a competing function-contract syntax or pretend that runtime
instrumentation proves all executions. ADRLogic remains deliberately relational
and project-level; Anodized remains code-local and behavior-oriented.

The current integration gap is a stable cross-tool result envelope, durable clause
IDs linking contracts to ADRs, explicit verifier capabilities, and reproducible
tool/version/config capture. These belong in ADRProof's provider protocol and proof
ledger, not in a fork of Anodized.

## Rust verification tools

- [Verus](https://github.com/verus-lang/verus) verifies an annotated subset of Rust
  using specifications and automated solvers. Integrate it for functional and
  concurrent code proofs; do not translate Verus into ADRLogic.
- [Creusot](https://github.com/creusot-rs/creusot) translates Rust verification
  conditions through Coma to Why3. It is a future deductive-proof provider, with
  its toolchain and assumptions recorded in the ledger.
- [Kani](https://github.com/model-checking/kani) is a bit-precise bounded model
  checker for Rust safety and correctness harnesses. Import bounds and harness
  identity explicitly: a bounded success is not an unbounded theorem.
- [Aeneas](https://github.com/AeneasVerif/aeneas) translates Charon LLBC/MIR-derived
  programs to a pure calculus with F*, Coq, HOL4, and Lean backends. It targets a
  subset of Rust and currently has stated limitations around unsafe and concurrent
  code. Integrate its generated proof artifacts; do not reproduce MIR semantics.
- [Specula](https://github.com/specula-org/Specula) uses agents to create TLA+
  models and model-check concurrent/distributed systems, then reproduce bugs.
  Its separation between agent-produced models and TLC checking is useful, but the
  generated model still has a specification gap and agents remain untrusted.

These tools solve different problems. ADRProof's shared layer is identity,
provenance, invocation policy, evidence import, and cross-artifact relationships,
not a universal proof calculus.

## Relational and solver backends

- [Z3](https://github.com/Z3Prover/z3) is the first backend. ADRProof emits stable
  SMT-LIB2, uses named assertions for cores, stores the input artifact, pins the
  executable version, applies a timeout, and fails closed on `unknown` or errors.
- [Alloy](https://github.com/AlloyTools/org.alloytools.alloy) describes and explores
  relational structures in bounded scopes. It is attractive for counterexample
  exploration and architecture instances. Its bounded result must retain scope;
  ADRProof should invoke Alloy rather than clone its analyzer.
- [Soufflé](https://github.com/souffle-lang/souffle) compiles Datalog/Horn-clause
  analyses to native parallel code. It fits recursive reachability and large code
  fact graphs. Datalog derivation and SMT satisfiability are distinct backend
  capabilities and should remain so.

## What ADRProof itself owns

ADRProof owns Markdown/front-matter ingestion, ADR lifecycle resolution,
`EffectiveSpecification`, a small typed relational ADRLogic frontend, stable
provenance, deterministic structural fact extraction, backend/provider contracts,
result normalization, diagnostics, artifacts, and the proof ledger. It initially
supports finite Bool/entity/relation formulas and global SMT satisfiability.

It does not own natural-language truth, full Rust semantics, theorem proving for
functions, bounded model checking, temporal model checking, visualization authoring,
or AI reasoning. C4/Mermaid/PlantUML will be generated projections of the same
formal project model, never an independent input authority.

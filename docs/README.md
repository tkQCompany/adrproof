# ADRProof documentation

The public documentation is organized by authority rather than chronology.

## Normative architecture decisions

The canonical Architecture Decision Records live in [`adr/`](adr/):

- [`0001-meta-verifier-not-rust-prover.md`](adr/0001-meta-verifier-not-rust-prover.md)
- [`0002-smtlib-z3-process-boundary.md`](adr/0002-smtlib-z3-process-boundary.md)
- [`0003-global-effective-specification.md`](adr/0003-global-effective-specification.md)
- [`0004-project-intent-ir.md`](adr/0004-project-intent-ir.md)
- [`0005-fact-coverage-and-immutable-evidence.md`](adr/0005-fact-coverage-and-immutable-evidence.md)
- [`0006-versioned-external-provider-process.md`](adr/0006-versioned-external-provider-process.md)

English is the canonical language for normative documentation. See
[`DOCUMENTATION_LANGUAGE.md`](DOCUMENTATION_LANGUAGE.md).

## Architecture and trust boundaries

- [`architecture.md`](architecture.md)
- [`TRUST_MODEL.md`](TRUST_MODEL.md)
- [`MODELING_AND_LANGUAGE_STRATEGY.md`](MODELING_AND_LANGUAGE_STRATEGY.md)
- [`PROJECT_SPEC_STATE_ROOTS.md`](PROJECT_SPEC_STATE_ROOTS.md)
- [`PROOF_GRAPH.md`](PROOF_GRAPH.md)

## Evidence contracts

- [`SCENARIO_EVIDENCE.md`](SCENARIO_EVIDENCE.md)
- [`NATIVE_TEST_EVIDENCE_AND_BUNDLES.md`](NATIVE_TEST_EVIDENCE_AND_BUNDLES.md)
- [`SIGNED_BUNDLES_SCHEMAS_POLICIES_SARIF.md`](SIGNED_BUNDLES_SCHEMAS_POLICIES_SARIF.md)
- [`QUINT_MODEL_EVIDENCE.md`](QUINT_MODEL_EVIDENCE.md)
- [`RUST_QUINT_CORRESPONDENCE.md`](RUST_QUINT_CORRESPONDENCE.md)
- [`SQL_MIGRATION_FACT_PROVIDER.md`](SQL_MIGRATION_FACT_PROVIDER.md)
- [`EXTERNAL_PROVIDER_PROTOCOL.md`](EXTERNAL_PROVIDER_PROTOCOL.md)

## Project context

- [`MILESTONE_2_BASELINE.md`](MILESTONE_2_BASELINE.md)
- [`landscape.md`](landscape.md)

Customer- or project-specific pilot reports are not part of the initial public
repository. A pilot may be published separately after its owner approves the
code, configuration, evidence, and documentation boundary.

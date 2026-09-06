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
- [`0007-reviewed-requirements-and-formalizations.md`](adr/0007-reviewed-requirements-and-formalizations.md)
- [`0008-architect-workflow-and-bounded-feedback.md`](adr/0008-architect-workflow-and-bounded-feedback.md)
- [`0009-long-term-maturity-and-project-independence.md`](adr/0009-long-term-maturity-and-project-independence.md)

English is the canonical language for normative documentation. See
[`DOCUMENTATION_LANGUAGE.md`](DOCUMENTATION_LANGUAGE.md).

## Architecture and trust boundaries

- [`architecture.md`](architecture.md)
- [`TRUST_MODEL.md`](TRUST_MODEL.md)
- [`MODELING_AND_LANGUAGE_STRATEGY.md`](MODELING_AND_LANGUAGE_STRATEGY.md)
- [`PROJECT_SPEC_STATE_ROOTS.md`](PROJECT_SPEC_STATE_ROOTS.md)
- [`PROOF_GRAPH.md`](PROOF_GRAPH.md)

## Evidence contracts

- [`REQUIREMENT_INVENTORY.md`](REQUIREMENT_INVENTORY.md) — experimental declared
  inventory and mapping gaps; not formalization approval or proof evidence.
- [`FORMALIZATION_REVIEWS.md`](FORMALIZATION_REVIEWS.md) — experimental hash-bound
  human attestations and current/stale review assessment, separate from proof.
- [`SCENARIO_EVIDENCE.md`](SCENARIO_EVIDENCE.md)
- [`NATIVE_TEST_EVIDENCE_AND_BUNDLES.md`](NATIVE_TEST_EVIDENCE_AND_BUNDLES.md)
- [`SIGNED_BUNDLES_SCHEMAS_POLICIES_SARIF.md`](SIGNED_BUNDLES_SCHEMAS_POLICIES_SARIF.md)
- [`QUINT_MODEL_EVIDENCE.md`](QUINT_MODEL_EVIDENCE.md)
- [`RUST_QUINT_CORRESPONDENCE.md`](RUST_QUINT_CORRESPONDENCE.md)
- [`SQL_MIGRATION_FACT_PROVIDER.md`](SQL_MIGRATION_FACT_PROVIDER.md)
- [`EXTERNAL_PROVIDER_PROTOCOL.md`](EXTERNAL_PROVIDER_PROTOCOL.md)
- [`WRITING_EXTERNAL_PROVIDERS.md`](WRITING_EXTERNAL_PROVIDERS.md)
- [`MIGRATING_FACT_PROVIDERS.md`](MIGRATING_FACT_PROVIDERS.md)
- [`VERSIONING.md`](VERSIONING.md)

## Project context

- [Current framework plan](../AKTUALNY_PLAN_RAMOWY.md) — implementation compass,
  45 architect-facing criteria, baseline gaps and session handoff requirements.
- [`MILESTONE_2_BASELINE.md`](MILESTONE_2_BASELINE.md)
- [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md)
- [`STABLE_0_2_RUNBOOK.md`](STABLE_0_2_RUNBOOK.md) — inactive historical procedure,
  superseded by ADRP-0009; not a current release instruction.
- [`SOURCE_RELEASES.md`](SOURCE_RELEASES.md)
- [`SUPPORTED_PLATFORMS.md`](SUPPORTED_PLATFORMS.md)
- [`CI_ADOPTION.md`](CI_ADOPTION.md)
- [`releases/BETA_REVIEW_TEMPLATE.md`](releases/BETA_REVIEW_TEMPLATE.md)
- [`releases/PILOT_REVIEW_TEMPLATE.md`](releases/PILOT_REVIEW_TEMPLATE.md)
- [`releases/0.2.0-beta.1.md`](releases/0.2.0-beta.1.md)
- [`landscape.md`](landscape.md)

Customer- or project-specific pilot reports are not part of the initial public
repository. A pilot may be published separately after its owner approves the
code, configuration, evidence, and documentation boundary.

# Changelog

All notable changes to ADRProof are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and package versions follow Semantic Versioning. Protocol compatibility is
defined separately in [`docs/VERSIONING.md`](docs/VERSIONING.md).

## [Unreleased]

### Fixed

- Relevant Cargo manifest fingerprints now use root-relative identities even
  when Cargo canonicalizes an aliased project root (for example `/var` on
  macOS). Manifest edits correctly stale evidence instead of leaving a false
  current PASS. Rerun affected verification; historical evidence is not rewritten.
- Evidence stores use portable filenames without the colon in logical IDs,
  fixing Windows write failures across consistency, scenario, native-test,
  model, model-validation and correspondence evidence. JSON IDs remain unchanged;
  existing colon-named files remain readable on filesystems supporting them.

### Planned

- Reproducible stable `0.2.0` release artifacts after the beta observation
  period.

### Changed

- Portable CI now includes library and command-help regressions; POSIX execution
  fixtures are explicitly separated from platform-neutral core tests. Expanded
  macOS/Windows coverage awaits its first remote run.
- CI adoption guidance separates provider conformance from an architectural
  gate, with explicit input pins, negative controls, and staged approval.
- Beta and isolated-pilot review templates, reproducibility issue reporting,
  and the 0.2 maintenance/security policy clarify release readiness.
- CI now enforces the source-only `publish = false` policy and the portable
  provider test exercises the real versioned `provider check --json` CLI on
  every supported runner.
- Stable releases have a deterministic source-archive generator, a CI
  reproducibility gate, and an explicit supported-platform and limitations
  matrix. No binary or crates.io distribution is introduced.
- Release archives now include deterministic commit/tree manifests and reject
  unsafe tracked paths, mismatched version tags, or publishable Cargo metadata.
- CI validates clean-checkout documentation, command help, dependency licenses,
  and CodeQL results; every third-party GitHub Action is pinned by full commit.

## [0.2.0-beta.1] - 2026-08-31

### Added

- Portable external-provider conformance fixtures with accepted and rejected
  golden responses.
- Versioned provider-check JSON report and stable diagnostic code families.
- Linux, macOS, and Windows CI, plus platform-specific provider process-tree
  cleanup and expanded negative process-boundary tests.
- Provider authoring, migration, versioning, threat-model, and release guides.
- A sanitized beta gate record for the successful commit-pinned private pilot.

### Fixed

- External-provider semantic input identities remain stable when operating
  systems canonicalize a root to a different physical spelling (for example,
  macOS `/var` to `/private/var`).
- Targeted Windows conformance runs no longer compile the Unix-only historical
  regression module.

### Changed

- Protocol v1, its request/response schemas, provider-check report v1, exit
  behavior, and diagnostic families are frozen for beta compatibility.

## [0.2.0-alpha.1] - 2026-08-31

### Added

- Versioned JSON process protocol for explicitly configured external fact
  providers.
- Strict provider identity, provenance, coverage, input, timeout, and output
  validation.
- `adrproof provider check [PROVIDER-ID]` conformance command.
- Neutral Python reference provider and public request/response schemas.
- External-provider configuration, executable, and declared-input tracking for
  evidence staleness.

### Security

- Provider failures, malformed responses, identity mismatches, undeclared
  inputs, excessive output, and timeouts fail closed.
- Configured providers are documented as trusted executables, not sandboxed
  code.

## [0.1.0] - 2026-08-31

### Added

- Public ADRProof baseline: global ADRLogic consistency, Project Intent Model,
  Cargo and PostgreSQL facts, evidence ledger, scenarios, Quint integration,
  Rust-to-Quint correspondence, native-test evidence, bundles, signatures,
  policies, and SARIF.

[Unreleased]: https://github.com/tkQCompany/adrproof/compare/0.2.0-beta.1...HEAD
[0.2.0-beta.1]: https://github.com/tkQCompany/adrproof/compare/105fb808d91027bae3b42207de14614e4eb54c2e...0.2.0-beta.1
[0.2.0-alpha.1]: https://github.com/tkQCompany/adrproof/compare/23db11ab5f903308c4ba278a8b3f529a6a4afb91...105fb808d91027bae3b42207de14614e4eb54c2e
[0.1.0]: https://github.com/tkQCompany/adrproof/commit/23db11ab5f903308c4ba278a8b3f529a6a4afb91

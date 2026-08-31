# Changelog

All notable changes to ADRProof are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and package versions follow Semantic Versioning. Protocol compatibility is
defined separately in [`docs/VERSIONING.md`](docs/VERSIONING.md).

## [Unreleased]

### Added

- Portable external-provider conformance fixtures with accepted and rejected
  golden responses.
- Versioned provider-check JSON report and stable diagnostic code families.
- Linux, macOS, and Windows CI, plus platform-specific provider process-tree
  cleanup and expanded negative process-boundary tests.
- Provider authoring, migration, versioning, threat-model, and release guides.

### Fixed

- External-provider semantic input identities remain stable when operating
  systems canonicalize a root to a different physical spelling (for example,
  macOS `/var` to `/private/var`).
- Targeted Windows conformance runs no longer compile the Unix-only historical
  regression module.

### Planned

- Private cross-project pilot using only commit-pinned public ADRProof inputs.

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

[Unreleased]: https://github.com/tkQCompany/adrproof/compare/105fb808d91027bae3b42207de14614e4eb54c2e...HEAD
[0.2.0-alpha.1]: https://github.com/tkQCompany/adrproof/compare/23db11ab5f903308c4ba278a8b3f529a6a4afb91...105fb808d91027bae3b42207de14614e4eb54c2e
[0.1.0]: https://github.com/tkQCompany/adrproof/commit/23db11ab5f903308c4ba278a8b3f529a6a4afb91

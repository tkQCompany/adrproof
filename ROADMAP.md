# ADRProof roadmap

ADRProof evolves as a deterministic meta-verifier. It standardizes the boundary
between project intent, facts, specialized verification tools, and immutable
evidence; it does not attempt to replace those tools with one universal logic.

## 0.1 — public baseline

Status: released as source at commit
`23db11ab5f903308c4ba278a8b3f529a6a4afb91`.

- global ADRLogic consistency through a pinned Z3 process;
- typed Project Intent Model and proof graph;
- Cargo and PostgreSQL migration fact providers;
- scoped Closed/Partial fact coverage;
- Quint, scenario, correspondence, and native-test evidence;
- immutable evidence, staleness, bundles, signatures, policies, and SARIF.

## 0.2 — external provider protocol

Status: beta candidate. The public alpha began at commit
`105fb808d91027bae3b42207de14614e4eb54c2e`. The conformance kit, portable CI,
and a private commit-pinned cross-project pilot are now complete. Package
version `0.2.0-beta.1` freezes protocol v1 and its machine-readable contracts;
the maintainer still owns publication and tagging.

The milestone makes fact extraction extensible without compiling every provider
into ADRProof.

### 0.2.0 vertical slice

- [x] versioned JSON request and response schemas;
- [x] explicit provider configuration in `adrproof.json`;
- [x] process boundary with timeout, output limit, captured diagnostics, and
  process-tree termination;
- [x] positive facts, artifacts, provenance, and scoped coverage reuse the
  existing Project Intent Model;
- [x] provider configuration, executable, and declared files become semantic
  inputs for evidence staleness;
- [x] deterministic ordering and collision rejection;
- [x] neutral reference provider and contract tests;
- [x] publish reproducible provider conformance fixtures for other languages;
- [x] add an explicit `provider check` conformance command;
- [x] run a private cross-project pilot without adding customer-specific code to
  the public repository.

### Release progression

- **alpha**: the vertical slice is public, but diagnostics and protocol details
  may still change in response to conformance and pilot findings;
- **beta**: protocol v1, its schemas, exit behavior, and machine-readable
  diagnostics are frozen; only compatible fixes are accepted;
- **stable**: the portable conformance suite passes on all supported platforms,
  the private pilot is complete, documentation works from a clean checkout, and
  no release-blocking protocol defect remains.

Package versions and protocol versions are independent. ADRProof may release a
new package without changing the provider protocol. An incompatible protocol
change requires a new protocol identifier and schemas; it never silently changes
the meaning of v1. See [`docs/VERSIONING.md`](docs/VERSIONING.md).

### Acceptance criteria

An external provider cannot produce a current PASS unless it:

1. is explicitly configured and version-pinned;
2. exits successfully within its resource boundary;
3. returns the exact supported schema and matching identity;
4. declares every semantic input used by its facts;
5. emits only deterministic or authoritative provenance;
6. makes every completeness claim explicit through scoped coverage.

## Near-term delivery and CI adoption

The remaining 0.2 work is tracked below. A checked preparation item does not
imply that a consuming project has enabled CI or that stable has been released.

- [x] Configure broader macOS library/CLI coverage and Windows core regressions;
  isolate POSIX execution fixtures and keep Rust 1.98.0 as the tested minimum.
- [ ] Confirm the expanded platform jobs and CodeQL on the exact pushed commit;
  only then update the supported-platform claims.
- [x] Provide environment/reproducibility issue forms and an explicit compatible
  patch policy without changing frozen v1 contracts.
- [x] Provide beta-gate and isolated-pilot review templates.
- [x] Document a neutral CI adoption contract covering pins, rules, non-vacuity,
  fail-closed behavior, private evidence, promotion and rollback.
- [ ] Review branch protection using authenticated maintainer access. The
  unauthenticated API returned 401 on 2026-09-04, not a verified protection state.
- [ ] Create and populate the `0.2.0` milestone; the public milestone list was
  empty on 2026-09-04. Resolve release metadata during the maintainer handoff.
- [ ] Obtain consuming-controller approval of an exact repeat-pilot pin set,
  required clauses, coverage and negative controls; preserve historical locks.
- [ ] Execute the isolated repeat pilot and review a sanitized result.
- [ ] Complete the beta observation window and stable-release gates. The beta
  was published at 2026-08-31 20:17:59 UTC; fourteen days elapse on
  2026-09-14 at 20:17:59 UTC, not at the start of that day.
- [ ] Prepare the stable release candidate, then hand push/tag/publication to
  the maintainer following the release runbook.
- [ ] Obtain separate approval for a private shadow CI job; keep actual failed
  verification results visible without making the check required yet.
- [ ] Evaluate repeatability, rule-negative controls, artifact handling and
  resource budgets before proposing a required merge check.
- [ ] Treat any deployment use as a separate owner decision tied to the exact
  promoted artifact; architectural consistency is not release authorization.

See [`docs/CI_ADOPTION.md`](docs/CI_ADOPTION.md) for the adoption contract and
[`docs/STABLE_0_2_RUNBOOK.md`](docs/STABLE_0_2_RUNBOOK.md) for the stable gate.
CI preparation and approved manual pilots may proceed during beta observation;
neither requires an incompatible protocol/report extension.

## Later candidates

- a separately versioned provider-check report that can expose semantic input
  fingerprints and a path/runtime-independent semantic digest without changing
  report v1;
- provider conformance kits and a stable SDK surface;
- additional proof backends through similarly versioned process protocols;
- richer type-aware code facts without claiming a semantic refinement proof;
- portable evidence transparency and remote verification policies;
- projections of the proof graph into C4, Mermaid, PlantUML, or SysML views.

These are candidates, not commitments. New authority claims require a normative
ADR and executable tests before they enter a release.

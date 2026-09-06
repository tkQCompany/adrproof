# ADRProof roadmap

ADRProof evolves as a deterministic meta-verifier. It standardizes the boundary
between project intent, facts, specialized verification tools, and immutable
evidence; it does not attempt to replace those tools with one universal logic.

## Current product direction

The [current framework plan](AKTUALNY_PLAN_RAMOWY.md) is the implementation
compass for the accepted architect-facing recommendations. ADRP-0007/0008/0009
define reviewed requirement/formalization links, a coherent workflow with bounded
external feedback, and long-term product maturity. Their target capabilities are
not implemented merely by accepting the decisions. Read the plan's baseline and
checkpoints before choosing the next slice.

The first stable product target is **1.0**, after sustained usefulness in multiple
maintainer projects and mature specification languages. Developmental 0.x
releases are not a declaration of whole-product stability. No release date,
community participation or fixed waiting period is required.

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
- **stable product (1.0)**: sustained use across real changes in multiple
  maintainer projects demonstrates practical
  value, and the syntax and semantics of the supported specification languages
  are mature enough for a compatibility commitment. The portable conformance
  suite must also pass on all supported platforms, a repeated private pilot must
  pass, documentation must work from a clean checkout, and no release blocker
  may remain. Technical readiness alone does not establish product maturity.

There is no calendar deadline or minimum waiting period for stable promotion.
Elapsed time and absence of community reports are not acceptance evidence.
Assessment relies on maintainer experience and feedback from approved consuming
project integrations: useful findings, missed violations, false alarms,
specification limitations, and the cost of maintaining rules across changes.
Specification-language maturity is distinct from the already frozen provider
protocol and report contracts; those compatibility commitments remain in force.

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

Compatible 0.2 maintenance and integration preparation are tracked below. New
product capability follows the framework plan with separately versioned contracts
where required. A checked preparation item does not
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
- [ ] Resolve release metadata and the appropriate milestone during a separately
  approved release handoff; there is no automatic stable 0.2 promotion.
- [ ] Obtain consuming-controller approval of an exact repeat-pilot pin set,
  required clauses, coverage and negative controls; preserve historical locks.
- [ ] Execute the isolated repeat pilot and review a sanitized result.
- [ ] Accumulate and review sustained-use evidence, including consuming-project
  CI feedback and specification-language limitations, before deciding maturity.
- [ ] Only after the maintainer accepts the multi-project maturity assessment,
  define and review the 1.0 release-specific procedure and technical gates;
  push/tag/publication remain maintainer actions.
- [ ] Obtain separate approval for a private shadow CI job; keep actual failed
  verification results visible without making the check required yet.
- [ ] Evaluate repeatability, rule-negative controls, artifact handling and
  resource budgets before proposing a required merge check.
- [ ] Treat any deployment use as a separate owner decision tied to the exact
  promoted artifact; architectural consistency is not release authorization.

See [`docs/CI_ADOPTION.md`](docs/CI_ADOPTION.md) for the adoption contract and
the [release checklist](docs/RELEASE_CHECKLIST.md) for technical controls.
The [old 0.2 runbook](docs/STABLE_0_2_RUNBOOK.md) is historical and inactive.
Development, approved manual pilots and separately approved CI adoption may
proceed during prerelease use; none requires waiting for a stable label or
changing the frozen protocol/report contracts.

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

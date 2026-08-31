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

Status: in development as package version `0.2.0-alpha.1`.

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
- [ ] publish reproducible provider conformance fixtures for other languages;
- [x] add an explicit `provider check` conformance command;
- [ ] run a private cross-project pilot without adding customer-specific code to
  the public repository.

### Acceptance criteria

An external provider cannot produce a current PASS unless it:

1. is explicitly configured and version-pinned;
2. exits successfully within its resource boundary;
3. returns the exact supported schema and matching identity;
4. declares every semantic input used by its facts;
5. emits only deterministic or authoritative provenance;
6. makes every completeness claim explicit through scoped coverage.

## Later candidates

- provider conformance kits and a stable SDK surface;
- additional proof backends through similarly versioned process protocols;
- richer type-aware code facts without claiming a semantic refinement proof;
- portable evidence transparency and remote verification policies;
- projections of the proof graph into C4, Mermaid, PlantUML, or SysML views.

These are candidates, not commitments. New authority claims require a normative
ADR and executable tests before they enter a release.

# Minimal architecture

The dependency direction is:

```text
Markdown ADRs -> ADRLogic AST -> lifecycle/type validation
             -> Project Intent Model + typed proof/dependency graph
             -> relational proof obligation -> stable SMT-LIB2
             -> pinned Z3 process
             -> SAT | UNSAT | UNKNOWN | timeout | failure
             -> stable text/JSON diagnostics + proof-ledger.json
```

All active clauses are asserted in one solver context. Pairwise checking is not a
valid substitute. Superseded and deprecated decisions stay queryable as history,
but only accepted (and, by policy, proposed when enabled) decisions surviving
lifecycle resolution contribute clauses.

ADRLogic is a frontend, not the core IR. Cargo metadata and future artifact
frontends add independently-provenanced nodes and facts to the Project Intent
Model. Specialized proof artifacts may remain opaque.

The core API separates `ConstraintBackend`, `CodeFactProvider`,
`RustProofProvider`, and `TemporalProofProvider`. Provider results carry capability,
tool identity, assumptions/bounds, status, artifacts and provenance. No adapter may
upgrade `UNKNOWN`, `UNVERIFIED`, `STALE`, or execution errors to PASS. Future time/version/feature predicates
belong in an explicit evaluation context used to compute `EffectiveSpecification`.

## Cargo fact semantics

`CargoMetadataProvider` executes `cargo metadata --format-version 1 --no-deps
--offline`.
It emits deterministic `package`, `workspace_member`, and
`declares_direct_dependency(source, actual_package)` facts. Path, registry and git
declarations are visible without resolving or downloading dependencies. Facts
retain normal/dev/build kind, optionality, target condition, local alias, actual
package identity, and source kind. Rename cannot hide the actual package.

The provider declares CLOSED coverage only for normal, non-optional,
unconditional manifest declarations, across path/registry/git sources. Only this
slice enters Z3 and receives explicit negative facts. Dev/build, optional,
target-conditioned and feature-activation semantics remain represented attributes
or open/unsupported domains; their absence is never interpreted as false. Without
the matching coverage claim, an otherwise SAT obligation using this provider-owned
relation becomes UNVERIFIED, never PASS. No transitive edge is inferred and Rust
source is not parsed.

## Evidence validity

Immutable evidence records a proof-obligation ID, backend identity/version, configuration
fingerprint, ordered content hashes of ADR and Cargo manifest inputs, the generated
obligation hash, historic result, current validity, timestamp, and diagnostics.
Files are atomically appended under `.adrproof/evidence/<evidence-id>.json`.
Comparing current fingerprints with stored evidence yields PASS/FAIL only
when they match; otherwise current validity is STALE while the historic result is
preserved. Absence yields UNVERIFIED. Absolute checkout paths are normalized away.
Backend version and the semantic configuration (`timeout_ms` and core-minimization
flags) also participate in validity. Corrupt evidence fails the query safely. The
JSON proof ledger embeds the newest evidence while history remains append-only.

Exit codes are 0 for current SAT/PASS, 1 for UNSAT/FAIL, 2 for invalid input or I/O,
3 for UNKNOWN, 4 for timeout, 5 for solver failure, and 6 for fact-provider failure.
`STALE`, `UNVERIFIED`, and `ERROR` can never be mapped to exit code 0.

Filesystem roles and semantic path identity are defined in
[`PROJECT_SPEC_STATE_ROOTS.md`](PROJECT_SPEC_STATE_ROOTS.md). Project and
specification roots are inputs; generated artifacts live under the state root.
Fingerprints use `project:`/`spec:` root-relative identities rather than checkout
paths.

An SMT unsat core is only a conflicting subset, not necessarily minimal. The Z3
adapter requests Z3's core minimization and returns a stable conflict list, but does
not claim to enumerate every MUS. The backend boundary is deliberately compatible
with later deterministic deletion-based shrinking and MUS enumeration.

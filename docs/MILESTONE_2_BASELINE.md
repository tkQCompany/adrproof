# Milestone 2 baseline audit

Verified on 2026-08-15 before refactoring: all nine existing unit tests passed,
`cargo fmt --check` passed, Clippy with `-D warnings` passed, and the formal ADR
self-check returned SAT with Z3 4.13.4. The reported vertical slice and listed
documents/examples were present.

The architectural report overstated two boundaries rather than runtime behavior:
`Expr` and `EffectiveSpecification` still served simultaneously as parser output,
semantic IR, and backend input; and the version-1 ledger represented one Z3 run but
had no independent evidence validity/staleness model. This milestone corrects
those boundaries without changing the existing ADR syntax or SAT/UNSAT behavior.

The next audit found a soundness discrepancy: version 2 claimed a closed world for
direct dependencies while enumerating only local path dependencies. Registry and
git declarations could therefore be mistaken for absence. Version 3 replaces that
relation with explicitly covered manifest declarations across path, registry, and
git sources.

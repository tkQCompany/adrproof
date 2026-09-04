# Isolated CI pilot review — template

Status: **NOT EVALUATED**. This is a neutral template, not a report of a run.
Keep a filled detailed record in the private integration workspace. Publish only
an independently sanitized summary approved by the consuming project's owner.

## Private execution record

- Controller approval and bounded scope: TBD
- Exact verifier, target, specification, provider and harness pins/digests: TBD
- Baseline and candidate revisions, immutable input check: TBD
- Provider identities/versions, runtimes, solver and Rust versions: TBD
- Rule/clause IDs, nonempty expected artifacts and coverage: TBD
- What each rule proves, and explicitly does not prove: TBD
- Worker isolation, credentials/network policy, artifact access/retention: TBD
- Runtime/resource budget and observed measurements: TBD

## Acceptance matrix

| Control | Expected | Observed / private evidence reference |
| --- | --- | --- |
| Approved baseline, all intended rules selected | PASS | TBD |
| Known architectural violation per selected rule | FAIL | TBD |
| Empty/missing obligation or expected artifact | FAIL | TBD |
| Required evidence absent or stale | FAIL | TBD |
| Undeclared/missing provider input | FAIL | TBD |
| Wrong provider identity/version | FAIL | TBD |
| Malformed report or timeout | FAIL | TBD |
| Missing required report/artifact upload | FAIL | TBD |
| Repeat unchanged inputs and relocate checkout | Same semantic result | TBD |
| Concurrent runs | Isolated state, no cross-run evidence | TBD |
| Source checkout integrity before/after | Unchanged | TBD |

Distinguish provider-conformance PASS from architectural gate PASS. Record the
actual exit status even in shadow mode. Use NOT APPLICABLE only with a scoped
explanation and controller approval, not to bypass a required control.

## Sanitized public summary

- Public ADRProof full commit: TBD
- Protocol/report identifiers and tested generic behaviors: TBD
- Bounded result, known limitations and neutral reproductions: TBD
- Owner's sanitization approval: TBD

Omit consuming-product names, private commits/paths, source, configuration,
patches, raw provenance and evidence. Hashes do not make private data public-safe.

## Integration decision (private)

- Remain disabled / repeat manual pilot / request shadow approval: TBD
- Follow-up items and responsible owners: TBD
- Required merge check and deployment: **NOT AUTHORIZED** by this template

See [`../CI_ADOPTION.md`](../CI_ADOPTION.md) for promotion and rollback rules.

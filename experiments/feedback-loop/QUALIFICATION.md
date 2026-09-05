# Qualification record — 2026-09-05

**Status: fixture qualification passed; A/B/C repair experiment NOT RUN.**

Baseline ADRProof: `e6914b742bf1f5ddb08eecb368e2413807fd14c0`.
The verifier core and roadmap were not changed. Rust 1.98.0 and real Z3 4.13.4
were used. Binary, fixture, specification and harness SHA-256 values are recorded
in the local run's `protocol.json`, `versions.json`, per-case result files and
`artifact-hashes.json`. Raw output is retained under the ignored `dist/` directory.

## Observed controls

| Input/control | Observed outcome |
| --- | --- |
| Four initial violations: direct, alias, table, code-use | All compile/test successfully; all ADRProof UNSAT and independent rejection |
| Four manually authored valid repairs | All compile/test successfully; all ADRProof SAT and independent acceptance |
| Architecture-clean implementation returning empty strings | ADRProof SAT; functional tests FAIL; rejected |
| Comment-only relevant manifest change after PASS | Status STALE |
| Required relation with no observed fact or coverage | UNVERIFIED, exit 3 |
| Missing solver executable | Error, exit 5, no success JSON |
| Removal of a package from the workspace | Independent integrity rejection |
| Detached child after leader exit | No delayed child marker; launcher completed promptly |
| Detached child on timeout | Limit exceeded, nonzero exit, no delayed child marker |

The third row is an intentional demonstration of why architectural PASS is
not total correctness. It is not an LLM repair and not a false-PASS rate estimate.
The original inputs were read-only during compiler/verifier execution. The
comment mutation used a separate copy, preserving the other case snapshots.

## Model transport probe (not a scored session)

Codex CLI 0.153.2, requested model `gpt-5.6-sol`, reasoning `high`, ephemeral
execution with user configuration ignored and major execution tools disabled,
returned `READY` without tool calls. Reported usage:

- input: 10,078 tokens;
- cached input: 0;
- output: 5 tokens;
- reasoning output: 0 tokens.

The originally suggested 8,000-token session limit is therefore not viable with
this transport's context overhead. The user was asked to choose a revised
budget/full matrix, an initial nine-session pilot, or qualification only. No
repair-model calls were made while that choice was pending. No monetary price
is inferred from subscription-token usage.

## Still required before scoring

- Accepted model-call budget and a frozen run protocol matching that choice.
- Qualification of structured patch transport, leakage prevention and identical
  stopping rules in A/B/C; the one successful READY probe does not establish this.
- Candidate-execution resource qualification. Current per-process rlimits and
  namespace cleanup are not aggregate cgroup memory/pids enforcement.
- Independent A/B/C runs and evaluation of their actual proposed patches.

Do not treat these preparation results as evidence that detailed ADRProof
feedback improves LLM repair success. The hypothesis remains untested.

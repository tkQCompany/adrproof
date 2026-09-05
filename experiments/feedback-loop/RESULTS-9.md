# Feedback pilot result — 2026-09-05

**Completed: nine real model sessions. Outcome: ceiling effect, inconclusive
for the benefit of ADRProof feedback. No verifier-core or roadmap changes.**

The [frozen nine-session protocol](PILOT-9.md) was written before repair calls.
All three arms repaired the same code-use violation in all three repetitions,
on their first proposal. No replacement runs, human patch edits, model tool
calls, protocol failures, functional regressions or repair-objective false PASS
were observed. The independent file/TOML oracle and unchanged functional tests
accepted every candidate, and real ADRProof/Z3 returned fresh SAT/PASS.

| Feedback | Accepted | Proposals per session | Regressions / false PASS | Input tokens, total | Output tokens, total |
| --- | --- | --- | --- | --- | --- |
| A: compiler/tests | 3/3 | 1 | 0 / 0 | 33,771 | 296 |
| B: plus verdict/status | 3/3 | 1 | 0 / 0 | 33,830 | 287 |
| C: plus detailed diagnostics | 3/3 | 1 | 0 / 0 | 51,481 | 317 |

Every actual proposal removed the direct driver dependency and changed
`domain::company_name` to call `repository::company_name`. Neither the frozen
specification nor any protected file was changed. This is a manifest-boundary
check, not a proof of arbitrary Rust control flow or functional correctness.

## Repetitions and observed cost

| Order | Arm / repeat | Accepted / proposals | Session seconds | Input / output tokens |
| --- | --- | --- | --- | --- |
| 1 | A / 1 | yes / 1 | 5.70 | 11,263 / 114 |
| 2 | B / 1 | yes / 1 | 4.45 | 11,278 / 74 |
| 3 | C / 1 | yes / 1 | 5.64 | 17,161 / 123 |
| 4 | B / 2 | yes / 1 | 5.58 | 11,276 / 108 |
| 5 | C / 2 | yes / 1 | 4.93 | 17,159 / 74 |
| 6 | A / 2 | yes / 1 | 5.40 | 11,255 / 108 |
| 7 | C / 3 | yes / 1 | 6.26 | 17,161 / 120 |
| 8 | A / 3 | yes / 1 | 4.45 | 11,253 / 74 |
| 9 | B / 3 | yes / 1 | 7.44 | 11,276 / 105 |

Total reported usage: 119,082 input tokens, 900 output tokens, zero cached input
and zero cache-write input. The separate `reasoning_output_tokens` field totals
222 (A: 70, B: 61, C: 91); it is reported as received, not added to output tokens.
Monetary cost is unavailable for the subscription transport. These figures
exclude earlier readiness probes and the coordinating conversation.

Scored sessions total 49.85 seconds: 47.17 seconds in model transport and 2.54
seconds in lock/compiler/verifier subprocesses, plus harness overhead. Baseline
and runner qualification time is excluded. Median session time A/B/C was
5.40 / 5.58 / 5.64 seconds; nine short calls cannot establish a latency effect.
C used approximately 52% more input tokens than A, without improving this score.
Requested model/effort was `gpt-5.6-sol` / `high`, CLI 0.153.2 throughout; no
immutable backend model snapshot was attested by the transport.

## What this establishes — and does not

Existing CLI output is usable by a separate, bounded proposal/evaluation
harness without extending ADRProof. In C, the initial check identified ADR-0100
C1 at `spec:architecture.md:9` and the contradicting manifest fact at
`project:domain/Cargo.toml:8`; the existing model supplied provenance and qualified
closed coverage, and the ledger supplied evidence identities/fingerprints.
No new diagnostic abstraction was needed for this fixture.

It does **not** establish that the feedback improves repair effectiveness. The
requirement and tiny source already made the fix obvious in A. The preregistered
practical improvement criterion was not met. This weakens the case for spending
extra diagnostic tokens on this simple task, not the general hypothesis.

No second/third repair iteration was needed, so the live retry path and recovery
from unsuccessful model proposals were not exercised. Three repeats of one
fixture are not three distinct architecture problems or statistical confirmation.
Tests cover four visible input/output examples, not comprehensive functionality
or adversarial safety. Zero observed false PASS is not a universal guarantee.

**Recommendation:** keep the current architecture and roadmap. Do not build a
general agent platform or automatically run the proposed 36-session matrix.
If another experiment is separately approved, select several less obvious,
independently scored violations first and freeze their protocol before calls.
The current result is useful as a transport/evaluation feasibility pilot only.

## Audit and local evidence

Four protocol tests passed (structured output/usage, tool rejection, write and
dependency allowlists, feedback separation). Actual kernel settings were 2 GiB
`memory.max`, zero `memory.swap.max`, and 128 `pids.max`; the contained fork probe
was denied after 125 children and namespace/service cleanup completed promptly.
Read-only project and 512 MiB / 64 MiB state/tmpfs capacities were checked.
This is not a production runner security qualification. Earlier handcrafted
negative controls are recorded separately in [QUALIFICATION.md](QUALIFICATION.md).

A post-run audit checked the artifact manifest, frozen script/spec/source
hashes, exact model-proposal application, every initial/continued prompt's
allowed feedback, absence of tool events, actual usage, and independent scoring.
The audit passed; it reconciles artifacts and does not introduce new criteria.
The unchanged ADRProof regression suite also passed: 116 tests across all targets
with `cargo test --locked --offline --all-targets`.

Local ignored artifact directory: `dist/feedback-loop-pilot-nine-20260905/`.
Contains `protocol.json`, versions/limits/isolation checks, all nine prompts and
raw event streams, proposed files, compiler/verifier/evidence outputs,
`summary.json`, `artifact-hashes.json`, and post-run `audit.json`. Raw transcripts
are not added to Git. Hashes are local integrity anchors, not remote attestations.

- Verifier source: `e6914b742bf1f5ddb08eecb368e2413807fd14c0`.
- ADRProof binary SHA-256: `74ef2afbd6883b782322eab37daef9b9e65b446a89319fa36223600fc1f112e0`.
- Z3 4.13.4 binary SHA-256: `e0385660ab6f1314049376c6188e70ab91692cdca4680b7b1cb42cac258ea836`.
- Protocol JSON SHA-256: `06a7c567bba2869e38ad799ccd4dff02ae464b7d7020a88472aa83421e3a13d1`.
- Artifact manifest SHA-256: `0fb697b870347d7be24ef1b1e7a7d7a65bfacaea2e883cf505ab0559b4d0a96a`.

Recheck locally, without model calls:

```sh
python3 -m unittest discover -s experiments/feedback-loop -p 'test_*.py' -v
python3 experiments/feedback-loop/audit_pilot.py dist/feedback-loop-pilot-nine-20260905
```

`run_pilot.py` accepts the same binary/toolchain/output arguments as `qualify.py`.
Without `--execute-nine`, it runs only readiness checks; the flag enables nine
model sessions and therefore requires an explicitly accepted execution budget.
Use on a local Linux host with Bubblewrap and a working user systemd manager,
not in public CI with personal account credentials. OpenAI Docs informed the
JSONL/structured-output transport; see the official source in [PILOT-9.md](PILOT-9.md).

# Bounded architecture-feedback experiment

This is a local, neutral experiment, not a product feature, CI integration,
or a claim that a verifier improves a model's intelligence. It does not change
the ADRProof roadmap. Do not publish model transcripts without review.

Current record: the separately approved [nine-session pilot](PILOT-9.md) is
complete; see [actual results and limitations](RESULTS-9.md). All arms scored
3/3 on the first attempt: a ceiling effect, not evidence of feedback benefit.
The original broader proposal and historical preparation stages below remain
for context; they do not authorize additional model runs.

## Preregistered design

- Verifier baseline: `e6914b742bf1f5ddb08eecb368e2413807fd14c0`.
- One frozen rule, derived from the public Rust workspace example: `domain`
  must not directly depend on `fake_sqlx`.
- Four initial violations: direct dependency, renamed dependency, dependency
  table amid legal changes, and source code using the prohibited dependency.
- A receives compiler/test feedback; B additionally receives only the verifier
  outcome; C additionally receives recorded conflicts, facts and coverage.
- Every arm receives the same requirement, specification and initial source for
  its case. No prior conversation or cross-session memory is admitted.
- Proposed full matrix: 4 cases × 3 arms × 3 independent repetitions = 36 runs.
- Maximum 3 patch proposals and 10 minutes per run. Abort on tool use outside
  the experimental transport, protected-file edits or infrastructure failures.
- The initially suggested 8,000-token cap is **not qualified**: a Codex CLI
  transport probe alone used 10,078 input tokens and 5 output tokens. Full LLM
  execution requires a separately accepted budget. Never label a completed
  call's usage as a hard generation-time token limit.

## Independent acceptance

The evaluator parses TOML independently of ADRProof. It requires the domain
package metadata to remain unchanged, repository to remain a normal mandatory
dependency, and every extra dependency to be an explicitly approved fixture
dependency. Moving the driver to dev/build/optional/target dependencies or
changing its alias is not an accepted repair.

Only `domain/Cargo.toml` and `domain/src/lib.rs` may be changed. The evaluator
preserves the specification, package layout, adapters and acceptance tests.
Tests assert behavior for whitespace, mixed case, empty input and multiple
names; deleting the public operation is not a repair. The code-use case needs
a code change as well as a manifest change.

These fixtures are deliberately small. Three cases may have a ceiling effect:
all arms can infer the fix from the requirement alone. A 100% score everywhere
would not demonstrate a benefit from ADRProof feedback.

## Qualification before LLM runs

`qualify.py` uses a real Z3 executable supplied by the operator. It does not use
the unit tests' fake solver backend. All target executions run in disposable
Bubblewrap PID/network/filesystem namespaces without credentials or access to
the host project. The project is read-only during compilation and verification;
only lock generation gets a writable disposable copy. A timeout kills the
outer process group, and namespace teardown terminates its contained processes.

Per-process virtual memory, CPU time, file size and captured output are bounded.
These are **not aggregate cgroup memory/pids limits** and do not establish the
resource qualification of a production integration runner. LLM runs remain
separately gated; this script only executes known, locally constructed fixtures.

Run from the repository root:

```sh
python3 experiments/feedback-loop/qualify.py \
  --adrproof target/debug/adrproof --z3 /path/to/z3 \
  --toolchain /path/to/pinned-rust-toolchain \
  --output dist/feedback-loop-qualification
```

The output directory must not exist. Outputs include source/spec/script hashes,
binary hashes and tool versions, actual exit codes, verifier state, independent
acceptance results, and a manifest linking the artifacts. `summary.json`
distinguishes qualification from LLM results. No model repairs or A/B/C scores
may be inferred from qualification results.

## Analysis rules for a later accepted run

Record repair success, behavioral regressions, false PASS, iterations, wall time,
input/output/cached tokens, tool time and unavailable monetary costs explicitly.
Count a repair only when both independent acceptance and fresh ADRProof pass.
Do not reveal independent architecture-oracle diagnostics as feedback to A/B/C;
that oracle is for scoring, not repair hints. Visible test feedback is identical
across arms. All arms use the same stopping rule.

For 12 observations per arm, a practical exploratory criterion is at least three
additional accepted repairs in C versus A without more regressions or false PASS.
C versus B determines whether detail helps beyond a verdict. Report outcomes per
case and repeat; repetitions on one case are not independent architecture tasks.
Equal near-perfect results are inconclusive, not evidence of improvement.

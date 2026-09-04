# Adopting ADRProof in CI

This is an integration review contract, not an installed workflow or permission
to enable a consuming project's CI. Project-specific configuration, fixtures,
lock files, and evidence belong in that project's private integration workspace.
Protocol v1 is sufficient for an initial pilot; a future semantic-digest report
is not a prerequisite.

## What is being gated?

`provider check --json` tests configured provider conformance. It does not check
architectural obligations. `check --json` combines the effective specification
and extracted facts and reports consistency. SAT means those inputs are
consistent, not that an arbitrary implementation satisfies every prose ADR.

Before approving a gate, enumerate the required ADR/clause IDs, expected fact
relations, relevant artifacts, evidence obligations, and applicable coverage
scopes. Reject missing/empty selections in the consuming harness. A successful
process exit with no intended obligation checked is not acceptable coverage.
Demonstrate a negative fixture for each proposed rule and explain its authority:
a declared manifest pattern is not a resolved set of workspace members.

`status`, `facts`, `explain`, and `impact` are inspection commands, not substitutes
for gate verdicts. In particular, a successful query exit is not evidence PASS.

## Reproducible inputs and safe execution

The private run manifest must record:

- full ADRProof commit and tree, Cargo lock digest, Rust version, binary digest;
- full target-project and specification commits/trees, with no dirty inputs;
- provider source revision, executable digest, configured identity/version and
  declared input set; retain provider runtime versions too;
- selected obligation IDs, harness/policy revision, and all relevant tool
  versions, including the configured Z3 executable;
- job/run identity, input digests, command exit codes, report and artifact digests.

Build ADRProof from the approved source commit with `cargo build --locked
--release`; no crates.io publication or floating dependency on a branch is
needed. Never silently substitute another target HEAD when an approved object
is unavailable. Source archives can be verified using
[`SOURCE_RELEASES.md`](SOURCE_RELEASES.md).

Use an isolated target checkout and a separate specification checkout. Allocate
a fresh external state directory per run, with no shared mutable evidence across
concurrent jobs. Pin the baseline and candidate independently; compare them
using the same verifier, specification, provider, and policy. Do not silently
change the rules while attributing a difference to a code change.

Providers execute trusted code; ADRProof is not a sandbox. Use disposable,
least-privilege workers without deployment credentials or unnecessary network
access. Do not run untrusted pull-request providers with privileged credentials.
Enforce read-only inputs at the worker boundary where required, and verify that
source trees remain unchanged. See [`TRUST_MODEL.md`](TRUST_MODEL.md).

## Command boundary

After resolving and validating every path/pin above, the harness invokes:

```sh
"$ADRPROOF_BIN" provider check --json \
  --project-root "$PROJECT_ROOT" --spec-root "$SPEC_ROOT" \
  --state-root "$PROVIDER_STATE"
"$ADRPROOF_BIN" check --json \
  --project-root "$PROJECT_ROOT" --spec-root "$SPEC_ROOT" \
  --state-root "$CHECK_STATE"
```

This illustrates command arguments, not a complete fail-closed shell wrapper.
The harness must capture stdout, stderr and exit status for **each** invocation,
stop on failure, and validate the report and obligation selection. It must also
produce any required scenario/native/model evidence before evaluating the final
gate, using the documented evidence commands and freshness requirements. Missing,
stale, unknown, malformed, timeout or unverified required results cannot pass.

`check` returns 0 for SAT, 1 for UNSAT, 2 for invalid input, 3 for unknown or
unverified, 4 for timeout, and 5 for solver failure. Invocation/I/O failures are
also failures, regardless of whether a structured report was produced. Preserve
the real result through pipes, artifact upload and cleanup; neither `|| true`
nor a later successful command may turn a failed verification into success.

The existing general check JSON is not the versioned provider-check report.
Pin the CLI revision and test any harness parsing against that revision. Do not
assume all reports have a shared schema/version field.

## Rollout and acceptance

| Stage | Entry condition | Meaning and exit evidence |
| --- | --- | --- |
| Disabled | Current default until owner approval | No consuming CI changes |
| Isolated manual pilot | Approved pins, rules, trusted runner | Baseline passes; known violations, missing input and stale evidence fail |
| Shadow job | Controller approves workflow and data handling | Real failures remain visible, but the job is not a required merge/deploy check |
| Required merge check | Agreed repeatability, runtime budget, no unexplained false passes/failures | Exact check name is required by branch policy; bypass process is documented |
| Deployment input | Separate deployment-owner decision | Verify accepted evidence belongs to the exact artifact being promoted; do not infer release authorization from SAT |

For promotion, test negative controls for undeclared/missing input, wrong provider
identity/version, malformed output, timeout, lost report, failed artifact upload,
and an actual rule violation. Repeat on an unchanged pin set and a relocated
checkout. Set and measure time/resource budgets; do not invent universal limits.
Use [`releases/PILOT_REVIEW_TEMPLATE.md`](releases/PILOT_REVIEW_TEMPLATE.md).

Keep raw reports private by default: paths, source fragments, provenance and
solver diagnostics may identify the consuming product. Define who may read
artifacts, what must be retained, and a bounded retention period before enabling
uploads. Public summaries need owner-approved sanitization; digests are integrity
checks, not anonymization. Archive/report availability required by the gate must
fail closed if its upload fails.

Rollback restores the previous approved pins or explicitly disables the required
check with a recorded owner decision. It does not rewrite historical failures
as PASS, reuse stale evidence, or alter a release silently. Push, release and
deployment authority remain with the relevant human maintainers.

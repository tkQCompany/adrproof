# Protected required set and read-only gate (experimental)

This P36/P37 slice composes existing inventory, formalization review and proof
freshness assessments. It is not an agent orchestrator, a solver invocation, or
automatic approval to enable a consuming project's CI.

## Trust boundary

`gate prepare` emits a **draft** required set. An authorized human separately
reviews its scope, changes it to `approved`, supplies a `human_attestation`
reviewer and rationale, and pins the SHA-256 of the resulting **raw file bytes**
in a protected integration configuration. `gate evaluate` requires both that
file and the independently supplied `--baseline-sha256`. Computing this argument
from the candidate file in the same untrusted job defeats the protection.

Changing a baseline, including removing obligations, narrowing mappings or
replacing a review head, requires separate approval and a new protected pin.
The gate never approves, imports, rewrites or repairs a baseline or evidence.
Reviewer names are unsigned attestations, not authenticated identities. The
runner, gate binary, pin, review store and evidence writers must be outside the
repair agent's control. Hashes do not defeat a malicious authorized writer.
Use an immutable checkout/snapshot during assessment; this is not atomic with
concurrent edits. State must be physically disjoint from project and spec roots.
Native inputs must remain under their declared roots. Native-input and migration
trees must contain plain files/directories without child aliases and have depth
at most 128. Unsupported entries or interrupted evidence-store writes fail closed.

## Required set

The independent `adrproof-required-set-v1alpha1` format freezes all inventory/ADR
byte fingerprints, all active constraint IDs, and exact current review heads.
There must be at least one active reviewed normative requirement and constraint.
Whole-file invalidation is intentionally conservative: even unrelated prose
changes require formalization review and a separately approved baseline update.
Keeping an old, otherwise current review after deleting a newer head cannot
satisfy the pinned set.

Exactly one global `PO:project-consistency` check is always required, with an
explicit full backend version and timeout configuration. All effective clauses
participate; individual clauses cannot be deselected. Optional `--native-test ID`
checks pin the existing definition's semantics (including input exclusions,
minimum counts, required tests and authority). Their latest imported evidence
must be current and PASS. Other scenario/model/correspondence checks are **not**
implicitly included. Include their independent gates separately.

Required coverage records retain relation and scope. Each must currently be
Closed. Additionally, partial coverage for any relation used by the global
obligation and unresolved closed-world absence prevent PASS. This is deliberately
more conservative than a positive-fact-only proof under Partial coverage.

## Commands

```sh
adrproof gate prepare --project-root PROJECT --spec-root SPEC --state-root STATE \
  --backend-version 'Z3 version ...' --timeout-ms 5000 [--native-test ID] --json
adrproof gate evaluate --project-root PROJECT --spec-root SPEC --state-root STATE \
  --baseline APPROVED.json --baseline-sha256 PROTECTED_SHA256 --json
```

Preparation requires current reviews but not passing proof evidence. Output goes
only to stdout, not to state. Keep the baseline outside the inventoried Markdown
tree. Evaluation reports `adrproof-gate-report-v1alpha1`: PASS/0 only when every
required check and review is current and passing; FAIL/1 for a current refutation;
INCOMPLETE/3 for stale, missing, unknown, unverified, scope/review drift or partial
coverage; ERROR/2 for malformed input, wrong pin, unsupported execution or I/O.
ERROR takes precedence over FAIL, and FAIL over INCOMPLETE. Individual results
retain freshness and evidence identity; no fallback to older passing evidence.

This first implementation supports in-process ADRLogic and static SQL facts,
plus imported native-test results. Cargo manifests or configured external
providers are explicitly unsupported: evaluating their freshness today may run
subprocesses. Neither Cargo, external providers, tests nor Z3 run in this gate.
Existing commands and provider/report/ledger formats are unchanged.

PASS means only consistency of the selected formal obligations with current
available evidence, plus the explicitly selected imported checks. It is not
correctness of prose, a repair, a proof of all program behavior, or an independent
replay of proof certificates. Functional/security controls remain independent.

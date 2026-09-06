# Executed-fact snapshots and read-only admission (experimental)

This implements the next bounded P36/P37 adapter slice. It does not install a
consumer workflow or provide an operating-system sandbox.
The separate [neutral Linux producer harness](ISOLATED_PRODUCER.md) now provides
a Bubblewrap recipe and measured local qualification. Snapshot capture itself
still has no sandbox, and a real consumer must approve its concrete host/profile.

## Two different authorities

`snapshot capture` **executes** the existing Cargo/external fact providers. Run it
only in an externally isolated, trusted producer against an immutable source
export. That producer must protect the ADRProof binary, Cargo/runtime tools,
environment, network policy and writable locations. The command itself is not a
sandbox and cannot stop a malicious provider writing elsewhere. Do not invoke it
on an active developer checkout with untrusted provider code.

The caller supplies `--producer-context-sha256`: an externally verified digest
of that execution profile (including ADRProof binary and provider runtime/tool
versions, environment/ambient-input policy and isolation setup). This is an
attestation, not automatic environment discovery. The producer must reject or
requalify dependencies on clock, network, undeclared external files or mutable
environment. A dishonest producer can still forge evidence.
State is an operational output location, not a permitted hidden fact input. The
profile must enforce or qualify that restriction; tree equality cannot discover
an executable's dependency on old state contents.
The profile must also qualify root-relocation invariance and ensure that mutable
project files are input data, never dynamically loaded provider implementation.
Transitive provider code/dependencies must reside in the pinned specification
tree or protected runtime profile; entry-point hashing cannot discover them.

After successful capture, the trusted integration transfers the snapshot's raw
file SHA-256 through a protected job output/artifact channel. It must not compute
the admission pin from a candidate-supplied snapshot. Pins are per capture; this
does not require human approval per code edit. Human approval remains necessary
for a changed required set or execution profile.

## What is checked rather than merely trusted

Capture fingerprints **all** project/specification files and directories before
and after extraction, retaining paths, contents and permission bits. An addition,
deletion, permission change or modification rejects capture. This double scan is
not an atomic snapshot and cannot detect a change-and-restore race; read-only
mounts/immutable exports remain a producer responsibility.

Only plain source-export trees are supported: no child symlinks, special files,
`.git`, `target` or `.adrproof` entries; no user-selectable exclusion list. State
is physically disjoint. Limits are 128 directory levels, 100,000 entries and
256 MiB of input bytes. Export without build artifacts before running Cargo;
pre-generate a lockfile outside capture if Cargo would create one. These limits
are fail-closed safety bounds, not a scalability claim.

All semantic inputs returned by existing extraction must resolve inside these
trees and appear in the tree manifest. External path dependencies are unsupported.
The snapshot stores the normalized Project Intent Model, input identities and
fingerprints, preserving facts, provenance and scoped coverage. Its independent
schema is `adrproof-fact-snapshot-v1alpha1`; provider protocol/report v1 is unchanged.
The snapshot's extraction policy covers the complete spec tree plus configured
provider entry points/configuration. The required set pins this policy, including
permissions. A changed provider or spec-side script therefore requires separate
baseline approval even after a fresh snapshot and passing proof are produced.

Read-only validation checks the independent pin, schema, context, complete current
tree and semantic input fingerprints. It never executes Cargo, providers, tests
or a solver. A missing/changed input (including a newly added workspace member or
configuration file) is an explicit error, not a cache hit or PASS. The gate also
reuses its existing review, scope, coverage and latest-proof assessments: a valid
fact snapshot alone is not architectural PASS. Generated SMT and relevant proof
fingerprints must match current proof evidence; contradictory later captures do
not reuse an older PASS with a different model.
Captured constraints and decisions are additionally checked against a fresh,
in-process lowering of the current specification; an empty or substituted
obligation model cannot bypass this check by receiving a new transport pin.

## Separate opt-in gate version

Legacy `gate prepare/evaluate` and their v1alpha1 formats retain the in-process
only contract. New `gate prepare-snapshot/evaluate-snapshot` use
`adrproof-required-set-v1alpha2` and `adrproof-gate-report-v1alpha2`. Their required
set additionally pins the producer context. A snapshot is mandatory on every
invocation, with `--snapshot PATH --snapshot-sha256 PROTECTED_SHA256`. Missing
snapshots, wrong versions, incompatible profiles and attempts to use the legacy
path fail closed. Old required sets are not silently upgraded.

```sh
# Trusted isolated producer only; output must be outside both source trees:
adrproof snapshot capture --project-root PROJECT --spec-root SPEC --state-root STATE \
  --producer-context-sha256 PROFILE_SHA256 --json > SNAPSHOT.json
# Read-only; both draft preparation and admission validate the pinned snapshot:
adrproof gate prepare-snapshot --project-root PROJECT --spec-root SPEC --state-root STATE \
  --backend-version 'Z3 version ...' --timeout-ms 5000 \
  --snapshot SNAPSHOT.json --snapshot-sha256 PROTECTED_SNAPSHOT_SHA256 --json
adrproof gate evaluate-snapshot --project-root PROJECT --spec-root SPEC --state-root STATE \
  --baseline APPROVED.json --baseline-sha256 PROTECTED_BASELINE_SHA256 \
  --snapshot SNAPSHOT.json --snapshot-sha256 PROTECTED_SNAPSHOT_SHA256 --json
```

Capture emits no proof and writes no state itself (invoked providers may write).
It never emits a snapshot after a failed provider, changed input tree or invalid
semantic input. Errors use exit 2; successful capture/draft preparation uses 0,
which is **not** proof PASS. Gate exits retain PASS/0, FAIL/1, INCOMPLETE/3 and
ERROR/2. The gate report identifies the snapshot and declared producer context.

Qualification in this slice uses neutral disposable source trees, actual Cargo
metadata and a tiny native external-provider fixture. This is not qualification
of a production container, every runtime, or any private project's CI boundary.

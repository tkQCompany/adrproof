# Neutral Linux producer recipe and review packet

This is an experimental **integration harness**, outside the Rust verifier core.
It does not enable a consumer workflow or approve a real specification. It uses
Bubblewrap with no fallback when namespace creation or a security option fails.

## Inputs and authority

Run `scripts/isolated_producer.py` using a trusted system Python in isolated mode
(`python3 -I`). The host supervisor, Python and its dependencies, kernel and
Bubblewrap must be protected from the candidate. The harness is not a defense
against a compromised host, kernel vulnerabilities, resource-exhaustion attacks,
malicious already-approved providers or side channels.

An operator supplies a prepared runtime filesystem containing `/tools/adrproof`,
`/tools/cargo`, `/tools/rustc`, `/usr/bin/python3`, their runtime dependencies and
empty mount points `/project`, `/spec`, `/state`, `/tmp`, `/proc`, `/dev`.
Also provide an empty plain `/probe.py` file for the qualification probe mount.
Runtime construction/installing packages is separate from admission. No downloads,
package installation or candidate build occurs in the producer. A neutral test
builder exists only to exercise the recipe with installed trusted tools; it is
not a reproducible cross-host runtime distribution.

`profile` records content/mode identities of the entire runtime, the supervisor,
probe, Python and Bubblewrap binaries, kernel release and fixed execution policy.
Review this descriptor and pin its raw SHA-256 outside candidate control. Profile
generation is not approval. Host dependency protection remains an external duty;
hashing Python's entry point does not authenticate all its loaded libraries.

`produce` requires independently expected profile, project-export and spec-export
digests plus a unique run ID. It rejects mismatches **before executing a provider**.
The spec export, provider implementation and transitive dependencies must be
approved independently of project-code repair. Mutable project files must be
treated only as data, not executable provider plugins. The full runtime/source
manifests are checked again after execution. No exclusions can weaken these pins.
Use immutable source/runtime staging: read-only bind mounts stop sandbox writes,
but do not stop a host process editing the backing files or a change/restore race.

## Enforced boundary

The runtime becomes a read-only root filesystem, with read-only `/project` and
`/spec` binds. No host root, home, credentials, old evidence, output directory or
container-engine socket is mounted. A private PID, IPC, UTS, user and network
namespace is required, capabilities are dropped, nested user namespaces disabled,
and a new session separates terminals. Only fixed non-secret environment values
are supplied. The hostname and source paths are fixed for relocation.

Fresh `/state` and `/tmp` tmpfs mounts, minimal `/dev` and private `/proc` are the
only writable/dynamic areas. Only snapshot stdout/stderr leave the namespace;
providers cannot open the supervisor's artifact directory by path. Wall time,
CPU, file-output and address-space limits bound a run; these do not replace host
cgroup/VM limits on aggregate memory, processes and disk usage. The command always
executes the fixed `adrproof snapshot capture`, never candidate command strings.

Clock, entropy, CPU/kernel observations and state created within a run remain
observable. The harness **does not prove** providers ignore these inputs. Approve
and test deterministic, relocation-independent provider semantics separately.
No network namespace is shared even if the host session cannot create one: fail
closed instead. Windows/macOS are unsupported by this harness.

## Protected artifact transfer

After successful execution and input rechecking, the host supervisor checks the
snapshot schema/context, writes a fresh output directory exclusively, and emits
a receipt binding run ID, source/spec/profile digests and raw snapshot SHA-256.
It prints the receipt digest for a protected job-output channel. Neither provider
stdout nor a candidate-provided receipt supplies its own trusted admission pin.

`verify-transfer` requires that independently transported receipt digest and the
expected run/source/spec/profile tuple; it rejects tampering and cross-run replay.
It emits the snapshot digest for `gate evaluate-snapshot`. This verifies transport,
not architecture: the gate must still validate current trees, approved policy,
reviews, coverage and proof evidence. Capture success is never called PASS.

## Operator sequence

```sh
python3 -I scripts/isolated_producer.py profile --runtime RUNTIME --bwrap /usr/bin/bwrap
python3 -I scripts/isolated_producer.py digest --tree PROJECT_EXPORT
python3 -I scripts/isolated_producer.py digest --tree SPEC_EXPORT
# Pins below must be supplied by protected orchestration after separate review.
python3 -I scripts/isolated_producer.py qualify --runtime RUNTIME --bwrap /usr/bin/bwrap \
  --profile PROFILE.json --profile-sha256 TRUSTED_PROFILE_SHA256
python3 -I scripts/isolated_producer.py produce --runtime RUNTIME --bwrap /usr/bin/bwrap \
  --profile PROFILE.json --profile-sha256 TRUSTED_PROFILE_SHA256 \
  --project PROJECT_EXPORT --project-sha256 EXPECTED_PROJECT_SHA256 \
  --spec SPEC_EXPORT --spec-sha256 APPROVED_SPEC_SHA256 --run-id UNIQUE_RUN --output NEW_DIRECTORY
python3 -I scripts/isolated_producer.py verify-transfer --receipt RECEIPT.json \
  --receipt-sha256 TRUSTED_RECEIPT_SHA256 --snapshot SNAPSHOT.json \
  --project-sha256 EXPECTED_PROJECT_SHA256 --spec-sha256 APPROVED_SPEC_SHA256 \
  --profile-sha256 TRUSTED_PROFILE_SHA256 --run-id UNIQUE_RUN
```

`qualify` uses only disposable synthetic source trees and asserts read-only mounts,
hidden host paths/environment, network isolation, no capabilities, no-new-privileges,
nested-userns denial, writable fresh scratch and timeout handling. Qualification
is a required operator review step, not a universal sandbox-security certificate.
Tests additionally cover changed runtime/profile/spec pins, nonzero capture,
artifact corruption, receipt replacement and replay. The actual measured results
and remaining acceptance items are recorded in the framework-plan checkpoint.

Before any real consumer pilot, its controller must approve the concrete runtime,
provider dependency/ambient-input review, host resource limits, immutable staging,
unique run-ID source, protected pin channel and artifact access/retention rules.

## Reproduction and review disposition

```sh
cargo build --locked
# Explicit opt-in: a missing namespace facility is an error, never a skipped test.
ADRPROOF_RUN_BWRAP_TESTS=1 python3 -I -m unittest discover -s scripts/tests -v
```

The default unit invocation skips live namespaces and must not be cited as an
isolation result. The Linux public CI job runs only these unit controls. Local
live qualification uses installed trusted tools copied to a disposable runtime;
the profile is deterministic for identical tool/source bytes and modes, not a
promise that two hosts construct identical runtimes. The temporary runtime is
removed after testing; no binary distribution or accepted production profile is
created. Do not use the synthetic test profile as an approval.

Current review packet:

- Implemented: mandatory namespace/mount/environment policy, runtime/source pins,
  bounded execution, exclusive artifact publication and run-bound receipt checks.
- Measured locally: 14 boundary assertions, actual Cargo metadata, actual neutral
  Python-provider capture/transfer, runtime-drift rejection and missing-inventory
  gate rejection. Test details and scope are in the framework checkpoint.
- Not approved: any real runtime/provider profile, consumer CI, credentials,
  artifact retention or deployment use. Host cgroup/VM limits and a protected
  job-output channel are mandatory integration work, not provided by this script.
- Not inferred: complete determinism, security of approved code, resistance to
  kernel exploits or global correctness from capture/transport success.
- Before rollout: record exact Git commit/tree identities and their export-digest
  mapping in trusted job metadata. The receipt binds file-tree digests and run ID,
  not Git history; it is only part of [the CI adoption contract](CI_ADOPTION.md).

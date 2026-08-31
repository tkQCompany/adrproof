# Quint model evidence

## Purpose and soundness boundary

`QuintModelEvidence` answers one narrow question:

> Did the selected property hold in the fingerprinted Quint model under the
> recorded backend, finite constants, bounds, semantic flags, timeout, and
> fairness assumptions?

It does not answer whether a Rust implementation conforms to that model.
Formal-model checking and implementation correspondence are different proof
obligations and different evidence kinds. A parent that claims anything about
implementation behavior must require implementation evidence and, where used,
scenario-to-model validation evidence in addition to model evidence.

## Pinned toolchain

The Phase 6C pilot pins:

- `@informalsystems/quint` 0.32.0;
- Apalache 0.56.1, the version pinned by that Quint release;
- TLC 2.19, shipped in the pinned Apalache distribution;
- a compatible OpenJDK runtime.

The selection was verified against the current [Quint CLI manual](https://quint.sh/docs/quint),
[model-checker guide](https://quint.sh/docs/model-checkers), and
[Quint FAQ](https://quint.sh/faq) on 2026-08-17. The relevant CLI is:

```text
quint verify MODEL --backend tlc --invariant PROPERTY
quint verify MODEL --backend tlc --temporal PROPERTY
quint verify MODEL --backend apalache --invariant PROPERTY --max-steps N
```

Quint uses Apalache as its Quint-to-TLA+ translation service before invoking
TLC. Evidence nevertheless records `backend=tlc` and the observed TLC version,
because TLC performs the property check. The translation dependency remains
visible in diagnostics and the pinned Quint/Apalache versions.

The executable and its cache location are operational configuration:

```text
ADRPROOF_QUINT=/path/to/quint
ADRPROOF_QUINT_HOME=/path/to/quint-cache
```

ADRProof checks `quint --version` before every execution. A mismatch is `ERROR`.

## Backend and authority semantics

TLC is used for exhaustive exploration of an explicitly finite model. A TLC
PASS means that no violating behavior exists in the complete reachable state
graph of that configured finite model. It is not a proof for larger domains.

Apalache is used for symbolic bounded checking. `max_steps` is mandatory in an
ADRProof Apalache definition. An Apalache PASS means only that no
counterexample was found through the recorded number of steps. It is never
reported as unbounded safety or universal liveness.

Temporal properties are routed to TLC. ADRProof records their fairness
assumptions and refuses an empty fairness list. An Apalache temporal request is
`UNVERIFIED/unsupported`, never PASS.

## Completion and status mapping

ADRProof does not trust process exit status alone. PASS or a property FAIL
requires an explicit completion marker from the model checker and matching
tool versions. This protects against a real observed failure mode where Quint
exited zero after its local backend server failed before checking the model.

| Model-checker outcome | ADRProof result |
|---|---|
| complete, no counterexample, ordinary property | `PASS` |
| complete, counterexample, ordinary property | `FAIL` |
| complete, counterexample, required reachability witness | `PASS` |
| complete, no counterexample, required reachability witness | `FAIL` |
| timeout | `ERROR` + `incomplete_timeout` |
| unsupported temporal/backend combination | `UNVERIFIED` |
| parse, type, version, process, or backend failure | `ERROR` |

An infrastructure failure is never an invariant FAIL. A behavior-admission
PASS means only that the model admits the selected witness.

## Evidence record

Each immutable JSON record contains at least:

- model check, model, and property IDs;
- result at execution and separately computed `current_validity`;
- model and property fingerprints;
- Quint and underlying backend versions;
- constants, finite bounds, and `max_steps` where relevant;
- explicit `model_bindings` from each recorded constant/bound selector to a
  `pure val` in the Quint source;
- fairness assumptions;
- exhaustive or bounded exploration semantics;
- completion semantics;
- state statistics reported by the backend;
- the counterexample output when present;
- explicit authority, scope, and `does_not_prove` statements;
- semantic input fingerprints and configuration hash;
- evidence ID and execution timestamp.

TLC statistics include generated states, distinct states, queue remainder,
depth, and duration when emitted. Apalache does not expose equivalent
explicit-state counts through this CLI, so those fields remain null rather
than being invented.

## Fingerprints, immutability, and staleness

Model evidence depends on:

- `spec:models/...qnt` content;
- the individual model-check JSON definition;
- property name and kind;
- constants and bounds;
- fairness assumptions;
- backend choice and pinned versions;
- `max_steps`, timeout, and semantic flags;
- authority metadata.

Changing any of these inputs makes historic evidence STALE. Evidence files are
content-derived, append-only records under `state_root/model-evidence`.
Counterexamples are stored inside the immutable evidence record.

Logical `spec:` identities, not absolute filesystem paths, are fingerprinted.
Relocating identical project, specification, and state roots does not make
evidence stale.

Pure formal-model evidence deliberately does not fingerprint Rust source. A
Rust change cannot make a model-only claim stale. Scenario evidence depends on
the implementation. Scenario-to-model validation depends on both sides.

Before a backend starts, ADRProof checks every declared `model_bindings` entry
against the selected model. JSON booleans, numbers, strings, and arrays have a
deterministic Quint representation; for example `constants.worker_ids` may bind
to `pure val ADRPROOF_WORKER_IDS = Set("A", "B")`. A missing or mismatched
declaration is infrastructure `ERROR`, never model PASS. Bounds are therefore
machine-bound semantic inputs rather than descriptive metadata.

## Scenario-to-model validation

`ModelValidationEvidence` implements a deliberately small relation:

```text
ImplementationScenario corresponds_to ModelTracePattern
```

For each mapping it requires:

- current immutable scenario evidence with the expected observed result;
- current model behavior-admission evidence;
- the documented abstract trace pattern.

S8 deliberately expects the implementation scenario result `FAIL`, because
that FAIL is the observation that both workers submitted the same batch. The
corresponding model check expects a counterexample to `S8NotAdmitted`; finding
that counterexample yields a behavior-admission PASS.

Missing evidence produces `UNVERIFIED`, a mismatch produces validation `FAIL`,
and a stale dependency makes validation STALE. Validation evidence is itself
immutable and is stored separately under
`state_root/model-validation-evidence`.

This is not trace equivalence and not an implementation refinement proof.

## Parent aggregation

Parents may require `relational`, `scenario`, `model`, `model_validation`, and
`correspondence` children. Existing ALL semantics remain conservative:

- every required child PASS/CURRENT: parent PASS;
- current child FAIL: parent FAIL;
- stale child: parent STALE;
- missing/unknown child: parent UNVERIFIED;
- child infrastructure ERROR: parent ERROR.

Therefore neither a model PASS nor a validation PASS can occupy an
implementation-scenario slot. A stale correspondence child blocks parent PASS.

## CLI

```text
adrproof model list ...roots...
adrproof model check MODEL-CHECK-ID ...roots...
adrproof model validate [VALIDATION-ID] ...roots...
adrproof model status [MODEL-CHECK-ID] ...roots...
adrproof explain MODEL:MODEL-CHECK-ID ...roots...
adrproof explain MODEL-VALIDATION:VALIDATION-ID ...roots...
```

Generated backend output is directed to `state_root`; the specification root
can remain read-only.

## Known limitations

- There is no general Rust-to-Quint refinement checker.
- Static Rust-to-Quint evidence checks selected syntax and named actions; it
  does not resolve types, macros, traits, or dynamic dispatch.
- Trace-pattern validation is selected finite correspondence, not equivalence.
- The initial integration model bounds one event, two workers, and two task slots.
- The main model contains U and M watermark components but omits H, the oldest
  open transaction, from its authority.
- Apalache state-count statistics are not available through the current Quint
  CLI integration.
- TLC counterexamples are retained as deterministic textual backend output;
  Apalache ITF normalization can be added later without changing authority.

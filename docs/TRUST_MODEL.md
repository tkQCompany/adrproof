# ADRProof trust model

## Trusted / machine-checked path

The CI verdict is produced only by the ADR Markdown/ADRLogic parser, lifecycle and
reference validation, typed IR validation, deterministic fact providers, stable
IR-to-backend lowering, a pinned formal backend, and deterministic result-policy
code. For external proof systems, the configured executable, its pinned version,
its relevant dependencies and the adapter interpreting its machine-readable result
join the trusted computing base. Input hashes, configuration and versions are
recorded in the proof ledger.

External fact providers are never auto-discovered. Configuration is explicit,
the executable must live under a selected input root, and protocol v1 accepts
only deterministic or authoritative provenance. This process boundary limits
protocol authority and failure propagation; it is not an OS sandbox. A configured
provider still has the operating-system permissions of ADRProof and must be
reviewed as trusted code.

This does not mean every trusted component is assumed bug-free. It means a defect
there can invalidate the verdict, so the component must be small, reviewable,
tested, pinned, and replaceable where possible. Generated SMT-LIB is retained for
independent replay.

At the evidence layer, `PASS` means the obligation passed on exactly the recorded
input fingerprints; `FAIL` means it was refuted; `UNKNOWN` means a verifier could
not decide; `UNVERIFIED` means no suitable current evidence exists; `STALE` means
historic evidence exists but an input fingerprint changed; `NOT_APPLICABLE` means
the obligation is outside the evaluated scope; and `ERROR` means infrastructure or
tool execution failed. Only current `PASS` evidence passes CI. Missing evidence,
UNKNOWN, STALE, malformed output, timeout, version mismatch, and crashes never do.

## External-provider threat model

Configured external providers are trusted executables but may still be buggy,
misconfigured, compromised, or overconfident. ADRProof therefore treats their
output as untrusted protocol data: it bounds stdout/stderr and execution time,
checks exact versions and wire shape, validates logical inputs and provenance,
and rejects unsupported authority or completeness claims.

These controls protect the verification decision; they are not an operating
system sandbox. A provider runs with the invoking user's filesystem, network,
environment, and syscall authority. Projects must independently sandbox code
they do not trust. Automatic provider download or execution of a command found
only through `PATH` is outside the 0.2 trust boundary.

Residual risks include a provider modifying files before ADRProof terminates it,
platform failure to kill descendants, resource consumption below configured
limits, and deterministic extraction from an incorrect source specification.
None of these risks is converted into proof authority by a successful process
exit alone.

Imported native-test evidence trusts the native runner and the deterministic
adapter that normalizes its output. ADRProof independently checks the declared
command, working directory, minimum pass count, maximum skips, zero failures,
non-empty execution, named required tests, and current fingerprints. A portable
bundle verifies completeness and SHA-256 integrity of copied ledger files; it
does not rerun or independently validate the underlying tools.

Absence is machine-checked evidence only inside a provider's explicit CLOSED fact
domain. OPEN/PARTIAL or missing coverage cannot justify negation and yields
UNVERIFIED when the obligation otherwise appears satisfiable.

At the Z3 protocol boundary, `SAT` produces current PASS evidence and `UNSAT`
produces FAIL evidence for the consistency obligation.

## Untrusted assistance

LLMs, agents, embedding search, RAG, natural-language contradiction classifiers,
generated explanations, suggested fixes, and translations from prose to formal
clauses are untrusted. They may author or edit inputs and explain deterministic
evidence, but cannot issue a CI PASS and cannot validate their own translation.

## The semantic/specification gap

A formally consistent specification may be incomplete, vacuous, or mistranslate
human intent. This is the **semantic/specification gap**. Solver success establishes
only a property of the formal model, not that the model is the intended system.
Human review, traceability, coverage reports, non-vacuity checks, and independent
validation reduce this gap; they do not erase it. Any property not supported by a
configured machine-checkable backend is reported as `UNVERIFIED`, never PASS.

Code extraction has the same boundary: `cargo metadata` can establish workspace,
package, crate dependency and feature facts. A syntax parser alone cannot establish
full Rust name resolution or behavioral semantics. Domain concepts therefore use
explicit annotations, and behavioral claims are delegated to dedicated tools.

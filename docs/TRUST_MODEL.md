# ADRProof trust model

## Trusted / machine-checked path

The CI verdict is produced only by the ADR Markdown/ADRLogic parser, lifecycle and
reference validation, typed IR validation, deterministic fact providers, stable
IR-to-backend lowering, a pinned formal backend, and deterministic result-policy
code. For external proof systems, the configured executable, its pinned version,
its relevant dependencies and the adapter interpreting its machine-readable result
join the trusted computing base. Input hashes, configuration and versions are
recorded in the proof ledger.

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

# ADRProof

ADRProof is a deterministic, CI-oriented meta-verifier for project architecture
specifications. Its first milestone checks the global consistency of formal clauses
embedded in Markdown ADRs. It does not use an LLM in its verification core and it
does not attempt to model the semantics of Rust.

```sh
cargo run -- check examples/database-architecture
cargo run -- check examples/database-architecture --json
cargo run -- facts examples/rust-workspace-architecture --json
cargo run -- facts --project-root examples/rust-workspace-architecture --summary --json
cargo run -- explain ADR-0100:C1 examples/rust-workspace-architecture --json
cargo run -- impact --path examples/rust-workspace-architecture/domain/Cargo.toml examples/rust-workspace-architecture --json
cargo run -- status examples/rust-workspace-architecture --json
cargo run -- model list --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- model check MODEL-CHECK-ID --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- model validate --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- correspondence list --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- correspondence check CORRESPONDENCE-ID --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- correspondence status [CORRESPONDENCE-ID] --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- native-test list --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- native-test import NATIVE-TEST-ID --report REPORT.json --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- native-test status [NATIVE-TEST-ID] --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- provider check [PROVIDER-ID] --project-root PROJECT --spec-root SPEC --state-root STATE --json
cargo run -- diagnose --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- bundle create --output NEW_DIRECTORY --project-root PROJECT --spec-root SPEC --state-root STATE
cargo run -- bundle verify BUNDLE_DIRECTORY --json
```

The [roadmap](ROADMAP.md) tracks release authority and current milestone scope.
Release history is recorded in the [changelog](CHANGELOG.md), while package,
protocol, and schema compatibility are defined in
[versioning](docs/VERSIONING.md).

Read-only/external verification can independently select `--project-root`,
`--spec-root`, and `--state-root`; see
[project/spec/state roots](docs/PROJECT_SPEC_STATE_ROOTS.md).

The executable expects Z3 4.13.4 by default (`ADRPROOF_Z3` may select another
executable). The exact accepted version is configured in `adrproof.json`.

See [the architecture](docs/architecture.md), [the research landscape](docs/landscape.md),
the [modeling/language strategy](docs/MODELING_AND_LANGUAGE_STRATEGY.md), and
[the trust model](docs/TRUST_MODEL.md). The [documentation index](docs/README.md)
separates normative design records from explanatory material.

When the checked directory contains `Cargo.toml`, `check` invokes `cargo metadata
--format-version 1 --no-deps --offline` and adds its covered facts to the Project Intent
Model. Exit codes are: 0 current PASS/SAT, 1 FAIL/UNSAT, 2 invalid input or I/O,
3 UNKNOWN, 4 timeout, 5 solver failure, and 6 fact-provider failure. Historic
PASS evidence is immutable; changed inputs, backend version, or semantic
configuration make its computed validity STALE and never a current PASS.

When `project_root/migrations` exists, `facts` and `check` also statically analyze
the ordered forward PostgreSQL migration stream with the pinned `pg_query` parser.
See [the SQL provider contract](docs/SQL_MIGRATION_FACT_PROVIDER.md), especially
its explicit `Closed`/`Partial` coverage boundary.

ADRProof 0.2 adds explicitly configured external fact providers through a
versioned JSON process boundary. Provider configuration, executable bytes, and
declared logical inputs participate in evidence staleness; malformed output,
timeouts, undeclared inputs, and non-deterministic provenance fail closed. See
the [external provider protocol](docs/EXTERNAL_PROVIDER_PROTOCOL.md) and its
[neutral example](examples/external-provider/). Provider authors should follow
the [author guide](docs/WRITING_EXTERNAL_PROVIDERS.md) and the
[migration guide](docs/MIGRATING_FACT_PROVIDERS.md).

Quint model checking is a specialized external backend with explicit
formal-model-only authority. TLC exhaustive results, Apalache bounded results,
temporal fairness, counterexamples, cross-validation, and implementation/model
separation are documented in
[Quint model evidence](docs/QUINT_MODEL_EVIDENCE.md).

Selected Rust-to-Quint transition correspondence is a separate, static AST
evidence kind. It checks declared calls, order, syntax/string fragments, and
named Quint actions, but does not claim semantic refinement or a type-resolved
call graph. See [Rust–Quint correspondence](docs/RUST_QUINT_CORRESPONDENCE.md)
for the exact authority boundary.

Imported native test summaries are first-class, immutable evidence with command,
threshold, named-test and non-vacuity checks. Evidence bundles copy the ledger
into a portable, SHA-256-addressed directory for offline integrity checks. See
[native test evidence and bundles](docs/NATIVE_TEST_EVIDENCE_AND_BUNDLES.md).

## Development provenance

ADRProof was initiated and is directed by Tomasz Krzal to address a concrete
need for deterministic verification of architectural decisions. A substantial
part of its technical exploration, implementation, tests, and documentation has
been developed through human-directed collaboration with OpenAI Codex.

ADRProof is therefore described as a human-directed, AI-assisted open-source
project, not as exclusively human-written software. This development process
does not put an LLM in the verification core. See [AI usage](AI_USAGE.md) for the
project's disclosure and contribution policy.

## License

ADRProof is licensed under the [Apache License 2.0](LICENSE). Attribution
information accompanying distributions is recorded in [NOTICE](NOTICE).

# Contributing to ADRProof

Thank you for considering a contribution. ADRProof treats reproducibility,
explicit authority boundaries, and reviewable evidence as product requirements.

## Before opening a pull request

1. Discuss substantial behavioral or architectural changes in an issue first.
2. Keep normative Architecture Decision Records in English and follow the
   policy in [`docs/DOCUMENTATION_LANGUAGE.md`](docs/DOCUMENTATION_LANGUAGE.md).
3. Do not submit credentials, private evidence, customer code, production
   configuration, or integration artifacts without the affected project's
   explicit approval.
4. Add or update tests for changed behavior.
5. Run the local quality gate:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo test --locked --all-targets
   cargo test --locked external_provider::conformance_tests
   cargo audit --deny warnings
   ```

CI repeats the test suite on Linux, macOS, and Windows with Rust 1.98.0, the
minimum supported Rust version.

## AI-assisted contributions

Material use of generative AI in design, implementation, tests, or documentation
must be disclosed in the pull request. The following commit trailer is
recommended:

```text
AI-Assisted-By: OpenAI Codex
```

See [`AI_USAGE.md`](AI_USAGE.md) for the complete provenance policy. A contributor
remains responsible for reviewing the contribution, holding the necessary rights,
and preventing disclosure of confidential or third-party material.

## Licensing

Unless explicitly stated otherwise, contributions intentionally submitted for
inclusion in ADRProof are provided under the Apache License, Version 2.0, in
accordance with section 5 of [`LICENSE`](LICENSE).

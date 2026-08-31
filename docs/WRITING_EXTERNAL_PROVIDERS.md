# Writing an external fact provider

An external provider is a deterministic adapter from project-owned inputs to
the Project Intent Model. It is not a general plugin API and it does not receive
proof authority merely because it exits successfully.

## Implementation checklist

1. Choose a stable provider ID and implementation version.
2. Read one JSON request from stdin and reject unknown request schema versions.
3. Use physical roots only to locate files; never include checkout paths in fact
   IDs, arguments, attributes, coverage, or artifacts.
4. Return exactly one UTF-8 JSON response on stdout.
5. Declare every file whose contents influence facts or completeness.
6. Use normalized `project:` or `spec:` logical paths.
7. Emit an artifact for every provenance source used by a fact.
8. Prefix every fact ID with `<provider-id>:`.
9. Emit positive observations only.
10. Use deterministic or authoritative provenance; never label an LLM or human
    inference as deterministic extraction.
11. Start with `partial` coverage. Claim `closed` only when the implementation
    enumerates the complete documented scope and every dependency of that claim
    is a declared input.
12. Keep diagnostics deterministic and free of secrets and physical paths.

## Validation workflow

Use the language-neutral fixtures first:

```sh
cargo test --locked external_provider::conformance_tests
```

Then configure the provider in an isolated example and inspect its inputs:

```sh
adrproof provider check PROVIDER-ID \
  --project-root PROJECT --spec-root SPEC --state-root STATE --summary

adrproof provider check PROVIDER-ID \
  --project-root PROJECT --spec-root SPEC --state-root STATE --json
```

Test at least: malformed output, non-zero exit, timeout, oversized output,
identity mismatch, undeclared input, path traversal, symlink escape, duplicate
IDs, source-artifact mismatch, and a change to every declared input.

## Portability

The configured executable must be a real executable file inside project or
specification root. Unix shebang scripts do not run natively on Windows. Ship a
host-native executable or configure platform-specific launchers. Do not rely on
tools discovered through `PATH` for the provider executable.

## Compatibility

Pin both configured provider version and protocol. Keep output compatible with
the exact v1 schemas. See [`VERSIONING.md`](VERSIONING.md) before changing a
field or authority claim.

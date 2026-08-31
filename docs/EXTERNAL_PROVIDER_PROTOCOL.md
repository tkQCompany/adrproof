# External fact provider protocol v1

ADRProof 0.2 can run explicitly configured fact providers without linking them
into the ADRProof binary. Providers receive a JSON request on standard input and
return one JSON response on standard output.

The portable schemas are:

- [`external-provider-request-v1.schema.json`](../schemas/external-provider-request-v1.schema.json)
- [`external-provider-response-v1.schema.json`](../schemas/external-provider-response-v1.schema.json)

The Rust deserializer and semantic validation remain the enforcement boundary.

## Configuration

`adrproof.json` in `specification_root` takes precedence; otherwise ADRProof reads
the file in `project_root`.

```json
{
  "z3_version": "4.13.4",
  "timeout_ms": 10000,
  "external_providers": [
    {
      "id": "component-manifest",
      "protocol": "adrproof-external-provider-v1",
      "version": "1.0.0",
      "executable": "providers/component_provider.py",
      "args": [],
      "timeout_ms": 5000,
      "parameters": {
        "manifest": "component.json"
      }
    }
  ]
}
```

Relative executable paths are resolved against the directory containing
`adrproof.json`. The executable must be a file inside `project_root` or
`specification_root`; a command found only through `PATH` is not accepted. The
configured file is trusted code. The process boundary provides protocol and
failure isolation, not filesystem, network, or syscall sandboxing.

## Request

The request contains:

- the exact request schema version;
- configured provider identity and version;
- physical project, specification, and state roots;
- an owned string-to-string parameter map.

Physical roots are operational addresses and must not be copied into fact IDs or
semantic identities. Provider output uses logical `project:` and `spec:` paths.

## Response and authority

A response contains the matching provider identity/version, declared input
files, source artifacts, positive facts, scoped coverage, and diagnostics.

Version 1 enforces the following rules:

- the response schema, provider ID, and version must match exactly;
- a fact ID starts with `<provider-id>:`;
- fact relations use ADRLogic identifier syntax and have at least one argument;
- facts are positive observations (`value: true`);
- provenance is `deterministically_extracted` or `authoritative`;
- every provenance source is a declared, existing logical input;
- every fact source has a corresponding source artifact in the response;
- coverage is owned by the configured provider and explicitly states `closed`
  or `partial`, its scope, qualifiers, and human-readable meaning;
- duplicate and cross-provider artifact/fact IDs are rejected.

An empty fact list is valid. It can establish absence only when accompanied by a
sound Closed coverage claim. Missing or Partial coverage remains UNKNOWN or
UNVERIFIED where absence matters.

## Execution boundary

- current directory: `state_root`;
- stdin: exactly one JSON request;
- stdout: exactly one JSON response, limited to 8 MiB;
- stderr: captured as diagnostics, also limited to 8 MiB;
- timeout: provider-specific or the global configuration timeout, between 1 ms
  and 600,000 ms;
- timeout cleanup: the provider process group is terminated on Unix;
- non-zero exit, timeout, malformed output, or contract violation: provider
failure, CLI exit code 6.

Validate every configured provider, or one selected provider, without running a
proof backend:

```sh
adrproof provider check --project-root PROJECT --spec-root SPEC --state-root STATE --json
adrproof provider check component-manifest --project-root PROJECT --spec-root SPEC --state-root STATE --json
```

The runner sorts providers, inputs, facts, artifacts, coverage, and diagnostics
before merging them into the Project Intent Model. Configuration, executable,
and declared inputs participate in evidence fingerprints. Provider dependencies
that are not declared as inputs remain outside ADRProof's staleness authority and
must not be used to justify a Closed claim.

## Reference example

[`examples/external-provider/`](../examples/external-provider/) contains a small
Python provider for a neutral component manifest. Run it with separate roots:

```sh
state="$(mktemp -d)"
cargo run -- check \
  --project-root examples/external-provider/project \
  --spec-root examples/external-provider/spec \
  --state-root "$state"
```

The example is explanatory and is not a general package or architecture parser.

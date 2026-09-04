# Versioning and compatibility

ADRProof has three separately versioned surfaces.

## Package version

The Cargo package follows Semantic Versioning. Before `1.0.0`, a minor package
release may contain public API changes, but published protocol identifiers keep
their compatibility rules below.

## External-provider protocol

The configured protocol identifier, request schema version, and response schema
version form one compatibility contract:

| Surface | Version 1 value |
| --- | --- |
| protocol | `adrproof-external-provider-v1` |
| request | `adrproof-external-provider-request-v1` |
| response | `adrproof-external-provider-response-v1` |

ADRProof accepts these values exactly and fails closed on unknown values.

A package release may fix implementation bugs while continuing to implement
protocol v1. Compatible documentation clarifications and additional conformance
fixtures do not change the protocol. A change that adds a required field,
removes or reinterprets an existing field, broadens authority, or changes
closed-world semantics requires a new protocol and schema version.

During `0.2.0-alpha.*`, v1 was a release candidate that could still be replaced
if the conformance suite or private pilot exposed a contract defect. From
`0.2.0-beta.1`, the published v1 wire contract, diagnostic families, exit
behavior, and provider-check report v1 are frozen. New machine-readable fields
that are not explicitly permitted by an existing schema require a new schema
version.

## Evidence and auxiliary schemas

Every persisted evidence or interchange document carries its own
`schema_version`. Readers reject unknown schema versions unless a document
explicitly defines forward-compatible extension fields. Package upgrades never
silently reinterpret an existing schema version.

## Compatibility policy

- patch package releases preserve supported protocol and schema behavior;
- adding support for a new protocol does not remove older supported protocols;
- removing a protocol requires a documented deprecation cycle;
- authority claims can only be strengthened through a new normative ADR and
  executable tests;
- machine-readable output changes are called out in the changelog.

## Maintenance of the 0.2 line

After stable 0.2.0, compatible defect fixes use 0.2.x patch releases. A correction
that restores the documented v1 behavior requires a regression test and a
changelog entry. If a correction changes previously emitted facts or verdicts,
consumers must rerun verification on the new package commit; old evidence is
not retroactively upgraded.

New required fields, new fields in a closed schema, changed completeness or
provenance authority, and reinterpretation of existing values require a new
schema/protocol identifier as applicable. Package 0.3 alone does not authorize
changing v1. A provider-check report v2 need not change the provider wire
protocol if that wire contract remains unchanged.

Report a v1 defect using the bug form with the exact package commit, protocol
and schema identifiers, command, exit code, expected result, and a neutral
reproduction. Report sensitive findings through SECURITY.md. Proposals for new
authority use the protocol-change form and an ADR before implementation.

The latest 0.2 release and main receive fixes under SECURITY.md. This policy
does not promise indefinite support for every pre-1.0 minor line.

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

During `0.2.0-alpha.*`, v1 is a release candidate and may still be replaced by a
new identifier before beta if the conformance suite or private pilot exposes a
contract defect. From `0.2.0-beta.1`, the published v1 wire contract is frozen.

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

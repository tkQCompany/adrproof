# Versioned interchange schemas

These JSON Schemas document ADRProof's externally exchanged artifacts. Schema
identifiers are immutable: incompatible changes require a new file and a new
`schema_version`; readers fail closed on unknown versions.

- `evidence-bundle-v1.schema.json` — `bundle.json`.
- `bundle-signature-v1.schema.json` — optional `bundle.sig.json`.
- `native-test-report-v1.schema.json` — normalized native test import.
- `diagnostic-policy-v1.schema.json` — explicit, owned, expiring waivers.
- `external-provider-request-v1.schema.json` — request written to a configured
  external fact provider's standard input.
- `external-provider-response-v1.schema.json` — facts, coverage, provenance,
  and declared semantic inputs returned by that provider.
- `provider-check-report-v1.schema.json` — stable success/error report emitted
  by `provider check --json`.

The Rust deserializers remain the enforcement boundary. These schemas are the
portable contract for CI, editors, and downstream integrations.

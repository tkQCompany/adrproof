# Migrating a fact extractor to an external provider

Move an extractor outside the ADRProof binary when its input format or release
cadence belongs to another project, or when maintaining it in ADRProof would
couple the verifier to customer-specific code.

## Migration sequence

1. Record the existing relation names, fact ID rules, artifacts, provenance,
   coverage scope, and unsupported cases.
2. Create neutral golden inputs and expected facts without customer data.
3. Implement the external process with `partial` coverage.
4. Run `provider check` and compare sorted facts and artifacts with the original
   extractor.
5. Verify that configuration, executable bytes, and all declared inputs affect
   evidence staleness.
6. Add explicit diagnostics for every input the extractor cannot understand.
7. Strengthen a coverage slice to `closed` only after an executable absence test
   demonstrates complete enumeration for that exact scope.
8. Run both implementations during a transition window and reject differences.
9. Remove the old implementation only after the new provider passes the public
   conformance kit and a private commit-pinned pilot.

## Authority preservation

A migration must not silently broaden authority. If the original extractor had
partial coverage, the external provider remains partial until completeness is
separately justified. A textual match, LLM classification, or hand-maintained
mapping cannot be relabeled as deterministic extraction.

## Project-specific integrations

Keep customer/project manifests, patches, executable providers, evidence, and
lock files in a private integration workspace. Only generalized behavior,
neutral reproductions, schemas, and tests belong in public ADRProof.

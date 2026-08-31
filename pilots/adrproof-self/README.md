# ADRProof self-pilot — Phase 8A

This self-contained pilot verifies ADRProof's own root Cargo boundary: Serde
remains direct, while Tokio, SQLx, and Reqwest remain absent as direct
dependencies. A disposable mutation adds Tokio and must turn the baseline PASS
into FAIL. The pilot then creates and verifies an offline evidence bundle.

The claim is intentionally limited to direct Cargo metadata. It does not infer
source call graphs or transitive-runtime behavior.

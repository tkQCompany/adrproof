---
id: ADR-SELF-001
status: accepted
---

# Keep the verifier core synchronous and free of product-service clients

ADRProof is an orchestrator around deterministic local providers and process
boundaries. Its root package must retain Serde as a direct interchange-format
dependency and must not acquire direct dependencies on an async runtime,
database client, or HTTP client.

```adrlogic
entity Package { adrproof, serde, tokio, sqlx, reqwest };
relation declares_direct_dependency(Package, Package);

rule C1 "ADRProof core keeps its synchronous local-process boundary" {
    declares_direct_dependency(adrproof, serde)
    && !declares_direct_dependency(adrproof, tokio)
    && !declares_direct_dependency(adrproof, sqlx)
    && !declares_direct_dependency(adrproof, reqwest);
}
```

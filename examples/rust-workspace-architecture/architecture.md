---
id: ADR-0100
status: accepted
---

# Rust workspace boundaries

The domain package must not directly depend on the database driver. Repository is
the intended adapter boundary.

```adrlogic
entity Package { web, domain, repository, search, llm_extract, fake_sqlx };
relation declares_direct_dependency(Package, Package);
rule C1 "domain must not depend directly on fake_sqlx" {
    !declares_direct_dependency(domain, fake_sqlx);
}
```

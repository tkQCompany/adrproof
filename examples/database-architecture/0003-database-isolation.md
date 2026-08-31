---
id: ADR-0003
status: accepted
---

# Database isolation

The domain crate must remain independent of SQLx.

```adrlogic
entity Crate { domain, sqlx };
relation depends_on(Crate, Crate);
rule C7 "domain must not depend on sqlx" {
    !depends_on(domain, sqlx);
}
```

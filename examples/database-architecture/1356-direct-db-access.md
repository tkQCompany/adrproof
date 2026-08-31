---
id: ADR-1356
status: accepted
---

# Direct database access

The new decision currently conflicts with ADR-0003.

```adrlogic
entity Crate { domain, sqlx };
relation depends_on(Crate, Crate);
rule C4 "domain must depend directly on sqlx" {
    depends_on(domain, sqlx);
}
```

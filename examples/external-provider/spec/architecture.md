---
id: EXTERNAL-PROVIDER-EXAMPLE
status: accepted
---

# Component manifest boundary

```adrlogic
entity Component { api };
entity ComponentKind { service };
relation component_kind(Component, ComponentKind);
rule C1 "the API component is a service" {
    component_kind(api, service);
}
```

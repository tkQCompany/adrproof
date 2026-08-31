---
id: ADRP-0006
status: accepted
---

# Use a versioned process boundary for external fact providers

ADRProof cannot standardize every source language or verification system. It can
standardize how an explicitly selected provider declares facts, provenance,
coverage, and the semantic inputs that make evidence stale.

An external provider is therefore a configured executable using a versioned JSON
request/response protocol over standard input and standard output. The process
boundary is inspectable and language-neutral. It is not an operating-system
sandbox: configuring an executable is an explicit decision to run that code.

Version 1 accepts positive observed facts only. Absence is derived solely from a
separate Closed coverage claim. Provider output marked `llm_derived` or
`human_authored` cannot enter the machine-PASS fact path. Unknown schemas,
identity/version mismatches, undeclared inputs, collisions, timeouts, excessive
output, and non-zero exits fail closed as provider errors.

The configuration file, provider executable, and all provider-declared source
files are semantic inputs. A change to any relevant input invalidates historic
current evidence even if physical checkout paths change.

```adrlogic
bool external_provider_inputs_are_explicit;
bool external_provider_failures_fail_closed;
rule C1 "external provider inputs participate in evidence validity" {
    external_provider_inputs_are_explicit;
}
rule C2 "external provider failures never become PASS" {
    external_provider_failures_fail_closed;
}
```

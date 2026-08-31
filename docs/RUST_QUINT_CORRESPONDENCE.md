# Static Rust–Quint correspondence evidence

## Purpose

`RustQuintStaticCorrespondenceProvider` answers a deliberately narrow question:

> Does the fingerprinted Rust syntax for each selected function contain the
> declared calls, ordering, strings, and AST fragments, and does the
> fingerprinted Quint model declare the corresponding named actions?

It is an implementation-correspondence child, not a Rust verifier or refinement
proof. A model PASS cannot substitute for it, and its PASS cannot establish that
the model is behaviorally equivalent to the implementation.

## Definition and execution

Definitions live under `spec_root/correspondence/checks/*.json`. Each transition
selects one project-relative Rust file/function and one or more Quint actions. It
may require:

- named direct or method calls;
- a call subsequence;
- string-literal fragments;
- compact syntax-tree token fragments;
- explicit authority and `does_not_prove` text.

Rust is parsed with `syn`; syntax-tree tokens are produced with `quote`. Functions
may be selected as a unique name or `Owner::method`. Quint action names are read
from explicit `action NAME` declarations. The provider does not compile the Rust
project or infer meaning from names.

```text
adrproof correspondence list ...roots...
adrproof correspondence check ID ...roots...
adrproof correspondence status [ID] ...roots...
adrproof explain CORRESPONDENCE:ID ...roots...
```

## Results and evidence

- every requirement present: `PASS`;
- missing function, action, call, ordering, string, or AST fragment: `FAIL`;
- unreadable or invalid Rust syntax: `ERROR`;
- changed definition, model, Rust input, or provider version: historic record
  becomes `STALE`.

Evidence is immutable under `state_root/correspondence-evidence`. Logical
`project:` and `spec:` identities make identical relocated roots semantically
equivalent. The graph artifact `correspondence-graph.json` uses `Defines`,
`RelevantTo`, and `EvidenceFor` edges.

## Trust boundary

PASS does not prove:

- type-resolved call targets, macro expansion, trait dispatch, or runtime flow;
- that calls execute on every path or share one runtime transaction;
- that every Rust behavior appears in the model;
- that every model action is implemented;
- semantic refinement, liveness, or universal implementation correctness.

Those claims require other children such as deterministic scenarios, formal model
evidence, relational evidence, or a future type-aware provider.

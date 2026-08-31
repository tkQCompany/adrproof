# External-provider v1 conformance fixtures

This directory is the language-neutral executable contract for provider
responses. [`cases.json`](cases.json) lists every response fixture and whether a
conforming ADRProof implementation must accept or reject it.

The fixtures use provider identity `fixture@1.0.0`. Logical inputs resolve under
[`roots/`](roots/). Physical roots in [`request.example.json`](request.example.json)
are illustrative and must be replaced by the roots received at runtime.

Run the authoritative suite from the repository root:

```sh
cargo test --locked external_provider::conformance_tests
```

Provider authors in any language can use the valid responses as golden output
examples and the invalid responses to test their own serializers. Passing these
JSON fixtures does not by itself establish process behavior: the ADRProof suite
also checks timeouts, output limits, process cleanup, and executable/input root
containment.

Fixture acceptance is normative for protocol v1. Human-readable error text is
non-normative; `error_contains` records the minimum stable concept each rejected
case must communicate, while automation should branch on diagnostic codes.

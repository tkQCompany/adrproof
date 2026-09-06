---
id: ADRP-0009
status: accepted
---

# Establish product maturity through sustained use across projects

## Context

ADRProof is an independent, pre-1.0 product. Its first consuming project is a
source of experience, not its semantic owner or a public implementation
dependency. Architectural maintenance creates value across repeated changes;
technical conformance and a short pilot do not establish that long-term value.

## Decision

The first stable product target is 1.0.0, following demonstrated usefulness over
sustained use in multiple maintainer projects and mature specification-language
syntax and semantics. There is no fixed date, minimum waiting period, automatic
promotion, or required community participation. An approximate year is a possible
experience horizon, not a release gate. A normal 0.x package number does not
declare mature product stability.

Assessment includes valuable findings, missed violations, misleading findings,
maintenance time, specification migrations, runtime/resource costs, and limits
encountered in actual approved CI integrations. A green suite and lack of reported
defects do not replace these observations. Early usability measurements are
useful even before long-term value can be assessed.

Package versions follow Semantic Versioning; published protocol/report/schema
identifiers retain their explicit compatibility contracts independently of the
product's pre-1.0 status. New authority or incompatible machine-readable behavior
requires the corresponding versioned contract and tests. Existing releases and
historical evidence are not rewritten. Document the public interface and its
compatibility/migration policy before 1.0.

The public repository contains only product-neutral code, fixtures and approved
sanitized findings. Consuming-project specifications, pins, patches and evidence
remain outside it. Changes to a consuming project require its controller's
approval; push, tag, release and development direction remain maintainer actions.
Development and separately approved pilots/CI adoption need not wait for 1.0.
Distribution remains source-only on GitHub, never crates.io.

## Consequences

This supersedes the earlier plan to promote beta to a stable 0.2 product. The
old 0.2 runbook is retained only as an explicitly inactive historical reference;
a release-specific 1.0 procedure must be reviewed when justified. It does not
bump the package version or authorize a release, integration change or monitor.

[The framework plan](../../AKTUALNY_PLAN_RAMOWY.md) maintains delivery focus and
evidence links. Normative ADRs control decisions; the plan must not silently
override them. [Versioning](../VERSIONING.md) controls published compatibility.

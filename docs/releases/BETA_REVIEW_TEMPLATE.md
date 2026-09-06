# Stable product 1.0 maturity review — template

Status: **NOT EVALUATED**. Copy and fill this record; placeholders are not PASS.
This template does not authorize publication.

## Identities and maturity assessment

- Reviewer and review timestamp (UTC): TBD
- Public beta publication timestamp and release URL: TBD
- Sustained-use scope across multiple maintainer projects, changes exercised,
  and owner-approved sanitized evidence references: TBD
- User and approved integration-controller assessment of practical usefulness: TBD
- Specification-language syntax/semantics maturity and migration expectations: TBD
- Beta tag, full commit and tree: TBD
- Proposed stable source commit and tree: TBD
- Versioned-contract compatibility and documented migration review since beta: TBD

There is no calendar deadline or minimum waiting period. Record what actual use
has established; elapsed time, a single pilot, green CI, or absence of community
reports cannot substitute for a maturity assessment.

## Evidence

| Gate | Observed result / exact reference | Decision |
| --- | --- | --- |
| Sustained-use benefits, useful/misleading findings, missed violations and limitations reviewed | TBD | NOT EVALUATED |
| Specification-language syntax/semantics and rule-maintenance costs reviewed | TBD | NOT EVALUATED |
| Compatibility and migration expectations for specification evolution documented | TBD | NOT EVALUATED |
| Protocol/report v1 compatibility | TBD | NOT EVALUATED |
| Open issues/PRs and release blockers triaged | TBD | NOT EVALUATED |
| Isolated repeated pilot on approved pins | TBD | NOT EVALUATED |
| Linux/macOS/Windows CI on candidate commit | TBD | NOT EVALUATED |
| Formatting, lint, tests, dependency audit and licenses | TBD | NOT EVALUATED |
| CodeQL and workflow policy | TBD | NOT EVALUATED |
| Clean-checkout documentation and internal links | TBD | NOT EVALUATED |
| Public source/archive neutrality inspection | TBD | NOT EVALUATED |
| Package, changelog, support and security policy agree | TBD | NOT EVALUATED |
| Source-only policy and no crates.io publication | TBD | NOT EVALUATED |
| Milestone complete, branch protection reviewed | TBD | NOT EVALUATED |

Record FAIL, BLOCKED or NOT EVALUATED explicitly. Absence of a reported issue
does not establish a successful check. Private evidence stays outside this
record; cite only an owner-approved sanitized pilot summary.

## Decision and publication handoff

- Remaining blockers, limitations and owners: TBD
- Maintainer decision: NO GO until explicitly recorded
- After GO: final release commit/tree and annotated tag target: TBD
- Reproduced archive/manifest/checksum comparison and digests: TBD
- Public release URL and artifact inspection: TBD

Follow [ADRP-0009](../adr/0009-long-term-maturity-and-project-independence.md)
and the [release checklist](../RELEASE_CHECKLIST.md). The old 0.2 promotion
procedure is inactive; require a reviewed 1.0-specific procedure before release.
Do not mark post-publication checks complete before the maintainer publishes.

# Requirement inventory (experimental)

This is the first bounded implementation of [ADRP-0007](adr/0007-reviewed-requirements-and-formalizations.md),
covering parts of framework-plan P02/P03/P04. It inspects declared coverage of
requirements, not program correctness or equivalence between prose and logic.

## Contract

`adrproof inventory --spec-root PATH [--json]` reads `PATH/requirements.json`
and Markdown ADRs below the same directory. Use a dedicated specification
directory: every discovered `.md` file must be an ADR. As in existing ADR
discovery, `.git`, `target` and `.adrproof` subtrees are ignored; an inventory
entry pointing into an ignored subtree remains unresolved, not covered. No solver, Cargo,
provider, test runner or evidence store is invoked. Project and state roots are
not read or written by this command. Symlinks within the specification directory
are rejected, as are absolute, parent-relative and non-portable source paths.

The input schema identifier is `adrproof-requirements-v1alpha1`; the independent
report identifier is `adrproof-inventory-report-v1alpha1`. Neither changes
external-provider protocol v1 or provider-check report v1. Unknown fields and
versions, duplicate IDs, invalid ranges and contradictory mapping declarations
are invalid input, not omissions silently discarded.

```json
{
  "schema_version": "adrproof-requirements-v1alpha1",
  "adrs": [{
    "id": "ADR-1",
    "source": "architecture.md",
    "requirements": [{
      "id": "REQ-1",
      "kind": "normative",
      "start_line": 6,
      "end_line": 6,
      "mapping": "mapped",
      "constraints": ["ADR-1:C1"],
      "reason": "This rule is the author's proposed formalization."
    }]
  }]
}
```

`adrs` must be nonempty. Each entry pins an ADR ID to a unique relative `.md`
path; it cannot disappear merely because the file was deleted. Conversely,
discovered ADRs absent from this list are reported as uninventoried. Requirement
IDs are globally unique. Inclusive, one-based line ranges identify nonempty text;
they are declarations of selection, not automatic recognition of requirements.
Overlapping selections are permitted (distinct requirements can share prose).

Kinds are `normative` and `rationale`. Rationale requires a nonempty reason and
cannot map to constraints. Mapping declarations are `unmapped` (empty constraint
list), `partial` or `mapped` (both require a nonempty list). Partial mapping
requires a reason. An active ADR without a normative requirement remains a gap.
This conservative first version has no manual exclusion or applicability filter.

Only effective ADRLogic constraint IDs (`ADR-ID:clause-id`) are supported as
mapping targets in this slice. One requirement can reference several constraints;
several requirements can reference the same constraint, including one in another
ADR. These are declared links in the existing Project Intent Model, not a second
proof registry. No claim is made that a linked rule checks the selected prose.

Existing lifecycle semantics determine active decisions: accepted ADRs not
superseded by an accepted ADR. Proposed, deprecated and superseded decisions stay
visible with a reason. Missing ADRs, source/ID mismatches, missing mappings,
partial mappings and absent/inactive constraint targets remain explicit gaps.
Historical mapping gaps do not enter the active gap set; malformed input still
fails regardless of lifecycle. Active ADRs not in the inventory are never hidden.

## Outcomes and authority

- Exit **0**, `RECORDED_UNREVIEWED`: the declaration has no detected inventory or
  active mapping gaps. This is **not PASS**, not approval and not a CI proof gate.
- Exit **3**, `INCOMPLETE`: the report lists inventory or active mapping gaps.
- Exit **2**: invalid input or I/O; diagnostic on stderr, no successful report.

Every valid report states `review_status: NOT_ASSESSED` and
`verification_status: NOT_RUN`. JSON ordering and logical `spec:` paths are
deterministic. Human-authored provenance means declared authorship, not approval.
Even `RECORDED_UNREVIEWED` cannot establish that an author listed every normative
sentence or chose appropriate constraints. FactCoverage and evidence freshness
remain separate concepts. Existing `check`, `status` and `diagnose` do not yet
enforce or load this inventory; consumers must not substitute it for those checks.

## Interface to the next review slice (P06)

The report provides content SHA-256 fingerprints of the exact inventory and ADR
bytes parsed, including entire formalization-bearing files. No mtime is used.
The model carries requirement/decision IDs, source selection, kind, declared
mapping and target constraint IDs. An independently versioned approval must
bind these identities and semantics plus both sides' content hashes and reviewer
provenance. Start with whole-file invalidation, including shared declarations and
cross-ADR formalization files; narrowing hash scope requires a separate tested
contract. A prose edit or changed mapping must require reassessment even if the
old logic still passes. Rerunning this command must never create an approval.

The separate [review workflow](FORMALIZATION_REVIEWS.md) now implements hash-bound
external attestations and review-freshness assessment. It does not change this
inventory report's `NOT_ASSESSED` result. There is no protected inventory
baseline, required-evidence aggregate gate, specialized-verifier mapping or
incremental checking yet. Protect specification changes by code review outside
ADRProof in the meantime. The inspected checkout must remain unchanged during
the run: per-file parsing and fingerprints share the same bytes, but this is not
an atomic filesystem snapshot or a sandbox against concurrent hostile mutation.

Try the [neutral example](../examples/requirement-inventory/requirements.json):

```sh
cargo run --locked -- inventory --spec-root examples/requirement-inventory --json
```

The example contains a deliberately unmapped requirement and exits 3. Its mapped
rule illustrates declaration consistency, not extracted implementation evidence.

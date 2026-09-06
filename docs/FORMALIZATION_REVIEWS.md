# Formalization reviews (experimental)

This bounded P06–P08 implementation builds on the
[requirement inventory](REQUIREMENT_INVENTORY.md). It records an externally
approved interpretation and assesses its freshness; it never proves prose/logic
equivalence or program correctness. Provider and inventory v1alpha1 reports are
unchanged. The new record and report use independent identifiers:
`adrproof-formalization-review-v1alpha1` and `adrproof-review-report-v1alpha1`.

## Workflow and trust boundary

```sh
state="$(mktemp -d)"
cargo run --locked -- review prepare REQ-boundary \
  --spec-root examples/requirement-inventory --state-root "$state" --json
# Send the prepared JSON to an authorized HUMAN reviewer outside the repair loop.
# The reviewer supplies decision, reviewer identity/approval reference and rationale.
# Save that externally approved document as /path/to/approved-review.json.
cargo run --locked -- review import --report /path/to/approved-review.json \
  --spec-root examples/requirement-inventory --state-root "$state" --json
cargo run --locked -- review status \
  --spec-root examples/requirement-inventory --state-root "$state" --json
```

`prepare` emits a `draft`, with no reviewer or rationale. It does not write or
approve anything. Import accepts only `approved` or `no_semantic_change`, with
nonempty rationale and a reviewer object containing `kind: human_attestation`,
`identity` and `approval_reference` (for example an independently approved review
identifier). The implementation does not authenticate that identity, fetch the
reference, or determine whether a human actually approved it. An LLM must not fill
these fields to approve its own translation. Operators must isolate the approved
documents, specification and review store from repair-agent writes, and authorize
the import step externally. This is an **unsigned human-attestation boundary**,
not a secure autonomous admission gate. No real approval is created by tests or
examples; synthetic test reviewers only validate mechanics.

All commands require explicit spec/state roots. State must be physically disjoint
from the specification tree (including aliases). Imports write only the
`formalization-reviews` subdirectory of the selected state root. Assessment and
preparation do not create state, invoke providers/solvers or read proof evidence.
Do not colocate unrelated projects in one review store.

## Binding and history

Each record names one requirement and contains:

- the existing requirement ID, a snapshot of its existing Project Intent Model
  selection (logical source, lines, kind, target IDs and provenance), and its hash;
- sorted content hashes of **all** inventory/ADR bytes parsed in that snapshot;
- a formalization hash of global effective declarations and constraints, plus the
  selected constraint IDs, excluding source line positions;
- the review decision, reviewer provenance, rationale and previous review ID.

This first version deliberately invalidates conservatively on any inventory or
ADR byte change, even an unrelated ADR or formatting edit. Whole-file hashes
include prose, formal clauses and shared declarations. The formalization hash is
only a structural comparison for no-semantic-change reapproval; it cannot override
changed input hashes. Physical checkout/state paths and mtimes are not bindings.
The checkout must remain stable while inspected; this is not an atomic filesystem
snapshot against concurrent hostile mutation.

This is not large-inventory performance qualification: full input snapshots are
stored per requirement/review. Finer dependency selection or shared snapshot
storage requires its own measured, soundness-preserving follow-up.

Preparation requires an active normative requirement with a complete declared
mapping and existing targets. Other inventory gaps may remain: approving one
interpretation does not silently approve the whole project. Import recomputes the
binding and rejects stale, altered or incomplete submissions. Importing the exact
same already-stored current record is idempotent. New records must explicitly
extend the latest record for that requirement using `supersedes` from preparation.
No timestamp or directory listing order chooses the latest record. Missing parents,
branches, cycles, bad filenames/hashes, unknown fields/versions and corrupt records
are errors. Records are content-addressed and created without overwriting earlier
files; reapproval appends a new record. Inspection validates the whole store.
Use one externally serialized importer. Concurrent writers are not a transaction:
a detected branch or interrupted partial write fails closed and requires operator
recovery from protected history; ADRProof does not silently delete either record.

`no_semantic_change` requires a predecessor and unchanged formalization hash
(including target IDs). A reviewer can reapprove edited prose without a dummy
contract edit. Changed formulas, rule descriptions, declarations or target sets
require a full `approved` review. This conservative check is not a theorem of
semantic equivalence. Merely preparing a fresh draft or rerunning verification
never renews approval.

Hashes detect alteration under retained filenames, not a forgery by someone who
can rewrite the entire trusted store and recalculate hashes. Deleting the newest
record can roll back the apparent head unless the integration pins/protects the
store. Authentication, revocation and protected inventory-baseline enforcement
remain separate future work; append-only API behavior is not filesystem WORM.

## Results

`review status` reports `CURRENT`, `STALE`, `MISSING`, `INELIGIBLE`, `INACTIVE` or
`REMOVED` per requirement, with changed logical inputs, binding diagnostics and
review history. It examines the latest record, never an older matching record.
Historical requirements remain visible; reviewed requirements removed from the
inventory are also reported, not forgotten. Inventory gaps, removed requirements,
an empty active normative set or any non-current required review yield
`INCOMPLETE` (exit 3). Otherwise the result is `REVIEWS_CURRENT` (exit 0).
Neither outcome is architectural PASS; `verification_status` is always `NOT_RUN`.
Invalid input/store/I/O exits 2, with a diagnostic on stderr and no success report.
Prepare/import exit 0 means only that the respective operation succeeded.

Existing `inventory`, `check`, `status` and `diagnose` do not consume these records
or enforce a review/evidence aggregate gate. Required evidence, FactCoverage,
functional/security checks and approval of narrowed scope remain independent.

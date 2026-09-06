# Current framework plan — ADRProof

Updated: 2026-09-06. Canonical language: English; the filename is retained as
requested by the maintainer. Status: **inventory and bounded formalization-review slices implemented**.
Initial baseline: `96948ce`; first implementation based on pushed `b321057`.
This document is a durable implementation compass,
not a claim that the target capabilities already exist or permission to publish.

## 1. Purpose and authority

Help architects and coding tools maintain an inspectable set of constraints
derived from **reviewed formalizations of approved requirements**. Connect the
requirements to appropriate checks and current evidence across project changes.
Reduce manual bookkeeping without hiding scope, assumptions or unverified gaps.

The desired LLM benefit is narrower search and faster compliant repairs through
explicit constraints and feedback. This is a hypothesis, not a generation-time
guarantee. A model may still propose violating code. Only the required checks can
admit a candidate, and only for their declared scope. No promise of complete
program correctness follows from admission.

Authority order for this work:

1. Explicit maintainer decisions, materialized in normative ADRs.
2. Accepted [ADRP-0007](docs/adr/0007-reviewed-requirements-and-formalizations.md),
   [ADRP-0008](docs/adr/0008-architect-workflow-and-bounded-feedback.md), and
   [ADRP-0009](docs/adr/0009-long-term-maturity-and-project-independence.md),
   alongside earlier ADRs and published compatibility contracts.
3. This plan: priorities, implementation status and acceptance evidence.
4. [ROADMAP.md](ROADMAP.md): release-line history and delivery summaries.

If these diverge, stop the conflicting change and reconcile the documents; do
not silently reinterpret a frozen schema or an accepted decision. Accepted ADR
status records a design decision, not machine-verified implementation conformance.

## 2. Boundaries that must survive implementation

- Keep the deterministic core model-independent; no LLM decides PASS or approves
  its own translation. Human review cannot be relabeled deterministic extraction.
- Reuse the Project Intent Model, typed graph, provenance, hashes and immutable
  evidence. Do not build a parallel requirements database without demonstrated need.
- Distinguish inventory coverage, formalization approval, fact coverage and proof
  validity. A fresh proof of an outdated formalization must not admit a candidate.
- Use content hashes, not file timestamps, to bind reviewed content. A prose edit
  may require review without requiring a changed formal contract.
- Preserve global constraint consistency. Incremental work may not omit facts,
  required obligations or interactions merely to obtain a green result.
- Keep contract languages native to specialized verifiers. One workflow does
  not imply one universal DSL or a complete Rust prover inside ADRProof.
- A test, a bounded model check and a deductive proof have different authority.
  Signatures establish integrity/authorship, not correctness of human intent.
- Missing, partial, stale, unknown and error outcomes cannot become required PASS.
  Exceptions and scope changes need explicit approval and remain visible.
- Keep functional/security checks independent and specifications protected from
  code-repair agents. Generation guidance is not a substitute for verification.
- Keep consuming-project material private and external. Controller approval is
  required for changes there; no consuming CI is enabled by this plan.
- Preserve protocol/report v1; version any incompatible new surface separately.
  No package bump, push, release, crates.io publication or model-call batch is
  authorized merely by recording this plan.

## 3. Recommended work sequence

These are bounded design/implementation slices, not completed phases. Before each
slice, define exact scope, dependencies and acceptance controls. Obtain missing
authority rather than treating a broad direction as approval for external changes.

1. **Requirement inventory:** represent the required set and its lifecycle;
   report unmapped, partial and excluded requirements without silent omission.
2. **Reviewed formalizations:** bind requirement, mapping and formal-artifact
   hashes to explicit review provenance; test stale review independently of proof.
3. **Required evidence gate:** compose the required checks with current reviews,
   coverage and evidence, preserving assumptions and fail-closed results.
4. **Architect-facing usability:** concise rule patterns, discoverable relation
   semantics, nonempty selections and actionable diagnostics using existing CLI.
5. **Specialized verifier integration:** select one justified Rust contract
   verifier, qualify its actual result/assumption/input boundary, and link its
   native contracts to requirements. Prusti is a candidate, not a selected or
   implemented backend. Do not encode a proof as an ordinary provider fact.
6. **Use and evaluate:** approved isolated pilots, then separately approved
   shadow/required CI. Measure usefulness and maintenance costs across multiple
   projects. Additional LLM studies require a frozen protocol and accepted budget.

## 4. Architect-facing checklist: preserve the 45 recommendations

Each item is a target acceptance concern, not an unconditional demand for a new
subsystem. Current foundations may satisfy parts of it. Mark completion only with
specific implementation and verification references; until then leave unchecked.

### Requirements and intent (ADRP-0007)

- [ ] P01 — Show applicable and historical ADRs with explicit lifecycle reasons.
- [ ] P02 — Account for every required ADR; expose absent/partial formalization.
- [ ] P03 — Link requirement fragments to one or many checks and shared evidence.
- [ ] P04 — Separate normative requirements from rationale and alternatives.
- [ ] P05 — Define supported applicability contexts; expose unsupported ones.
- [x] P06 — Bind text/formalization correspondence to reviewed content hashes
  (current ADRLogic inventory; unsigned external human-attestation trust boundary).
- [x] P07 — Detect changed requirements requiring formalization reassessment
  (conservative inventory/ADR byte invalidation; no automatic semantic equivalence).
- [x] P08 — Allow reviewed no-semantic-change edits without dummy contract edits
  (explicit reapproval, unchanged effective formalization/target projection).
- [ ] P09 — Give textual models an explicit specification or generated-view role.

### Usability (ADRP-0008)

- [ ] P10 — Provide concise patterns for justified common architecture rules.
- [ ] P11 — Make relation names, semantics, coverage and examples discoverable.
- [ ] P12 — Reduce duplicated entity inventories; reject empty/changed selections.
- [ ] P13 — Offer one coherent workflow while keeping native contract languages.
- [ ] P14 — Keep LLM proposals distinguishable from human-approved formalizations.
- [ ] P15 — Explain a rule's actual semantics and what it does not check.
- [ ] P16 — Explain missing facts/coverage and how to address the gap safely.
- [ ] P17 — Separate rule-author, provider-author and CI-operator responsibilities.
- [ ] P18 — Associate positive/negative controls with the tested rule version.

### Verification capability (ADRP-0001, ADRP-0004, ADRP-0008)

- [ ] P19 — Preserve global, not merely pairwise, relational consistency.
- [ ] P20 — Separate specification consistency from implementation conformance.
- [ ] P21 — State Cargo declaration scope; justify additional dependency semantics.
- [ ] P22 — Distinguish analyzed migration state from a running database's state.
- [ ] P23 — Ease external fact-provider qualification without weakening v1.
- [ ] P24 — Integrate a selected Rust contract verifier with explicit proof scope.
- [ ] P25 — Preserve test-evidence scope; never imply universal correctness.
- [ ] P26 — Keep finite/bounded/temporal model assumptions visible and actionable.
- [ ] P27 — Never relabel static Rust/model correspondence as semantic refinement.

### Evidence and change (ADRP-0005, ADRP-0007)

- [ ] P28 — Extend existing provenance paths through formalization reviews.
- [ ] P29 — Use content/identity fingerprints rather than modification times.
- [ ] P30 — Define all relevant tool, configuration and dependency proof inputs.
- [ ] P31 — Test relocation invariance for every added adapter.
- [ ] P32 — Optimize rechecking only with a sound dependency/coverage boundary.
- [ ] P33 — Preserve history and explain changes between evaluated revisions.
- [ ] P34 — Distinguish signed results from independently checkable proof certificates.
- [ ] P35 — Check compatible assumptions/scopes when composing required results.
- [ ] P36 — Require approval for removed obligations or narrowed gate inventories.

### Operations and product maturity (ADRP-0008, ADRP-0009)

- [ ] P37 — Evaluate an explicit required check set with one unambiguous outcome.
- [ ] P38 — Prioritize actionable diagnostics over raw backend output.
- [ ] P39 — Support reviewed exceptions/migrations without rewriting failures.
- [ ] P40 — Preserve project/spec/state separation and clarify its safety boundary.
- [ ] P41 — Qualify isolated runner patterns outside the verifier core.
- [ ] P42 — Keep platform/backend support claims synchronized with actual CI.
- [ ] P43 — Measure representative large inventories; do not infer scalability.
- [ ] P44 — Track long-term benefits, misses, false alarms and maintenance effort.
- [ ] P45 — Establish 1.0 maturity across multiple projects with migration policy.

## 5. Baseline and gaps — do not confuse direction with delivery

Existing foundations: relational ADRLogic/Z3, decision lifecycle, typed proof
graph, Cargo and bounded PostgreSQL facts, external-provider v1, scoped
Closed/Partial coverage, immutable evidence/freshness, scenarios, native-test
imports, Quint models, bounded correspondence, bundles/signatures and diagnostics.
See [architecture](docs/architecture.md) and [trust model](docs/TRUST_MODEL.md).

Not established at the initial checkpoint: complete prose-requirement coverage,
hash-bound formalization approval (now implemented within the bounded review slice
below), an end-to-end required inventory/review gate,
a Prusti adapter, comprehensive Rust correctness, a turnkey architect workflow,
or measured long-term value and large-inventory performance. Existing capabilities
can satisfy checklist items only after their relevant acceptance boundary is
checked; the unchecked list does not imply every mechanism is absent.

The [nine-session feedback pilot](experiments/feedback-loop/RESULTS-9.md) completed
with 3/3 first-proposal repairs in every arm. It demonstrated feasibility, not
better repair success or faster generation from ADRProof feedback. Do not rewrite
that result as success evidence for the new hypothesis.

## 6. Session handoff and definition of done

At the start of an implementation session, read this plan and the relevant
normative ADRs. Check the actual checkout and user changes, identify the checklist
IDs in scope, and choose the smallest useful slice. Do not implement all 45 items
merely because the comparison was accepted.

For each slice, record a dated checkpoint here with:

- checklist IDs and exact scope; changed schema/authority, if any;
- implementation commit or artifact references and known limitations;
- positive and negative tests, including missing/stale/tampered inputs;
- compatibility, privacy and resource-boundary review;
- remaining work and the next bounded step, with required owner approvals.

Mark an item complete only when implemented and verified, not when its design
document is written. Do not require a new abstraction if an existing CLI or small
report change suffices. Document deviations here and in the controlling ADR
before relying on changed behavior. Keep raw consuming-project evidence outside
this file. Push/release remain maintainer actions.

### Checkpoint 2026-09-06

- Accepted direction recorded in ADRP-0007/0008/0009 and this plan.
- Documentation only; no new runtime capability, schema or consuming CI enabled.
- Validation: all 116 tests passed with `cargo test --locked --offline
  --all-targets`; `python3 scripts/check-markdown-links.py` and `git diff --check`
  passed. CLI `status` against `docs/adr` in fresh external state reports the new
  three ADRs as `unverified_intent`, not as implemented machine-checked guarantees.
- Recommended next slice: P02/P03/P04 inventory contract and neutral omission
  controls, with the interface to P06 review binding defined before code changes.
- Stable product target: 1.0 after sustained usefulness across multiple projects
  and mature specification languages; no calendar or community-feedback gate.

### Implementation checkpoint 2026-09-06 — declared inventory

- Scope: bounded portions of P01/P02/P03/P04, reusing P29/P31 foundations;
  compatible P42 platform-documentation backlog closed for pushed `b321057`.
  Do not mark these broad product criteria complete: approved formalizations,
  general applicability/exclusions and heterogeneous evidence links remain absent.
- Artifacts: [inventory contract](docs/REQUIREMENT_INVENTORY.md),
  [implementation](src/inventory.rs), [controls](tests/inventory.rs), and
  [neutral example](examples/requirement-inventory/requirements.json).
  `inventory` inspects a nonempty declared ADR set and discovered ADRs, exposes
  missing/partial/unmapped entries and lifecycle, and adds selected requirements
  plus many-to-many declared ADRLogic links to the existing Project Intent Model.
  Prose-only decision provenance now retains its actual source.
- Authority and compatibility: independent `adrproof-requirements-v1alpha1` and
  `adrproof-inventory-report-v1alpha1` surfaces. `RECORDED_UNREVIEWED` (exit 0)
  means no detected inventory gaps, not approved interpretation or proof PASS;
  `INCOMPLETE` exits 3. No provider/report v1 or persisted evidence schema change,
  no new runtime dependency, no new backend, no LLM execution and no consumer CI.
- P06 interface was defined before implementation: exact parsed inventory/ADR
  bytes have content fingerprints; requirement IDs, selections, kind and mapped
  constraints are explicit. Future review must bind these plus reviewer identity
  and both sides' hashes. No approval is synthesized by inventory or verification.
- Positive/negative controls: 15 new inventory tests cover declared many-to-many
  and cross-ADR links, missing/deleted/uninventoried ADRs, lifecycle, inactive or
  deleted constraints, unmapped/partial/no-normative entries, malformed/duplicate
  input, path escape/symlinks, read-only CLI outcomes, stable relocation and
  content changes with unchanged mtime. A prose edit changes fingerprints without
  changing formal clauses; this tests the review input boundary, not an implemented
  stale-review assessment. The clean-checkout smoke script includes the example.
- Local validation: all 131 tests passed (`cargo test --locked --offline
  --all-targets`), including the existing 116-test baseline; formatting and
  Clippy with warnings denied passed. Documentation links, workflow policy and
  whitespace checks passed after staging, including the newly added documents.
- Limitations: selections and rationale classification are author declarations;
  omitted sentences or an edited inventory baseline cannot be detected by hashes
  alone. The command does not approve mappings, consult evidence, run providers
  or gate existing `check`/`status`/`diagnose`. Protect specification changes in
  external review. A stable checkout is required; this is not an atomic snapshot.
- P42: exact successful CI/CodeQL links and tested platform scope are recorded
  in [supported platforms](docs/SUPPORTED_PLATFORMS.md). The newly added portable
  inventory test step awaits its own post-push result. Branch-protection review,
  release metadata and private pilot/CI approvals remain owner-controlled backlog;
  they were not inferred from this implementation request.
- Next bounded slice: P06/P07/P08 hash-bound formalization-review records and
  independent current/stale/missing review assessment, starting with whole-file
  invalidation. Add negative controls where old logic passes but prose/mapping
  changed, and an explicit no-semantic-change reapproval. Do not yet aggregate
  these into a required-evidence gate or select a Rust verifier.

### Implementation checkpoint 2026-09-06 — formalization reviews (P06–P08)

- Based on pushed `75b2464`. Its CI, including macOS/Windows inventory tests,
  and CodeQL are now both successful; exact run links are retained in
  [supported platforms](docs/SUPPORTED_PLATFORMS.md). Review tests await the new
  implementation's own post-push CI, not inferred success from that baseline.
- Implemented `review prepare`, `review import` and `review status` according to
  [the contract](docs/FORMALIZATION_REVIEWS.md), written before implementation.
  [Code](src/reviews.rs) reuses Project Intent Model requirement snapshots and
  input fingerprints, with separately versioned attestation records rather than
  pretending a review is solver evidence. Both spec and state roots are explicit.
- P06–P08 are complete for the current ADRLogic inventory and stated trust
  boundary: whole inventory/ADR hashes, normalized selection/mapping and effective
  global formalization bind one review. Draft preparation cannot approve. Import
  validates an externally supplied attestation against current inputs; reapproval
  appends to a content-addressed per-requirement chain without overwriting history.
  Assessment reports current/stale/missing/ineligible/inactive/removed separately.
- `no_semantic_change` requires a previous review, reviewer/rationale and an
  unchanged global formalization/selected-target projection. Prose or formatting
  can be reapproved without editing clauses. This is not an equivalence proof.
- Verification: 19 new [review controls](tests/reviews.rs), 150 tests total,
  passed locally with the locked offline all-target suite; formatting and Clippy
  with warnings denied passed. Controls cover unchanged-mtime prose edits with
  a fresh consistency PASS, stale/missing proof with a current review, malformed
  and tampered input, missing predecessors, branches, stale-head replay, deleted
  requirements/targets, lifecycle, cross-ADR inputs and root relocation/aliases.
  The consistency integration test uses a finite Boolean fixture backend, not a
  real Z3 installation or any claim of new backend qualification.
- Validation note: one subsequent full run hit `Text file busy` (OS error 26)
  while starting executables in the unchanged external-provider tests
  `schema_and_identity_mismatches_are_rejected` and
  `nonzero_exit_and_output_limits_fail_closed`. The next full 150-test run and
  Clippy passed; each affected test also passed three isolated repetitions.
  Root cause is not established. Keep this intermittent process-fixture issue
  visible for a separate reproduction; no automatic retry or v1 behavior change
  was added to conceal it.
- Safety: no real requirement was approved, no provider/inventory v1alpha1 report
  or proof ledger schema changed, no dependency/package bump, external project
  mutation, consumer CI, model-call experiment or publication occurred.
- Limits: unsigned attestation is not authenticated human identity. A protected,
  externally authorized single-writer import boundary is required; arbitrary
  store writes or tail deletion are not defeated by hashes. No revocation or
  protected baseline for removed/narrowed obligations exists yet. Global byte
  invalidation can be costly; large-inventory scaling is unqualified. Existing
  `check`/`status`/`diagnose` still do not enforce a required-review/evidence gate.
- Next bounded step: define the P36/P37 protected required-set and aggregate-gate
  contract, then a minimal read-only composition using existing review and evidence
  assessments. Require nonempty explicit obligations, current reviews, current
  scoped evidence and negative controls against removal/narrowing. Do not enable
  consuming CI or infer approval of changed specifications from this checkpoint.

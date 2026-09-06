---
id: ADRP-0008
status: accepted
---

# Provide one architectural assurance workflow, not one universal prover

## Context

Architects need useful, maintainable constraints rather than another project to
administer. An LLM coding against those constraints may benefit from explicit
requirements and deterministic feedback, but current evidence does not establish
faster generation or universally compliant proposals.

## Decision

ADRProof should make an approved constraint set inspectable and actionable for
architects and coding tools. It combines reviewed requirements, applicable
constraints, selected facts, obligations, assumptions and current evidence.
This is not an automatic translation of all prose into an exhaustive model.

Provide a consistent workflow and clear final gate while preserving specialized
languages and verifiers. Common architectural rules should have concise,
reviewable patterns backed by the existing IR. Users must be able to inspect
their exact semantics, selection scope and unsupported cases. Separate the
responsibilities of rule author, provider author and CI operator.

Program semantics stay with specialized verifiers, as required by ADRP-0001 and
ADRP-0004. A future Rust contract-verifier adapter must identify the actual
function/contract, result, assumptions, relevant code and dependencies, tool
configuration and supported scope. A fact-provider success or process exit alone
must not masquerade as a deductive proof. No particular verifier is selected by
this decision, and no new contract language or complete Rust semantics is added.

The target final gate checks the explicitly required inventory, reviewed
formalizations and fresh evidence. Existing commands and reports should be
reused or extended narrowly before adding new abstractions. Diagnostics must
distinguish a violation, missing coverage, stale evidence/review and tool error;
an unsat core is not necessarily a unique code defect or a repair recipe.

## LLM assistance boundary

External orchestration may run a bounded proposal, check, diagnosis, revision
and recheck loop. Model choice and orchestration do not belong in the
deterministic verification core. The loop has explicit attempt/time budgets and
stops on success within scope, exhaustion, tool failure, prohibited edits or a
required specification decision. Functional and security checks stay independent.

The agent cannot obtain acceptance by weakening rules, narrowing selections,
editing tests/evidence or approving its own formalization. Implementation repair
and specification change are separate workflows with separate authority.
Input integrity and approval are enforced by the integration boundary, not a
promise that an LLM will voluntarily respect them.

Feedback may guide search away from known violations, but this is a system-level
effectiveness hypothesis. ADRProof does not enforce constrained token generation,
increase model intelligence, or guarantee that every proposal is compliant. A
candidate is admitted only after the required checks; admission covers their
specified scope, not all program behavior or human intent.

## Consequences and acceptance

The existing [nine-session pilot](../../experiments/feedback-loop/RESULTS-9.md)
had identical first-attempt success in A/B/C and no demonstrated feedback benefit.
Further claims need separately approved, repeated comparisons with fixed inputs,
specifications, budgets and independent scoring, including regressions, false
PASS and token/time costs. Do not run more model experiments merely because this
direction is accepted.

Implementation must demonstrate rule usability, nonempty selections, preserved
diagnostic authority, protected specification/evidence boundaries and failing
negative controls. It is not complete merely because the CLI has one command.
This ADR records direction; it does not install CI, a Rust proof adapter or an
agent platform. See [the framework plan](../../AKTUALNY_PLAN_RAMOWY.md).

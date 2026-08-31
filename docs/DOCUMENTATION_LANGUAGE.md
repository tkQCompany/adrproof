# Documentation language policy

## Canonical language

English is the canonical language for ADRProof source documentation, normative
Architecture Decision Records, public schemas, command help, and contribution
material. A normative change is complete only when its English version is
current.

## Existing non-English decisions

A non-English ADR should not be hidden with `.gitignore` merely because of its
language. Before public merge, translate it into English and make the English
document canonical. Preserve its ADR identifier, decision status, context,
constraints, and consequences; translation must not create a second independent
decision.

If retaining the original is useful, place it under
`docs/translations/<language-code>/` with the same basename and add a prominent
header linking to the canonical English ADR. Translations are informational and
must state that the English document controls if the texts diverge.

Example header:

```markdown
> Informational Polish translation. The canonical and normative document is
> the matching file under `docs/adr/`. If the texts differ, the English ADR
> controls.
```

## Confidentiality is separate from language

Documentation is excluded from the public repository because of ownership,
confidentiality, security, or product-boundary concerns—not because it is written
in a particular language. A translation must never be used to publish material
that the source project's owner has not approved for disclosure.

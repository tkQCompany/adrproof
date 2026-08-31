# AI usage and development provenance

## Summary

ADRProof is a human-directed, AI-assisted open-source project. It was initiated
and is directed by Tomasz Krzal in response to a concrete need for deterministic,
CI-oriented verification of architectural decisions. OpenAI Codex has provided
substantial assistance with technical exploration, implementation, tests,
documentation, and repository-level analysis.

The project must not be represented as exclusively human-written. Equally, the
use of AI assistance must not be understood as autonomous project ownership,
maintainership, legal authorship, endorsement, or a guarantee by OpenAI.

## Responsibilities

The human maintainer:

- defines project goals, priorities, constraints, and acceptable risk;
- decides which proposed changes are accepted or rejected;
- controls release direction, commits intended for publication, tags, merges,
  and pushes;
- coordinates changes that affect integrated projects with their respective
  maintainers or control processes;
- assumes responsibility for versions published as ADRProof releases.

AI assistance may include:

- exploring designs and trade-offs;
- proposing or implementing source code and refactorings;
- creating tests, formal models, fixtures, scripts, and documentation;
- inspecting repositories and diagnosing failures;
- running checks and organizing evidence for human review.

AI tools do not independently approve releases, set project direction, own the
repository, or act as legal contributors or maintainers.

## Verification boundary

AI assistance during development does not make the ADRProof verification core
AI-based or probabilistic. The verification core is implemented as conventional
software and uses explicit inputs, deterministic checks, pinned external tools,
and reviewable evidence contracts as documented by the project.

Generated or AI-assisted changes may still be incomplete or incorrect. They are
subject to the same compilation, test, model, evidence, security, licensing, and
maintainer-acceptance requirements as any other contribution. Passing automated
checks is evidence within their declared scope, not a substitute for human
responsibility or a warranty.

## Contribution disclosure

Contributors should disclose material use of generative AI when it substantially
affected a contribution's design, implementation, tests, or documentation. A
commit trailer such as the following is recommended:

```text
AI-Assisted-By: OpenAI Codex
```

This trailer records development provenance. It is not a claim that the AI
system is a legal author, copyright holder, committer, or endorser.

Contributors remain responsible for ensuring that they have the right to submit
their inputs and contribution, that confidential or third-party material is not
improperly disclosed, and that the contribution complies with the project
license and applicable law.

## Licensing

ADRProof is distributed under the Apache License, Version 2.0. AI-assisted
development does not create a separate license tier and does not change the
permissions or obligations in `LICENSE`. Attribution information for distributed
copies and derivative works is recorded in `NOTICE`.

The project does not add a custom restriction based on whether software is used
or modified by humans, AI systems, or a combination of both.
